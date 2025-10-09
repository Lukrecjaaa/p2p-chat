use crate::cli::commands::{Node, UiNotification};
use crate::crypto::{Identity, StorageEncryption};
use crate::network::NetworkLayer;
use crate::storage::{
    MessageHistory, SeenTracker, SledFriendsStore, SledOutboxStore, SledSeenTracker,
};
use crate::sync::{SyncEngine, SyncStores};
use crate::types::Message;
use crate::ui::run_tui;
use anyhow::Result;
use libp2p::Multiaddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};

pub async fn run(
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

    let sync_stores = SyncStores::new(
        friends.clone(),
        outbox.clone(),
        history.clone(),
        seen.clone(),
    );

    let (sync_engine_instance, sync_event_tx, mut sync_event_rx) = SyncEngine::new_with_network(
        Duration::from_secs(30),
        identity.clone(),
        sync_stores,
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
            let already_seen = match seen_clone.is_seen(&message.id).await {
                Ok(flag) => flag,
                Err(e) => {
                    error!("Failed to check if message {} was seen: {}", message.id, e);
                    false
                }
            };

            if already_seen {
                debug!("Received duplicate message {}, ignoring", message.id);
                continue;
            }

            if let Err(e) = node_clone.history.store_message(message.clone()).await {
                error!("Failed to store incoming message {}: {}", message.id, e);
                continue;
            }

            if let Err(e) = seen_clone.mark_seen(message.id).await {
                error!("Failed to mark message {} as seen: {}", message.id, e);
            }

            let _ = node_clone
                .ui_notify_tx
                .send(UiNotification::NewMessage(message));
        }
    });

    run_tui(node, ui_notify_rx).await
}
