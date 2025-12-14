//! This module defines the log mode functionality for the user interface.
use super::UIMode;

mod input;
mod render;

/// Manages the state and logic for the log input and display.
pub struct LogMode {
    /// History of user input commands within the log mode.
    input_history: Vec<String>,
    /// Current index in the input history for navigation.
    history_index: Option<usize>,
}

impl LogMode {
    /// Creates a new `LogMode` instance.
    pub fn new() -> Self {
        Self {
            input_history: Vec::new(),
            history_index: None,
        }
    }
}
