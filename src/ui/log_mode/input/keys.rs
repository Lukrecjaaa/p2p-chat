//! This module contains key event handling logic for the log UI mode.
use crate::ui::log_mode::LogMode;
use crate::ui::{UIAction, UIState};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use tracing::debug;

impl LogMode {
    /// Handles a key event in log mode.
    ///
    /// This function processes various key presses, including typing characters,
    /// navigating input history, moving the cursor, and scrolling through logs.
    ///
    /// # Arguments
    ///
    /// * `state` - The current UI state.
    /// * `key` - The `KeyEvent` to handle.
    /// * `_action_tx` - The sender for dispatching UI actions (unused in log mode input).
    ///
    /// # Errors
    ///
    /// This function returns an error if a command execution fails.
    pub async fn handle_key(
        &mut self,
        state: &mut UIState,
        key: KeyEvent,
        _action_tx: &mpsc::UnboundedSender<UIAction>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                if !state.input_buffer.trim().is_empty() {
                    let input = state.input_buffer.clone();
                    self.input_history.push(input.clone());
                    self.history_index = None;

                    self.execute_log_command(&input, state).await?;

                    state.input_buffer.clear();
                    state.cursor_pos = 0;
                }
            }
            KeyCode::Char(c) => {
                state.safe_insert_char(c);
                self.history_index = None;
            }
            KeyCode::Backspace => {
                if state.safe_remove_char_before() {
                    self.history_index = None;
                }
            }
            KeyCode::Delete => {
                state.safe_remove_char_at();
            }
            KeyCode::Left => {
                state.safe_cursor_left();
            }
            KeyCode::Right => {
                state.safe_cursor_right();
            }
            KeyCode::Home => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    state.horizontal_scroll_offset =
                        state.horizontal_scroll_offset.saturating_sub(10);
                } else {
                    state.safe_cursor_home();
                }
            }
            KeyCode::End => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    state.horizontal_scroll_offset =
                        state.horizontal_scroll_offset.saturating_add(10);
                } else {
                    state.safe_cursor_end();
                }
            }
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.navigate_history(state, true);
                } else {
                    state.log_scroll_offset = state.log_scroll_offset.saturating_add(1);
                    state.update_log_scroll_state(state.terminal_size.1 as usize);
                }
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.navigate_history(state, false);
                } else {
                    state.log_scroll_offset = state.log_scroll_offset.saturating_sub(1);
                    state.update_log_scroll_state(state.terminal_size.1 as usize);
                }
            }
            KeyCode::PageUp => {
                state.log_scroll_offset = state.log_scroll_offset.saturating_add(10);
                state.update_log_scroll_state(state.terminal_size.1 as usize);
            }
            KeyCode::PageDown => {
                state.log_scroll_offset = state.log_scroll_offset.saturating_sub(10);
                state.update_log_scroll_state(state.terminal_size.1 as usize);
            }
            KeyCode::Esc => {
                state.jump_to_bottom_log();
            }
            KeyCode::Tab => {
                debug!("Tab pressed in log mode - autocompletion not yet implemented");
            }
            _ => {}
        }

        Ok(())
    }
}
