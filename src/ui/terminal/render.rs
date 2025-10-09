use anyhow::Result;
use crossterm::{
    cursor, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{stdout, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::ui::UIMode;

use super::TerminalUI;

impl TerminalUI {
    pub(super) fn render(&mut self) -> Result<()> {
        let mut stdout = stdout();

        queue!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        let (width, height) = self.state.terminal_size;
        let message_area_height = (height as f32 * 0.8) as u16;
        let status_line_row = message_area_height;
        let input_area_row = status_line_row + 1;
        let input_area_height = height - input_area_row;

        match &self.state.mode {
            UIMode::Chat => {
                self.chat_mode.render(
                    &mut stdout,
                    &self.state,
                    (0, 0, width, message_area_height),
                    self.node.as_deref(),
                )?;
            }
            UIMode::Logs { .. } => {
                self.log_mode.render(
                    &mut stdout,
                    &self.state,
                    (0, 0, width, message_area_height),
                )?;
            }
        }

        self.render_status_line(&mut stdout, status_line_row, width)?;
        self.render_input_area(&mut stdout, input_area_row, width, input_area_height)?;

        stdout.flush()?;
        Ok(())
    }

    fn render_status_line(&self, stdout: &mut impl Write, row: u16, width: u16) -> Result<()> {
        queue!(
            stdout,
            cursor::MoveTo(0, row),
            SetBackgroundColor(Color::DarkGrey),
            SetForegroundColor(Color::White)
        )?;

        let status_text = match &self.state.mode {
            UIMode::Chat => format!(
                " Status: Chat Mode | Peers: {} | F9: Logs | Ctrl+C: Exit",
                self.state.connected_peers_count
            ),
            UIMode::Logs { filter, level } => {
                let filter_text = filter
                    .as_ref()
                    .map(|f| format!(" | Filter: {}", f))
                    .unwrap_or_default();
                format!(
                    " Status: Log Mode | Level: {:?}{} | Entries: {} | F9: Chat",
                    level,
                    filter_text,
                    self.state.logs.len()
                )
            }
        };

        let display_text = if status_text.chars().count() > width as usize {
            status_text.chars().take(width as usize).collect::<String>()
        } else {
            status_text.clone()
        };

        queue!(stdout, Print(&display_text))?;

        let padding = width as usize - UnicodeWidthStr::width(display_text.as_str());
        if padding > 0 {
            queue!(stdout, Print(" ".repeat(padding)))?;
        }

        queue!(stdout, ResetColor)?;
        Ok(())
    }

    fn render_input_area(
        &self,
        stdout: &mut impl Write,
        row: u16,
        width: u16,
        height: u16,
    ) -> Result<()> {
        queue!(stdout, cursor::MoveTo(0, row))?;

        let prompt = match &self.state.mode {
            UIMode::Chat => "p2p> ",
            UIMode::Logs { .. } => "log> ",
        };

        queue!(
            stdout,
            SetForegroundColor(Color::Cyan),
            Print(prompt),
            ResetColor
        )?;

        queue!(stdout, Print(&self.state.input_buffer))?;

        if matches!(self.state.mode, UIMode::Chat) {
            if let Some(suggestion) = self.chat_mode.get_current_suggestion() {
                if suggestion.starts_with(&self.state.input_buffer)
                    && suggestion != &self.state.input_buffer
                {
                    let input_char_count = self.state.input_buffer.chars().count();
                    let hint: String = suggestion.chars().skip(input_char_count).collect();
                    queue!(
                        stdout,
                        SetForegroundColor(Color::DarkGrey),
                        Print(hint),
                        ResetColor
                    )?;
                }
            }
        }

        let input_display_width: usize = self
            .state
            .input_buffer
            .chars()
            .take(self.state.cursor_pos)
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
            .sum();

        let prompt_display_width: usize = prompt
            .chars()
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
            .sum();

        let cursor_x = prompt_display_width + input_display_width;
        if cursor_x < width as usize {
            queue!(stdout, cursor::MoveTo(cursor_x as u16, row))?;
        }

        if height > 2 {
            let help_row = row + height - 1;
            queue!(stdout, cursor::MoveTo(0, help_row))?;

            let help_text = match &self.state.mode {
                UIMode::Chat =>
                    " Tab: complete | ↑↓: history | PgUp/Down: scroll | Ctrl+Home/End: H-scroll | F9: logs",
                UIMode::Logs { .. } =>
                    " Tab: complete | ↑↓: scroll | Ctrl+Home/End: H-scroll | F9: chat",
            };

            queue!(
                stdout,
                SetForegroundColor(Color::DarkGrey),
                Print(help_text),
                ResetColor
            )?;
        }

        Ok(())
    }
}
