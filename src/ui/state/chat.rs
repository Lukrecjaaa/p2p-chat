//! This module contains chat-related functionalities for the `UIState`.
use chrono::Utc;

use crate::types::Message;
use crate::ui::mode::UIMode;

use super::{ChatMessageEntry, UIState};

impl UIState {
    /// Adds a new `Message` to the UI state.
    ///
    /// This function stores the message along with its reception timestamp.
    /// If the UI is in chat mode and at the bottom of the scroll, it resets
    /// the scroll offset. Otherwise, it adjusts the scroll offset to keep
    /// new messages visible if not explicitly scrolled up.
    ///
    /// # Arguments
    ///
    /// * `message` - The `Message` to add.
    pub fn add_message(&mut self, message: Message) {
        if self.contains_message(&message.id) {
            return;
        }
        self.messages.push(ChatMessageEntry {
            message,
            received_at: Utc::now(),
        });

        if matches!(self.mode, UIMode::Chat) {
            if self.is_at_bottom_chat {
                self.scroll_offset = 0;
            } else {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                self.update_chat_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

    /// Adds a generic chat message string to the UI state.
    ///
    /// This is typically used for system messages or user input echoes.
    /// It handles scroll adjustment similarly to `add_message`.
    ///
    /// # Arguments
    ///
    /// * `message` - The string content of the chat message.
    pub fn add_chat_message(&mut self, message: String) {
        let line_count = message.lines().count();
        self.chat_messages.push((Utc::now(), message));

        if matches!(self.mode, UIMode::Chat) {
            if self.is_at_bottom_chat {
                self.scroll_offset = 0;
            } else {
                self.scroll_offset = self.scroll_offset.saturating_add(line_count);
                self.update_chat_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

    /// Adds a block of history output to the UI state.
    ///
    /// This is used for displaying multi-line output from commands like `history`.
    /// Each line is prefixed with a special marker for rendering purposes.
    ///
    /// # Arguments
    ///
    /// * `message` - The string content of the history output.
    pub fn add_history_output(&mut self, message: String) {
        let current_timestamp = Utc::now();
        let line_count = message.lines().count();

        for line in message.lines() {
            let marked_line = if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("__HISTORY_OUTPUT__{}", line)
            };
            self.chat_messages.push((current_timestamp, marked_line));
        }

        if matches!(self.mode, UIMode::Chat) {
            if self.is_at_bottom_chat {
                self.scroll_offset = 0;
            } else {
                self.scroll_offset = self.scroll_offset.saturating_add(line_count);
                self.update_chat_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

    /// Updates the chat scroll state based on the current terminal height.
    ///
    /// This ensures that the `scroll_offset` remains within valid bounds and
    /// updates `is_at_bottom_chat`.
    ///
    /// # Arguments
    ///
    /// * `terminal_height` - The current height of the terminal in lines.
    pub fn update_chat_scroll_state(&mut self, terminal_height: usize) {
        let total_items = self.calculate_total_chat_items();
        let visible_lines = terminal_height.saturating_sub(2); // Account for input and status lines
        let max_scroll = if total_items > visible_lines {
            total_items.saturating_sub(visible_lines)
        } else {
            0
        };

        self.scroll_offset = self.scroll_offset.min(max_scroll);
        self.is_at_bottom_chat = self.scroll_offset == 0;
    }

    /// Calculates the total number of displayable items in the chat view.
    ///
    /// This includes both actual messages and generic chat messages.
    ///
    /// # Returns
    ///
    /// The total count of items that can be displayed.
    pub fn calculate_total_chat_items(&self) -> usize {
        let message_count = self.messages.len();
        let chat_line_count: usize = self
            .chat_messages
            .iter()
            .map(|(_, msg)| msg.lines().count())
            .sum();

        message_count + chat_line_count
    }

    /// Scrolls the chat view to the bottom.
    pub fn jump_to_bottom_chat(&mut self) {
        self.scroll_offset = 0;
        self.is_at_bottom_chat = true;
    }
}
