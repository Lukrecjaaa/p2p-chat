use crate::crypto::Identity;
use crate::network::NetworkHandle;
use crate::storage::{FriendsStore, MessageStore, OutboxStore};
use crate::sync::SyncEngine;
use crate::types::{EncryptedMessage, Friend, Message};
use anyhow::Result;
use libp2p::PeerId;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};

pub enum MailboxDeliveryResult {
    Success(usize),
    Failure,
}

pub struct Node {
    pub identity: Arc<Identity>,
    pub friends: Arc<dyn FriendsStore + Send + Sync>,
    pub history: Arc<dyn MessageStore + Send + Sync>,
    pub outbox: Arc<dyn OutboxStore + Send + Sync>,
    pub network: NetworkHandle,
    pub ui_notify_tx: mpsc::UnboundedSender<UiNotification>,
    pub sync_engine: Arc<Mutex<SyncEngine>>,
}

pub enum UiNotification {
    NewMessage(Message),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

impl Node {
    // This helper function is now more generic to be used by the active retry logic.
    pub async fn forward_to_mailboxes(
        &self,
        message: &Message,
        friend: &Friend,
        providers: &HashSet<PeerId>,
    ) -> Result<MailboxDeliveryResult> {
        if providers.is_empty() {
            return Ok(MailboxDeliveryResult::Failure);
        }

        info!(
            "Attempting to forward message to {} known mailbox providers",
            providers.len()
        );

        let recipient_hash =
            crate::crypto::StorageEncryption::derive_recipient_hash(&friend.e2e_public_key);
        let encrypted_msg = EncryptedMessage {
            id: message.id,
            sender: self.identity.peer_id,
            recipient_hash,
            encrypted_content: message.content.clone(),
            timestamp: message.timestamp,
            nonce: message.nonce,
            sender_pub_key: self.identity.hpke_public_key(),
        };

        // Try to send to at least 2 mailboxes for redundancy
        let min_replicas = 2;
        let max_attempts = providers.len().min(4); // Don't spam too many mailboxes
        let mut forwarded_count = 0;
        let mut failed_attempts = 0;

        // Get mailboxes sorted by performance instead of random shuffle
        let candidate_mailboxes = {
            let sync_engine = self.sync_engine.lock().await;
            sync_engine.rank_mailboxes_subset(providers)
        };

        for peer_id in candidate_mailboxes.iter().take(max_attempts) {
            let start_time = std::time::Instant::now();
            match self
                .network
                .mailbox_put(*peer_id, recipient_hash, encrypted_msg.clone())
                .await
            {
                Ok(true) => {
                    let response_time = start_time.elapsed();
                    info!(
                        "Successfully forwarded message {} to mailbox {} ({}/{})",
                        message.id,
                        peer_id,
                        forwarded_count + 1,
                        min_replicas
                    );
                    forwarded_count += 1;

                    // Update performance tracking (fire and forget to avoid blocking)
                    let sync_engine_clone = self.sync_engine.clone();
                    let peer_id_copy = *peer_id;
                    tokio::spawn(async move {
                        if let Ok(mut sync_engine) = sync_engine_clone.try_lock() {
                            sync_engine.update_mailbox_performance(
                                peer_id_copy,
                                true,
                                response_time,
                            ).await;
                        }
                    });

                    // Continue until we reach minimum replicas
                    if forwarded_count >= min_replicas {
                        break;
                    }
                }
                Ok(false) => {
                    let response_time = start_time.elapsed();
                    debug!("Mailbox {} rejected message {}", peer_id, message.id);
                    failed_attempts += 1;

                    // Update performance tracking (fire and forget to avoid blocking)
                    let sync_engine_clone = self.sync_engine.clone();
                    let peer_id_copy = *peer_id;
                    tokio::spawn(async move {
                        if let Ok(mut sync_engine) = sync_engine_clone.try_lock() {
                            sync_engine.update_mailbox_performance(
                                peer_id_copy,
                                false,
                                response_time,
                            ).await;
                        }
                    });
                }
                Err(e) => {
                    let response_time = start_time.elapsed();
                    debug!("Failed to forward message to mailbox {}: {}", peer_id, e);
                    failed_attempts += 1;

                    // Update performance tracking (fire and forget to avoid blocking)
                    let sync_engine_clone = self.sync_engine.clone();
                    let peer_id_copy = *peer_id;
                    tokio::spawn(async move {
                        if let Ok(mut sync_engine) = sync_engine_clone.try_lock() {
                            sync_engine.update_mailbox_performance(
                                peer_id_copy,
                                false,
                                response_time,
                            ).await;
                        }
                    });
                }
            }
        }

        if forwarded_count > 0 {
            info!(
                "Message {} successfully stored in {} mailboxes",
                message.id, forwarded_count
            );
            Ok(MailboxDeliveryResult::Success(forwarded_count))
        } else {
            debug!(
                "Failed to store message {} in any mailboxes after {} attempts",
                message.id, failed_attempts
            );
            Ok(MailboxDeliveryResult::Failure)
        }
    }
}
