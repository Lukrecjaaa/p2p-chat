use super::{NetworkLayer, NetworkResponse, P2PBehaviourEvent};
use crate::storage::MailboxStore;
use crate::sync::{DhtQueryResult, SyncEvent};
use crate::types::{ChatRequest, ChatResponse, MailboxRequest, MailboxResponse, Message};
use anyhow::Result;
use libp2p::{
    kad,
    request_response::{self, OutboundRequestId, ResponseChannel},
    swarm::SwarmEvent,
};
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};

impl NetworkLayer {
    pub(super) async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<P2PBehaviourEvent>,
        incoming_messages: &mpsc::UnboundedSender<Message>,
    ) -> Result<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on: {}", address);
            }

            SwarmEvent::Behaviour(P2PBehaviourEvent::Chat(chat_event)) => {
                self.handle_chat_event(chat_event, incoming_messages)
                    .await?;
            }

            SwarmEvent::Behaviour(P2PBehaviourEvent::Mailbox(mailbox_event)) => {
                self.handle_mailbox_event(mailbox_event).await?;
            }

            SwarmEvent::Behaviour(P2PBehaviourEvent::Discovery(discovery_event)) => {
                self.handle_discovery_event(discovery_event).await?;
            }

            SwarmEvent::Behaviour(P2PBehaviourEvent::Ping(ping_event)) => match ping_event {
                libp2p::ping::Event { peer, result, .. } => match result {
                    Ok(rtt) => {
                        trace!("Ping to {} successful: RTT is {:?}", peer, rtt);
                    }
                    Err(failure) => {
                        warn!("Ping to {} failed: {:?}", peer, failure);
                    }
                },
            },

            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connection established with peer: {}", peer_id);
                if let Some(ref sync_tx) = self.sync_event_tx {
                    let _ = sync_tx.send(SyncEvent::PeerConnected(peer_id));
                }
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Disconnected from peer: {} (cause: {:?})", peer_id, cause);
            }

            SwarmEvent::IncomingConnection { .. } => {
                trace!("Incoming connection");
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!("Outgoing connection error to {:?}: {}", peer_id, error);

                if let Some(peer_id) = peer_id {
                    if let Some(ref sync_tx) = self.sync_event_tx {
                        let _ = sync_tx.send(SyncEvent::PeerConnectionFailed(peer_id));
                    }
                }
            }

            SwarmEvent::IncomingConnectionError { error, .. } => {
                warn!("Incoming connection error: {}", error);
            }

            _ => {}
        }

        Ok(())
    }

    async fn handle_chat_event(
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

    async fn handle_mailbox_event(
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

    async fn handle_mailbox_request(
        &mut self,
        request: MailboxRequest,
        channel: ResponseChannel<MailboxResponse>,
    ) -> Result<()> {
        debug!("Network mailbox request: {:?}", request);

        let response = if let Some(ref storage) = self.mailbox_storage {
            match request {
                MailboxRequest::Put { recipient, message } => {
                    match storage.store_message(recipient, message).await {
                        Ok(()) => {
                            info!(
                                "Successfully stored message in mailbox for recipient: {:?}",
                                &recipient[..8]
                            );

                            if let Err(e) = self.start_providing_for_recipient(recipient) {
                                debug!(
                                    "Failed to register as provider for recipient {:?}: {}",
                                    &recipient[..8],
                                    e
                                );
                            } else {
                                debug!(
                                    "Registered as provider for recipient: {:?}",
                                    &recipient[..8]
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
                                "Fetched {} messages for recipient: {:?}",
                                messages.len(),
                                &recipient[..8]
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
                                "Deleted {} messages for recipient: {:?}",
                                deleted,
                                &recipient[..8]
                            );

                            match storage.fetch_messages(recipient, 1).await {
                                Ok(remaining_messages) if remaining_messages.is_empty() => {
                                    debug!(
                                        "No more messages for recipient {:?}, could stop DHT announcement",
                                        &recipient[..8]
                                    );
                                }
                                Ok(_) => {
                                    debug!(
                                        "Still have messages for recipient {:?}, keeping DHT announcement",
                                        &recipient[..8]
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

    async fn handle_discovery_event(
        &mut self,
        event: crate::net::discovery::DiscoveryBehaviourEvent,
    ) -> Result<()> {
        use crate::net::discovery::DiscoveryBehaviourEvent;

        match event {
            DiscoveryBehaviourEvent::Mdns(mdns_event) => match mdns_event {
                libp2p::mdns::Event::Discovered(list) => {
                    for (peer_id, multiaddr) in list {
                        info!("Discovered peer via mDNS: {} at {}", peer_id, multiaddr);

                        if self.blocked_peers.contains_key(&peer_id) {
                            debug!("Skipping mDNS discovery for blocked peer {}", peer_id);
                            continue;
                        }

                        self.swarm
                            .behaviour_mut()
                            .discovery
                            .add_peer_address(peer_id, multiaddr.clone());

                        if let Err(e) = self.swarm.dial(multiaddr) {
                            trace!(
                                "Failed to proactively dial discovered peer {}: {}",
                                peer_id,
                                e
                            );
                        }
                    }
                }
                libp2p::mdns::Event::Expired(list) => {
                    for (peer_id, _) in list {
                        trace!("mDNS record expired for peer: {}", peer_id);
                    }
                }
            },
            DiscoveryBehaviourEvent::Kademlia(kad_event) => {
                self.handle_kademlia_event(kad_event).await?;
            }
        }

        Ok(())
    }

    async fn handle_kademlia_event(&mut self, event: kad::Event) -> Result<()> {
        match event {
            kad::Event::OutboundQueryProgressed { id, result, .. } => match result {
                kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                    key,
                    providers,
                    ..
                })) => {
                    if !providers.is_empty() {
                        trace!("Found {} providers for key: {:?}", providers.len(), key);
                    }

                    if let Some(sync_tx) = &self.sync_event_tx {
                        let dht_result = DhtQueryResult::ProvidersFound {
                            providers: providers.into_iter().collect(),
                            finished: false,
                        };
                        let _ = sync_tx.send(SyncEvent::DhtQueryResult {
                            query_id: id,
                            result: dht_result,
                        });
                    }
                }
                kad::QueryResult::GetProviders(Ok(
                    kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. },
                )) => {
                    trace!("DHT query {} finished with no additional providers", id);

                    if let Some(sync_tx) = &self.sync_event_tx {
                        let dht_result = DhtQueryResult::ProvidersFound {
                            providers: HashSet::new(),
                            finished: true,
                        };
                        let _ = sync_tx.send(SyncEvent::DhtQueryResult {
                            query_id: id,
                            result: dht_result,
                        });
                    }
                }
                kad::QueryResult::GetProviders(Err(e)) => {
                    error!("DHT provider query {} failed: {:?}", id, e);

                    if let Some(sync_tx) = &self.sync_event_tx {
                        let dht_result = DhtQueryResult::QueryFailed {
                            error: format!("{:?}", e),
                        };
                        let _ = sync_tx.send(SyncEvent::DhtQueryResult {
                            query_id: id,
                            result: dht_result,
                        });
                    }
                }
                _ => {}
            },
            kad::Event::RoutingUpdated { peer, .. } => {
                trace!("Kademlia routing table updated for peer: {}", peer);
            }
            _ => {}
        }

        Ok(())
    }
}
