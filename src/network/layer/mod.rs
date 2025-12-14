//! This module defines the `NetworkLayer`, which is the main entry point for
//! interacting with the network.
//!
//! It is responsible for creating and managing the `libp2p` `Swarm`, and for
//! handling network events.
mod builder;
mod providers;
mod runtime;
mod state;

pub use state::NetworkLayer;
