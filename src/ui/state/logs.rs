//! This module contains log-related functionalities for the `UIState`.
use crate::ui::{log_entry::LogEntry, mode::UIMode};

use super::UIState;

impl UIState {
    /// Adds a batch of new log entries to the UI state.
    ///
    /// This function appends new log entries to the buffer, enforcing `max_log_entries`.
    /// If the UI is in log mode and at the bottom of the scroll, it resets the
    /// scroll offset. Otherwise, it adjusts the scroll offset to keep new
    /// messages visible if not explicitly scrolled up.
    ///
    /// # Arguments
    ///
    /// * `entries` - A `Vec` of `LogEntry` to add.
    pub fn add_log_batch(&mut self, entries: Vec<LogEntry>) {
        let new_entries_count = entries.len();

        for entry in entries {
            if self.logs.len() >= self.max_log_entries {
                self.logs.pop_front();
            }
            self.logs.push_back(entry);
        }

        if matches!(self.mode, UIMode::Logs { .. }) {
            if self.is_at_bottom_log {
                self.log_scroll_offset = 0;
            } else {
                self.log_scroll_offset = self.log_scroll_offset.saturating_add(new_entries_count);
                self.update_log_scroll_state(self.terminal_size.1 as usize);
            }
        }
    }

    /// Triggers a refresh of the log display.
    ///
    /// This function resets the log scroll offset and marks the view as being
    /// at the bottom, useful when filter settings change.
    pub fn refresh_logs(&mut self) {
        if matches!(self.mode, UIMode::Logs { .. }) {
            self.log_scroll_offset = 0;
            self.is_at_bottom_log = true;
        }
    }

    /// Updates the log scroll state based on the current terminal height.
    ///
    /// This ensures that the `log_scroll_offset` remains within valid bounds and
    /// updates `is_at_bottom_log`.
    ///
    /// # Arguments
    ///
    /// * `terminal_height` - The current height of the terminal in lines.
    pub fn update_log_scroll_state(&mut self, terminal_height: usize) {
        let filtered_logs = self.filtered_logs();
        let total_logs = filtered_logs.len();
        let visible_lines = terminal_height.saturating_sub(2); // Account for input and status lines
        let max_scroll = if total_logs > visible_lines {
            total_logs.saturating_sub(visible_lines)
        } else {
            0
        };

        self.log_scroll_offset = self.log_scroll_offset.min(max_scroll);
        self.is_at_bottom_log = self.log_scroll_offset == 0;
    }

    /// Scrolls the log view to the bottom.
    pub fn jump_to_bottom_log(&mut self) {
        self.log_scroll_offset = 0;
        self.is_at_bottom_log = true;
    }

    /// Returns a vector of log entries filtered by the current `UIMode::Logs` settings.
    ///
    /// Logs can be filtered by minimum `Level` and by a text filter string,
    /// which can include exclusions prefixed with `-`.
    pub fn filtered_logs(&self) -> Vec<&LogEntry> {
        match &self.mode {
            UIMode::Logs { filter, level } => self
                .logs
                .iter()
                .filter(|entry| {
                    entry.level <= *level
                        && filter
                            .as_ref()
                            .map(|f| {
                                if let Some(exclusion) = f.strip_prefix('-') {
                                    !entry.module.contains(exclusion)
                                } else {
                                    entry.module.contains(f) || entry.message.contains(f)
                                }
                            })
                            .unwrap_or(true)
                })
                .collect(),
            _ => self.logs.iter().collect(),
        }
    }
}
