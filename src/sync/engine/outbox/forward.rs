use std::time::Instant;

use anyhow::{anyhow, Result};
use tracing::{debug, info};

use crate::crypto::StorageEncryption;
use crate::network::NetworkHandle;
use crate::types::EncryptedMessage;

use super::super::SyncEngine;

impl SyncEngine {
    pub(super) async fn forward_pending_message(
        &mut self,
        network: &NetworkHandle,
        message: &crate::types::Message,
    ) -> Result<bool> {
        let Some(friend) = self.friends.get_friend(&message.recipient).await? else {
            return Err(anyhow!(
                "Cannot forward message {}: recipient {} not in friends list.",
                message.id,
                message.recipient
            ));
        };

        let recipient_hash = StorageEncryption::derive_recipient_hash(&friend.e2e_public_key);
        let encrypted_msg = EncryptedMessage {
            id: message.id,
            sender: self.identity.peer_id,
            recipient_hash,
            encrypted_content: message.content.clone(),
            timestamp: message.timestamp,
            nonce: message.nonce,
            sender_pub_key: self.identity.hpke_public_key(),
        };

        let candidate_mailboxes = self.rank_mailboxes_subset(&self.discovered_mailboxes);
        if candidate_mailboxes.is_empty() {
            debug!(
                "No available (non-backed-off) mailboxes to forward message {}.",
                message.id
            );
            return Ok(false);
        }

        let min_replicas = 2;
        let max_attempts = candidate_mailboxes.len().min(4);
        let mut forwarded_count = 0;
        let mut mailboxes_to_forget = Vec::new();

        for peer_id in candidate_mailboxes.iter().take(max_attempts) {
            if !self.discovered_mailboxes.contains(peer_id) {
                debug!(
                    "Skipping mailbox forwarding to {} - was removed during iteration",
                    peer_id
                );
                continue;
            }

            let start_time = Instant::now();
            match network
                .mailbox_put(*peer_id, recipient_hash, encrypted_msg.clone())
                .await
            {
                Ok(true) => {
                    self.update_mailbox_performance(*peer_id, true, start_time.elapsed()).await;
                    info!(
                        "Successfully forwarded pending message {} to mailbox {} ({}/{})",
                        message.id,
                        peer_id,
                        forwarded_count + 1,
                        min_replicas
                    );
                    forwarded_count += 1;

                    if forwarded_count >= min_replicas {
                        break;
                    }
                }
                Ok(false) => {
                    self.update_mailbox_performance(*peer_id, false, start_time.elapsed()).await;
                    debug!(
                        "Mailbox {} rejected pending message {}",
                        peer_id, message.id
                    );

                    if self.should_forget_mailbox(*peer_id) {
                        mailboxes_to_forget.push(*peer_id);
                    }
                }
                Err(err) => {
                    self.update_mailbox_performance(*peer_id, false, start_time.elapsed()).await;
                    debug!(
                        "Failed to forward pending message {} to mailbox {}: {}",
                        message.id, peer_id, err
                    );

                    if self.should_forget_mailbox(*peer_id) {
                        mailboxes_to_forget.push(*peer_id);
                    }
                }
            }
        }

        for mailbox_id in mailboxes_to_forget {
            self.forget_failing_mailbox(mailbox_id).await;
        }

        Ok(forwarded_count > 0)
    }
}
