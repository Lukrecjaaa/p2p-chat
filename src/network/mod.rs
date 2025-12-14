//! This module contains the networking layer of the application.
//!
//! It is responsible for creating and managing the `libp2p` `Swarm`, and for
//! handling all network events and commands.
mod behaviour;
mod commands;
mod handle;
mod handlers;
mod layer;
mod message;

pub use behaviour::P2PBehaviourEvent;
pub use handle::NetworkHandle;
pub use layer::NetworkLayer;
pub use message::{NetworkCommand, NetworkResponse};
