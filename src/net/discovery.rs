//! This module defines the peer discovery mechanism for the network.
//!
//! It combines mDNS for local peer discovery and Kademlia for decentralized
//! peer discovery in the wider network.
use anyhow::Result;
use libp2p::{kad, mdns, PeerId};

/// The `libp2p` network behaviour for peer discovery.
#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct DiscoveryBehaviour {
    /// The mDNS behaviour for local peer discovery.
    pub mdns: mdns::tokio::Behaviour,
    /// The Kademlia behaviour for decentralized peer discovery.
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

impl DiscoveryBehaviour {
    /// Creates a new `DiscoveryBehaviour`.
    ///
    /// # Arguments
    ///
    /// * `local_peer_id` - The `PeerId` of the local node.
    ///
    /// # Errors
    ///
    /// This function will return an error if the mDNS behaviour cannot be created.
    pub fn new(local_peer_id: PeerId) -> Result<Self> {
        // Initialize mDNS for local discovery.
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

        // Initialize Kademlia DHT.
        let store = kad::store::MemoryStore::new(local_peer_id);
        let mut kademlia = kad::Behaviour::new(local_peer_id, store);

        // Set Kademlia to server mode to participate in the DHT.
        kademlia.set_mode(Some(kad::Mode::Server));

        Ok(Self { mdns, kademlia })
    }

    /// Bootstraps the Kademlia DHT.
    ///
    /// This will start the process of finding other peers in the network.
    ///
    /// # Errors
    ///
    /// This function will return an error if the bootstrap process fails.
    pub fn bootstrap(&mut self) -> Result<()> {
        if let Err(e) = self.kademlia.bootstrap() {
            tracing::warn!("Failed to bootstrap Kademlia: {}", e);
        }
        Ok(())
    }

    /// Starts providing a key in the Kademlia DHT.
    ///
    /// This announces to the network that the local node can provide information
    /// about the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to start providing.
    ///
    /// # Errors
    ///
    /// This function will return an error if the providing process fails to start.
    pub fn start_providing(&mut self, key: kad::RecordKey) -> Result<()> {
        self.kademlia.start_providing(key)?;
        Ok(())
    }

    /// Gets the providers for a given key from the Kademlia DHT.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to get providers for.
    ///
    /// # Returns
    ///
    /// A `QueryId` for the get providers query.
    pub fn get_providers(&mut self, key: kad::RecordKey) -> kad::QueryId {
        self.kademlia.get_providers(key)
    }

    /// Adds a known address for a peer to the Kademlia DHT.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the peer.
    /// * `address` - The `Multiaddr` of the peer.
    pub fn add_peer_address(&mut self, peer_id: PeerId, address: libp2p::Multiaddr) {
        self.kademlia.add_address(&peer_id, address);
    }
}
