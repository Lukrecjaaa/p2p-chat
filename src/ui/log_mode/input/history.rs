//! This module contains logic for navigating command history in log mode.
use crate::ui::log_mode::LogMode;
use crate::ui::UIState;

impl LogMode {
    /// Navigates through the input history in log mode.
    ///
    /// # Arguments
    ///
    /// * `state` - The current UI state, which holds the input buffer.
    /// * `up` - If `true`, navigates to an older history entry; if `false`, navigates to a newer entry.
    pub(crate) fn navigate_history(&mut self, state: &mut UIState, up: bool) {
        if self.input_history.is_empty() {
            return;
        }

        let new_index = if up {
            match self.history_index {
                None => Some(self.input_history.len() - 1),
                Some(0) => Some(0),
                Some(i) => Some(i - 1),
            }
        } else {
            match self.history_index {
                None => None,
                Some(i) if i + 1 >= self.input_history.len() => None,
                Some(i) => Some(i + 1),
            }
        };

        self.history_index = new_index;

        if let Some(index) = new_index {
            state.input_buffer = self.input_history[index].clone();
            state.safe_cursor_end();
        } else {
            state.input_buffer.clear();
            state.safe_cursor_home();
        }
    }
}
