//! This module provides the networking capabilities for the application.
//!
//! It is responsible for building the `libp2p` transport and defining the
//! network behaviours for chat, discovery, and mailboxes.
pub mod chat;
pub mod discovery;
pub mod mailbox;

use anyhow::Result;
use libp2p::{
    core::{transport::Boxed, upgrade::Version},
    identity, noise, tcp, yamux, PeerId, Transport,
};

// Type alias for the transport.
type BoxedTransport = Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>;

pub use chat::ChatBehaviour;
pub use discovery::DiscoveryBehaviour;
pub use mailbox::MailboxBehaviour;

/// Builds the `libp2p` transport.
///
/// This function creates a TCP-based transport that is secured with Noise
/// and multiplexed with Yamux.
///
/// # Arguments
///
/// * `keypair` - The `identity::Keypair` of the local node.
///
/// # Errors
///
/// This function will return an error if the transport cannot be built.
pub fn build_transport(keypair: &identity::Keypair) -> Result<BoxedTransport> {
    let tcp = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
    let noise = noise::Config::new(keypair)?;
    let yamux = yamux::Config::default();

    let transport = tcp
        .upgrade(Version::V1)
        .authenticate(noise)
        .multiplex(yamux)
        .boxed();

    Ok(transport)
}
