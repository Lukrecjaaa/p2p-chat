use anyhow::Result;

use super::{commands, context::CommandContext};

pub(super) async fn execute_chat_command(cmd_line: &str, context: CommandContext) -> Result<()> {
    let parts: Vec<&str> = cmd_line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    commands::dispatch(&parts, &context).await
}
