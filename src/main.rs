mod cli;
mod crypto;
mod logging;
mod mailbox;
mod net;
mod network;
mod storage;
mod sync;
mod types;
mod ui;

use crate::cli::commands::Node;
use crate::cli::commands::UiNotification;
use crate::crypto::{Identity, StorageEncryption};
use crate::mailbox::MailboxNode;
use crate::network::NetworkLayer;
use crate::storage::{
    MessageHistory, SeenTracker, SledFriendsStore, SledOutboxStore, SledSeenTracker,
};
use crate::sync::SyncEngine;
use crate::types::Message;
use crate::ui::run_tui;
use anyhow::Result;
use base64::prelude::*;
use clap::Parser;
use libp2p::Multiaddr;
use std::str::FromStr;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "p2p-messenger")]
#[command(about = "A P2P E2E encrypted messenger")]
struct Cli {
    #[arg(long, help = "Run in mailbox node mode")]
    mailbox: bool,

    #[arg(long, help = "Port to listen on (random free port if not specified)")]
    port: Option<u16>,

    #[arg(long, help = "Config file path")]
    config: Option<String>,

    #[arg(long, default_value = "data", help = "Data directory")]
    data_dir: String,

    #[arg(long, help = "Enable storage encryption")]
    encrypt: bool,
}

fn find_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let port = match cli.port {
        Some(p) => p,
        None => find_free_port()?,
    };

    if cli.mailbox {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("info,p2p_chat=debug"))
            .init();
    }

    println!("🚀 Starting P2P E2E Messenger");
    println!(
        "Mode: {}",
        if cli.mailbox {
            "Mailbox Node"
        } else {
            "Client"
        }
    );
    println!("Port: {}", port);
    println!("Data directory: {}", cli.data_dir);
    println!();

    std::fs::create_dir_all(&cli.data_dir)?;

    let identity_path = format!("{}/identity.json", cli.data_dir);
    let identity = Arc::new(Identity::load_or_generate(&identity_path)?);

    println!("Identity loaded:");
    println!("  Peer ID: {}", identity.peer_id);
    println!(
        "  E2E Public Key: {}",
        BASE64_STANDARD.encode(&identity.hpke_public_key())
    );
    println!();

    let db_path = format!("{}/db", cli.data_dir);
    let db = sled::open(&db_path)?;

    let encryption = if cli.encrypt {
        println!("🔐 Storage encryption enabled");
        let password = "default_password";
        let salt = StorageEncryption::generate_salt();
        Some(StorageEncryption::new(password, &salt)?)
    } else {
        None
    };

    if cli.mailbox {
        run_mailbox_node(identity, db, encryption, port).await
    } else {
        run_client(identity, db, encryption, port).await
    }
}

async fn run_mailbox_node(
    identity: Arc<Identity>,
    db: sled::Db,
    encryption: Option<StorageEncryption>,
    port: u16,
) -> Result<()> {
    println!("📬 Starting mailbox node");

    let mut mailbox_node = MailboxNode::new(
        identity.clone(),
        db,
        encryption,
        1000,
        Duration::from_secs(7 * 24 * 60 * 60),
    )?;

    let stats = mailbox_node.get_stats();
    println!("Mailbox configuration:");
    println!(
        "  Max storage per user: {} messages",
        stats.max_storage_per_user
    );
    println!("  Retention period: {:?}", stats.retention_period);
    println!();

    let listen_addr = Multiaddr::from_str(&format!("/ip4/0.0.0.0/tcp/{}", port))?;

    let mailbox_storage = mailbox_node.storage.clone();
    let (mut network_layer, _network_handle) = NetworkLayer::new_with_mailbox_storage(
        identity, listen_addr, true, Some(mailbox_storage), vec![]
    )?;
    
    network_layer.bootstrap_dht()?;

    mailbox_node.run_with_network(network_layer).await
}

async fn run_client(
    identity: Arc<Identity>,
    db: sled::Db,
    encryption: Option<StorageEncryption>,
    port: u16,
) -> Result<()> {
    println!("💬 Starting client mode");

    let friends = Arc::new(SledFriendsStore::new(db.clone(), encryption.clone())?);
    let history = Arc::new(MessageHistory::new(db.clone(), encryption.clone())?);
    let outbox = Arc::new(SledOutboxStore::new(db.clone(), encryption.clone())?);
    let seen = Arc::new(SledSeenTracker::new(db.clone())?);

    let listen_addr = Multiaddr::from_str(&format!("/ip4/0.0.0.0/tcp/{}", port))?;

    let bootstrap_nodes = vec![];

    let (mut network_layer, network_handle) =
        NetworkLayer::new(identity.clone(), listen_addr, false, bootstrap_nodes)?;

    let (incoming_tx, mut incoming_rx) = mpsc::unbounded_channel::<Message>();
    let (ui_notify_tx, ui_notify_rx) = mpsc::unbounded_channel::<UiNotification>();

    let (sync_engine_instance, sync_event_tx, mut sync_event_rx) = SyncEngine::new_with_network(
        Duration::from_secs(30),
        identity.clone(),
        friends.clone(),
        outbox.clone(),
        history.clone(),
        seen.clone(),
        network_handle.clone(),
        ui_notify_tx.clone(),
    )?;
    let sync_engine = Arc::new(Mutex::new(sync_engine_instance));
    
    network_layer.set_sync_event_sender(sync_event_tx.clone());

    let node = Arc::new(Node {
        identity,
        friends: friends.clone(),
        history: history.clone(),
        outbox: outbox.clone(),
        network: network_handle,
        ui_notify_tx,
        sync_engine: sync_engine.clone(),
    });

    println!("Client initialized. Starting network and TUI...\n");

    let sync_engine_clone = sync_engine.clone();
    tokio::spawn(async move {
        let interval_duration = {
            let engine = sync_engine_clone.lock().await;
            engine.interval
        };
        let mut interval_timer = tokio::time::interval(interval_duration);

        info!("Starting sync engine with interval {:?}", interval_duration);

        // Initial discovery and sync cycle
        {
            let mut engine = sync_engine_clone.lock().await;
            if let Err(e) = engine.initial_discovery().await {
                error!("Initial mailbox discovery failed: {}", e);
            }
            if let Err(e) = engine.sync_cycle().await {
                error!("Initial sync cycle failed: {}", e);
            }
        }

        loop {
            tokio::select! {
                _ = interval_timer.tick() => {
                    let mut engine = sync_engine_clone.lock().await;
                    if let Err(e) = engine.sync_cycle().await {
                        error!("Sync cycle failed: {}", e);
                    }
                }
                event = sync_event_rx.recv() => {
                    if let Some(event) = event {
                        let mut engine = sync_engine_clone.lock().await;
                        if let Err(e) = engine.handle_event(event).await {
                            error!("Failed to handle sync event: {}", e);
                        }
                    } else {
                        info!("Sync event channel closed, stopping engine.");
                        break;
                    }
                }
            }
        }
    });
    
    network_layer.set_sync_event_sender(sync_event_tx);
    
    tokio::spawn(async move {
        if let Err(e) = network_layer.run(incoming_tx).await {
            error!("Network layer error: {}", e);
        }
    });

    let node_clone = node.clone();
    let seen_clone = seen.clone();
    tokio::spawn(async move {
        while let Some(message) = incoming_rx.recv().await {
            match seen_clone.is_seen(&message.id).await {
                Ok(true) => {
                    debug!("Received duplicate message {}, ignoring", message.id);
                    continue;
                }
                Ok(false) => {
                    if let Err(e) = seen_clone.mark_seen(message.id).await {
                        error!("Failed to mark message {} as seen: {}", message.id, e);
                    }
                }
                Err(e) => {
                    error!("Failed to check if message {} was seen: {}", message.id, e);
                }
            }

            if let Err(e) = node_clone.history.store_message(message.clone()).await {
                error!("Failed to store incoming message {}: {}", message.id, e);
                continue;
            }

            let _ = node_clone
                .ui_notify_tx
                .send(UiNotification::NewMessage(message));
        }
    });

    run_tui(node, ui_notify_rx).await
}