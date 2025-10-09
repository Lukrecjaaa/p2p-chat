use crate::types::Message;
use crossterm::event::KeyEvent;

use super::log_entry::LogEntry;

#[derive(Debug)]
pub enum UIEvent {
    NewMessage(Message),
    NewLogBatch(Vec<LogEntry>),
    RefreshLogs,
    ChatMessage(String),
    HistoryOutput(String),
    KeyPress(KeyEvent),
    Resize(u16, u16),
    UpdatePeersCount(usize),
    UpdateDiscoveredPeers(Vec<String>),
}
