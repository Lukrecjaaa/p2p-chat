use anyhow::{anyhow, Result};
use tracing::{debug, info, warn};

use crate::network::NetworkHandle;

use super::super::SyncEngine;

impl SyncEngine {
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
                    // Update delivery status to Delivered
                    if let Err(e) = self
                        .history
                        .update_delivery_status(&message.id, crate::types::DeliveryStatus::Delivered)
                        .await
                    {
                        warn!("Failed to update delivery status for message {}: {}", message.id, e);
                    }

                    // Send notification to UI
                    let _ = self.ui_notify_tx.send(crate::cli::commands::UiNotification::DeliveryStatusUpdate {
                        message_id: message.id,
                        new_status: crate::types::DeliveryStatus::Delivered,
                    });

                    // Send notification to Web UI
                    if let Some(ref web_tx) = self.web_notify_tx {
                        let _ = web_tx.send(crate::cli::commands::UiNotification::DeliveryStatusUpdate {
                            message_id: message.id,
                            new_status: crate::types::DeliveryStatus::Delivered,
                        });
                    }

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

                // Update delivery status to Delivered
                if let Err(e) = self
                    .history
                    .update_delivery_status(&message.id, crate::types::DeliveryStatus::Delivered)
                    .await
                {
                    warn!("Failed to update delivery status for message {}: {}", message.id, e);
                }

                // Send notification to UI
                let _ = self.ui_notify_tx.send(crate::cli::commands::UiNotification::DeliveryStatusUpdate {
                    message_id: message.id,
                    new_status: crate::types::DeliveryStatus::Delivered,
                });

                // Send notification to Web UI
                if let Some(ref web_tx) = self.web_notify_tx {
                    let _ = web_tx.send(crate::cli::commands::UiNotification::DeliveryStatusUpdate {
                        message_id: message.id,
                        new_status: crate::types::DeliveryStatus::Delivered,
                    });
                }

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
