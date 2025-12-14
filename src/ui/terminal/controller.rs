//! This module defines the `TerminalUI` controller, which manages the main
//! loop for the terminal user interface.
use std::sync::Arc;

use anyhow::Result;
use chrono::{Duration, Utc};
use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::cli::commands::Node;
use crate::logging::LogBuffer;
use crate::types::Message;
use crate::ui::{ChatMode, LogMode, UIAction, UIEvent, UIState};

/// Manages the terminal user interface, including state, rendering, and event handling.
pub struct TerminalUI {
    /// The current state of the user interface.
    pub(super) state: UIState,
    /// The chat mode specific logic and state.
    pub(super) chat_mode: ChatMode,
    /// The log mode specific logic and state.
    pub(super) log_mode: LogMode,
    /// Receiver for UI events from various parts of the application.
    pub(super) event_rx: mpsc::UnboundedReceiver<UIEvent>,
    /// Sender for dispatching UI actions.
    pub(super) action_tx: mpsc::UnboundedSender<UIAction>,
    /// An optional reference to the application's core `Node`.
    pub(super) node: Option<Arc<Node>>,
    /// An optional reference to the `LogBuffer`.
    pub(super) log_buffer: Option<Arc<LogBuffer>>,
}

impl TerminalUI {
    /// Creates a new `TerminalUI` instance.
    ///
    /// # Arguments
    ///
    /// * `event_rx` - The receiver for `UIEvent`s.
    /// * `action_tx` - The sender for `UIAction`s.
    pub fn new(
        event_rx: mpsc::UnboundedReceiver<UIEvent>,
        action_tx: mpsc::UnboundedSender<UIAction>,
    ) -> Self {
        Self {
            state: UIState::new(),
            chat_mode: ChatMode::new(),
            log_mode: LogMode::new(),
            event_rx,
            action_tx,
            node: None,
            log_buffer: None,
        }
    }

    /// Sets the application's core `Node`.
    pub fn set_node(&mut self, node: Arc<Node>) {
        self.node = Some(node);
    }

    /// Sets the `LogBuffer` for the UI.
    pub fn set_log_buffer(&mut self, log_buffer: Arc<LogBuffer>) {
        self.log_buffer = Some(log_buffer);
    }

    /// Updates the list of friends in the chat mode's completer.
    pub fn update_friends(&mut self, friends: Vec<String>) {
        self.chat_mode.update_friends(friends);
    }

    /// Updates the list of discovered peers in the chat mode's completer.
    pub fn update_discovered_peers(&mut self, peers: Vec<String>) {
        self.chat_mode.update_discovered_peers(peers);
    }

    /// Preloads initial messages into the UI state.
    ///
    /// This is typically used to display recent message history on startup.
    ///
    /// # Arguments
    ///
    /// * `messages` - A `Vec` of `Message`s to preload.
    pub fn preload_messages(&mut self, messages: Vec<Message>) {
        let count = messages.len();
        let earliest_timestamp = messages
            .iter()
            .filter_map(|msg| chrono::DateTime::<Utc>::from_timestamp_millis(msg.timestamp))
            .min();

        self.state.replace_messages(messages);

        if count > 0 {
            if let Some(earliest) = earliest_timestamp {
                let header_ts = earliest
                    .checked_sub_signed(Duration::milliseconds(1))
                    .unwrap_or(earliest);
                self.state.chat_messages.push((
                    header_ts,
                    format!(
                        "__HISTORY_OUTPUT__History: last {} message{}",
                        count,
                        if count == 1 { "" } else { "s" }
                    ),
                ));
            }
        }
    }

    /// Runs the main event loop for the terminal UI.
    ///
    /// This function continuously listens for UI events, handles them,
    /// and re-renders the terminal display.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal initialization fails or if an
    /// unrecoverable error occurs within the event handling or rendering loop.
    pub async fn run(&mut self) -> Result<()> {
        self.initialize_terminal()?;

        debug!("Starting terminal UI loop");

        loop {
            if let Some(event) = self.event_rx.recv().await {
                if let Err(e) = self.handle_event(event).await {
                    error!("Error handling UI event: {}", e);
                }
            }

            self.render()?;
        }
    }
}
