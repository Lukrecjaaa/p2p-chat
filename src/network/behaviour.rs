use libp2p::{ping, swarm::NetworkBehaviour};

use crate::net::{ChatBehaviour, DiscoveryBehaviour, MailboxBehaviour};

#[derive(NetworkBehaviour)]
pub struct P2PBehaviour {
    pub chat: ChatBehaviour,
    pub mailbox: MailboxBehaviour,
    pub discovery: DiscoveryBehaviour,
    pub ping: ping::Behaviour,
}
