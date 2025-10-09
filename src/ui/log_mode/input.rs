use super::super::{UIAction, UIState};
use super::{LogMode, UIMode};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use tracing::{debug, Level};

impl LogMode {
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

    async fn execute_log_command(&self, input: &str, state: &mut UIState) -> Result<()> {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "level" => {
                if parts.len() >= 2 {
                    let level_str = parts[1].to_lowercase();
                    let new_level = match level_str.as_str() {
                        "trace" => Level::TRACE,
                        "debug" => Level::DEBUG,
                        "info" => Level::INFO,
                        "warn" => Level::WARN,
                        "error" => Level::ERROR,
                        _ => {
                            debug!(
                                "Invalid log level: {}. Use trace/debug/info/warn/error",
                                parts[1]
                            );
                            return Ok(());
                        }
                    };

                    if let UIMode::Logs { filter, .. } = &state.mode {
                        state.mode = UIMode::Logs {
                            filter: filter.clone(),
                            level: new_level,
                        };
                    }
                    debug!("Log level set to: {:?}", new_level);
                } else {
                    debug!("Usage: level <trace|debug|info|warn|error>");
                }
            }
            "filter" => {
                if parts.len() >= 2 {
                    let filter = parts[1..].join(" ");
                    if let UIMode::Logs { level, .. } = &state.mode {
                        state.mode = UIMode::Logs {
                            filter: Some(filter.clone()),
                            level: *level,
                        };
                    }
                    debug!("Log filter set to: {}", filter);
                } else if let UIMode::Logs { level, .. } = &state.mode {
                    state.mode = UIMode::Logs {
                        filter: None,
                        level: *level,
                    };
                    debug!("Log filter cleared");
                }
            }
            "clear" => {
                state.logs.clear();
                state.log_scroll_offset = 0;
                debug!("Log buffer cleared");
            }
            "tail" => {
                if parts.len() >= 2 {
                    if let Ok(count) = parts[1].parse::<usize>() {
                        let keep_count = count.min(state.logs.len());
                        let remove_count = state.logs.len() - keep_count;
                        for _ in 0..remove_count {
                            state.logs.pop_front();
                        }
                        state.log_scroll_offset = 0;
                        debug!("Showing last {} log entries", keep_count);
                    } else {
                        debug!("Usage: tail <number>");
                    }
                } else {
                    debug!("Usage: tail <number>");
                }
            }
            "export" => {
                if parts.len() >= 2 {
                    debug!("Log export not yet implemented: {}", parts[1]);
                } else {
                    debug!("Usage: export <filename>");
                }
            }
            "help" => {
                debug!(
                    "Log commands: level <level>, filter <text>, clear, tail <n>, export <file>"
                );
            }
            _ => {
                debug!(
                    "Unknown log command: {}. Type 'help' for available commands.",
                    parts[0]
                );
            }
        }

        Ok(())
    }
}
