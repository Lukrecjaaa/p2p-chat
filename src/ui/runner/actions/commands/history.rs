use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use libp2p::PeerId;

use crate::types::Message;

use super::super::context::CommandContext;
use super::super::resolver::resolve_peer_id;

const DEFAULT_HISTORY_LIMIT: usize = 20;
const MAX_HISTORY_LIMIT: usize = 1000;

pub async fn show_history(parts: &[&str], context: &CommandContext) -> Result<()> {
    if parts.len() < 2 || parts.len() > 3 {
        context.emit_chat("Usage: history <peer_id_or_nickname> [message_count]");
        return Ok(());
    }

    let peer_id = match resolve_peer_id(parts[1], context).await {
        Ok(id) => id,
        Err(e) => {
            context.emit_chat(format!("❌ {}", e));
            return Ok(());
        }
    };

    let limit = match parse_limit(parts.get(2)) {
        Ok(limit) => limit,
        Err(msg) => {
            context.emit_chat(msg);
            return Ok(());
        }
    };

    match context
        .node()
        .history
        .get_history(&context.node().identity.peer_id, &peer_id, limit)
        .await
    {
        Ok(messages) => {
            if messages.is_empty() {
                context.emit_chat(format!("No message history with {}", peer_id));
                return Ok(());
            }

            let mut output = format!(
                "Message history with {} (last {} messages):",
                peer_id,
                messages.len()
            );

            for msg in messages {
                output.push_str(&format!(
                    "\n  [{}] {}",
                    format_timestamp(msg.timestamp),
                    format_direction(&msg, context).await
                ));

                let content = decrypt_content(&msg, context).await;
                output.push(' ');
                output.push_str(&content);
            }

            context.emit_history(output);
        }
        Err(e) => {
            context.emit_chat(format!("❌ Failed to get message history: {}", e));
        }
    }

    Ok(())
}

fn parse_limit(raw: Option<&&str>) -> Result<usize, String> {
    match raw {
        None => Ok(DEFAULT_HISTORY_LIMIT),
        Some(value) => match value.parse::<usize>() {
            Ok(count) if (1..=MAX_HISTORY_LIMIT).contains(&count) => Ok(count),
            _ => Err("❌ Message count must be between 1 and 1000".to_string()),
        },
    }
}

fn format_timestamp(timestamp_ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(timestamp_ms)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "Invalid timestamp".to_string())
}

async fn format_direction(msg: &Message, context: &CommandContext) -> String {
    if msg.sender == context.node().identity.peer_id {
        let label = lookup_peer_label(msg.recipient, context).await;
        format!("\x1b[94mYou -> {}\x1b[0m", label)
    } else {
        let label = lookup_peer_label(msg.sender, context).await;
        format!("\x1b[92m{} -> You\x1b[0m", label)
    }
}

async fn lookup_peer_label(peer_id: PeerId, context: &CommandContext) -> String {
    match context
        .node()
        .friends
        .get_friend(&peer_id)
        .await
        .ok()
        .flatten()
    {
        Some(friend) => friend.nickname.unwrap_or_else(|| short_peer(peer_id)),
        None => short_peer(peer_id),
    }
}

fn short_peer(peer_id: PeerId) -> String {
    let peer_str = peer_id.to_string();
    if peer_str.len() > 8 {
        format!("{}...", &peer_str[..8])
    } else {
        peer_str
    }
}

async fn decrypt_content(msg: &Message, context: &CommandContext) -> String {
    let other_pubkey = if msg.sender == context.node().identity.peer_id {
        context
            .node()
            .friends
            .get_friend(&msg.recipient)
            .await
            .ok()
            .flatten()
            .map(|f| f.e2e_public_key)
    } else {
        context
            .node()
            .friends
            .get_friend(&msg.sender)
            .await
            .ok()
            .flatten()
            .map(|f| f.e2e_public_key)
    };

    match other_pubkey {
        Some(pub_key) => match context.node().identity.decrypt_from(&pub_key, &msg.content) {
            Ok(plaintext) => String::from_utf8_lossy(&plaintext).to_string(),
            Err(_) => "[Decryption Failed]".to_string(),
        },
        None => "[Cannot decrypt - unknown peer]".to_string(),
    }
}
