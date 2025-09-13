use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use sled::Db;
use std::time::Duration;
use uuid::Uuid;

#[async_trait]
pub trait SeenTracker {
    async fn mark_seen(&self, msg_id: Uuid) -> Result<()>;
    async fn is_seen(&self, msg_id: &Uuid) -> Result<bool>;
    async fn cleanup_old(&self, max_age: Duration) -> Result<()>;
}

pub struct SledSeenTracker {
    tree: sled::Tree,
}

impl SledSeenTracker {
    pub fn new(db: Db) -> Result<Self> {
        let tree = db.open_tree("seen")?;
        Ok(Self { tree })
    }
}

#[async_trait]
impl SeenTracker for SledSeenTracker {
    async fn mark_seen(&self, msg_id: Uuid) -> Result<()> {
        let key = msg_id.to_string();
        let timestamp = Utc::now().timestamp();
        
        self.tree.insert(key.as_bytes(), &timestamp.to_be_bytes())?;
        self.tree.flush_async().await?;
        Ok(())
    }

    async fn is_seen(&self, msg_id: &Uuid) -> Result<bool> {
        let key = msg_id.to_string();
        Ok(self.tree.contains_key(key.as_bytes())?)
    }

    async fn cleanup_old(&self, max_age: Duration) -> Result<()> {
        let cutoff = Utc::now().timestamp() - max_age.as_secs() as i64;
        let mut keys_to_remove = Vec::new();
        
        for result in self.tree.iter() {
            let (key, value) = result?;
            if value.len() >= 8 {
                let timestamp = i64::from_be_bytes(
                    value[..8].try_into().unwrap()
                );
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