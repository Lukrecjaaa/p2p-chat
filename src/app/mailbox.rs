//! This module contains the primary entry point for running a mailbox node.
use crate::crypto::{Identity, StorageEncryption};
use crate::mailbox::MailboxNode;
use crate::network::NetworkLayer;
use anyhow::Result;
use libp2p::Multiaddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

/// Runs a mailbox node.
///
/// This function initializes and runs a mailbox node, which is responsible for
/// storing and forwarding messages for other peers in the network.
///
/// # Arguments
///
/// * `identity` - The identity of the node.
/// * `db` - The database instance for storing mailbox data.
/// * `encryption` - The encryption key for the storage.
/// * `port` - The port to listen on for incoming connections.
///
/// # Errors
///
/// This function will return an error if the mailbox node fails to start.
pub async fn run(
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
        identity,
        listen_addr,
        true,
        Some(mailbox_storage),
        vec![],
    )?;

    network_layer.bootstrap_dht()?;

    mailbox_node.run_with_network(network_layer).await
}
