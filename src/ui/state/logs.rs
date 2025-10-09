use crate::ui::{log_entry::LogEntry, mode::UIMode};

use super::UIState;

impl UIState {
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

    pub fn refresh_logs(&mut self) {
        if matches!(self.mode, UIMode::Logs { .. }) {
            self.log_scroll_offset = 0;
            self.is_at_bottom_log = true;
        }
    }

    pub fn update_log_scroll_state(&mut self, terminal_height: usize) {
        let filtered_logs = self.filtered_logs();
        let total_logs = filtered_logs.len();
        let visible_lines = terminal_height.saturating_sub(2);
        let max_scroll = if total_logs > visible_lines {
            total_logs.saturating_sub(visible_lines)
        } else {
            0
        };

        self.log_scroll_offset = self.log_scroll_offset.min(max_scroll);
        self.is_at_bottom_log = self.log_scroll_offset == 0;
    }

    pub fn jump_to_bottom_log(&mut self) {
        self.log_scroll_offset = 0;
        self.is_at_bottom_log = true;
    }

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
