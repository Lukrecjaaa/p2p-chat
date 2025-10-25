use anyhow::Result;
use tracing::{error, trace};
use uuid::Uuid;

use crate::cli::UiNotification;
use crate::types::{DeliveryStatus, EncryptedMessage, Message};

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
            delivery_status: DeliveryStatus::Delivered,
        })
    }
}
