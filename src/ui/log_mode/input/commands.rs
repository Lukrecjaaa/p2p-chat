use crate::ui::log_mode::{LogMode, UIMode};
use crate::ui::UIState;
use anyhow::Result;
use tracing::{debug, Level};

impl LogMode {
    pub(crate) async fn execute_log_command(&self, input: &str, state: &mut UIState) -> Result<()> {
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

                    if let UIMode::Logs { filter, .. } = state.mode.clone() {
                        state.mode = UIMode::Logs {
                            filter,
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
                    if let UIMode::Logs { level, .. } = state.mode.clone() {
                        state.mode = UIMode::Logs {
                            filter: Some(filter.clone()),
                            level,
                        };
                    }
                    debug!("Log filter set to: {}", filter);
                } else if let UIMode::Logs { level, .. } = state.mode.clone() {
                    state.mode = UIMode::Logs {
                        filter: None,
                        level,
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
