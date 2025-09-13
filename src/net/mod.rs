pub mod chat;
pub mod mailbox;
pub mod discovery;

use anyhow::Result;
use libp2p::{
    core::{transport::Boxed, upgrade::Version},
    identity,
    noise,
    tcp,
    yamux,
    PeerId, Transport,
};

// Type alias for transport
type BoxedTransport = Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>;

pub use chat::ChatBehaviour;
pub use mailbox::MailboxBehaviour;
pub use discovery::DiscoveryBehaviour;

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