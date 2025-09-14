pub mod terminal;
pub mod chat_mode;
pub mod log_mode; 
pub mod completers;
pub mod runner;

pub use terminal::TerminalUI;
pub use chat_mode::ChatMode;
pub use log_mode::LogMode;
pub use runner::run_tui;

use crate::types::Message;
use chrono::{DateTime, Utc};
use crossterm::event::KeyEvent;
use tracing::Level;

#[derive(Debug, Clone)]
pub enum UIMode {
    Chat,
    Logs { 
        filter: Option<String>,
        level: Level,
    },
}

impl Default for UIMode {
    fn default() -> Self {
        Self::Chat
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: Level,
    pub module: String,
    pub message: String,
}

#[derive(Debug)]
pub enum UIEvent {
    NewMessage(Message),
    NewLogBatch(Vec<LogEntry>), // Batched log entries for performance
    RefreshLogs, // Trigger re-filtering of logs with current level
    ChatMessage(String), // Command results, system messages, etc. for chat area
    HistoryOutput(String), // History command output without timestamp
    KeyPress(KeyEvent),
    Resize(u16, u16),
    UpdatePeersCount(usize),
    UpdateDiscoveredPeers(Vec<String>),
}

#[derive(Debug)]
pub enum UIAction {
    SendMessage(String, String), // recipient, message  
    ExecuteCommand(String),
    Exit,
}

pub struct UIState {
    pub mode: UIMode,
    pub last_log_mode: Option<UIMode>, // Preserve log mode settings when switching
    pub messages: Vec<Message>,
    pub chat_messages: Vec<(DateTime<Utc>, String)>, // System messages, command results, etc. with timestamps
    pub logs: std::collections::VecDeque<LogEntry>,
    pub scroll_offset: usize,
    pub log_scroll_offset: usize,
    pub horizontal_scroll_offset: usize,
    pub is_at_bottom_chat: bool, // Track if user is at bottom of chat
    pub is_at_bottom_log: bool,  // Track if user is at bottom of logs
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
            logs: std::collections::VecDeque::with_capacity(10000),
            scroll_offset: 0,
            log_scroll_offset: 0,
            horizontal_scroll_offset: 0,
            is_at_bottom_chat: true, // Start at bottom
            is_at_bottom_log: true,  // Start at bottom
            input_buffer: String::new(),
            cursor_pos: 0,
            terminal_size: (80, 24),
            max_log_entries: 10000,
            connected_peers_count: 0,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        
        // Handle scroll position based on user's current position
        if matches!(self.mode, UIMode::Chat) {
            if self.is_at_bottom_chat {
                // Auto-scroll to bottom when user is already at bottom
                self.scroll_offset = 0;
            } else {
                // Preserve scroll position by adjusting offset for new message
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                // Clamp to valid bounds
                self.update_chat_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

    pub fn add_chat_message(&mut self, message: String) {
        let line_count = message.lines().count();
        self.chat_messages.push((Utc::now(), message));
        
        // Handle scroll position based on user's current position
        if matches!(self.mode, UIMode::Chat) {
            if self.is_at_bottom_chat {
                // Auto-scroll to bottom when user is already at bottom
                self.scroll_offset = 0;
            } else {
                // Preserve scroll position by adjusting offset for new message lines
                self.scroll_offset = self.scroll_offset.saturating_add(line_count);
                // Clamp to valid bounds
                self.update_chat_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

    pub fn add_history_output(&mut self, message: String) {
        // Use current timestamp for proper chronological ordering, but mark as history output
        let current_timestamp = Utc::now();
        let line_count = message.lines().count();
        
        for line in message.lines() {
            // Use a special marker prefix to identify history output
            let marked_line = if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("__HISTORY_OUTPUT__{}", line)
            };
            self.chat_messages.push((current_timestamp, marked_line));
        }
        
        // Handle scroll position based on user's current position
        if matches!(self.mode, UIMode::Chat) {
            if self.is_at_bottom_chat {
                // Auto-scroll to bottom when user is already at bottom
                self.scroll_offset = 0;
            } else {
                // Preserve scroll position by adjusting offset for new history lines
                self.scroll_offset = self.scroll_offset.saturating_add(line_count);
                // Clamp to valid bounds
                self.update_chat_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

    pub fn add_log_batch(&mut self, entries: Vec<LogEntry>) {
        let new_entries_count = entries.len();
        
        // Add multiple log entries efficiently
        for entry in entries {
            if self.logs.len() >= self.max_log_entries {
                self.logs.pop_front();
            }
            self.logs.push_back(entry);
        }
        
        // Handle scroll position based on user's current position
        if matches!(self.mode, UIMode::Logs { .. }) {
            if self.is_at_bottom_log {
                // Auto-scroll to bottom when user is already at bottom
                self.log_scroll_offset = 0;
            } else {
                // Preserve scroll position by adjusting offset for new entries
                self.log_scroll_offset = self.log_scroll_offset.saturating_add(new_entries_count);
                // Clamp to valid bounds
                self.update_log_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

    pub fn refresh_logs(&mut self) {
        // This method is called when log level changes
        // The filtering is handled in filtered_logs(), so we just need to reset scroll
        if matches!(self.mode, UIMode::Logs { .. }) {
            self.log_scroll_offset = 0;
            self.is_at_bottom_log = true;
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match &self.mode {
            UIMode::Chat => {
                // Preserve the last used log level, or default to DEBUG if never set
                match &self.last_log_mode {
                    Some(UIMode::Logs { filter, level }) => UIMode::Logs {
                        filter: filter.clone(),
                        level: *level,
                    },
                    _ => UIMode::Logs {
                        filter: None,
                        level: Level::DEBUG,
                    },
                }
            },
            UIMode::Logs { .. } => {
                // Save current log mode state before switching
                self.last_log_mode = Some(self.mode.clone());
                UIMode::Chat
            },
        };
        // Reset scroll when switching modes
        self.scroll_offset = 0;
        self.log_scroll_offset = 0;
        self.is_at_bottom_chat = true;
        self.is_at_bottom_log = true;
    }

    // Helper methods for scroll management
    pub fn update_chat_scroll_state(&mut self, terminal_height: usize) {
        let total_items = self.calculate_total_chat_items();
        let visible_lines = terminal_height.saturating_sub(2); // Account for input line and status
        let max_scroll = if total_items > visible_lines {
            total_items.saturating_sub(visible_lines)
        } else {
            0
        };
        
        // Clamp scroll offset to valid range
        self.scroll_offset = self.scroll_offset.min(max_scroll);
        
        // Update bottom tracking
        self.is_at_bottom_chat = self.scroll_offset == 0;
    }
    
    pub fn update_log_scroll_state(&mut self, terminal_height: usize) {
        let filtered_logs = self.filtered_logs();
        let total_logs = filtered_logs.len();
        let visible_lines = terminal_height.saturating_sub(2); // Account for input line and status
        let max_scroll = if total_logs > visible_lines {
            total_logs.saturating_sub(visible_lines)
        } else {
            0
        };
        
        // Clamp scroll offset to valid range
        self.log_scroll_offset = self.log_scroll_offset.min(max_scroll);
        
        // Update bottom tracking
        self.is_at_bottom_log = self.log_scroll_offset == 0;
    }
    
    pub fn calculate_total_chat_items(&self) -> usize {
        // Count messages (one item each)
        let message_count = self.messages.len();
        
        // Count chat message lines (split by newlines)
        let chat_line_count: usize = self.chat_messages
            .iter()
            .map(|(_, msg)| msg.lines().count())
            .sum();
        
        message_count + chat_line_count
    }
    
    pub fn jump_to_bottom_chat(&mut self) {
        self.scroll_offset = 0;
        self.is_at_bottom_chat = true;
    }
    
    pub fn jump_to_bottom_log(&mut self) {
        self.log_scroll_offset = 0;
        self.is_at_bottom_log = true;
    }

    pub fn filtered_logs(&self) -> Vec<&LogEntry> {
        match &self.mode {
            UIMode::Logs { filter, level } => {
                self.logs
                    .iter()
                    .filter(|entry| {
                        // Level filtering
                        entry.level <= *level &&
                        // Module filtering
                        if let Some(f) = filter {
                            if f.starts_with('-') {
                                // Exclude filter
                                !entry.module.contains(&f[1..])
                            } else {
                                // Include filter
                                entry.module.contains(f) || entry.message.contains(f)
                            }
                        } else {
                            true
                        }
                    })
                    .collect()
            }
            _ => self.logs.iter().collect(),
        }
    }
    
    // Helper functions for Unicode-safe string operations
    pub fn safe_insert_char(&mut self, c: char) {
        // Convert cursor position (character index) to byte index for insertion
        let char_indices: Vec<_> = self.input_buffer.char_indices().collect();
        let byte_pos = if self.cursor_pos >= char_indices.len() {
            self.input_buffer.len()
        } else {
            char_indices[self.cursor_pos].0
        };
        
        self.input_buffer.insert(byte_pos, c);
        self.cursor_pos += 1;
    }
    
    pub fn safe_remove_char_before(&mut self) -> bool {
        if self.cursor_pos > 0 {
            let char_indices: Vec<_> = self.input_buffer.char_indices().collect();
            if self.cursor_pos <= char_indices.len() {
                let byte_pos = char_indices[self.cursor_pos - 1].0;
                self.input_buffer.remove(byte_pos);
                self.cursor_pos -= 1;
                return true;
            }
        }
        false
    }
    
    pub fn safe_remove_char_at(&mut self) -> bool {
        let char_indices: Vec<_> = self.input_buffer.char_indices().collect();
        if self.cursor_pos < char_indices.len() {
            let byte_pos = char_indices[self.cursor_pos].0;
            self.input_buffer.remove(byte_pos);
            return true;
        }
        false
    }
    
    pub fn safe_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }
    
    pub fn safe_cursor_right(&mut self) {
        let char_count = self.input_buffer.chars().count();
        if self.cursor_pos < char_count {
            self.cursor_pos += 1;
        }
    }
    
    pub fn safe_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }
    
    pub fn safe_cursor_end(&mut self) {
        self.cursor_pos = self.input_buffer.chars().count();
    }
}