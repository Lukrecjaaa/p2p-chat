//! This module implements the `MailboxStore` trait for `SledMailboxStore`.
use super::{MailboxStore, SledMailboxStore};
use crate::types::EncryptedMessage;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use std::time::Duration;
use tracing::warn;
use uuid::Uuid;

#[async_trait]
impl MailboxStore for SledMailboxStore {
    /// Stores an encrypted message for a recipient in the mailbox.
    ///
    /// This function also enforces storage limits by removing the oldest messages
    /// if the maximum storage per user is exceeded.
    ///
    /// # Arguments
    ///
    /// * `recipient_hash` - The hash of the recipient's public key.
    /// * `msg` - The `EncryptedMessage` to store.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be stored or if
    /// there are issues with the underlying `sled` database.
    async fn store_message(&self, recipient_hash: [u8; 32], msg: EncryptedMessage) -> Result<()> {
        let key = self.make_message_key(&recipient_hash, &msg.id);
        let value = self.serialize_message(&msg)?;

        self.tree.insert(key, value)?;

        // Enforce storage limits by cleaning up old messages if necessary
        let mut existing: Vec<(Vec<u8>, i64, u64)> = Vec::new();
        for entry in self.tree.scan_prefix(recipient_hash) {
            match entry {
                Ok((key, value)) => match self.deserialize_message(&value) {
                    Ok(existing_msg) => {
                        existing.push((key.to_vec(), existing_msg.timestamp, existing_msg.nonce));
                    }
                    Err(err) => {
                        warn!(
                            "Dropping corrupt mailbox message for recipient {:?}: {}",
                            &recipient_hash[..8],
                            err
                        );
                        self.tree.remove(&key)?;
                    }
                },
                Err(err) => {
                    warn!(
                        "Failed to iterate mailbox entries for recipient {:?}: {}",
                        &recipient_hash[..8],
                        err
                    );
                }
            }
        }

        if existing.len() > self.max_storage_per_user {
            existing.sort_by(|a, b| (a.1, a.2).cmp(&(b.1, b.2)));
            let excess = existing.len() - self.max_storage_per_user;
            for (key, _, _) in existing.into_iter().take(excess) {
                self.tree.remove(key)?;
            }
        }

        self.tree.flush_async().await?;
        Ok(())
    }

    /// Fetches a limited number of messages for a recipient from the mailbox.
    ///
    /// The messages are sorted by timestamp and nonce for deterministic ordering.
    /// Corrupt messages encountered during fetching are logged and removed.
    ///
    /// # Arguments
    ///
    /// * `recipient_hash` - The hash of the recipient's public key.
    /// * `limit` - The maximum number of messages to fetch.
    ///
    /// # Returns
    ///
    /// A `Vec` of `EncryptedMessage`s.
    ///
    /// # Errors
    ///
    /// This function will return an error if there are issues with the underlying
    /// `sled` database.
    async fn fetch_messages(
        &self,
        recipient_hash: [u8; 32],
        limit: usize,
    ) -> Result<Vec<EncryptedMessage>> {
        let mut messages = Vec::new();

        for result in self.tree.scan_prefix(recipient_hash) {
            match result {
                Ok((key, value)) => match self.deserialize_message(&value) {
                    Ok(msg) => {
                        messages.push(msg);
                        if messages.len() >= limit {
                            break;
                        }
                    }
                    Err(err) => {
                        warn!(
                            "Removing corrupt mailbox message for recipient {:?}: {}",
                            &recipient_hash[..8],
                            err
                        );
                        self.tree.remove(&key)?;
                    }
                },
                Err(err) => {
                    warn!(
                        "Failed to iterate mailbox entries for recipient {:?}: {}",
                        &recipient_hash[..8],
                        err
                    );
                }
            }
        }

        // Sort by timestamp/nonce for deterministic ordering.
        messages.sort_by_key(|msg| (msg.timestamp, msg.nonce));
        Ok(messages)
    }

    /// Deletes specific messages for a recipient from the mailbox.
    ///
    /// # Arguments
    ///
    /// * `recipient_hash` - The hash of the recipient's public key.
    /// * `msg_ids` - A `Vec` of message IDs to delete.
    ///
    /// # Returns
    ///
    /// The number of messages successfully deleted.
    ///
    /// # Errors
    ///
    /// This function will return an error if there are issues with the underlying
    /// `sled` database.
    async fn delete_messages(&self, recipient_hash: [u8; 32], msg_ids: Vec<Uuid>) -> Result<usize> {
        let mut deleted = 0;

        for msg_id in msg_ids {
            let key = self.make_message_key(&recipient_hash, &msg_id);
            if self.tree.remove(key)?.is_some() {
                deleted += 1;
            }
        }

        self.tree.flush_async().await?;
        Ok(deleted)
    }

    /// Cleans up messages older than `max_age` from the mailbox.
    ///
    /// Corrupt messages encountered during cleanup are logged and removed.
    ///
    /// # Arguments
    ///
    /// * `max_age` - The maximum age for messages to be retained. Messages older
    ///               than this duration will be deleted.
    ///
    /// # Errors
    ///
    /// This function will return an error if there are issues with the underlying
    /// `sled` database.
    async fn cleanup_expired(&self, max_age: Duration) -> Result<()> {
        let cutoff = Utc::now().timestamp_millis() - max_age.as_millis() as i64;
        let mut keys_to_remove = Vec::new();

        for result in self.tree.iter() {
            match result {
                Ok((key, value)) => match self.deserialize_message(&value) {
                    Ok(msg) => {
                        if msg.timestamp < cutoff {
                            keys_to_remove.push(key.to_vec());
                        }
                    }
                    Err(err) => {
                        warn!("Removing corrupt mailbox entry during cleanup: {}", err);
                        keys_to_remove.push(key.to_vec());
                    }
                },
                Err(err) => {
                    warn!("Failed to iterate mailbox for cleanup: {}", err);
                }
            }
        }

        for key in keys_to_remove {
            self.tree.remove(key)?;
        }

        self.tree.flush_async().await?;
        Ok(())
    }
}
