use chrono::{DateTime, Utc};
use tracing::Level;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: Level,
    pub module: String,
    pub message: String,
}
