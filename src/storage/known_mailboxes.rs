//! This module defines the storage interface and implementation for managing
//! known mailbox nodes, including their performance statistics.
use crate::crypto::StorageEncryption;
use anyhow::Result;
use async_trait::async_trait;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a known mailbox node with its associated performance statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownMailbox {
    /// The `PeerId` of the mailbox node.
    pub peer_id: PeerId,
    /// The timestamp of when the mailbox was last seen.
    pub last_seen: i64,
    /// The number of successful interactions with this mailbox.
    pub success_count: u32,
    /// The number of failed interactions with this mailbox.
    pub failure_count: u32,
}

impl KnownMailbox {
    /// Creates a new `KnownMailbox` entry.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the new mailbox.
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            last_seen: current_timestamp(),
            success_count: 0,
            failure_count: 0,
        }
    }

    /// Updates the `last_seen` timestamp to the current time.
    pub fn touch(&mut self) {
        self.last_seen = current_timestamp();
    }
}

/// Returns the current Unix timestamp in milliseconds.
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// A trait for managing known mailbox nodes.
#[async_trait]
pub trait KnownMailboxesStore: Send + Sync {
    /// Adds a new `KnownMailbox` to the store.
    async fn add_mailbox(&self, mailbox: KnownMailbox) -> Result<()>;
    /// Retrieves a `KnownMailbox` by its `PeerId`.
    async fn get_mailbox(&self, peer_id: &PeerId) -> Result<Option<KnownMailbox>>;
    /// Lists all known mailboxes.
    async fn list_mailboxes(&self) -> Result<Vec<KnownMailbox>>;
    /// Removes a `KnownMailbox` from the store.
    async fn remove_mailbox(&self, peer_id: &PeerId) -> Result<()>;
    /// Increments the success count for a mailbox.
    async fn increment_success(&self, peer_id: &PeerId) -> Result<()>;
    /// Increments the failure count for a mailbox.
    async fn increment_failure(&self, peer_id: &PeerId) -> Result<()>;
}

/// A `KnownMailboxesStore` implementation using `sled` for storage.
pub struct SledKnownMailboxesStore {
    tree: sled::Tree,
    encryption: Option<StorageEncryption>,
}

impl SledKnownMailboxesStore {
    /// Creates a new `SledKnownMailboxesStore`.
    ///
    /// # Arguments
    ///
    /// * `db` - The `sled::Db` instance to use for storage.
    /// * `encryption` - Optional `StorageEncryption` for encrypting data.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `sled` tree cannot be opened.
    pub fn new(db: Db, encryption: Option<StorageEncryption>) -> Result<Self> {
        let tree = db.open_tree("known_mailboxes")?;
        Ok(Self { tree, encryption })
    }

    /// Serializes a `KnownMailbox` and optionally encrypts it.
    fn serialize_mailbox(&self, mailbox: &KnownMailbox) -> Result<Vec<u8>> {
        let serialized = serde_json::to_vec(mailbox)?;

        if let Some(ref encryption) = self.encryption {
            encryption.encrypt_value(&serialized)
        } else {
            Ok(serialized)
        }
    }

    /// Deserializes a `KnownMailbox` and optionally decrypts it.
    fn deserialize_mailbox(&self, data: &[u8]) -> Result<KnownMailbox> {
        let decrypted = if let Some(ref encryption) = self.encryption {
            encryption.decrypt_value(data)?
        } else {
            data.to_vec()
        };

        Ok(serde_json::from_slice(&decrypted)?)
    }
}

#[async_trait]
impl KnownMailboxesStore for SledKnownMailboxesStore {
    async fn add_mailbox(&self, mailbox: KnownMailbox) -> Result<()> {
        let key = mailbox.peer_id.to_bytes();
        let value = self.serialize_mailbox(&mailbox)?;
        self.tree.insert(key, value)?;
        self.tree.flush_async().await?;
        Ok(())
    }

    async fn get_mailbox(&self, peer_id: &PeerId) -> Result<Option<KnownMailbox>> {
        let key = peer_id.to_bytes();
        match self.tree.get(key)? {
            Some(data) => Ok(Some(self.deserialize_mailbox(&data)?)),
            None => Ok(None),
        }
    }

    async fn list_mailboxes(&self) -> Result<Vec<KnownMailbox>> {
        let mut mailboxes = Vec::new();

        for result in self.tree.iter() {
            let (_key, value) = result?;
            mailboxes.push(self.deserialize_mailbox(&value)?);
        }

        Ok(mailboxes)
    }

    async fn remove_mailbox(&self, peer_id: &PeerId) -> Result<()> {
        let key = peer_id.to_bytes();
        self.tree.remove(key)?;
        self.tree.flush_async().await?;
        Ok(())
    }

    async fn increment_success(&self, peer_id: &PeerId) -> Result<()> {
        if let Some(mut mailbox) = self.get_mailbox(peer_id).await? {
            mailbox.success_count += 1;
            mailbox.failure_count = 0; // Reset consecutive failures
            mailbox.touch();
            self.add_mailbox(mailbox).await?;
        }
        Ok(())
    }

    async fn increment_failure(&self, peer_id: &PeerId) -> Result<()> {
        if let Some(mut mailbox) = self.get_mailbox(peer_id).await? {
            mailbox.failure_count += 1;
            mailbox.touch();
            self.add_mailbox(mailbox).await?;
        }
        Ok(())
    }
}
