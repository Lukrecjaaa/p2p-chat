//! This module handles the initial setup of the application.
use super::args::AppArgs;
use crate::crypto::{Identity, StorageEncryption};
use anyhow::{anyhow, Result};
use base64::prelude::*;
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

/// Contains all the necessary components for the application to run.
///
/// This struct is created by the `prepare` function and passed to the
/// appropriate `run` function (either for a client or a mailbox node).
pub struct PreparedApp {
    /// The command-line arguments.
    pub args: AppArgs,
    /// The port to listen on for P2P connections.
    pub port: u16,
    /// The port for the Web UI.
    pub web_port: u16,
    /// The user's identity.
    pub identity: Arc<Identity>,
    /// The database instance.
    pub db: sled::Db,
    /// The encryption key for the storage, if enabled.
    pub encryption: Option<StorageEncryption>,
}

/// Prepares the application for running.
///
/// This function performs the following steps:
/// 1. Finds free ports if not specified.
/// 2. Configures logging.
/// 3. Prints a start banner.
/// 4. Creates the data directory.
/// 5. Loads or generates the user's identity.
/// 6. Prints identity information.
/// 7. Opens the database.
/// 8. Sets up storage encryption if enabled.
///
/// # Arguments
///
/// * `args` - The command-line arguments.
///
/// # Errors
///
/// This function will return an error if any of the setup steps fail.
pub fn prepare(args: AppArgs) -> Result<PreparedApp> {
    let port = args.port.unwrap_or(find_free_port()?);
    let web_port = args.web_port.unwrap_or(find_free_port()?);

    configure_logging(args.mailbox);
    print_start_banner(&args, port, web_port);

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
        web_port,
        identity,
        db,
        encryption,
    })
}

/// Configures logging for the application.
///
/// If running in mailbox mode, it sets a more verbose logging level.
fn configure_logging(mailbox_mode: bool) {
    if mailbox_mode {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("info,p2p_chat=debug"))
            .try_init();
    }
}

/// Prints a banner with startup information.
fn print_start_banner(args: &AppArgs, port: u16, web_port: u16) {
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
    if !args.mailbox {
        println!("Web UI: http://127.0.0.1:{}", web_port);
    }
    println!("Data directory: {}", args.data_dir);
    println!();
}

/// Prints information about the user's identity.
fn print_identity_info(identity: &Arc<Identity>) {
    println!("Identity loaded:");
    println!("  Peer ID: {}", identity.peer_id);
    println!(
        "  E2E Public Key: {}",
        BASE64_STANDARD.encode(identity.hpke_public_key())
    );
    println!();
}

/// Resolves the encryption password.
///
/// The password can be provided via a command-line argument or an environment variable.
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

/// Loads an encryption salt from a file, or creates a new one if it doesn't exist.
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

/// Finds a free TCP port on the local machine.
fn find_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}
