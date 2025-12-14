//! This module contains command handlers related to managing friends.
use anyhow::Result;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use libp2p::PeerId;
use std::str::FromStr;

use crate::types::Friend;

use super::super::context::CommandContext;

/// Adds a new friend to the application's friend list.
///
/// This command requires the friend's Peer ID, their E2E public key, and optionally a nickname.
///
/// Usage: `friend <peer_id> <e2e_key> [nickname]`
///
/// # Arguments
///
/// * `parts` - A slice of strings representing the command arguments.
/// * `context` - The `CommandContext` providing access to the application's state and node.
///
/// # Errors
///
/// This function returns an error if adding the friend to storage fails.
pub async fn add_friend(parts: &[&str], context: &CommandContext) -> Result<()> {
    if !(3..=4).contains(&parts.len()) {
        context.emit_chat("Usage: friend <peer_id> <e2e_key> [nickname]");
        return Ok(());
    }

    let peer_id = match PeerId::from_str(parts[1]) {
        Ok(id) => id,
        Err(e) => {
            context.emit_chat(format!("❌ Invalid peer ID: {}", e));
            return Ok(());
        }
    };

    let e2e_public_key = match BASE64_STANDARD.decode(parts[2]) {
        Ok(key) => key,
        Err(e) => {
            context.emit_chat(format!("❌ Invalid base64 key: {}", e));
            return Ok(());
        }
    };

    let nickname = parts.get(3).map(|s| s.to_string());

    let friend = Friend {
        peer_id,
        e2e_public_key,
        nickname: nickname.clone(),
    };

    match context.node().friends.add_friend(friend).await {
        Ok(()) => {
            context.emit_chat(format!(
                "✅ Added friend: {} ({})",
                peer_id,
                nickname.unwrap_or_else(|| "no nickname".to_string())
            ));
        }
        Err(e) => {
            context.emit_chat(format!("❌ Failed to add friend: {}", e));
        }
    }

    Ok(())
}

/// Lists all friends currently stored in the application.
///
/// The output includes the friend's Peer ID and their nickname (if available).
///
/// # Arguments
///
/// * `context` - The `CommandContext` providing access to the application's state and node.
///
/// # Errors
///
/// This function returns an error if retrieving the friend list from storage fails.
pub async fn list_friends(context: &CommandContext) -> Result<()> {
    match context.node().friends.list_friends().await {
        Ok(friends) => {
            if friends.is_empty() {
                context.emit_chat("No friends added yet.");
            } else {
                let mut output = format!("Friends ({}):", friends.len());
                for friend in friends {
                    let nickname = friend.nickname.as_deref().unwrap_or("(no nickname)");
                    output.push_str(&format!("\n  {} - {}", friend.peer_id, nickname));
                }
                context.emit_chat(output);
            }
        }
        Err(e) => {
            context.emit_chat(format!("❌ Failed to list friends: {}", e));
        }
    }

    Ok(())
}
