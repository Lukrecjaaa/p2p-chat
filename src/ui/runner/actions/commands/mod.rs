mod friends;
mod history;
mod info;
mod peers;
mod send;

use anyhow::Result;

use super::context::CommandContext;

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
