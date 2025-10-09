use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::debug;

use crate::cli::commands::Node;
use crate::ui::{UIAction, UIEvent};

use super::context::CommandContext;
use super::execute::execute_chat_command;

pub async fn handle_ui_action(
    action: UIAction,
    node: &Arc<Node>,
    ui_sender: mpsc::UnboundedSender<UIEvent>,
) -> Result<()> {
    if let UIAction::ExecuteCommand(ref command) = action {
        if command.trim() == "exit" {
            let _ = ui_sender.send(UIEvent::ChatMessage("👋 Goodbye!".to_string()));
            sleep(Duration::from_millis(50)).await;
            std::process::exit(0);
        }
    } else if matches!(action, UIAction::Exit) {
        let _ = ui_sender.send(UIEvent::ChatMessage("👋 Goodbye!".to_string()));
        sleep(Duration::from_millis(50)).await;
        std::process::exit(0);
    }

    let context = CommandContext::new(node.clone(), ui_sender.clone());
    tokio::spawn(async move {
        let cmd_to_run = match action {
            UIAction::SendMessage(recipient, message) => {
                format!("send {} {}", recipient, message)
            }
            UIAction::ExecuteCommand(command) => command,
            UIAction::Exit => return,
        };

        debug!("Executing command in background: '{}'", cmd_to_run);
        if let Err(e) = execute_chat_command(&cmd_to_run, context.clone()).await {
            context.emit_chat(format!("❌ Error: {}", e));
        }
    });

    Ok(())
}
