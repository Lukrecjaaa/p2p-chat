//! This module contains command handlers for displaying application information and help.
use anyhow::Result;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;

use super::super::context::CommandContext;

/// Displays the user's identity information.
///
/// This includes the Peer ID and the E2E public key.
///
/// # Arguments
///
/// * `context` - The `CommandContext` providing access to the user's identity.
pub async fn show_info(context: &CommandContext) -> Result<()> {
    let output = format!(
        "Your Identity:\n  Peer ID: {}\n  E2E Public Key: {}",
        context.node().identity.peer_id,
        BASE64_STANDARD.encode(context.node().identity.hpke_public_key())
    );
    context.emit_chat(output);
    Ok(())
}

/// Displays a message about mailbox discovery status.
///
/// # Arguments
///
/// * `context` - The `CommandContext` for emitting chat messages.
pub async fn show_check_message(context: &CommandContext) -> Result<()> {
    context.emit_chat("✅ Mailbox discovery runs automatically every few seconds.");
    Ok(())
}

/// Displays available commands and their usage.
///
/// # Arguments
///
/// * `context` - The `CommandContext` for emitting chat messages.
pub async fn show_help(context: &CommandContext) -> Result<()> {
    let help_text = "Available commands:\n  friend <peer_id> <e2e_key> [nickname] - Add a friend and optionally assign a nickname\n  friends                     - List all friends\n  send <peer_id_or_nickname> <message>    - Send a message\n  history <peer_id_or_nickname> [count] - Show message history (default: 20, max: 1000)\n  peers                       - Show connected peers\n  info                        - Show your identity\n  check                       - Check for new messages in mailboxes\n  help                        - Show this help\n  exit                        - Exit the application";
    context.emit_chat(help_text);
    Ok(())
}
