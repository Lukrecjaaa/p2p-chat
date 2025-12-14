//! This module provides utility functions for resolving peer IDs.
use std::str::FromStr;

use anyhow::{anyhow, Result};
use libp2p::PeerId;

use super::context::CommandContext;

/// Resolves a `PeerId` from a string, which can be either a direct `PeerId`
/// or a friend's nickname.
///
/// # Arguments
///
/// * `destination` - The string to resolve (Peer ID or nickname).
/// * `context` - The `CommandContext` for accessing friend information.
///
/// # Returns
///
/// A `Result` containing the resolved `PeerId` or an error if the peer
/// cannot be found.
pub(crate) async fn resolve_peer_id(destination: &str, context: &CommandContext) -> Result<PeerId> {
    if let Ok(peer_id) = PeerId::from_str(destination) {
        return Ok(peer_id);
    }

    let friends = context.node().friends.list_friends().await?;
    friends
        .into_iter()
        .find(|f| f.nickname.as_deref() == Some(destination))
        .map(|f| f.peer_id)
        .ok_or_else(|| anyhow!("Peer not found by ID or nickname: '{}'", destination))
}
