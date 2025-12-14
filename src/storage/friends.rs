//! This module defines the storage interface and implementation for managing friends.
use crate::crypto::StorageEncryption;
use crate::types::Friend;
use anyhow::Result;
use async_trait::async_trait;
use libp2p::PeerId;
use sled::Db;

/// A trait for managing friends.
#[async_trait]
pub trait FriendsStore {
    /// Adds a new friend to the store.
    ///
    /// # Arguments
    ///
    /// * `friend` - The `Friend` to add.
    ///
    /// # Errors
    ///
    /// This function will return an error if the friend cannot be added.
    async fn add_friend(&self, friend: Friend) -> Result<()>;

    /// Retrieves a friend from the store by their `PeerId`.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the friend to retrieve.
    ///
    /// # Returns
    ///
    /// An `Option` containing the `Friend` if found, otherwise `None`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the friend cannot be retrieved.
    async fn get_friend(&self, peer_id: &PeerId) -> Result<Option<Friend>>;

    /// Lists all friends in the store.
    ///
    /// # Returns
    ///
    /// A `Vec` containing all `Friend`s in the store.
    ///
    /// # Errors
    ///
    /// This function will return an error if the friends cannot be listed.
    async fn list_friends(&self) -> Result<Vec<Friend>>;
}

/// A `FriendsStore` implementation using `sled` for storage.
pub struct SledFriendsStore {
    tree: sled::Tree,
    encryption: Option<StorageEncryption>,
}

impl SledFriendsStore {
    /// Creates a new `SledFriendsStore`.
    ///
    /// # Arguments
    ///
    /// * `db` - The `sled::Db` instance to use for storage.
    /// * `encryption` - The optional `StorageEncryption` to use for encrypting friend data.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `friends` tree cannot be opened.
    pub fn new(db: Db, encryption: Option<StorageEncryption>) -> Result<Self> {
        let tree = db.open_tree("friends")?;
        Ok(Self { tree, encryption })
    }

    /// Serializes a `Friend` and encrypts it if encryption is enabled.
    fn serialize_friend(&self, friend: &Friend) -> Result<Vec<u8>> {
        let serialized = serde_json::to_vec(friend)?;

        if let Some(ref encryption) = self.encryption {
            encryption.encrypt_value(&serialized)
        } else {
            Ok(serialized)
        }
    }

    /// Decrypts and deserializes a `Friend`.
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
