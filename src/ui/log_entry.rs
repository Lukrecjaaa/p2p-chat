//! This module defines the structure for a single log entry.
use chrono::{DateTime, Utc};
use tracing::Level;

/// Represents a single log entry with timestamp, level, module, and message.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// The timestamp when the log entry was created.
    pub timestamp: DateTime<Utc>,
    /// The log level (e.g., INFO, DEBUG, ERROR).
    pub level: Level,
    /// The module path where the log originated.
    pub module: String,
    /// The log message content.
    pub message: String,
}
