//! This module defines the composite `NetworkBehaviour` for the application.
use libp2p::{ping, swarm::NetworkBehaviour};

use crate::net::{ChatBehaviour, DiscoveryBehaviour, MailboxBehaviour};

/// The composite `NetworkBehaviour` for the application.
///
/// This struct combines all the individual network behaviours into a single
/// behaviour that can be used by the `libp2p` `Swarm`.
#[derive(NetworkBehaviour)]
pub struct P2PBehaviour {
    /// The behaviour for sending and receiving chat messages.
    pub chat: ChatBehaviour,
    /// The behaviour for interacting with mailbox nodes.
    pub mailbox: MailboxBehaviour,
    /// The behaviour for peer discovery.
    pub discovery: DiscoveryBehaviour,
    /// The behaviour for pinging other peers to keep connections alive.
    pub ping: ping::Behaviour,
}
