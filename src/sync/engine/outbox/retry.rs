//! This module contains logic for retrying the delivery of messages in the outbox.
use anyhow::{anyhow, Result};
use tracing::{debug, info, warn};

use crate::network::NetworkHandle;

use super::super::SyncEngine;

impl SyncEngine {
    /// Retries sending all pending messages in the outbox.
    ///
    /// This function attempts direct delivery to connected peers first. If direct
    /// delivery fails or the peer is not connected, it then attempts to forward
    /// the message to available mailbox providers.
    ///
    /// # Errors
    ///
    /// This function will return an error if there are issues accessing the
    /// outbox or network. Individual message delivery failures are logged
    /// but do not stop the overall retry process.
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
            let delivered_direct = self.attempt_direct_delivery(&network, &message).await?;

            if delivered_direct {
                continue;
            }

            if self.discovered_mailboxes.is_empty() {
                debug!("No mailboxes discovered to forward message {}.", message.id);
                continue;
            }

            match self.forward_pending_message(&network, &message).await {
                Ok(true) => {
                    // Message successfully forwarded to mailbox
                    // Delivery status will be updated when recipient fetches and sends confirmation
                    self.outbox.remove_pending(&message.id).await?;
                    info!(
                        "Removed message {} from outbox after successful mailbox forward.",
                        message.id
                    );
                }
                Ok(false) => {
                    debug!(
                        "Failed to forward message {} to any mailboxes, will retry later.",
                        message.id
                    );
                }
                Err(e) => {
                    warn!(
                        "Unable to forward message {} via mailbox: {}",
                        message.id, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Attempts to directly deliver a message to its recipient.
    ///
    /// This function checks the `BackoffManager` to see if a direct attempt
    /// is allowed. If successful, the message is removed from the outbox.
    ///
    /// # Arguments
    ///
    /// * `network` - The `NetworkHandle` to use for sending the message.
    /// * `message` - The message to attempt direct delivery for.
    ///
    /// # Returns
    ///
    /// `true` if direct delivery was successful, `false` otherwise.
    ///
    /// # Errors
    ///
    /// This function will return an error if there are issues sending the message.
    async fn attempt_direct_delivery(
        &mut self,
        network: &NetworkHandle,
        message: &crate::types::Message,
    ) -> Result<bool> {
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

                // Direct delivery succeeded
                // Delivery confirmation from recipient will update the status
                self.outbox.remove_pending(&message.id).await?;
                info!(
                    "Successfully delivered message {} directly to {}",
                    message.id, message.recipient
                );
                Ok(true)
            }
            Err(e) => {
                if should_try_direct {
                    self.backoff_manager.record_failure(message.recipient);
                }
                debug!(
                    "Direct retry for message {} to {} failed: {}. Attempting mailbox forward.",
                    message.id, message.recipient, e
                );
                Ok(false)
            }
        }
    }
}
