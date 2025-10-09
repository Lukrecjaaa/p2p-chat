use super::super::UIState;
use super::LogMode;
use anyhow::Result;
use crossterm::{
    cursor, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io::Write;
use tracing::Level;

impl LogMode {
    pub fn render(
        &self,
        stdout: &mut impl Write,
        state: &UIState,
        area: (u16, u16, u16, u16),
    ) -> Result<()> {
        let (x, y, width, height) = area;

        let filtered_logs = state.filtered_logs();
        let total_logs = filtered_logs.len();
        let visible_lines = height as usize;

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

        for (line_idx, log_idx) in (start_idx..end_idx).enumerate() {
            if let Some(log_entry) = filtered_logs.get(log_idx) {
                queue!(stdout, cursor::MoveTo(x, y + line_idx as u16))?;

                let timestamp = log_entry
                    .timestamp
                    .with_timezone(&chrono::Local)
                    .format("%H:%M:%S%.3f");

                let level_color = match log_entry.level {
                    Level::ERROR => Color::Red,
                    Level::WARN => Color::Yellow,
                    Level::INFO => Color::Blue,
                    Level::DEBUG => Color::White,
                    Level::TRACE => Color::DarkGrey,
                };

                let log_line = format!(
                    "{} {:5} [{}] {}",
                    timestamp,
                    format!("{:?}", log_entry.level),
                    log_entry.module,
                    log_entry.message
                );

                let scrolled_line = if state.horizontal_scroll_offset < log_line.chars().count() {
                    log_line
                        .chars()
                        .skip(state.horizontal_scroll_offset)
                        .collect::<String>()
                } else {
                    String::new()
                };

                let display_line = if scrolled_line.chars().count() > width as usize {
                    let truncated: String =
                        scrolled_line.chars().take(width as usize - 3).collect();
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

        if state.log_scroll_offset > 0 {
            queue!(
                stdout,
                cursor::MoveTo(width - 15, y),
                SetForegroundColor(Color::Yellow),
                Print(format!("↑ +{} more logs", state.log_scroll_offset)),
                ResetColor
            )?;
        }

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
