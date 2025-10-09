use crate::crypto::StorageEncryption;
use crate::types::Message;
use anyhow::Result;
use async_trait::async_trait;
use sled::Db;
use uuid::Uuid;

#[async_trait]
pub trait OutboxStore {
    async fn add_pending(&self, msg: Message) -> Result<()>;
    async fn get_pending(&self) -> Result<Vec<Message>>;
    async fn remove_pending(&self, msg_id: &Uuid) -> Result<()>;
}

pub struct SledOutboxStore {
    tree: sled::Tree,
    encryption: Option<StorageEncryption>,
}

impl SledOutboxStore {
    pub fn new(db: Db, encryption: Option<StorageEncryption>) -> Result<Self> {
        let tree = db.open_tree("outbox")?;
        Ok(Self { tree, encryption })
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

        // Sort by timestamp for consistent ordering
        messages.sort_by_key(|msg| msg.timestamp);
        Ok(messages)
    }

    async fn remove_pending(&self, msg_id: &Uuid) -> Result<()> {
        let key = msg_id.to_string();
        self.tree.remove(key.as_bytes())?;
        self.tree.flush_async().await?;
        Ok(())
    }
}
