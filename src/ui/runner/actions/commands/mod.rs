//! This module contains command dispatching logic for the UI runner.
//!
//! It maps command strings to their respective handler functions.
mod friends;
mod history;
mod info;
mod peers;
mod send;

use anyhow::Result;

use super::context::CommandContext;

/// Dispatches a command to the appropriate handler function.
///
/// This function takes a parsed command (parts) and the command context,
/// then executes the corresponding command handler.
///
/// # Arguments
///
/// * `parts` - A slice of strings representing the command and its arguments.
/// * `context` - The `CommandContext` providing access to the application's state.
///
/// # Returns
///
/// A `Result` indicating success or failure of the command execution.
pub async fn dispatch(parts: &[&str], context: &CommandContext) -> Result<()> {
    match parts[0] {
        "send" => send::handle_send(parts, context).await,
        "friend" => friends::add_friend(parts, context).await,
        "friends" => friends::list_friends(context).await,
        "history" => history::show_history(parts, context).await,
        "peers" => peers::list_peers(context).await,
        "info" => info::show_info(context).await,
        "check" => info::show_check_message(context).await,
        "help" => info::show_help(context).await,
        "exit" => Ok(()),
        _ => {
            context.emit_chat(format!(
                "❌ Unknown command: {}. Type 'help' for available commands.",
                parts[0]
            ));
            Ok(())
        }
    }
}
