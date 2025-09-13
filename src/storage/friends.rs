use crate::types::Friend;
use crate::crypto::StorageEncryption;
use anyhow::Result;
use async_trait::async_trait;
use libp2p::PeerId;
use sled::Db;

#[async_trait]
pub trait FriendsStore {
    async fn add_friend(&self, friend: Friend) -> Result<()>;
    async fn get_friend(&self, peer_id: &PeerId) -> Result<Option<Friend>>;
    async fn list_friends(&self) -> Result<Vec<Friend>>;
}

pub struct SledFriendsStore {
    tree: sled::Tree,
    encryption: Option<StorageEncryption>,
}

impl SledFriendsStore {
    pub fn new(db: Db, encryption: Option<StorageEncryption>) -> Result<Self> {
        let tree = db.open_tree("friends")?;
        Ok(Self { tree, encryption })
    }

    fn serialize_friend(&self, friend: &Friend) -> Result<Vec<u8>> {
        let serialized = serde_json::to_vec(friend)?;
        
        if let Some(ref encryption) = self.encryption {
            encryption.encrypt_value(&serialized)
        } else {
            Ok(serialized)
        }
    }

    fn deserialize_friend(&self, data: &[u8]) -> Result<Friend> {
        let decrypted = if let Some(ref encryption) = self.encryption {
            encryption.decrypt_value(data)?
        } else {
            data.to_vec()
        };

        Ok(serde_json::from_slice(&decrypted)?)
    }
}

#[async_trait]
impl FriendsStore for SledFriendsStore {
    async fn add_friend(&self, friend: Friend) -> Result<()> {
        let key = friend.peer_id.to_bytes();
        let value = self.serialize_friend(&friend)?;
        self.tree.insert(key, value)?;
        self.tree.flush_async().await?;
        Ok(())
    }

    async fn get_friend(&self, peer_id: &PeerId) -> Result<Option<Friend>> {
        let key = peer_id.to_bytes();
        match self.tree.get(key)? {
            Some(data) => Ok(Some(self.deserialize_friend(&data)?)),
            None => Ok(None),
        }
    }

    async fn list_friends(&self) -> Result<Vec<Friend>> {
        let mut friends = Vec::new();
        
        for result in self.tree.iter() {
            let (_key, value) = result?;
            friends.push(self.deserialize_friend(&value)?);
        }
        
        Ok(friends)
    }

}