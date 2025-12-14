//! This module defines the events that can be sent to the UI.
use crate::types::Message;
use crossterm::event::KeyEvent;

use super::log_entry::LogEntry;

/// Represents an event that can be sent to the UI.
#[derive(Debug)]
pub enum UIEvent {
    /// A new message has arrived.
    NewMessage(Message),
    /// A batch of new log entries has arrived.
    NewLogBatch(Vec<LogEntry>),
    /// Request to refresh the displayed logs.
    RefreshLogs,
    /// A chat message to be displayed in the UI.
    ChatMessage(String),
    /// A block of text representing historical output, typically from command execution.
    HistoryOutput(String),
    /// A key press event from the terminal.
    KeyPress(KeyEvent),
    /// The terminal has been resized.
    Resize(u16, u16),
    /// Update the count of connected peers.
    UpdatePeersCount(usize),
    /// Update the list of discovered peers.
    UpdateDiscoveredPeers(Vec<String>),
}
