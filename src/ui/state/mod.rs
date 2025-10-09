mod chat;
mod input;
mod logs;

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use tracing::Level;
use uuid::Uuid;

use crate::types::Message;

use super::{log_entry::LogEntry, mode::UIMode};

#[derive(Debug)]
pub struct UIState {
    pub mode: UIMode,
    pub last_log_mode: Option<UIMode>,
    pub messages: Vec<ChatMessageEntry>,
    pub chat_messages: Vec<(DateTime<Utc>, String)>,
    pub logs: VecDeque<LogEntry>,
    pub scroll_offset: usize,
    pub log_scroll_offset: usize,
    pub horizontal_scroll_offset: usize,
    pub is_at_bottom_chat: bool,
    pub is_at_bottom_log: bool,
    pub input_buffer: String,
    pub cursor_pos: usize,
    pub terminal_size: (u16, u16),
    pub max_log_entries: usize,
    pub connected_peers_count: usize,
}

impl UIState {
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

    fn contains_message(&self, message_id: &Uuid) -> bool {
        self.messages
            .iter()
            .any(|entry| entry.message.id == *message_id)
    }
}

#[derive(Debug, Clone)]
pub struct ChatMessageEntry {
    pub message: Message,
    pub received_at: DateTime<Utc>,
}
