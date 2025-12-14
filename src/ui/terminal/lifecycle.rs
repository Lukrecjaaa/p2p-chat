//! This module handles the lifecycle of the terminal user interface.
use anyhow::Result;
use crossterm::{
    cursor, execute,
    terminal::{self},
};
use std::io::stdout;

use super::TerminalUI;

impl TerminalUI {
    /// Initializes the terminal for TUI mode.
    ///
    /// This function enables raw mode, enters the alternate screen, hides the cursor,
    /// and captures the initial terminal size.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup fails.
    pub(super) fn initialize_terminal(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;

        let (width, height) = terminal::size()?;
        self.state.terminal_size = (width, height);

        Ok(())
    }

    /// Cleans up the terminal, restoring it to its original state.
    ///
    /// This function disables raw mode, leaves the alternate screen, and shows the cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal cleanup fails.
    pub fn cleanup(&mut self) -> Result<()> {
        terminal::disable_raw_mode()?;
        execute!(stdout(), cursor::Show, terminal::LeaveAlternateScreen)?;
        Ok(())
    }
}

impl Drop for TerminalUI {
    /// Cleans up the terminal when the `TerminalUI` instance is dropped.
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
