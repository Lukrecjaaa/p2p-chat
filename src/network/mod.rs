use crate::crypto::Identity;
use crate::net::{build_transport, ChatBehaviour, DiscoveryBehaviour, MailboxBehaviour};
use crate::storage::SledMailboxStore;
use crate::sync::SyncEvent;
use crate::types::{EncryptedMessage, Message};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use libp2p::{
    kad, ping,
    request_response::OutboundRequestId,
    swarm::{NetworkBehaviour, Swarm},
    Multiaddr, PeerId,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};

mod commands;
mod handlers;

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

    pub async fn mailbox_put(
        &self,
        peer_id: PeerId,
        recipient: [u8; 32],
        message: EncryptedMessage,
    ) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxPut {
            peer_id,
            recipient,
            message,
            response: tx,
        })?;
        match rx.await? {
            NetworkResponse::MailboxPutResult { success } => Ok(success),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    pub async fn mailbox_fetch(
        &self,
        peer_id: PeerId,
        recipient: [u8; 32],
        limit: usize,
    ) -> Result<Vec<EncryptedMessage>> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxFetch {
            peer_id,
            recipient,
            limit,
            response: tx,
        })?;
        match rx.await? {
            NetworkResponse::MailboxMessages { messages } => Ok(messages),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    pub async fn mailbox_ack(
        &self,
        peer_id: PeerId,
        recipient: [u8; 32],
        msg_ids: Vec<uuid::Uuid>,
    ) -> Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxAck {
            peer_id,
            recipient,
            msg_ids,
            response: tx,
        })?;
        match rx.await? {
            NetworkResponse::MailboxAckResult { deleted } => Ok(deleted),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    pub async fn start_dht_provider_query(&self, key: kad::RecordKey) -> Result<kad::QueryId> {
        let (tx, rx) = oneshot::channel();
        self.command_sender
            .send(NetworkCommand::StartDhtProviderQuery { key, response: tx })?;
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
        let block_duration = Duration::from_secs(600);
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

        let mut cleanup_timer = tokio::time::interval(Duration::from_secs(300));

        loop {
            select! {
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
}
