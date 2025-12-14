//! This module defines the `CommandContext`, which provides access to the
//! application's state and UI for command handlers.
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::cli::commands::Node;
use crate::ui::UIEvent;

/// Provides context and utilities to command handlers.
///
/// This struct allows command handlers to interact with the core application
/// `Node` and send events back to the user interface.
#[derive(Clone)]
pub struct CommandContext {
    /// A reference to the application's core `Node`.
    node: Arc<Node>,
    /// The sender for dispatching `UIEvent`s to the UI.
    ui_sender: mpsc::UnboundedSender<UIEvent>,
}

impl CommandContext {
    /// Creates a new `CommandContext`.
    ///
    /// # Arguments
    ///
    /// * `node` - An `Arc` to the application's core `Node`.
    /// * `ui_sender` - An `mpsc::UnboundedSender` for `UIEvent`s.
    pub fn new(node: Arc<Node>, ui_sender: mpsc::UnboundedSender<UIEvent>) -> Self {
        Self { node, ui_sender }
    }

    /// Returns a reference to the application's core `Node`.
    pub fn node(&self) -> &Arc<Node> {
        &self.node
    }

    /// Emits a generic `UIEvent` to the user interface.
    ///
    /// # Arguments
    ///
    /// * `event` - The `UIEvent` to emit.
    pub fn emit(&self, event: UIEvent) {
        let _ = self.ui_sender.send(event);
    }

    /// Emits a chat message to be displayed in the UI.
    ///
    /// # Arguments
    ///
    /// * `message` - The message content.
    pub fn emit_chat<S: Into<String>>(&self, message: S) {
        self.emit(UIEvent::ChatMessage(message.into()));
    }

    /// Emits a history output block to be displayed in the UI.
    ///
    /// This is typically used for multi-line outputs from commands like `history`.
    ///
    /// # Arguments
    ///
    /// * `message` - The history output content.
    pub fn emit_history<S: Into<String>>(&self, message: S) {
        self.emit(UIEvent::HistoryOutput(message.into()));
    }
}
