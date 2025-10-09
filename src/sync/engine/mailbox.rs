use super::performance::MailboxPerformance;
use super::SyncEngine;
use crate::cli::UiNotification;
use crate::sync::retry::RetryPolicy;
use crate::types::{EncryptedMessage, Message};
use anyhow::{anyhow, Result};
use libp2p::PeerId;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

impl SyncEngine {
    pub async fn fetch_from_mailboxes(&mut self) -> Result<()> {
        if self.discovered_mailboxes.is_empty() {
            trace!("No mailbox nodes discovered, skipping fetch cycle.");
            return Ok(());
        }

        let available_mailboxes = self.get_available_mailboxes();

        if available_mailboxes.is_empty() {
            trace!("All discovered mailboxes are currently backed off, skipping fetch cycle.");
            return Ok(());
        }

        let mut total_processed = 0;
        for peer_id in available_mailboxes.iter() {
            if !self.discovered_mailboxes.contains(peer_id) {
                debug!(
                    "Skipping fetch from mailbox {} - was removed during iteration",
                    peer_id
                );
                continue;
            }

            if !self.backoff_manager.can_attempt(peer_id) {
                debug!("Skipping fetch from backed-off mailbox {}", peer_id);
                continue;
            }

            match self.fetch_from_single_mailbox(*peer_id).await {
                Ok(processed_ids) => {
                    total_processed += processed_ids.len();
                }
                Err(e) => {
                    error!("Scheduled fetch from mailbox {} failed: {}", peer_id, e);
                }
            }
        }

        if total_processed > 0 {
            info!(
                "Fetch cycle completed: {} messages processed across all mailboxes",
                total_processed
            );
        } else {
            trace!("Fetch cycle completed: no new messages found");
        }

        Ok(())
    }

    pub async fn fetch_from_single_mailbox(&mut self, peer_id: PeerId) -> Result<Vec<uuid::Uuid>> {
        let Some(network) = self.network.clone() else {
            debug!("No network handle available for single mailbox fetch");
            return Ok(vec![]);
        };

        let recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(
            &self.identity.hpke_public_key(),
        );

        debug!("Sync: Fetching messages from mailbox {}", peer_id);

        let start_time = std::time::Instant::now();
        let retry_policy = RetryPolicy::fast_mailbox();

        let fetch_result = retry_policy
            .retry_with_jitter(|| async {
                network
                    .mailbox_fetch(peer_id, recipient_hash, 100)
                    .await
                    .map_err(|e| anyhow!("Fetch failed: {}", e))
            })
            .await;

        match fetch_result {
            Ok(messages) => {
                self.update_mailbox_performance(peer_id, true, start_time.elapsed());

                if messages.is_empty() {
                    trace!("No messages found in mailbox {}", peer_id);
                    return Ok(vec![]);
                }
                info!(
                    "Retrieved {} messages from mailbox {}",
                    messages.len(),
                    peer_id
                );

                match self.process_mailbox_messages(messages).await {
                    Ok(processed_ids) => {
                        if !processed_ids.is_empty() {
                            info!(
                                "Successfully processed {} new messages from mailbox {}",
                                processed_ids.len(),
                                peer_id
                            );
                            if let Err(e) = self
                                .acknowledge_mailbox_messages(processed_ids.clone())
                                .await
                            {
                                error!("Failed to ACK messages to mailbox {}: {}", peer_id, e);
                            }
                        }
                        Ok(processed_ids)
                    }
                    Err(e) => {
                        error!("Failed to process messages from mailbox {}: {}", peer_id, e);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                let fast_policy = RetryPolicy::fast_mailbox();
                for _ in 0..fast_policy.max_attempts {
                    self.update_mailbox_performance(
                        peer_id,
                        false,
                        start_time.elapsed() / fast_policy.max_attempts,
                    );
                }

                if self.should_forget_mailbox(peer_id) {
                    self.forget_failing_mailbox(peer_id);
                }

                error!(
                    "Failed to fetch from mailbox {} after retries: {}",
                    peer_id, e
                );
                Err(e)
            }
        }
    }

    pub async fn process_mailbox_messages(
        &self,
        messages: Vec<EncryptedMessage>,
    ) -> Result<Vec<Uuid>> {
        let mut processed_msg_ids = Vec::new();

        for encrypted_msg in messages {
            if self.seen.is_seen(&encrypted_msg.id).await? {
                trace!(
                    "Message {} already seen, adding to ACK list",
                    encrypted_msg.id
                );
                processed_msg_ids.push(encrypted_msg.id);
                continue;
            }

            let message = self
                .reconstruct_message_from_mailbox(&encrypted_msg)
                .await?;

            if let Err(e) = self.history.store_message(message.clone()).await {
                error!(
                    "Failed to store mailbox message {} in history: {}",
                    encrypted_msg.id, e
                );
                continue;
            }

            if let Err(e) = self.seen.mark_seen(encrypted_msg.id).await {
                error!("Failed to mark message {} as seen: {}", encrypted_msg.id, e);
            }

            if let Err(e) = self.ui_notify_tx.send(UiNotification::NewMessage(message)) {
                trace!("UI notify channel closed while reporting message: {}", e);
            }

            processed_msg_ids.push(encrypted_msg.id);
        }

        Ok(processed_msg_ids)
    }

    pub async fn reconstruct_message_from_mailbox(
        &self,
        encrypted_msg: &EncryptedMessage,
    ) -> Result<Message> {
        Ok(Message {
            id: encrypted_msg.id,
            sender: encrypted_msg.sender,
            recipient: self.identity.peer_id,
            timestamp: encrypted_msg.timestamp,
            content: encrypted_msg.encrypted_content.clone(),
            nonce: encrypted_msg.nonce,
        })
    }

    pub async fn acknowledge_mailbox_messages(&self, msg_ids: Vec<Uuid>) -> Result<()> {
        if msg_ids.is_empty() {
            return Ok(());
        }

        let Some(network) = &self.network else {
            debug!("No network handle available for mailbox ACK");
            return Ok(());
        };

        let recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(
            &self.identity.hpke_public_key(),
        );

        info!(
            "Acknowledging {} messages to {} mailboxes",
            msg_ids.len(),
            self.get_mailbox_providers().len()
        );

        let mut total_deleted = 0;
        let mut successful_acks = 0;
        let mut failed_acks = 0;

        let retry_policy = RetryPolicy::fast_mailbox();

        for peer_id in self.get_mailbox_providers().iter() {
            let ack_result = retry_policy
                .retry_with_jitter(|| async {
                    network
                        .mailbox_ack(*peer_id, recipient_hash, msg_ids.clone())
                        .await
                        .map_err(|e| anyhow!("ACK failed: {}", e))
                })
                .await;

            match ack_result {
                Ok(deleted_count) => {
                    successful_acks += 1;
                    total_deleted += deleted_count;
                    if deleted_count > 0 {
                        info!(
                            "Mailbox {} confirmed deletion of {} messages",
                            peer_id, deleted_count
                        );
                    } else {
                        trace!("Mailbox {} had no messages to delete", peer_id);
                    }
                }
                Err(e) => {
                    failed_acks += 1;
                    warn!(
                        "Failed to ACK messages to mailbox {} after retries: {}",
                        peer_id, e
                    );
                }
            }
        }

        info!(
            "ACK summary: {} messages deleted across {} mailboxes, {}/{} ACKs successful",
            total_deleted,
            successful_acks,
            successful_acks,
            successful_acks + failed_acks
        );

        if failed_acks > 0 {
            warn!(
                "Failed to ACK to {} mailboxes - messages may remain stored",
                failed_acks
            );
        }

        Ok(())
    }

    pub(crate) fn update_mailbox_performance(
        &mut self,
        peer_id: PeerId,
        success: bool,
        response_time: Duration,
    ) {
        let perf = self
            .mailbox_performance
            .entry(peer_id)
            .or_insert_with(MailboxPerformance::new);

        if success {
            perf.success_count += 1;
            perf.consecutive_failures = 0;
            perf.last_success = Some(std::time::Instant::now());
            self.backoff_manager.record_success(&peer_id);
        } else {
            perf.failure_count += 1;
            perf.consecutive_failures += 1;
            perf.last_failure = Some(std::time::Instant::now());
            self.backoff_manager.record_failure(peer_id);
        }

        let new_weight = 0.3;
        let old_weight = 1.0 - new_weight;
        perf.avg_response_time = Duration::from_millis(
            ((perf.avg_response_time.as_millis() as f64 * old_weight)
                + (response_time.as_millis() as f64 * new_weight)) as u64,
        );
    }

    pub(super) fn forget_failing_mailbox(&mut self, peer_id: PeerId) {
        if self.discovered_mailboxes.remove(&peer_id) {
            warn!(
                "Temporarily forgetting failing mailbox {} due to persistent failures",
                peer_id
            );
            self.backoff_manager.record_failure(peer_id);
        }
    }

    pub(super) fn cleanup_failing_mailboxes(&mut self) {
        let mut mailboxes_to_forget = Vec::new();

        for peer_id in &self.discovered_mailboxes {
            if self.should_forget_mailbox(*peer_id) {
                mailboxes_to_forget.push(*peer_id);
            }
        }

        for peer_id in mailboxes_to_forget {
            self.forget_failing_mailbox(peer_id);
        }
    }
}
