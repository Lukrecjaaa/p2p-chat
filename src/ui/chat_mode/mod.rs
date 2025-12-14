//! This module defines the chat mode functionality for the user interface.
use super::completers::ChatCompleter;

mod input;
mod render;

/// Manages the state and logic for the chat input and display.
pub struct ChatMode {
    /// History of user input commands/messages.
    input_history: Vec<String>,
    /// Current index in the input history for navigation.
    history_index: Option<usize>,
    /// Completer for command and peer ID suggestions.
    completer: ChatCompleter,
    /// The currently suggested completion for the input.
    current_suggestion: Option<String>,
}

impl ChatMode {
    /// Creates a new `ChatMode` instance.
    pub fn new() -> Self {
        Self {
            input_history: Vec::new(),
            history_index: None,
            completer: ChatCompleter::new(Vec::new()),
            current_suggestion: None,
        }
    }

    /// Updates the list of friends for the completer.
    pub fn update_friends(&mut self, friends: Vec<String>) {
        self.completer.update_friends(friends);
    }

    /// Updates the list of discovered peers for the completer.
    pub fn update_discovered_peers(&mut self, peers: Vec<String>) {
        self.completer.update_discovered_peers(peers);
    }

    /// Returns the current suggestion string, if any.
    pub fn get_current_suggestion(&self) -> Option<&str> {
        self.current_suggestion.as_deref()
    }
}
