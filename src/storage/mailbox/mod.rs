mod operations;

use crate::crypto::StorageEncryption;
use crate::types::EncryptedMessage;
use anyhow::Result;
use async_trait::async_trait;
use sled::Db;
use uuid::Uuid;

#[async_trait]
pub trait MailboxStore {
    async fn store_message(&self, recipient_hash: [u8; 32], msg: EncryptedMessage) -> Result<()>;
    async fn fetch_messages(
        &self,
        recipient_hash: [u8; 32],
        limit: usize,
    ) -> Result<Vec<EncryptedMessage>>;
    async fn delete_messages(&self, recipient_hash: [u8; 32], msg_ids: Vec<Uuid>) -> Result<usize>;
    async fn cleanup_expired(&self, max_age: std::time::Duration) -> Result<()>;
}

pub struct SledMailboxStore {
    pub(crate) tree: sled::Tree,
    pub(crate) encryption: Option<StorageEncryption>,
    pub(crate) max_storage_per_user: usize,
}

impl SledMailboxStore {
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

    pub(crate) fn make_message_key(&self, recipient_hash: &[u8; 32], msg_id: &Uuid) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(recipient_hash);
        key.extend_from_slice(msg_id.as_bytes());
        key
    }

    pub(crate) fn serialize_message(&self, msg: &EncryptedMessage) -> Result<Vec<u8>> {
        let serialized = serde_json::to_vec(msg)?;

        if let Some(ref encryption) = self.encryption {
            encryption.encrypt_value(&serialized)
        } else {
            Ok(serialized)
        }
    }

    pub(crate) fn deserialize_message(&self, data: &[u8]) -> Result<EncryptedMessage> {
        let decrypted = if let Some(ref encryption) = self.encryption {
            encryption.decrypt_value(data)?
        } else {
            data.to_vec()
        };

        Ok(serde_json::from_slice(&decrypted)?)
    }
}
