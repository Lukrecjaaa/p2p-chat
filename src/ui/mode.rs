//! This module defines the different modes for the user interface.
use tracing::Level;

/// Represents the current mode of the user interface.
#[derive(Debug, Clone)]
pub enum UIMode {
    /// The chat mode, where users can send and receive messages.
    Chat,
    /// The logs mode, where users can view and filter application logs.
    Logs {
        /// An optional filter string to apply to the logs.
        filter: Option<String>,
        /// The minimum log level to display.
        level: Level,
    },
}

impl Default for UIMode {
    /// Returns the default UI mode, which is `Chat`.
    fn default() -> Self {
        Self::Chat
    }
}
