use super::super::{NetworkLayer, NetworkResponse};
use crate::types::{ChatRequest, ChatResponse, Message};
use anyhow::Result;
use libp2p::request_response::{self, OutboundRequestId, ResponseChannel};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

impl NetworkLayer {
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
        }

        Ok(())
    }

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
