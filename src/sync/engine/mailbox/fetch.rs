use std::time::Instant;

use anyhow::{anyhow, Result};
use libp2p::PeerId;
use tracing::{debug, error, info, trace};
use uuid::Uuid;

use crate::crypto::StorageEncryption;
use crate::sync::retry::RetryPolicy;

use super::super::SyncEngine;

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

    pub async fn fetch_from_single_mailbox(&mut self, peer_id: PeerId) -> Result<Vec<Uuid>> {
        let Some(network) = self.network.clone() else {
            debug!("No network handle available for single mailbox fetch");
            return Ok(vec![]);
        };

        let recipient_hash =
            StorageEncryption::derive_recipient_hash(&self.identity.hpke_public_key());

        debug!("Sync: Fetching messages from mailbox {}", peer_id);

        let start_time = Instant::now();
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
}
