//! This module contains the handlers for mailbox-related network events.
use super::super::{NetworkLayer, NetworkResponse};
use crate::storage::MailboxStore;
use crate::types::{MailboxRequest, MailboxResponse};
use anyhow::Result;
use libp2p::request_response::{self, OutboundRequestId, ResponseChannel};
use tracing::{debug, error, info, warn};

impl NetworkLayer {
    /// Handles an event from the `MailboxBehaviour`.
    ///
    /// This function is called when an event is received from the `MailboxBehaviour`.
    /// It dispatches the event to the appropriate handler.
    ///
    /// # Arguments
    ///
    /// * `event` - The `request_response::Event<MailboxRequest, MailboxResponse>` to handle.
    ///
    /// # Errors
    ///
    /// This function will return an error if handling the event fails.
    pub(super) async fn handle_mailbox_event(
        &mut self,
        event: request_response::Event<MailboxRequest, MailboxResponse>,
    ) -> Result<()> {
        match event {
            request_response::Event::Message { message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    self.handle_mailbox_request(request, channel).await?;
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    self.handle_mailbox_response(request_id, response).await?;
                }
            },
            request_response::Event::OutboundFailure {
                request_id, error, ..
            } => {
                warn!("Mailbox request failed: {:?}", error);
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    let _ = sender.send(NetworkResponse::Error(format!(
                        "Request failed: {:?}",
                        error
                    )));
                }
            }
            request_response::Event::InboundFailure { error, .. } => {
                warn!("Mailbox inbound failure: {:?}", error);
            }
            _ => {}
        }

        Ok(())
    }

    /// Handles an inbound mailbox request.
    async fn handle_mailbox_request(
        &mut self,
        request: MailboxRequest,
        channel: ResponseChannel<MailboxResponse>,
    ) -> Result<()> {
        // Log request with readable format.
        match &request {
            MailboxRequest::Put { recipient, message } => {
                debug!(
                    "Network mailbox request: Put {{ recipient: {}, message_id: {}, sender: {} }}",
                    hex::encode(&recipient[..8]),
                    message.id,
                    message.sender
                );
            }
            MailboxRequest::Fetch { recipient, limit } => {
                debug!(
                    "Network mailbox request: Fetch {{ recipient: {}, limit: {} }}",
                    hex::encode(&recipient[..8]),
                    limit
                );
            }
            MailboxRequest::Ack { recipient, msg_ids } => {
                debug!(
                    "Network mailbox request: Ack {{ recipient: {}, msg_ids: {:?} }}",
                    hex::encode(&recipient[..8]),
                    msg_ids
                );
            }
        }

        let response = if let Some(ref storage) = self.mailbox_storage {
            match request {
                MailboxRequest::Put { recipient, message } => {
                    match storage.store_message(recipient, message).await {
                        Ok(()) => {
                            info!(
                                "Successfully stored message in mailbox for recipient: {}",
                                hex::encode(&recipient[..8])
                            );

                            if let Err(e) = self.start_providing_for_recipient(recipient) {
                                debug!(
                                    "Failed to register as provider for recipient {}: {}",
                                    hex::encode(&recipient[..8]),
                                    e
                                );
                            } else {
                                debug!(
                                    "Registered as provider for recipient: {}",
                                    hex::encode(&recipient[..8])
                                );
                            }

                            MailboxResponse::PutResult { success: true }
                        }
                        Err(e) => {
                            error!("Failed to store mailbox message: {}", e);
                            MailboxResponse::PutResult { success: false }
                        }
                    }
                }
                MailboxRequest::Fetch { recipient, limit } => {
                    match storage.fetch_messages(recipient, limit).await {
                        Ok(messages) => {
                            info!(
                                "Fetched {} messages for recipient: {}",
                                messages.len(),
                                hex::encode(&recipient[..8])
                            );
                            MailboxResponse::Messages { items: messages }
                        }
                        Err(e) => {
                            error!("Failed to fetch mailbox messages: {}", e);
                            MailboxResponse::Messages { items: vec![] }
                        }
                    }
                }
                MailboxRequest::Ack { recipient, msg_ids } => {
                    match storage.delete_messages(recipient, msg_ids).await {
                        Ok(deleted) => {
                            info!(
                                "Deleted {} messages for recipient: {}",
                                deleted,
                                hex::encode(&recipient[..8])
                            );

                            match storage.fetch_messages(recipient, 1).await {
                                Ok(remaining_messages) if remaining_messages.is_empty() => {
                                    debug!(
                                        "No more messages for recipient {}, could stop DHT announcement",
                                        hex::encode(&recipient[..8])
                                    );
                                }
                                Ok(_) => {
                                    debug!(
                                        "Still have messages for recipient {}, keeping DHT announcement",
                                        hex::encode(&recipient[..8])
                                    );
                                }
                                Err(e) => {
                                    debug!("Failed to check remaining messages for cleanup: {}", e);
                                }
                            }

                            MailboxResponse::AckResult { deleted }
                        }
                        Err(e) => {
                            error!("Failed to delete mailbox messages: {}", e);
                            MailboxResponse::AckResult { deleted: 0 }
                        }
                    }
                }
            }
        } else {
            debug!("No mailbox storage available, returning default responses");
            match request {
                MailboxRequest::Put { .. } => MailboxResponse::PutResult { success: false },
                MailboxRequest::Fetch { .. } => MailboxResponse::Messages { items: vec![] },
                MailboxRequest::Ack { .. } => MailboxResponse::AckResult { deleted: 0 },
            }
        };

        let _ = self
            .swarm
            .behaviour_mut()
            .mailbox
            .send_response(channel, response);
        Ok(())
    }

    /// Handles an outbound mailbox response.
    async fn handle_mailbox_response(
        &mut self,
        request_id: OutboundRequestId,
        response: MailboxResponse,
    ) -> Result<()> {
        if let Some(sender) = self.pending_requests.remove(&request_id) {
            match response {
                MailboxResponse::PutResult { success } => {
                    let _ = sender.send(NetworkResponse::MailboxPutResult { success });
                }
                MailboxResponse::Messages { items } => {
                    let _ = sender.send(NetworkResponse::MailboxMessages { messages: items });
                }
                MailboxResponse::AckResult { deleted } => {
                    let _ = sender.send(NetworkResponse::MailboxAckResult { deleted });
                }
            }
        }

        Ok(())
    }
}
