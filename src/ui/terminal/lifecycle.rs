use anyhow::Result;
use crossterm::{
    cursor, execute,
    terminal::{self},
};
use std::io::stdout;

use super::TerminalUI;

impl TerminalUI {
    pub(super) fn initialize_terminal(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;

        let (width, height) = terminal::size()?;
        self.state.terminal_size = (width, height);

        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<()> {
        terminal::disable_raw_mode()?;
        execute!(stdout(), cursor::Show, terminal::LeaveAlternateScreen)?;
        Ok(())
    }
}

impl Drop for TerminalUI {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
