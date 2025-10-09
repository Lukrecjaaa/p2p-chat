use chrono::Utc;

use crate::types::Message;
use crate::ui::mode::UIMode;

use super::UIState;

impl UIState {
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);

        if matches!(self.mode, UIMode::Chat) {
            if self.is_at_bottom_chat {
                self.scroll_offset = 0;
            } else {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                self.update_chat_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

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

    pub fn update_chat_scroll_state(&mut self, terminal_height: usize) {
        let total_items = self.calculate_total_chat_items();
        let visible_lines = terminal_height.saturating_sub(2);
        let max_scroll = if total_items > visible_lines {
            total_items.saturating_sub(visible_lines)
        } else {
            0
        };

        self.scroll_offset = self.scroll_offset.min(max_scroll);
        self.is_at_bottom_chat = self.scroll_offset == 0;
    }

    pub fn calculate_total_chat_items(&self) -> usize {
        let message_count = self.messages.len();
        let chat_line_count: usize = self
            .chat_messages
            .iter()
            .map(|(_, msg)| msg.lines().count())
            .sum();

        message_count + chat_line_count
    }

    pub fn jump_to_bottom_chat(&mut self) {
        self.scroll_offset = 0;
        self.is_at_bottom_chat = true;
    }
}
