//! This module defines the storage interface and implementation for the mailbox.
//!
//! The mailbox stores encrypted messages for recipients until they can be fetched.
mod operations;

use crate::crypto::StorageEncryption;
use crate::types::EncryptedMessage;
use anyhow::Result;
use async_trait::async_trait;
use sled::Db;
use uuid::Uuid;

/// A trait for managing mailbox operations.
#[async_trait]
pub trait MailboxStore {
    /// Stores an encrypted message for a recipient.
    ///
    /// # Arguments
    ///
    /// * `recipient_hash` - The hash of the recipient's public key.
    /// * `msg` - The `EncryptedMessage` to store.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be stored.
    async fn store_message(&self, recipient_hash: [u8; 32], msg: EncryptedMessage) -> Result<()>;

    /// Fetches messages for a recipient.
    ///
    /// # Arguments
    ///
    /// * `recipient_hash` - The hash of the recipient's public key.
    /// * `limit` - The maximum number of messages to fetch.
    ///
    /// # Returns
    ///
    /// A `Vec` of `EncryptedMessage`s for the recipient.
    ///
    /// # Errors
    ///
    /// This function will return an error if the messages cannot be fetched.
    async fn fetch_messages(
        &self,
        recipient_hash: [u8; 32],
        limit: usize,
    ) -> Result<Vec<EncryptedMessage>>;

    /// Deletes messages for a recipient.
    ///
    /// # Arguments
    ///
    /// * `recipient_hash` - The hash of the recipient's public key.
    /// * `msg_ids` - A `Vec` of message IDs to delete.
    ///
    /// # Returns
    ///
    /// The number of messages that were deleted.
    ///
    /// # Errors
    ///
    /// This function will return an error if the messages cannot be deleted.
    async fn delete_messages(&self, recipient_hash: [u8; 32], msg_ids: Vec<Uuid>) -> Result<usize>;

    /// Cleans up expired messages from the mailbox.
    ///
    /// # Arguments
    ///
    /// * `max_age` - The maximum age for messages to be retained.
    ///
    /// # Errors
    ///
    /// This function will return an error if cleanup fails.
    async fn cleanup_expired(&self, max_age: std::time::Duration) -> Result<()>;
}

/// A `MailboxStore` implementation using `sled` for storage.
pub struct SledMailboxStore {
    pub(crate) tree: sled::Tree,
    pub(crate) encryption: Option<StorageEncryption>,
    pub(crate) max_storage_per_user: usize,
}

impl SledMailboxStore {
    /// Creates a new `SledMailboxStore`.
    ///
    /// # Arguments
    ///
    /// * `db` - The `sled::Db` instance to use for storage.
    /// * `encryption` - The optional `StorageEncryption` to use for encrypting messages.
    /// * `max_storage_per_user` - The maximum number of messages to store per user.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `mailbox` tree cannot be opened.
    pub fn new(
        db: Db,
        encryption: Option<StorageEncryption>,
        max_storage_per_user: usize,
    ) -> Result<Self> {
        let tree = db.open_tree("mailbox")?;
        Ok(Self {
            tree,
            encryption,
            max_storage_per_user,
        })
    }

    /// Creates a unique key for a message in the mailbox.
    pub(crate) fn make_message_key(&self, recipient_hash: &[u8; 32], msg_id: &Uuid) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(recipient_hash);
        key.extend_from_slice(msg_id.as_bytes());
        key
    }

    /// Serializes an `EncryptedMessage` and encrypts it if encryption is enabled.
    pub(crate) fn serialize_message(&self, msg: &EncryptedMessage) -> Result<Vec<u8>> {
        let serialized = serde_json::to_vec(msg)?;

        if let Some(ref encryption) = self.encryption {
            encryption.encrypt_value(&serialized)
        } else {
            Ok(serialized)
        }
    }

    /// Decrypts and deserializes an `EncryptedMessage`.
    pub(crate) fn deserialize_message(&self, data: &[u8]) -> Result<EncryptedMessage> {
        let decrypted = if let Some(ref encryption) = self.encryption {
            encryption.decrypt_value(data)?
        } else {
            data.to_vec()
        };

        Ok(serde_json::from_slice(&decrypted)?)
    }
}
