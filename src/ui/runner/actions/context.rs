use std::sync::Arc;

use tokio::sync::mpsc;

use crate::cli::commands::Node;
use crate::ui::UIEvent;

#[derive(Clone)]
pub struct CommandContext {
    node: Arc<Node>,
    ui_sender: mpsc::UnboundedSender<UIEvent>,
}

impl CommandContext {
    pub fn new(node: Arc<Node>, ui_sender: mpsc::UnboundedSender<UIEvent>) -> Self {
        Self { node, ui_sender }
    }

    pub fn node(&self) -> &Arc<Node> {
        &self.node
    }

    pub fn emit(&self, event: UIEvent) {
        let _ = self.ui_sender.send(event);
    }

    pub fn emit_chat<S: Into<String>>(&self, message: S) {
        self.emit(UIEvent::ChatMessage(message.into()));
    }

    pub fn emit_history<S: Into<String>>(&self, message: S) {
        self.emit(UIEvent::HistoryOutput(message.into()));
    }
}
