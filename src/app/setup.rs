use super::args::AppArgs;
use crate::crypto::{Identity, StorageEncryption};
use anyhow::{anyhow, Result};
use base64::prelude::*;
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

pub struct PreparedApp {
    pub args: AppArgs,
    pub port: u16,
    pub identity: Arc<Identity>,
    pub db: sled::Db,
    pub encryption: Option<StorageEncryption>,
}

pub fn prepare(args: AppArgs) -> Result<PreparedApp> {
    let port = args.port.unwrap_or(find_free_port()?);

    configure_logging(args.mailbox);
    print_start_banner(&args, port);

    std::fs::create_dir_all(&args.data_dir)?;

    let identity_path = format!("{}/identity.json", args.data_dir);
    let identity = Arc::new(Identity::load_or_generate(&identity_path)?);

    print_identity_info(&identity);

    let db_path = format!("{}/db", args.data_dir);
    let db = sled::open(&db_path)?;

    let encryption = if args.encrypt {
        println!("🔐 Storage encryption enabled");

        let password = resolve_encryption_password(&args)?;
        let salt_path = format!("{}/encryption_salt.bin", args.data_dir);
        let salt = load_or_create_salt(&salt_path)?;

        Some(StorageEncryption::new(&password, &salt)?)
    } else {
        None
    };

    Ok(PreparedApp {
        args,
        port,
        identity,
        db,
        encryption,
    })
}

fn configure_logging(mailbox_mode: bool) {
    if mailbox_mode {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("info,p2p_chat=debug"))
            .try_init();
    }
}

fn print_start_banner(args: &AppArgs, port: u16) {
    println!("🚀 Starting P2P E2E Messenger");
    println!(
        "Mode: {}",
        if args.mailbox {
            "Mailbox Node"
        } else {
            "Client"
        }
    );
    println!("Port: {}", port);
    println!("Data directory: {}", args.data_dir);
    println!();
}

fn print_identity_info(identity: &Arc<Identity>) {
    println!("Identity loaded:");
    println!("  Peer ID: {}", identity.peer_id);
    println!(
        "  E2E Public Key: {}",
        BASE64_STANDARD.encode(&identity.hpke_public_key())
    );
    println!();
}

fn resolve_encryption_password(args: &AppArgs) -> Result<String> {
    args
        .encryption_password
        .clone()
        .or_else(|| std::env::var("P2P_MESSENGER_PASSWORD").ok())
        .ok_or_else(|| {
            anyhow!(
                "Encryption password not provided. Supply --encryption-password or set P2P_MESSENGER_PASSWORD."
            )
        })
}

fn load_or_create_salt(path: &str) -> Result<[u8; 16]> {
    if Path::new(path).exists() {
        let bytes = std::fs::read(path)?;
        if bytes.len() != 16 {
            anyhow::bail!(
                "Encryption salt at '{}' has unexpected length {} (expected 16)",
                path,
                bytes.len()
            );
        }
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&bytes);
        Ok(salt)
    } else {
        let generated = StorageEncryption::generate_salt();
        std::fs::write(path, generated)?;
        Ok(generated)
    }
}

fn find_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}
