//! This module contains the builder logic for the `NetworkLayer`.
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use libp2p::{ping, swarm::Swarm, Multiaddr};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::crypto::Identity;
use crate::net::{build_transport, DiscoveryBehaviour};
use crate::storage::SledMailboxStore;

use super::super::behaviour::P2PBehaviour;
use super::super::handle::NetworkHandle;
use super::super::message::NetworkCommand;
use super::NetworkLayer;

impl NetworkLayer {
    /// Creates a new `NetworkLayer` and `NetworkHandle`.
    ///
    /// # Arguments
    ///
    /// * `identity` - The identity of the local node.
    /// * `listen_addr` - The address to listen on for incoming connections.
    /// * `is_mailbox` - Whether the node is a mailbox node.
    /// * `bootstrap_nodes` - A list of bootstrap nodes to connect to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the network layer cannot be created.
    pub fn new(
        identity: Arc<Identity>,
        listen_addr: Multiaddr,
        is_mailbox: bool,
        bootstrap_nodes: Vec<&str>,
    ) -> Result<(Self, NetworkHandle)> {
        Self::new_with_mailbox_storage(identity, listen_addr, is_mailbox, None, bootstrap_nodes)
    }

    /// Creates a new `NetworkLayer` and `NetworkHandle` with optional mailbox storage.
    ///
    /// # Arguments
    ///
    /// * `identity` - The identity of the local node.
    /// * `listen_addr` - The address to listen on for incoming connections.
    /// * `is_mailbox` - Whether the node is a mailbox node.
    /// * `mailbox_storage` - The storage for the mailbox, if this is a mailbox node.
    /// * `bootstrap_nodes` - A list of bootstrap nodes to connect to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the network layer cannot be created.
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

        let (command_sender, command_receiver) = mpsc::unbounded_channel::<NetworkCommand>();

        let network_layer = NetworkLayer {
            swarm,
            command_receiver,
            pending_requests: Default::default(),
            sync_event_tx: None,
            ui_notify_tx: None,
            mailbox_storage,
            blocked_peers: Default::default(),
        };

        let handle = NetworkHandle { command_sender };

        info!(
            "Network layer initialized for peer: {} (mailbox: {})",
            peer_id, is_mailbox
        );

        Ok((network_layer, handle))
    }
}
