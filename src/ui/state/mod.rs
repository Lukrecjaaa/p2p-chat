//! This module defines the central state management for the user interface.
mod chat;
mod input;
mod logs;

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use tracing::Level;
use uuid::Uuid;

use crate::types::Message;

use super::{log_entry::LogEntry, mode::UIMode};

/// Represents the overall state of the user interface.
///
/// This struct holds all the data and configuration necessary to render the UI
/// and respond to user interactions.
#[derive(Debug)]
pub struct UIState {
    /// The current operational mode of the UI (Chat or Logs).
    pub mode: UIMode,
    /// Stores the last log mode for easy switching back.
    pub last_log_mode: Option<UIMode>,
    /// A vector of chat message entries for display.
    pub messages: Vec<ChatMessageEntry>,
    /// A vector of generic chat messages, typically system messages or command output.
    pub chat_messages: Vec<(DateTime<Utc>, String)>,
    /// A deque of log entries for the log view.
    pub logs: VecDeque<LogEntry>,
    /// The current vertical scroll offset for the chat view.
    pub scroll_offset: usize,
    /// The current vertical scroll offset for the log view.
    pub log_scroll_offset: usize,
    /// The current horizontal scroll offset for text.
    pub horizontal_scroll_offset: usize,
    /// Indicates if the chat view is scrolled to the bottom.
    pub is_at_bottom_chat: bool,
    /// Indicates if the log view is scrolled to the bottom.
    pub is_at_bottom_log: bool,
    /// The current content of the input buffer.
    pub input_buffer: String,
    /// The current cursor position within the input buffer.
    pub cursor_pos: usize,
    /// The current size of the terminal (width, height).
    pub terminal_size: (u16, u16),
    /// The maximum number of log entries to retain.
    pub max_log_entries: usize,
    /// The count of currently connected peers.
    pub connected_peers_count: usize,
}

impl UIState {
    /// Creates a new `UIState` with default values.
    pub fn new() -> Self {
        Self {
            mode: UIMode::default(),
            last_log_mode: None,
            messages: Vec::new(),
            chat_messages: Vec::new(),
            logs: VecDeque::with_capacity(10000),
            scroll_offset: 0,
            log_scroll_offset: 0,
            horizontal_scroll_offset: 0,
            is_at_bottom_chat: true,
            is_at_bottom_log: true,
            input_buffer: String::new(),
            cursor_pos: 0,
            terminal_size: (80, 24),
            max_log_entries: 10000,
            connected_peers_count: 0,
        }
    }

    /// Toggles the UI mode between chat and logs.
    ///
    /// When switching to log mode for the first time or from chat mode,
    /// it initializes log mode settings if not already defined. 
    /// Resets scroll offsets and `is_at_bottom` flags upon mode change.
    pub fn toggle_mode(&mut self) {
        self.mode = match &self.mode {
            UIMode::Chat => match &self.last_log_mode {
                Some(UIMode::Logs { filter, level }) => UIMode::Logs {
                    filter: filter.clone(),
                    level: *level,
                },
                _ => UIMode::Logs {
                    filter: None,
                    level: Level::DEBUG,
                },
            },
            UIMode::Logs { .. } => {
                self.last_log_mode = Some(self.mode.clone());
                UIMode::Chat
            }
        };

        self.scroll_offset = 0;
        self.log_scroll_offset = 0;
        self.is_at_bottom_chat = true;
        self.is_at_bottom_log = true;
    }

    /// Replaces the current list of displayed messages with a new set.
    ///
    /// Resets scroll offset to the bottom after replacing messages.
    ///
    /// # Arguments
    ///
    /// * `messages` - A `Vec` of `Message`s to display.
    pub fn replace_messages(&mut self, messages: Vec<Message>) {
        self.messages.clear();
        for message in messages {
            let arrival = chrono::DateTime::<Utc>::from_timestamp_millis(message.timestamp)
                .unwrap_or_else(Utc::now);
            self.messages.push(ChatMessageEntry {
                message,
                received_at: arrival,
            });
        }
        self.scroll_offset = 0;
        self.is_at_bottom_chat = true;
    }

    /// Checks if a message with the given ID already exists in the UI state.
    ///
    /// # Arguments
    ///
    /// * `message_id` - The `Uuid` of the message to check.
    ///
    /// # Returns
    ///
    /// `true` if the message is found, `false` otherwise.
    fn contains_message(&self, message_id: &Uuid) -> bool {
        self.messages
            .iter()
            .any(|entry| entry.message.id == *message_id)
    }
}

/// Represents a chat message along with its reception timestamp.
#[derive(Debug, Clone)]
pub struct ChatMessageEntry {
    /// The actual `Message` content.
    pub message: Message,
    /// The `DateTime` when the message was received by the UI.
    pub received_at: DateTime<Utc>,
}
