//! This module defines the storage interface and implementation for managing
//! the message history.
use crate::crypto::StorageEncryption;
use crate::types::Message;
use anyhow::Result;
use async_trait::async_trait;
use libp2p::PeerId;
use sled::Db;

/// A trait for storing and retrieving messages.
#[async_trait]
pub trait MessageStore {
    /// Stores a message in the history.
    ///
    /// # Arguments
    ///
    /// * `msg` - The `Message` to store.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be stored.
    async fn store_message(&self, msg: Message) -> Result<()>;

    /// Retrieves a message by its ID.
    ///
    /// # Arguments
    ///
    /// * `msg_id` - The `Uuid` of the message to retrieve.
    ///
    /// # Returns
    ///
    /// An `Option` containing the `Message` if found, otherwise `None`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be retrieved.
    async fn get_message_by_id(&self, msg_id: &uuid::Uuid) -> Result<Option<Message>>;

    /// Retrieves the message history for a conversation.
    ///
    /// Messages are returned in chronological order.
    ///
    /// # Arguments
    ///
    /// * `own_id` - The `PeerId` of the local user.
    /// * `peer` - The `PeerId` of the other participant in the conversation.
    /// * `limit` - The maximum number of messages to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec` of `Message`s representing the conversation history.
    ///
    /// # Errors
    ///
    /// This function will return an error if the history cannot be retrieved.
    async fn get_history(
        &self,
        own_id: &PeerId,
        peer: &PeerId,
        limit: usize,
    ) -> Result<Vec<Message>>;

    /// Retrieves a limited number of the most recent messages from all conversations.
    ///
    /// Messages are returned in chronological order, up to the specified limit.
    ///
    /// # Arguments
    ///
    /// * `own_id` - The `PeerId` of the local user (used for filtering relevant messages).
    /// * `limit` - The maximum number of recent messages to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec` of `Message`s, sorted chronologically.
    ///
    /// # Errors
    ///
    /// This function will return an error if the messages cannot be retrieved.
    async fn get_recent_messages(&self, own_id: &PeerId, limit: usize) -> Result<Vec<Message>>;

    /// Retrieves messages before a specific message in a conversation.
    ///
    /// Messages are returned in chronological order.
    ///
    /// # Arguments
    ///
    /// * `own_id` - The `PeerId` of the local user.
    /// * `peer` - The `PeerId` of the other participant in the conversation.
    /// * `before_id` - The `Uuid` of the message to retrieve messages before.
    /// * `limit` - The maximum number of messages to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec` of `Message`s.
    ///
    /// # Errors
    ///
    /// This function will return an error if the messages cannot be retrieved.
    async fn get_messages_before(
        &self,
        own_id: &PeerId,
        peer: &PeerId,
        before_id: &uuid::Uuid,
        limit: usize,
    ) -> Result<Vec<Message>>;

    /// Retrieves messages after a specific message in a conversation.
    ///
    /// Messages are returned in chronological order.
    ///
    /// # Arguments
    ///
    /// * `own_id` - The `PeerId` of the local user.
    /// * `peer` - The `PeerId` of the other participant in the conversation.
    /// * `after_id` - The `Uuid` of the message to retrieve messages after.
    /// * `limit` - The maximum number of messages to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec` of `Message`s.
    ///
    /// # Errors
    ///
    /// This function will return an error if the messages cannot be retrieved.
    async fn get_messages_after(
        &self,
        own_id: &PeerId,
        peer: &PeerId,
        after_id: &uuid::Uuid,
        limit: usize,
    ) -> Result<Vec<Message>>;

    /// Updates the delivery status of a message.
    ///
    /// # Arguments
    ///
    /// * `msg_id` - The `Uuid` of the message to update.
    /// * `status` - The new `DeliveryStatus` for the message.
    ///
    /// # Errors
    ///
    /// This function will return an error if the status cannot be updated.
    async fn update_delivery_status(
        &self,
        msg_id: &uuid::Uuid,
        status: crate::types::DeliveryStatus,
    ) -> Result<()>;
}

/// A `MessageStore` implementation using `sled` for storage.
pub struct MessageHistory {
    tree: sled::Tree,
    encryption: Option<StorageEncryption>,
}

impl MessageHistory {
    /// Creates a new `MessageHistory` store.
    ///
    /// # Arguments
    ///
    /// * `db` - The `sled::Db` instance to use for storage.
    /// * `encryption` - Optional `StorageEncryption` for encrypting message data.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `sled` tree cannot be opened.
    pub fn new(db: Db, encryption: Option<StorageEncryption>) -> Result<Self> {
        let tree = db.open_tree("history")?;
        Ok(Self { tree, encryption })
    }

    /// Creates a canonical, ordered conversation ID from two `PeerId`s.
    ///
    /// This ensures that the conversation ID is always the same regardless of
    /// the order of the `PeerId`s.
    fn get_conversation_id(p1: &PeerId, p2: &PeerId) -> Vec<u8> {
        let mut p1_bytes = p1.to_bytes();
        let mut p2_bytes = p2.to_bytes();

        if p1_bytes > p2_bytes {
            std::mem::swap(&mut p1_bytes, &mut p2_bytes);
        }

        [p1_bytes, p2_bytes].concat()
    }

    /// Creates a composite key for storing a message, based on conversation ID, timestamp, and nonce.
    fn make_composite_key(conversation_id: &[u8], timestamp: i64, nonce: u64) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(conversation_id);
        key.extend_from_slice(&timestamp.to_be_bytes());
        key.extend_from_slice(&nonce.to_be_bytes());
        key
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
impl MessageStore for MessageHistory {
    async fn store_message(&self, msg: Message) -> Result<()> {
        let conversation_id = Self::get_conversation_id(&msg.sender, &msg.recipient);
        let key = Self::make_composite_key(&conversation_id, msg.timestamp, msg.nonce);
        let value = self.serialize_message(&msg)?;

        self.tree.insert(key, value)?;
        self.tree.flush_async().await?;
        Ok(())
    }

    async fn get_message_by_id(&self, msg_id: &uuid::Uuid) -> Result<Option<Message>> {
        // Scan all messages to find the one with the given ID.
        for result in self.tree.iter() {
            let (_key, value) = result?;
            let msg = self.deserialize_message(&value)?;

            if msg.id == *msg_id {
                return Ok(Some(msg));
            }
        }

        Ok(None)
    }

    async fn get_history(
        &self,
        own_id: &PeerId,
        peer: &PeerId,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let conversation_id = Self::get_conversation_id(own_id, peer);
        let mut messages = Vec::new();

        // Iterate in reverse to get most recent messages first.
        for result in self.tree.scan_prefix(&conversation_id).rev().take(limit) {
            let (_key, value) = result?;
            messages.push(self.deserialize_message(&value)?);
        }

        // Reverse again to get chronological order.
        messages.reverse();
        Ok(messages)
    }

    async fn get_recent_messages(&self, own_id: &PeerId, limit: usize) -> Result<Vec<Message>> {
        let tree = self.tree.clone();
        let encryption = self.encryption.clone();
        let own_id = *own_id;

        let mut messages: Vec<Message> =
            tokio::task::spawn_blocking(move || -> Result<Vec<Message>> {
                let mut collected = Vec::new();
                for result in tree.iter() {
                    let (_key, value) = result?;
                    let decrypted = if let Some(ref enc) = encryption {
                        enc.decrypt_value(&value)?
                    } else {
                        value.to_vec()
                    };
                    let message: Message = serde_json::from_slice(&decrypted)?;
                    if message.sender == own_id || message.recipient == own_id {
                        collected.push(message);
                    }
                }
                Ok(collected)
            })
            .await??;

        messages.sort_by_key(|msg| (msg.timestamp, msg.nonce));

        if messages.len() > limit {
            let drop_count = messages.len() - limit;
            messages.drain(0..drop_count);
        }

        Ok(messages)
    }

    async fn get_messages_before(
        &self,
        own_id: &PeerId,
        peer: &PeerId,
        before_id: &uuid::Uuid,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let conversation_id = Self::get_conversation_id(own_id, peer);

        // First, find the message with before_id to get its timestamp.
        let mut before_timestamp = None;
        let mut before_nonce = None;

        for result in self.tree.scan_prefix(&conversation_id) {
            let (_key, value) = result?;
            let msg = self.deserialize_message(&value)?;
            if msg.id == *before_id {
                before_timestamp = Some(msg.timestamp);
                before_nonce = Some(msg.nonce);
                break;
            }
        }

        let (before_ts, before_n) = match (before_timestamp, before_nonce) {
            (Some(ts), Some(n)) => (ts, n),
            _ => return Ok(Vec::new()), // Message not found.
        };

        // Collect all messages before this timestamp+nonce.
        let mut messages = Vec::new();
        for result in self.tree.scan_prefix(&conversation_id) {
            let (_key, value) = result?;
            let msg = self.deserialize_message(&value)?;

            // Include messages that are strictly before (timestamp, nonce).
            if msg.timestamp < before_ts || (msg.timestamp == before_ts && msg.nonce < before_n) {
                messages.push(msg);
            }
        }

        // Sort by timestamp and nonce, take last N (most recent before the target).
        messages.sort_by_key(|msg| (msg.timestamp, msg.nonce));
        if messages.len() > limit {
            let start_idx = messages.len() - limit;
            messages.drain(0..start_idx);
        }

        Ok(messages)
    }

    async fn get_messages_after(
        &self,
        own_id: &PeerId,
        peer: &PeerId,
        after_id: &uuid::Uuid,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let conversation_id = Self::get_conversation_id(own_id, peer);

        // First, find the message with after_id to get its timestamp.
        let mut after_timestamp = None;
        let mut after_nonce = None;

        for result in self.tree.scan_prefix(&conversation_id) {
            let (_key, value) = result?;
            let msg = self.deserialize_message(&value)?;
            if msg.id == *after_id {
                after_timestamp = Some(msg.timestamp);
                after_nonce = Some(msg.nonce);
                break;
            }
        }

        let (after_ts, after_n) = match (after_timestamp, after_nonce) {
            (Some(ts), Some(n)) => (ts, n),
            _ => return Ok(Vec::new()), // Message not found.
        };

        // Collect messages after this timestamp+nonce.
        let mut messages = Vec::new();
        for result in self.tree.scan_prefix(&conversation_id) {
            let (_key, value) = result?;
            let msg = self.deserialize_message(&value)?;

            // Include messages that are strictly after (timestamp, nonce).
            if msg.timestamp > after_ts || (msg.timestamp == after_ts && msg.nonce > after_n) {
                messages.push(msg);
                if messages.len() >= limit {
                    break;
                }
            }
        }

        // Sort by timestamp and nonce.
        messages.sort_by_key(|msg| (msg.timestamp, msg.nonce));

        Ok(messages)
    }

    async fn update_delivery_status(
        &self,
        msg_id: &uuid::Uuid,
        status: crate::types::DeliveryStatus,
    ) -> Result<()> {
        // Scan all messages to find the one with the given ID.
        for result in self.tree.iter() {
            let (key, value) = result?;
            let mut msg = self.deserialize_message(&value)?;

            if msg.id == *msg_id {
                // Update the delivery status.
                msg.delivery_status = status;

                // Re-serialize and store.
                let new_value = self.serialize_message(&msg)?;
                self.tree.insert(key, new_value)?;
                self.tree.flush_async().await?;
                return Ok(());
            }
        }

        // Message not found - not necessarily an error, might be old/deleted.
        Ok(())
    }
}
