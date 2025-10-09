use anyhow::Result;
use libp2p::{kad, mdns, PeerId};

#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct DiscoveryBehaviour {
    pub mdns: mdns::tokio::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

impl DiscoveryBehaviour {
    pub fn new(local_peer_id: PeerId) -> Result<Self> {
        // Initialize mDNS for local discovery
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

        // Initialize Kademlia DHT
        let store = kad::store::MemoryStore::new(local_peer_id);
        let mut kademlia = kad::Behaviour::new(local_peer_id, store);

        // Add bootstrap nodes - you can add known bootstrap peers here
        // For now, we'll configure it to work with local testing
        kademlia.set_mode(Some(kad::Mode::Server));

        Ok(Self { mdns, kademlia })
    }

    pub fn bootstrap(&mut self) -> Result<()> {
        if let Err(e) = self.kademlia.bootstrap() {
            tracing::warn!("Failed to bootstrap Kademlia: {}", e);
        }
        Ok(())
    }

    pub fn start_providing(&mut self, key: kad::RecordKey) -> Result<()> {
        self.kademlia.start_providing(key)?;
        Ok(())
    }

    pub fn get_providers(&mut self, key: kad::RecordKey) -> kad::QueryId {
        self.kademlia.get_providers(key)
    }

    pub fn add_peer_address(&mut self, peer_id: PeerId, address: libp2p::Multiaddr) {
        self.kademlia.add_address(&peer_id, address);
    }
}
