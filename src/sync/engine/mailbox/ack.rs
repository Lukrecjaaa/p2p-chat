//! This module contains logic for acknowledging messages in mailboxes.
use anyhow::{anyhow, Result};
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

use crate::crypto::StorageEncryption;
use crate::sync::retry::RetryPolicy;

use super::super::SyncEngine;

impl SyncEngine {
    /// Acknowledges a list of messages across all known mailbox providers.
    ///
    /// This function attempts to send an acknowledgment for each message ID to
    /// all discovered mailbox providers, ensuring that the messages are deleted
    /// from the mailboxes. It utilizes a retry policy for robustness.
    ///
    /// # Arguments
    ///
    /// * `msg_ids` - A `Vec` of `Uuid`s representing the messages to acknowledge.
    ///
    /// # Errors
    ///
    /// This function will return an error if network communication fails, but
    /// it attempts to acknowledge with multiple mailboxes for resilience.
    pub async fn acknowledge_mailbox_messages(&self, msg_ids: Vec<Uuid>) -> Result<()> {
        if msg_ids.is_empty() {
            return Ok(());
        }

        let Some(network) = &self.network else {
            debug!("No network handle available for mailbox ACK");
            return Ok(());
        };

        let recipient_hash =
            StorageEncryption::derive_recipient_hash(&self.identity.hpke_public_key());

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
}
