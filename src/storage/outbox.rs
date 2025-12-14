//! This module defines the storage interface and implementation for managing
//! outgoing messages that are pending delivery.
use crate::crypto::StorageEncryption;
use crate::types::Message;
use anyhow::Result;
use async_trait::async_trait;
use sled::Db;
use uuid::Uuid;

/// A trait for managing outgoing messages that are pending delivery.
#[async_trait]
pub trait OutboxStore {
    /// Adds a new message to the outbox.
    ///
    /// # Arguments
    ///
    /// * `msg` - The `Message` to add.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be added.
    async fn add_pending(&self, msg: Message) -> Result<()>;

    /// Retrieves all pending messages from the outbox.
    ///
    /// Messages are sorted by timestamp for consistent ordering.
    ///
    /// # Returns
    ///
    /// A `Vec` of `Message`s that are pending delivery.
    ///
    /// # Errors
    ///
    /// This function will return an error if the messages cannot be retrieved.
    async fn get_pending(&self) -> Result<Vec<Message>>;

    /// Removes a pending message from the outbox.
    ///
    /// # Arguments
    ///
    /// * `msg_id` - The `Uuid` of the message to remove.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be removed.
    async fn remove_pending(&self, msg_id: &Uuid) -> Result<()>;

    /// Returns the number of pending messages in the outbox.
    ///
    /// # Errors
    ///
    /// This function will return an error if the count cannot be retrieved.
    async fn count_pending(&self) -> Result<usize>;
}

/// An `OutboxStore` implementation using `sled` for storage.
pub struct SledOutboxStore {
    tree: sled::Tree,
    encryption: Option<StorageEncryption>,
}

impl SledOutboxStore {
    /// Creates a new `SledOutboxStore`.
    ///
    /// # Arguments
    ///
    /// * `db` - The `sled::Db` instance to use for storage.
    /// * `encryption` - The optional `StorageEncryption` to use for encrypting messages.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `outbox` tree cannot be opened.
    pub fn new(db: Db, encryption: Option<StorageEncryption>) -> Result<Self> {
        let tree = db.open_tree("outbox")?;
        Ok(Self { tree, encryption })
    }

    /// Serializes a `Message` and encrypts it if encryption is enabled.
    fn serialize_message(&self, msg: &Message) -> Result<Vec<u8>> {
        let serialized = serde_json::to_vec(msg)?;

        if let Some(ref encryption) = self.encryption {
            encryption.encrypt_value(&serialized)
        } else {
            Ok(serialized)
        }
    }

    /// Decrypts and deserializes a `Message`.
    fn deserialize_message(&self, data: &[u8]) -> Result<Message> {
        let decrypted = if let Some(ref encryption) = self.encryption {
            encryption.decrypt_value(data)?
        } else {
            data.to_vec()
        };

        Ok(serde_json::from_slice(&decrypted)?)
    }
}

#[async_trait]
impl OutboxStore for SledOutboxStore {
    async fn add_pending(&self, msg: Message) -> Result<()> {
        let key = msg.id.to_string();
        let value = self.serialize_message(&msg)?;

        self.tree.insert(key.as_bytes(), value)?;
        self.tree.flush_async().await?;
        Ok(())
    }

    async fn get_pending(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();

        for result in self.tree.iter() {
            let (_key, value) = result?;
            messages.push(self.deserialize_message(&value)?);
        }

        // Sort by timestamp for consistent ordering.
        messages.sort_by_key(|msg| msg.timestamp);
        Ok(messages)
    }

    async fn remove_pending(&self, msg_id: &Uuid) -> Result<()> {
        let key = msg_id.to_string();
        self.tree.remove(key.as_bytes())?;
        self.tree.flush_async().await?;
        Ok(())
    }

    async fn count_pending(&self) -> Result<usize> {
        Ok(self.tree.len())
    }
}
