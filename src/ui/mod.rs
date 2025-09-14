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
            input_buffer: String::new(),
            cursor_pos: 0,
            terminal_size: (80, 24),
            max_log_entries: 10000,
            connected_peers_count: 0,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        // Auto-scroll to bottom in chat mode
        if matches!(self.mode, UIMode::Chat) {
            self.scroll_offset = 0;
        }
    }

    pub fn add_chat_message(&mut self, message: String) {
        self.chat_messages.push((Utc::now(), message));
        // Auto-scroll to bottom in chat mode
        if matches!(self.mode, UIMode::Chat) {
            self.scroll_offset = 0;
        }
    }

    pub fn add_history_output(&mut self, message: String) {
        // Use current timestamp for proper chronological ordering, but mark as history output
        let current_timestamp = Utc::now();
        for line in message.lines() {
            // Use a special marker prefix to identify history output
            let marked_line = if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("__HISTORY_OUTPUT__{}", line)
            };
            self.chat_messages.push((current_timestamp, marked_line));
        }
        // Auto-scroll to bottom in chat mode
        if matches!(self.mode, UIMode::Chat) {
            self.scroll_offset = 0;
        }
    }

    pub fn add_log_batch(&mut self, entries: Vec<LogEntry>) {
        // Add multiple log entries efficiently
        for entry in entries {
            if self.logs.len() >= self.max_log_entries {
                self.logs.pop_front();
            }
            self.logs.push_back(entry);
        }
        
        // Only auto-scroll in log mode to prevent chat mode disruption
        if matches!(self.mode, UIMode::Logs { .. }) {
            self.log_scroll_offset = 0;
        }
    }

    pub fn refresh_logs(&mut self) {
        // This method is called when log level changes
        // The filtering is handled in filtered_logs(), so we just need to reset scroll
        if matches!(self.mode, UIMode::Logs { .. }) {
            self.log_scroll_offset = 0;
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