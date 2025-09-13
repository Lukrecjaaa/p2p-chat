use crate::crypto::StorageEncryption;
use crate::types::Message;
use anyhow::Result;
use async_trait::async_trait;
use libp2p::PeerId;
use sled::Db;

#[async_trait]
pub trait MessageStore {
    async fn store_message(&self, msg: Message) -> Result<()>;
    async fn get_history(&self, own_id: &PeerId, peer: &PeerId, limit: usize)
        -> Result<Vec<Message>>;
}

pub struct MessageHistory {
    tree: sled::Tree,
    encryption: Option<StorageEncryption>,
}

impl MessageHistory {
    pub fn new(db: Db, encryption: Option<StorageEncryption>) -> Result<Self> {
        let tree = db.open_tree("history")?;
        Ok(Self { tree, encryption })
    }

    /// Creates a canonical, ordered conversation ID from two PeerIds.
    fn get_conversation_id(p1: &PeerId, p2: &PeerId) -> Vec<u8> {
        let mut p1_bytes = p1.to_bytes();
        let mut p2_bytes = p2.to_bytes();

        if p1_bytes > p2_bytes {
            std::mem::swap(&mut p1_bytes, &mut p2_bytes);
        }

        [p1_bytes, p2_bytes].concat()
    }

    fn make_composite_key(
        conversation_id: &[u8],
        timestamp: i64,
        nonce: u64,
    ) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(conversation_id);
        key.extend_from_slice(&timestamp.to_be_bytes());
        key.extend_from_slice(&nonce.to_be_bytes());
        key
    }

    fn serialize_message(&self, msg: &Message) -> Result<Vec<u8>> {
        let serialized = serde_json::to_vec(msg)?;

        if let Some(ref encryption) = self.encryption {
            encryption.encrypt_value(&serialized)
        } else {
            Ok(serialized)
        }
    }

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

    async fn get_history(
        &self,
        own_id: &PeerId,
        peer: &PeerId,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let conversation_id = Self::get_conversation_id(own_id, peer);
        let mut messages = Vec::new();

        // Iterate in reverse to get most recent messages first
        for result in self.tree.scan_prefix(&conversation_id).rev().take(limit) {
            let (_key, value) = result?;
            messages.push(self.deserialize_message(&value)?);
        }

        // Reverse again to get chronological order
        messages.reverse();
        Ok(messages)
    }

}