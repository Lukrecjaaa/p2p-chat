//! This module defines the `MailboxNode`, which is responsible for storing and
//! forwarding messages for other peers in the network.
use crate::crypto::{Identity, StorageEncryption};
use crate::network::NetworkLayer;
use crate::storage::{MailboxStore, SledMailboxStore};
use anyhow::Result;
use libp2p::kad;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, trace};

/// Represents a mailbox node in the network.
///
/// A mailbox node stores messages for other peers and forwards them upon request.
pub struct MailboxNode {
    /// The identity of the mailbox node.
    pub identity: Arc<Identity>,
    /// The storage for mailbox messages.
    pub storage: Arc<SledMailboxStore>,
    /// The maximum number of messages to store per user.
    pub max_storage_per_user: usize,
    /// The duration for which messages are retained.
    pub retention_period: Duration,
}

impl MailboxNode {
    /// Creates a new `MailboxNode`.
    ///
    /// # Arguments
    ///
    /// * `identity` - The identity of the node.
    /// * `db` - The database instance for storing mailbox data.
    /// * `encryption` - The encryption key for the storage.
    /// * `max_storage_per_user` - The maximum number of messages to store per user.
    /// * `retention_period` - The duration for which messages are retained.
    ///
    /// # Errors
    ///
    /// This function will return an error if the mailbox storage cannot be created.
    pub fn new(
        identity: Arc<Identity>,
        db: sled::Db,
        encryption: Option<StorageEncryption>,
        max_storage_per_user: usize,
        retention_period: Duration,
    ) -> Result<Self> {
        let storage = Arc::new(SledMailboxStore::new(db, encryption, max_storage_per_user)?);

        Ok(Self {
            identity,
            storage,
            max_storage_per_user,
            retention_period,
        })
    }

    /// Runs the mailbox node with the given network layer.
    ///
    /// This function starts the mailbox node and its associated tasks, such as
    /// the cleanup task and the network event loop.
    ///
    /// # Arguments
    ///
    /// * `network_layer` - The network layer to use for communication.
    ///
    /// # Errors
    ///
    /// This function will return an error if the mailbox node fails to run.
    pub async fn run_with_network(&mut self, network_layer: NetworkLayer) -> Result<()> {
        info!(
            "Starting mailbox node with network layer: {}",
            self.identity.peer_id
        );
        info!(
            "Max storage per user: {} messages",
            self.max_storage_per_user
        );
        info!("Retention period: {:?}", self.retention_period);

        // Start the cleanup task.
        let storage_clone = self.storage.clone();
        let retention_period = self.retention_period;
        tokio::spawn(async move {
            Self::cleanup_task(storage_clone, retention_period).await;
        });

        // Channel for incoming messages (mailbox nodes don't need to handle chat messages).
        let (_incoming_tx, _incoming_rx) = mpsc::unbounded_channel::<crate::types::Message>();

        // Start network layer with mailbox request handling.
        let storage_for_network = self.storage.clone();
        tokio::spawn(async move {
            // Custom network event loop for mailbox node.
            if let Err(e) = Self::run_mailbox_network_loop(network_layer, storage_for_network).await
            {
                error!("Mailbox network loop error: {}", e);
            }
        });

        // Keep the main task alive.
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            info!("Mailbox node still running...");
        }
    }

    /// Runs the network event loop for the mailbox node.
    async fn run_mailbox_network_loop(
        mut network_layer: NetworkLayer,
        _storage: Arc<SledMailboxStore>,
    ) -> Result<()> {
        info!("Starting mailbox network event loop");

        // Register as a general mailbox provider in the DHT.
        if let Err(e) = network_layer.start_providing_mailbox() {
            error!("Failed to register as mailbox provider: {}", e);
        } else {
            info!("Successfully registered as mailbox provider in DHT");
        }

        let (incoming_tx, mut incoming_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Some(_message) = incoming_rx.recv().await {
                // Mailbox nodes don't typically handle chat messages directly.
                debug!("Received message in mailbox node (ignoring)");
            }
        });

        // Run the network layer.
        network_layer.run(incoming_tx).await
    }

    /// Periodically cleans up expired messages from the storage.
    async fn cleanup_task(storage: Arc<SledMailboxStore>, retention_period: Duration) {
        let mut cleanup_interval = interval(Duration::from_secs(60 * 60)); // 1 hour

        info!(
            "Starting cleanup task with retention period: {:?}",
            retention_period
        );

        loop {
            cleanup_interval.tick().await;

            trace!("Running message cleanup");

            if let Err(e) = storage.cleanup_expired(retention_period).await {
                error!("Cleanup failed: {}", e);
            } else {
                trace!("Cleanup completed successfully");
            }
        }
    }

    /// Returns statistics about the mailbox node.
    pub fn get_stats(&self) -> MailboxStats {
        MailboxStats {
            max_storage_per_user: self.max_storage_per_user,
            retention_period: self.retention_period,
        }
    }
}

/// Contains statistics about a mailbox node.
#[derive(Debug)]
pub struct MailboxStats {
    /// The maximum number of messages to store per user.
    pub max_storage_per_user: usize,
    /// The duration for which messages are retained.
    pub retention_period: Duration,
}

/// Creates a Kademlia record key for discovering mailbox providers.
pub fn make_mailbox_provider_key() -> kad::RecordKey {
    kad::RecordKey::new(&b"mailbox-providers".to_vec())
}

/// Creates a Kademlia record key for discovering the mailboxes for a specific recipient.
pub fn make_recipient_mailbox_key(recipient_hash: [u8; 32]) -> kad::RecordKey {
    kad::RecordKey::new(&format!("recipient-mailbox/{}", hex::encode(recipient_hash)).into_bytes())
}
