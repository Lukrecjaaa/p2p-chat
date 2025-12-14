//! This module contains the logic for executing chat commands.
use anyhow::Result;

use super::{commands, context::CommandContext};

/// Executes a chat command.
///
/// This function parses a command line string and dispatches it to the
/// appropriate command handler.
///
/// # Arguments
///
/// * `cmd_line` - The full command line string entered by the user.
/// * `context` - The `CommandContext` providing access to the application's state.
///
/// # Returns
///
/// A `Result` indicating success or failure of the command execution.
pub(super) async fn execute_chat_command(cmd_line: &str, context: CommandContext) -> Result<()> {
    let parts: Vec<&str> = cmd_line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    commands::dispatch(&parts, &context).await
}
