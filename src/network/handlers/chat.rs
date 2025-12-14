//! This module contains the handlers for chat-related network events.
use super::super::{NetworkLayer, NetworkResponse};
use crate::cli::commands::UiNotification;
use crate::types::{ChatRequest, ChatResponse, DeliveryStatus, Message};
use anyhow::Result;
use libp2p::request_response::{self, OutboundRequestId, ResponseChannel};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

impl NetworkLayer {
    /// Handles an event from the `ChatBehaviour`.
    ///
    /// This function is called when an event is received from the `ChatBehaviour`.
    /// It dispatches the event to the appropriate handler.
    ///
    /// # Arguments
    ///
    /// * `event` - The `request_response::Event<ChatRequest, ChatResponse>` to handle.
    /// * `incoming_messages` - The sender for incoming chat messages.
    ///
    /// # Errors
    ///
    /// This function will return an error if handling the event fails.
    pub(super) async fn handle_chat_event(
        &mut self,
        event: request_response::Event<ChatRequest, ChatResponse>,
        incoming_messages: &mpsc::UnboundedSender<Message>,
    ) -> Result<()> {
        match event {
            request_response::Event::Message { message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    self.handle_chat_request(request, channel, incoming_messages)
                        .await?;
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    self.handle_chat_response(request_id, response).await?;
                }
            },
            request_response::Event::OutboundFailure {
                request_id, error, ..
            } => {
                warn!("Chat request failed: {:?}", error);
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    let _ = sender.send(NetworkResponse::Error(format!(
                        "Request failed: {:?}",
                        error
                    )));
                }
            }
            request_response::Event::InboundFailure { error, .. } => {
                warn!("Chat inbound failure: {:?}", error);
            }
            _ => {}
        }

        Ok(())
    }

    /// Handles an inbound chat request.
    async fn handle_chat_request(
        &mut self,
        request: ChatRequest,
        channel: ResponseChannel<ChatResponse>,
        incoming_messages: &mpsc::UnboundedSender<Message>,
    ) -> Result<()> {
        match request {
            ChatRequest::SendMessage { message } => {
                info!("Received message from {}: {}", message.sender, message.id);

                if let Err(e) = incoming_messages.send(message.clone()) {
                    error!("Failed to forward incoming message: {}", e);
                    let _ = self.swarm.behaviour_mut().chat.send_response(
                        channel,
                        ChatResponse::MessageResult {
                            success: false,
                            message_id: None,
                        },
                    );
                } else {
                    let _ = self.swarm.behaviour_mut().chat.send_response(
                        channel,
                        ChatResponse::MessageResult {
                            success: true,
                            message_id: Some(message.id),
                        },
                    );
                }
            }
            ChatRequest::DeliveryConfirmation { confirmation } => {
                info!(
                    "Received delivery confirmation for message: {}",
                    confirmation.original_message_id
                );

                // Notify the UI about the delivery status update.
                if let Some(ref ui_tx) = self.ui_notify_tx {
                    let _ = ui_tx.send(UiNotification::DeliveryStatusUpdate {
                        message_id: confirmation.original_message_id,
                        new_status: DeliveryStatus::Delivered,
                    });
                }

                // Send a success response.
                let _ = self.swarm.behaviour_mut().chat.send_response(
                    channel,
                    ChatResponse::MessageResult {
                        success: true,
                        message_id: Some(confirmation.original_message_id),
                    },
                );
            }
            ChatRequest::ReadReceipt { receipt } => {
                info!("Received read receipt for message: {}", receipt.message_id);

                // Notify the UI about the read status update.
                if let Some(ref ui_tx) = self.ui_notify_tx {
                    let _ = ui_tx.send(UiNotification::DeliveryStatusUpdate {
                        message_id: receipt.message_id,
                        new_status: DeliveryStatus::Read,
                    });
                }

                // Send a success response.
                let _ = self.swarm.behaviour_mut().chat.send_response(
                    channel,
                    ChatResponse::MessageResult {
                        success: true,
                        message_id: Some(receipt.message_id),
                    },
                );
            }
        }

        Ok(())
    }

    /// Handles an outbound chat response.
    async fn handle_chat_response(
        &mut self,
        request_id: OutboundRequestId,
        response: ChatResponse,
    ) -> Result<()> {
        if let Some(sender) = self.pending_requests.remove(&request_id) {
            match response {
                ChatResponse::MessageResult { success, .. } => {
                    if success {
                        let _ = sender.send(NetworkResponse::MessageSent);
                    } else {
                        let _ = sender.send(NetworkResponse::Error(
                            "Message rejected by peer".to_string(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}
