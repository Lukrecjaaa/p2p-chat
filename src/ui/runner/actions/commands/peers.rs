//! This module contains the command handler for listing connected peers.
use anyhow::Result;

use crate::ui::UIEvent;

use super::super::context::CommandContext;

/// Lists all currently connected peers and their roles (friend, mailbox, or generic peer).
///
/// # Arguments
///
/// * `context` - The `CommandContext` providing access to the application's state and network.
///
/// # Errors
///
/// This function returns an error if retrieving the peer list from the network fails.
pub async fn list_peers(context: &CommandContext) -> Result<()> {
    match context.node().network.get_connected_peers().await {
        Ok(peers) => {
            let mailboxes = {
                let sync_engine = context.node().sync_engine.lock().await;
                sync_engine.get_mailbox_providers().clone()
            };

            context.emit(UIEvent::UpdatePeersCount(peers.len()));
            let peer_strings: Vec<String> = peers.iter().map(|p| p.to_string()).collect();
            context.emit(UIEvent::UpdateDiscoveredPeers(peer_strings));

            if peers.is_empty() {
                context.emit_chat("No peers currently connected.");
            } else {
                let mut output = format!("Connected peers ({}):", peers.len());
                for peer in &peers {
                    if let Ok(Some(friend)) = context.node().friends.get_friend(peer).await {
                        let nickname = friend.nickname.as_deref().unwrap_or("(no nickname)");
                        output.push_str(&format!("\n  {} - {} (👥 Friend)", peer, nickname));
                    } else if mailboxes.contains(peer) {
                        output.push_str(&format!("\n  {} - 📬 Mailbox", peer));
                    } else {
                        output.push_str(&format!("\n  {} - 🌐 Peer", peer));
                    }
                }
                context.emit_chat(output);
            }
        }
        Err(e) => {
            context.emit_chat(format!("❌ Failed to get peer list: {}", e));
        }
    }

    Ok(())
}
