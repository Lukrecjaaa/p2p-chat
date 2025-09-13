use crate::crypto::Identity;
use crate::net::{build_transport, ChatBehaviour, DiscoveryBehaviour, MailboxBehaviour};
use crate::storage::{MailboxStore, SledMailboxStore};
use crate::sync::periodic::SyncEvent;
use crate::types::{
    ChatRequest, ChatResponse, EncryptedMessage, MailboxRequest, MailboxResponse, Message,
};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use libp2p::{
    kad,
    ping,
    request_response::{self, OutboundRequestId, ResponseChannel},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    Multiaddr, PeerId,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, trace, warn};

#[derive(NetworkBehaviour)]
pub struct P2PBehaviour {
    pub chat: ChatBehaviour,
    pub mailbox: MailboxBehaviour,
    pub discovery: DiscoveryBehaviour,
    pub ping: ping::Behaviour,
}

pub struct NetworkLayer {
    swarm: Swarm<P2PBehaviour>,
    command_receiver: mpsc::UnboundedReceiver<NetworkCommand>,
    pending_requests: HashMap<OutboundRequestId, oneshot::Sender<NetworkResponse>>,
    sync_event_tx: Option<mpsc::UnboundedSender<SyncEvent>>,
    mailbox_storage: Option<Arc<SledMailboxStore>>,
    blocked_peers: HashMap<PeerId, std::time::Instant>,
}

#[derive(Debug)]
pub enum NetworkCommand {
    SendMessage {
        peer_id: PeerId,
        message: Message,
        response: oneshot::Sender<NetworkResponse>,
    },
    GetConnectedPeers {
        response: oneshot::Sender<NetworkResponse>,
    },
    MailboxPut {
        peer_id: PeerId,
        recipient: [u8; 32],
        message: EncryptedMessage,
        response: oneshot::Sender<NetworkResponse>,
    },
    MailboxFetch {
        peer_id: PeerId,
        recipient: [u8; 32],
        limit: usize,
        response: oneshot::Sender<NetworkResponse>,
    },
    MailboxAck {
        peer_id: PeerId,
        recipient: [u8; 32],
        msg_ids: Vec<uuid::Uuid>,
        response: oneshot::Sender<NetworkResponse>,
    },
    StartDhtProviderQuery {
        key: kad::RecordKey,
        response: oneshot::Sender<Result<kad::QueryId>>,
    },
}

#[derive(Debug)]
pub enum NetworkResponse {
    MessageSent,
    ConnectedPeers { peers: Vec<PeerId> },
    Error(String),
    MailboxPutResult { success: bool },
    MailboxMessages { messages: Vec<EncryptedMessage> },
    MailboxAckResult { deleted: usize },
}

#[derive(Clone)]
pub struct NetworkHandle {
    command_sender: mpsc::UnboundedSender<NetworkCommand>,
}

impl NetworkHandle {
    pub async fn send_message(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::SendMessage {
            peer_id,
            message,
            response: tx,
        })?;

        match rx.await? {
            NetworkResponse::MessageSent => Ok(()),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }
    
    pub async fn get_connected_peers(&self) -> Result<Vec<PeerId>> {
        let (tx, rx) = oneshot::channel();
        self.command_sender
            .send(NetworkCommand::GetConnectedPeers { response: tx })?;

        match rx.await? {
            NetworkResponse::ConnectedPeers { peers } => Ok(peers),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }
    pub async fn mailbox_put( &self, peer_id: PeerId, recipient: [u8; 32], message: EncryptedMessage) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxPut { peer_id, recipient, message, response: tx, })?;
        match rx.await? { NetworkResponse::MailboxPutResult { success } => Ok(success), NetworkResponse::Error(e) => Err(anyhow!(e)), _ => Err(anyhow!("Unexpected response")), }
    }
    pub async fn mailbox_fetch( &self, peer_id: PeerId, recipient: [u8; 32], limit: usize) -> Result<Vec<EncryptedMessage>> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxFetch { peer_id, recipient, limit, response: tx, })?;
        match rx.await? { NetworkResponse::MailboxMessages { messages } => Ok(messages), NetworkResponse::Error(e) => Err(anyhow!(e)), _ => Err(anyhow!("Unexpected response")), }
    }
    pub async fn mailbox_ack( &self, peer_id: PeerId, recipient: [u8; 32], msg_ids: Vec<uuid::Uuid>) -> Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxAck { peer_id, recipient, msg_ids, response: tx, })?;
        match rx.await? { NetworkResponse::MailboxAckResult { deleted } => Ok(deleted), NetworkResponse::Error(e) => Err(anyhow!(e)), _ => Err(anyhow!("Unexpected response")), }
    }

    pub async fn start_dht_provider_query(&self, key: kad::RecordKey) -> Result<kad::QueryId> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::StartDhtProviderQuery { key, response: tx })?;
        rx.await?
    }

}


impl NetworkLayer {
    pub fn new(
        identity: Arc<Identity>,
        listen_addr: Multiaddr,
        is_mailbox: bool,
        bootstrap_nodes: Vec<&str>,
    ) -> Result<(Self, NetworkHandle)> {
        Self::new_with_mailbox_storage(identity, listen_addr, is_mailbox, None, bootstrap_nodes)
    }
    
    pub fn new_with_mailbox_storage(
        identity: Arc<Identity>,
        listen_addr: Multiaddr,
        is_mailbox: bool,
        mailbox_storage: Option<Arc<SledMailboxStore>>,
        bootstrap_nodes: Vec<&str>,
    ) -> Result<(Self, NetworkHandle)> {
        let keypair = identity.libp2p_keypair.clone();
        let peer_id = identity.peer_id;

        let transport = build_transport(&keypair)?;

        let ping_config = ping::Config::new()
            .with_interval(Duration::from_secs(30))
            .with_timeout(Duration::from_secs(10));

        let mut behaviour = P2PBehaviour {
            chat: crate::net::chat::create_chat_behaviour(),
            mailbox: crate::net::mailbox::create_mailbox_behaviour(),
            discovery: DiscoveryBehaviour::new(peer_id)?,
            ping: ping::Behaviour::new(ping_config),
        };

        // Add bootstrap nodes to the DHT routing table.
        for node in bootstrap_nodes {
            match Multiaddr::from_str(node) {
                Ok(addr) => {
                    if let Some(peer_id) = addr.iter().find_map(|p| {
                        if let libp2p::multiaddr::Protocol::P2p(peer_id) = p {
                            Some(peer_id)
                        } else {
                            None
                        }
                    }) {
                        info!("Adding bootstrap node: {} -> {}", peer_id, addr);
                        behaviour.discovery.kademlia.add_address(&peer_id, addr);
                    } else {
                        warn!("Bootstrap address did not contain a PeerId: {}", node);
                    }
                }
                Err(e) => {
                    warn!("Failed to parse bootstrap address '{}': {}", node, e);
                }
            }
        }

        let swarm_config = libp2p::swarm::Config::with_tokio_executor()
            .with_idle_connection_timeout(Duration::from_secs(60 * 60));

        let mut swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

        swarm.listen_on(listen_addr)?;

        // Immediately trigger the bootstrap process.
        if let Err(e) = swarm.behaviour_mut().discovery.bootstrap() {
            warn!("Initial DHT bootstrap failed: {}", e);
        }

        let (command_sender, command_receiver) = mpsc::unbounded_channel();

        let network_layer = NetworkLayer {
            swarm,
            command_receiver,
            pending_requests: HashMap::new(),
            sync_event_tx: None,
            mailbox_storage,
            blocked_peers: HashMap::new(),
        };

        let handle = NetworkHandle { command_sender };

        info!(
            "Network layer initialized for peer: {} (mailbox: {})",
            peer_id, is_mailbox
        );

        Ok((network_layer, handle))
    }
    
    pub fn set_sync_event_sender(&mut self, sender: mpsc::UnboundedSender<SyncEvent>) {
        self.sync_event_tx = Some(sender);
    }

    pub fn bootstrap_dht(&mut self) -> Result<()> {
        self.swarm.behaviour_mut().discovery.bootstrap()
    }

    pub fn start_providing_mailbox(&mut self) -> Result<()> {
        use crate::mailbox::make_mailbox_provider_key;
        let key = make_mailbox_provider_key();
        self.swarm.behaviour_mut().discovery.start_providing(key)
    }
    
    pub fn start_providing_for_recipient(&mut self, recipient_hash: [u8; 32]) -> Result<()> {
        use crate::mailbox::make_recipient_mailbox_key;
        let key = make_recipient_mailbox_key(recipient_hash);
        self.swarm.behaviour_mut().discovery.start_providing(key)
    }
    
    fn cleanup_blocked_peers(&mut self) {
        let block_duration = Duration::from_secs(600); // 10 minutes
        let mut expired_peers = Vec::new();
        
        for (&peer_id, &blocked_time) in &self.blocked_peers {
            if blocked_time.elapsed() > block_duration {
                expired_peers.push(peer_id);
            }
        }
        
        for peer_id in expired_peers {
            info!("Unblocking peer {} after timeout", peer_id);
            self.blocked_peers.remove(&peer_id);
        }
    }
    
    pub async fn run(&mut self, incoming_messages: mpsc::UnboundedSender<Message>) -> Result<()> {
        info!("Starting network event loop");
        
        let mut cleanup_timer = tokio::time::interval(Duration::from_secs(300)); // Cleanup every 5 minutes

        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    if let Err(e) = self.handle_swarm_event(event, &incoming_messages).await {
                        error!("Error handling swarm event: {}", e);
                    }
                }

                command = self.command_receiver.recv() => {
                    match command {
                        Some(cmd) => {
                            if let Err(e) = self.handle_command(cmd).await {
                                error!("Error handling command: {}", e);
                            }
                        }
                        None => {
                            info!("Command channel closed, shutting down network layer");
                            break;
                        }
                    }
                }
                
                _ = cleanup_timer.tick() => {
                    self.cleanup_blocked_peers();
                }
            }
        }

        Ok(())
    }

    async fn handle_swarm_event(
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

            SwarmEvent::Behaviour(P2PBehaviourEvent::Ping(ping_event)) => {
                match ping_event {
                    ping::Event { peer, result, .. } => {
                        match result {
                            Ok(rtt) => {
                                trace!("Ping to {} successful: RTT is {:?}", peer, rtt);
                            }
                            Err(failure) => {
                                warn!("Ping to {} failed: {:?}", peer, failure);
                            }
                        }
                    }
                }
            }

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
                
                // If this connection failure is for a known mailbox, notify the sync engine
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
                    let _ = sender
                        .send(NetworkResponse::Error(format!("Request failed: {:?}", error)));
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
                    let _ = self
                        .swarm
                        .behaviour_mut()
                        .chat
                        .send_response(channel, ChatResponse::MessageResult { success: false, message_id: None });
                } else {
                    let _ = self
                        .swarm
                        .behaviour_mut()
                        .chat
                        .send_response(channel, ChatResponse::MessageResult { success: true, message_id: Some(message.id) });
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
                ChatResponse::MessageResult { success, message_id: _ } => {
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
                    let _ = sender
                        .send(NetworkResponse::Error(format!("Request failed: {:?}", error)));
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
                            info!("Successfully stored message in mailbox for recipient: {:?}", &recipient[..8]);
                            
                            // Register as provider for this specific recipient for better discovery
                            if let Err(e) = self.start_providing_for_recipient(recipient) {
                                debug!("Failed to register as provider for recipient {:?}: {}", &recipient[..8], e);
                            } else {
                                debug!("Registered as provider for recipient: {:?}", &recipient[..8]);
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
                            info!("Fetched {} messages for recipient: {:?}", messages.len(), &recipient[..8]);
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
                            info!("Deleted {} messages for recipient: {:?}", deleted, &recipient[..8]);
                            
                            // Check if there are any remaining messages for this recipient
                            // If not, we should stop advertising ourselves as having messages
                            match storage.fetch_messages(recipient, 1).await {
                                Ok(remaining_messages) if remaining_messages.is_empty() => {
                                    // No more messages for this recipient, stop advertising
                                    debug!("No more messages for recipient {:?}, could stop DHT announcement", &recipient[..8]);
                                    // Note: We don't have a direct way to stop providing a key in the current libp2p setup
                                    // This would require additional DHT management functionality
                                }
                                Ok(_) => {
                                    // Still have messages for this recipient, keep advertising
                                    debug!("Still have messages for recipient {:?}, keeping DHT announcement", &recipient[..8]);
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
                        
                        // Skip blocked peers
                        if self.blocked_peers.contains_key(&peer_id) {
                            debug!("Skipping mDNS discovery for blocked peer {}", peer_id);
                            continue;
                        }
                        
                        self.swarm.behaviour_mut().discovery.add_peer_address(peer_id, multiaddr.clone());
                        
                        // Proactively dial discovered peers to establish connections faster.
                        if let Err(e) = self.swarm.dial(multiaddr) {
                            trace!("Failed to proactively dial discovered peer {}: {}", peer_id, e);
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
        use crate::sync::periodic::{DhtQueryResult, SyncEvent};
        
        match event {
            kad::Event::OutboundQueryProgressed { id, result, .. } => {
                match result {
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders { 
                        key, providers, .. 
                    })) => {
                        if !providers.is_empty() {
                            trace!("Found {} providers for key: {:?}", providers.len(), key);
                        }
                        
                        if let Some(sync_tx) = &self.sync_event_tx {
                            let dht_result = DhtQueryResult::ProvidersFound {
                                providers: providers.into_iter().collect(),
                                finished: false, // More results may come
                            };
                            let _ = sync_tx.send(SyncEvent::DhtQueryResult { 
                                query_id: id, 
                                result: dht_result 
                            });
                        }
                    }
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. })) => {
                        trace!("DHT query {} finished with no additional providers", id);
                        
                        if let Some(sync_tx) = &self.sync_event_tx {
                            let dht_result = DhtQueryResult::ProvidersFound {
                                providers: HashSet::new(),
                                finished: true, // Query is complete
                            };
                            let _ = sync_tx.send(SyncEvent::DhtQueryResult { 
                                query_id: id, 
                                result: dht_result 
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
                                result: dht_result 
                            });
                        }
                    }
                    _ => { }
                }
            }
            kad::Event::RoutingUpdated { peer, .. } => {
                trace!("Kademlia routing table updated for peer: {}", peer);
            }
            _ => { }
        }
        
        Ok(())
    }

    async fn handle_command(&mut self, command: NetworkCommand) -> Result<()> {
        match command {
            NetworkCommand::SendMessage {
                peer_id,
                message,
                response,
            } => {
                if !self.swarm.is_connected(&peer_id) {
                    debug!("Peer {} not connected, failing send request immediately.", peer_id);
                    let _ = response.send(NetworkResponse::Error("Peer not connected".to_string()));
                    return Ok(());
                }

                let request = ChatRequest::SendMessage { message };
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .chat
                    .send_request(&peer_id, request);
                self.pending_requests.insert(request_id, response);
            }

            NetworkCommand::MailboxPut {
                peer_id,
                recipient,
                message,
                response,
            } => {
                let request = MailboxRequest::Put { recipient, message };
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .mailbox
                    .send_request(&peer_id, request);
                self.pending_requests.insert(request_id, response);
            }

            NetworkCommand::MailboxFetch {
                peer_id,
                recipient,
                limit,
                response,
            } => {
                let request = MailboxRequest::Fetch { recipient, limit };
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .mailbox
                    .send_request(&peer_id, request);
                self.pending_requests.insert(request_id, response);
            }

            NetworkCommand::MailboxAck {
                peer_id,
                recipient,
                msg_ids,
                response,
            } => {
                let request = MailboxRequest::Ack { recipient, msg_ids };
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .mailbox
                    .send_request(&peer_id, request);
                self.pending_requests.insert(request_id, response);
            }

            NetworkCommand::GetConnectedPeers { response } => {
                let peers: Vec<PeerId> = self.swarm.connected_peers().cloned().collect();
                let _ = response.send(NetworkResponse::ConnectedPeers { peers });
            }

            NetworkCommand::StartDhtProviderQuery { key, response } => {
                let query_id = self.swarm.behaviour_mut().discovery.get_providers(key);
                let _ = response.send(Ok(query_id));
            }

        }

        Ok(())
    }
}