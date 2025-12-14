//! This module contains the rendering logic for the `TerminalUI`.
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
    /// Renders the entire terminal UI.
    ///
    /// This function clears the screen, then renders the appropriate main view
    /// (chat or logs), the status line, and the input area.
    ///
    /// # Arguments
    ///
    /// * `stdout` - A mutable reference to the output stream.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the output stream fails.
    pub(super) fn render(&mut self) -> Result<()> {
        let mut stdout = stdout();

        queue!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        let (width, height) = self.state.terminal_size;
        let message_area_height = (height as f32 * 0.8) as u16; // 80% of height for messages
        let status_line_row = message_area_height; // Line below messages for status
        let input_area_row = status_line_row + 1; // Line below status for input
        let input_area_height = height - input_area_row; // Remaining height for input area

        // Render main content area based on current UI mode
        match &self.state.mode {
            UIMode::Chat => {
                self.chat_mode.render(
                    &mut stdout,
                    &self.state,
                    (0, 0, width, message_area_height),
                    self.node.as_deref(), // Pass node for message decryption in chat mode
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

        // Render status line
        self.render_status_line(&mut stdout, status_line_row, width)?;
        // Render input area
        self.render_input_area(&mut stdout, input_area_row, width, input_area_height)?;

        stdout.flush()?;
        Ok(())
    }

    /// Renders the status line at the bottom of the message area.
    ///
    /// This line displays the current UI mode, connected peer count, and mode-specific tips.
    ///
    /// # Arguments
    ///
    /// * `stdout` - A mutable reference to the output stream.
    /// * `row` - The row where the status line should be rendered.
    /// * `width` - The width of the terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the output stream fails.
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

        // Truncate status text if it's too long for the terminal width
        let display_text = if status_text.chars().count() > width as usize {
            status_text.chars().take(width as usize).collect::<String>()
        } else {
            status_text.clone()
        };

        queue!(stdout, Print(&display_text))?;

        // Fill remaining space with padding
        let padding = width as usize - UnicodeWidthStr::width(display_text.as_str());
        if padding > 0 {
            queue!(stdout, Print(" ".repeat(padding)))?;
        }

        queue!(stdout, ResetColor)?;
        Ok(())
    }

    /// Renders the input area where the user types commands or messages.
    ///
    /// This includes the input prompt, the current input buffer, and any
    /// autocompletion suggestions. Also renders help text.
    ///
    /// # Arguments
    ///
    /// * `stdout` - A mutable reference to the output stream.
    /// * `row` - The starting row for the input area.
    /// * `width` - The width of the terminal.
    /// * `height` - The height of the input area.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the output stream fails.
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

        // Print current input buffer
        queue!(stdout, Print(&self.state.input_buffer))?;

        // Render autocompletion suggestion for chat mode
        if matches!(self.state.mode, UIMode::Chat) {
            if let Some(suggestion) = self.chat_mode.get_current_suggestion() {
                // Only show hint if input is a prefix of the suggestion
                if suggestion.starts_with(&self.state.input_buffer)
                    && suggestion != self.state.input_buffer
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

        // Position cursor correctly
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

        // Render help text below the input line if enough height is available
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
