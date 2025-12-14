//! This module contains logic for processing messages fetched from mailboxes.
use anyhow::Result;
use tracing::{debug, error, trace};
use uuid::Uuid;
use std::ops::Deref;

use crate::cli::UiNotification;
use crate::types::{ChatRequest, DeliveryConfirmation, DeliveryStatus, EncryptedMessage, Message};

use super::super::SyncEngine;

impl SyncEngine {
    /// Processes a list of encrypted messages fetched from mailboxes.
    ///
    /// This function iterates through the messages, decrypts them, marks them as seen,
    /// stores them in the history, sends delivery confirmations, and notifies the UI.
    ///
    /// # Arguments
    ///
    /// * `messages` - A `Vec` of `EncryptedMessage`s fetched from a mailbox.
    ///
    /// # Returns
    ///
    /// A `Vec` of `Uuid`s representing the IDs of messages that were successfully processed.
    ///
    /// # Errors
    ///
    /// This function will return an error if message decryption, storage, or processing fails.
    pub async fn process_mailbox_messages(
        &self,
        messages: Vec<EncryptedMessage>,
    ) -> Result<Vec<Uuid>> {
        let mut processed_msg_ids = Vec::new();

        for encrypted_msg in messages {
            // Skip messages that have already been seen.
            if self.seen.is_seen(&encrypted_msg.id).await? {
                trace!(
                    "Message {} already seen, adding to ACK list",
                    encrypted_msg.id
                );
                processed_msg_ids.push(encrypted_msg.id);
                continue;
            }

            // Reconstruct the message from the encrypted version.
            let message = self
                .reconstruct_message_from_mailbox(&encrypted_msg)
                .await?;

            // Store the message in history.
            if let Err(e) = self.history.store_message(message.clone()).await {
                error!(
                    "Failed to store mailbox message {} in history: {}",
                    encrypted_msg.id, e
                );
                continue;
            }

            // Mark the message as seen.
            if let Err(e) = self.seen.mark_seen(encrypted_msg.id).await {
                error!("Failed to mark message {} as seen: {}", encrypted_msg.id, e);
            }

            // Send delivery confirmation back to sender.
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

            // Notify the UI about the new message.
            if let Err(e) = self.ui_notify_tx.send(UiNotification::NewMessage(message.clone())) {
                trace!("UI notify channel closed while reporting message: {}", e);
            }

            // Also send to web UI if available.
            if let Some(ref web_tx) = self.web_notify_tx {
                let _ = web_tx.send(UiNotification::NewMessage(message));
            }

            processed_msg_ids.push(encrypted_msg.id);
        }

        Ok(processed_msg_ids)
    }

    /// Reconstructs a `Message` from an `EncryptedMessage` fetched from a mailbox.
    ///
    /// This involves using the local identity's HPKE context to decrypt the content.
    ///
    /// # Arguments
    ///
    /// * `encrypted_msg` - The `EncryptedMessage` to reconstruct.
    ///
    /// # Returns
    ///
    /// The reconstructed `Message`.
    ///
    /// # Errors
    ///
    /// This function will return an error if decryption fails.
    pub async fn reconstruct_message_from_mailbox(
        &self,
        encrypted_msg: &EncryptedMessage,
    ) -> Result<Message> {
        let plaintext_content = self.identity.deref().decrypt_from(
            &encrypted_msg.sender_pub_key,
            &encrypted_msg.encrypted_content,
        )?;

        let plaintext_string = String::from_utf8(plaintext_content)?;

        Ok(Message {
            id: encrypted_msg.id,
            sender: encrypted_msg.sender,
            recipient: self.identity.peer_id, // Our peer_id is the recipient
            timestamp: encrypted_msg.timestamp,
            content: plaintext_string.into_bytes(),
            nonce: encrypted_msg.nonce,
            delivery_status: DeliveryStatus::Delivered, // Mark as delivered upon processing
        })
    }
}
