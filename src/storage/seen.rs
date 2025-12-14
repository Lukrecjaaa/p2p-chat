//! This module provides an interface and implementation for tracking messages that have been seen.
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use sled::Db;
use std::time::Duration;
use uuid::Uuid;

/// A trait for tracking seen messages.
#[async_trait]
pub trait SeenTracker {
    /// Marks a message as seen.
    ///
    /// # Arguments
    ///
    /// * `msg_id` - The `Uuid` of the message to mark as seen.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be marked as seen.
    async fn mark_seen(&self, msg_id: Uuid) -> Result<()>;

    /// Checks if a message has been seen.
    ///
    /// # Arguments
    ///
    /// * `msg_id` - The `Uuid` of the message to check.
    ///
    /// # Returns
    ///
    /// `true` if the message has been seen, `false` otherwise.
    ///
    /// # Errors
    ///
    /// This function will return an error if the seen status cannot be retrieved.
    async fn is_seen(&self, msg_id: &Uuid) -> Result<bool>;

    /// Cleans up old seen message records.
    ///
    /// # Arguments
    ///
    /// * `max_age` - The maximum age for seen records to be retained.
    ///
    /// # Errors
    ///
    /// This function will return an error if cleanup fails.
    async fn cleanup_old(&self, max_age: Duration) -> Result<()>;
}

/// A `SeenTracker` implementation using `sled` for storage.
pub struct SledSeenTracker {
    tree: sled::Tree,
}

impl SledSeenTracker {
    /// Creates a new `SledSeenTracker`.
    ///
    /// # Arguments
    ///
    /// * `db` - The `sled::Db` instance to use for storage.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `seen` tree cannot be opened.
    pub fn new(db: Db) -> Result<Self> {
        let tree = db.open_tree("seen")?;
        Ok(Self { tree })
    }
}

#[async_trait]
impl SeenTracker for SledSeenTracker {
    async fn mark_seen(&self, msg_id: Uuid) -> Result<()> {
        let key = msg_id.to_string();
        let timestamp = Utc::now().timestamp_millis();

        self.tree.insert(key.as_bytes(), &timestamp.to_be_bytes())?;
        self.tree.flush_async().await?;
        Ok(())
    }

    async fn is_seen(&self, msg_id: &Uuid) -> Result<bool> {
        let key = msg_id.to_string();
        Ok(self.tree.contains_key(key.as_bytes())?)
    }

    async fn cleanup_old(&self, max_age: Duration) -> Result<()> {
        let cutoff = Utc::now().timestamp_millis() - max_age.as_millis() as i64;
        let mut keys_to_remove = Vec::new();

        for result in self.tree.iter() {
            let (key, value) = result?;
            if value.len() >= 8 {
                let timestamp = i64::from_be_bytes(value[..8].try_into().unwrap());
                if timestamp < cutoff {
                    keys_to_remove.push(key.to_vec());
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
