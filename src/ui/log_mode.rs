use super::{UIState, UIAction, UIMode};
use anyhow::Result;
use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io::Write;
use tokio::sync::mpsc;
use tracing::{debug, Level};

pub struct LogMode {
    input_history: Vec<String>,
    history_index: Option<usize>,
}

impl LogMode {
    pub fn new() -> Self {
        Self {
            input_history: Vec::new(),
            history_index: None,
        }
    }

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
                    
                    // Execute log command
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
                    // Ctrl+Home: scroll horizontally left
                    state.horizontal_scroll_offset = state.horizontal_scroll_offset.saturating_sub(10);
                } else {
                    // Normal Home: move cursor to start of input
                    state.safe_cursor_home();
                }
            }
            KeyCode::End => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+End: scroll horizontally right
                    state.horizontal_scroll_offset = state.horizontal_scroll_offset.saturating_add(10);
                } else {
                    // Normal End: move cursor to end of input
                    state.safe_cursor_end();
                }
            }
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.navigate_history(state, true);
                } else {
                    // Scroll logs up
                    state.log_scroll_offset = state.log_scroll_offset.saturating_add(1);
                }
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.navigate_history(state, false);
                } else {
                    // Scroll logs down
                    state.log_scroll_offset = state.log_scroll_offset.saturating_sub(1);
                }
            }
            KeyCode::PageUp => {
                state.log_scroll_offset = state.log_scroll_offset.saturating_add(10);
            }
            KeyCode::PageDown => {
                state.log_scroll_offset = state.log_scroll_offset.saturating_sub(10);
            }
            KeyCode::Tab => {
                // TODO: Implement log command autocompletion
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

        let command = parts[0];
        match command {
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
                            debug!("Invalid log level: {}. Use trace/debug/info/warn/error", parts[1]);
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
                } else {
                    // Clear filter
                    if let UIMode::Logs { level, .. } = &state.mode {
                        state.mode = UIMode::Logs {
                            filter: None,
                            level: *level,
                        };
                    }
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
                    // TODO: Implement log export
                    debug!("Log export not yet implemented: {}", parts[1]);
                } else {
                    debug!("Usage: export <filename>");
                }
            }
            "help" => {
                debug!("Log commands: level <level>, filter <text>, clear, tail <n>, export <file>");
            }
            _ => {
                debug!("Unknown log command: {}. Type 'help' for available commands.", command);
            }
        }

        Ok(())
    }

    pub fn render(
        &self,
        stdout: &mut impl Write,
        state: &UIState,
        area: (u16, u16, u16, u16), // x, y, width, height
    ) -> Result<()> {
        let (x, y, width, height) = area;
        
        // Get filtered logs
        let filtered_logs = state.filtered_logs();
        let total_logs = filtered_logs.len();
        let visible_lines = height as usize;
        
        // Calculate visible log range
        let start_idx = if total_logs > visible_lines {
            if state.log_scroll_offset >= total_logs {
                0
            } else {
                total_logs.saturating_sub(visible_lines + state.log_scroll_offset)
            }
        } else {
            0
        };
        
        let end_idx = (start_idx + visible_lines).min(total_logs);
        
        // Render log entries
        for (line_idx, log_idx) in (start_idx..end_idx).enumerate() {
            if let Some(log_entry) = filtered_logs.get(log_idx) {
                queue!(stdout, cursor::MoveTo(x, y + line_idx as u16))?;
                
                // Format timestamp in local timezone
                let timestamp = log_entry.timestamp.with_timezone(&chrono::Local).format("%H:%M:%S%.3f");
                
                // Color based on log level
                let level_color = match log_entry.level {
                    Level::ERROR => Color::Red,
                    Level::WARN => Color::Yellow,
                    Level::INFO => Color::Blue,
                    Level::DEBUG => Color::White,
                    Level::TRACE => Color::DarkGrey,
                };
                
                // Format log line
                let log_line = format!(
                    "{} {:5} [{}] {}",
                    timestamp,
                    format!("{:?}", log_entry.level),
                    log_entry.module,
                    log_entry.message
                );
                
                // Apply horizontal scrolling first (Unicode-safe)
                let scrolled_line = if state.horizontal_scroll_offset < log_line.chars().count() {
                    log_line.chars().skip(state.horizontal_scroll_offset).collect::<String>()
                } else {
                    String::new()
                };
                
                // Then truncate if too long (Unicode-safe)
                let display_line = if scrolled_line.chars().count() > width as usize {
                    let truncated: String = scrolled_line.chars().take(width as usize - 3).collect();
                    format!("{}...", truncated)
                } else {
                    scrolled_line
                };
                
                queue!(
                    stdout,
                    SetForegroundColor(level_color),
                    Print(display_line),
                    ResetColor
                )?;
            }
        }

        // Show scroll indicator if there are more logs
        if state.log_scroll_offset > 0 {
            queue!(
                stdout,
                cursor::MoveTo(width - 15, y),
                SetForegroundColor(Color::Yellow),
                Print(format!("↑ +{} more logs", state.log_scroll_offset)),
                ResetColor
            )?;
        }
        
        // Show horizontal scroll indicator if horizontally scrolled
        if state.horizontal_scroll_offset > 0 {
            queue!(
                stdout,
                cursor::MoveTo(x, y),
                SetForegroundColor(Color::Yellow),
                Print(format!("← +{}", state.horizontal_scroll_offset)),
                ResetColor
            )?;
        }

        Ok(())
    }
}