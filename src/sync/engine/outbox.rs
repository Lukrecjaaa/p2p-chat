use super::SyncEngine;
use crate::types::EncryptedMessage;
use anyhow::{anyhow, Result};
use libp2p::PeerId;
use tracing::{debug, error, info, warn};

impl SyncEngine {
    pub async fn retry_outbox_for_peer(&self, target_peer: &PeerId) -> Result<()> {
        let pending_messages = self.outbox.get_pending().await?;

        if pending_messages.is_empty() {
            return Ok(());
        }

        let Some(network) = &self.network else {
            debug!("No network handle available for outbox retry");
            return Ok(());
        };

        debug!(
            "Retrying {} pending messages for peer {}",
            pending_messages.len(),
            target_peer
        );

        for message in pending_messages {
            if message.recipient != *target_peer {
                continue;
            }

            match network
                .send_message(message.recipient, message.clone())
                .await
            {
                Ok(()) => {
                    self.outbox.remove_pending(&message.id).await?;
                    info!(
                        "Successfully delivered message {} to {}",
                        message.id, message.recipient
                    );
                }
                Err(e) => {
                    debug!(
                        "Failed to deliver message {} to {}: {}",
                        message.id, message.recipient, e
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn retry_outbox(&mut self) -> Result<()> {
        let pending_messages = self.outbox.get_pending().await?;
        if pending_messages.is_empty() {
            return Ok(());
        }

        let Some(network) = self.network.clone() else {
            debug!("No network handle available for outbox retry");
            return Ok(());
        };

        let available_mailboxes = self.get_available_mailboxes();

        if self.discovered_mailboxes.is_empty() {
            debug!(
                "Have {} pending messages but no discovered mailboxes, triggering forced discovery",
                pending_messages.len()
            );
            if let Err(e) = self.discover_mailboxes_if_needed(true).await {
                warn!(
                    "Failed to trigger mailbox discovery for pending messages: {}",
                    e
                );
            }
        } else if available_mailboxes.is_empty() {
            debug!(
                "Have {} pending messages but all {} mailboxes are backed off",
                pending_messages.len(),
                self.discovered_mailboxes.len()
            );
        }

        debug!("Retrying {} pending messages", pending_messages.len());

        for message in pending_messages {
            let should_try_direct = self.backoff_manager.can_attempt(&message.recipient);

            let direct_result = if should_try_direct {
                debug!("Attempting direct delivery to peer {}", message.recipient);
                self.backoff_manager.record_attempt(message.recipient);
                network
                    .send_message(message.recipient, message.clone())
                    .await
            } else {
                debug!(
                    "Skipping direct delivery attempt to backed-off peer {}",
                    message.recipient
                );
                Err(anyhow!("Peer is backed off"))
            };

            match direct_result {
                Ok(()) => {
                    if should_try_direct {
                        self.backoff_manager.record_success(&message.recipient);
                    }
                    self.outbox.remove_pending(&message.id).await?;
                    info!(
                        "Successfully delivered message {} directly to {}",
                        message.id, message.recipient
                    );
                }
                Err(e) => {
                    if should_try_direct {
                        self.backoff_manager.record_failure(message.recipient);
                    }
                    debug!(
                        "Direct retry for message {} to {} failed: {}. Attempting mailbox forward.",
                        message.id, message.recipient, e
                    );

                    if self.discovered_mailboxes.is_empty() {
                        debug!("No mailboxes discovered to forward message {}.", message.id);
                        continue;
                    }

                    let Some(friend) = self.friends.get_friend(&message.recipient).await? else {
                        error!(
                            "Cannot forward message {}: recipient {} not in friends list.",
                            message.id, message.recipient
                        );
                        continue;
                    };

                    let recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(
                        &friend.e2e_public_key,
                    );
                    let encrypted_msg = EncryptedMessage {
                        id: message.id,
                        sender: self.identity.peer_id,
                        recipient_hash,
                        encrypted_content: message.content.clone(),
                        timestamp: message.timestamp,
                        nonce: message.nonce,
                        sender_pub_key: self.identity.hpke_public_key(),
                    };

                    let candidate_mailboxes =
                        self.rank_mailboxes_subset(&self.discovered_mailboxes);

                    if candidate_mailboxes.is_empty() {
                        debug!(
                            "No available (non-backed-off) mailboxes to forward message {}.",
                            message.id
                        );
                        continue;
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

                        let start_time = std::time::Instant::now();
                        match network
                            .mailbox_put(*peer_id, recipient_hash, encrypted_msg.clone())
                            .await
                        {
                            Ok(true) => {
                                self.update_mailbox_performance(
                                    *peer_id,
                                    true,
                                    start_time.elapsed(),
                                );
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
                                self.update_mailbox_performance(
                                    *peer_id,
                                    false,
                                    start_time.elapsed(),
                                );
                                debug!(
                                    "Mailbox {} rejected pending message {}",
                                    peer_id, message.id
                                );

                                if self.should_forget_mailbox(*peer_id) {
                                    mailboxes_to_forget.push(*peer_id);
                                }
                            }
                            Err(err) => {
                                self.update_mailbox_performance(
                                    *peer_id,
                                    false,
                                    start_time.elapsed(),
                                );
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
                        self.forget_failing_mailbox(mailbox_id);
                    }

                    let forwarded = forwarded_count > 0;

                    if forwarded {
                        self.outbox.remove_pending(&message.id).await?;
                        info!(
                            "Removed message {} from outbox after successful mailbox forward.",
                            message.id
                        );
                    } else {
                        debug!(
                            "Failed to forward message {} to any mailboxes, will retry later.",
                            message.id
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
