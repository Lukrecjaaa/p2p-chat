use anyhow::Result;
use tracing::{debug, error, trace};
use uuid::Uuid;

use crate::cli::UiNotification;
use crate::types::{ChatRequest, DeliveryConfirmation, DeliveryStatus, EncryptedMessage, Message};

use super::super::SyncEngine;

impl SyncEngine {
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

            // Send delivery confirmation back to sender
            let confirmation = DeliveryConfirmation {
                original_message_id: encrypted_msg.id,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };

            let confirmation_request = ChatRequest::DeliveryConfirmation { confirmation };

            if let Some(ref network) = self.network {
                let network_clone = network.clone();
                let sender = encrypted_msg.sender;
                tokio::spawn(async move {
                    if let Err(e) = network_clone.send_chat_request(sender, confirmation_request).await {
                        debug!("Failed to send delivery confirmation from mailbox: {}", e);
                    }
                });
            }

            if let Err(e) = self.ui_notify_tx.send(UiNotification::NewMessage(message.clone())) {
                trace!("UI notify channel closed while reporting message: {}", e);
            }

            // Also send to web UI if available
            if let Some(ref web_tx) = self.web_notify_tx {
                let _ = web_tx.send(UiNotification::NewMessage(message));
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
            delivery_status: DeliveryStatus::Delivered,
        })
    }
}
