use super::super::{UIAction, UIState};
use super::ChatMode;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use tracing::debug;

impl ChatMode {
    pub async fn handle_key(
        &mut self,
        state: &mut UIState,
        key: KeyEvent,
        action_tx: &mpsc::UnboundedSender<UIAction>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                if !state.input_buffer.trim().is_empty() {
                    let input = state.input_buffer.clone();
                    self.input_history.push(input.clone());
                    self.history_index = None;

                    if let Err(e) = self.execute_command(&input, action_tx).await {
                        debug!("Error executing command '{}': {}", input, e);
                    }

                    state.input_buffer.clear();
                    state.cursor_pos = 0;
                }
            }
            KeyCode::Char(c) => {
                state.safe_insert_char(c);
                self.history_index = None;
                self.update_suggestion(state);
            }
            KeyCode::Backspace => {
                if state.safe_remove_char_before() {
                    self.history_index = None;
                    self.update_suggestion(state);
                }
            }
            KeyCode::Delete => {
                state.safe_remove_char_at();
            }
            KeyCode::Left => {
                state.safe_cursor_left();
            }
            KeyCode::Right => {
                let char_count = state.input_buffer.chars().count();
                if state.cursor_pos == char_count {
                    if let Some(suggestion) = &self.current_suggestion {
                        state.input_buffer = suggestion.clone();
                        state.safe_cursor_end();
                        self.current_suggestion = None;
                    }
                } else {
                    state.safe_cursor_right();
                }
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
                self.navigate_history(state, true);
            }
            KeyCode::Down => {
                self.navigate_history(state, false);
            }
            KeyCode::PageUp => {
                state.scroll_offset = state.scroll_offset.saturating_add(10);
                state.update_chat_scroll_state(state.terminal_size.1 as usize);
            }
            KeyCode::PageDown => {
                state.scroll_offset = state.scroll_offset.saturating_sub(10);
                state.update_chat_scroll_state(state.terminal_size.1 as usize);
            }
            KeyCode::Esc => {
                state.jump_to_bottom_chat();
            }
            KeyCode::Tab => {
                let suggestions = self.completer.get_suggestions(&state.input_buffer);
                if let Some(first) = suggestions.first() {
                    state.input_buffer = first.clone();
                    state.safe_cursor_end();
                    self.current_suggestion = None;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn update_suggestion(&mut self, state: &UIState) {
        let char_count = state.input_buffer.chars().count();
        if state.cursor_pos == char_count && !state.input_buffer.trim().is_empty() {
            let suggestions = self.completer.get_suggestions(&state.input_buffer);
            if let Some(suggestion) = suggestions.first() {
                if suggestion.starts_with(&state.input_buffer) && suggestion != &state.input_buffer
                {
                    self.current_suggestion = Some(suggestion.clone());
                } else {
                    self.current_suggestion = None;
                }
            } else {
                self.current_suggestion = None;
            }
        } else {
            self.current_suggestion = None;
        }
    }

    fn navigate_history(&mut self, state: &mut UIState, up: bool) {
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

    async fn execute_command(
        &self,
        input: &str,
        action_tx: &mpsc::UnboundedSender<UIAction>,
    ) -> Result<()> {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        let command = parts[0];
        match command {
            "send" => {
                if parts.len() >= 3 {
                    let recipient = parts[1].to_string();
                    let message = parts[2..].join(" ");
                    let _ = action_tx.send(UIAction::SendMessage(recipient, message));
                } else {
                    debug!("Usage: send <recipient> <message>");
                }
            }
            _ => {
                let _ = action_tx.send(UIAction::ExecuteCommand(input.to_string()));
            }
        }

        Ok(())
    }
}
