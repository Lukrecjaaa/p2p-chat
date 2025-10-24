use crate::crypto::StorageEncryption;
use anyhow::Result;
use async_trait::async_trait;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownMailbox {
    pub peer_id: PeerId,
    pub last_seen: i64,
    pub success_count: u32,
    pub failure_count: u32,
}

impl KnownMailbox {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            last_seen: current_timestamp(),
            success_count: 0,
            failure_count: 0,
        }
    }

    pub fn touch(&mut self) {
        self.last_seen = current_timestamp();
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[async_trait]
pub trait KnownMailboxesStore: Send + Sync {
    async fn add_mailbox(&self, mailbox: KnownMailbox) -> Result<()>;
    async fn get_mailbox(&self, peer_id: &PeerId) -> Result<Option<KnownMailbox>>;
    async fn list_mailboxes(&self) -> Result<Vec<KnownMailbox>>;
    async fn remove_mailbox(&self, peer_id: &PeerId) -> Result<()>;
    async fn increment_success(&self, peer_id: &PeerId) -> Result<()>;
    async fn increment_failure(&self, peer_id: &PeerId) -> Result<()>;
}

pub struct SledKnownMailboxesStore {
    tree: sled::Tree,
    encryption: Option<StorageEncryption>,
}

impl SledKnownMailboxesStore {
    pub fn new(db: Db, encryption: Option<StorageEncryption>) -> Result<Self> {
        let tree = db.open_tree("known_mailboxes")?;
        Ok(Self { tree, encryption })
    }

    fn serialize_mailbox(&self, mailbox: &KnownMailbox) -> Result<Vec<u8>> {
        let serialized = serde_json::to_vec(mailbox)?;

        if let Some(ref encryption) = self.encryption {
            encryption.encrypt_value(&serialized)
        } else {
            Ok(serialized)
        }
    }

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
