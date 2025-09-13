use super::{UIState, UIEvent, UIAction, UIMode, ChatMode, LogMode};
use anyhow::Result;
use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use std::io::{stdout, Write};
use tokio::sync::mpsc;
use tracing::{debug, error};

pub struct TerminalUI {
    state: UIState,
    chat_mode: ChatMode,
    log_mode: LogMode,
    event_rx: mpsc::UnboundedReceiver<UIEvent>,
    action_tx: mpsc::UnboundedSender<UIAction>,
    node: Option<std::sync::Arc<crate::cli::commands::Node>>,
    log_buffer: Option<std::sync::Arc<crate::logging::LogBuffer>>,
}

impl TerminalUI {
    pub fn new(
        event_rx: mpsc::UnboundedReceiver<UIEvent>,
        action_tx: mpsc::UnboundedSender<UIAction>,
    ) -> Self {
        Self {
            state: UIState::new(),
            chat_mode: ChatMode::new(),
            log_mode: LogMode::new(),
            event_rx,
            action_tx,
            node: None,
            log_buffer: None,
        }
    }
    
    pub fn set_node(&mut self, node: std::sync::Arc<crate::cli::commands::Node>) {
        self.node = Some(node);
    }

    pub fn set_log_buffer(&mut self, log_buffer: std::sync::Arc<crate::logging::LogBuffer>) {
        self.log_buffer = Some(log_buffer);
    }

    pub async fn run(&mut self) -> Result<()> {
        self.initialize_terminal()?;
        
        debug!("Starting terminal UI loop");
        
        loop {
            // Handle events
            if let Some(event) = self.event_rx.recv().await {
                if let Err(e) = self.handle_event(event).await {
                    error!("Error handling UI event: {}", e);
                }
            }

            // Render current screen
            self.render()?;
        }
    }

    fn initialize_terminal(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(
            stdout(),
            terminal::EnterAlternateScreen,
            cursor::Hide
        )?;
        
        // Get initial terminal size
        let (width, height) = terminal::size()?;
        self.state.terminal_size = (width, height);
        
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<()> {
        terminal::disable_raw_mode()?;
        execute!(
            stdout(),
            cursor::Show,
            terminal::LeaveAlternateScreen
        )?;
        Ok(())
    }
    
    pub fn update_friends(&mut self, friends: Vec<String>) {
        self.chat_mode.update_friends(friends);
    }
    
    pub fn update_discovered_peers(&mut self, peers: Vec<String>) {
        self.chat_mode.update_discovered_peers(peers);
    }

    async fn handle_event(&mut self, event: UIEvent) -> Result<()> {
        match event {
            UIEvent::NewMessage(msg) => {
                self.state.add_message(msg);
            }
            UIEvent::ChatMessage(msg) => {
                self.state.add_chat_message(msg);
            }
            UIEvent::NewLogBatch(entries) => {
                self.state.add_log_batch(entries);
            }
            UIEvent::RefreshLogs => {
                self.state.refresh_logs();
            }
            UIEvent::KeyPress(key_event) => {
                self.handle_key_event(key_event).await?;
            }
            UIEvent::Resize(width, height) => {
                self.state.terminal_size = (width, height);
            }
            UIEvent::UpdatePeersCount(count) => {
                self.state.connected_peers_count = count;
            }
            UIEvent::UpdateDiscoveredPeers(peers) => {
                self.update_discovered_peers(peers);
            }
        }
        Ok(())
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        // Global keybinds
        match (key.code, key.modifiers) {
            (KeyCode::F(9), _) => {
                self.state.toggle_mode();
                // Notify log buffer of mode change and sync level
                if let Some(ref log_buffer) = self.log_buffer {
                    log_buffer.set_ui_mode(self.state.mode.clone());
                    // Sync log level when switching to log mode
                    if let UIMode::Logs { level, .. } = &self.state.mode {
                        log_buffer.set_display_level(*level);
                    }
                }
                return Ok(());
            }
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                let _ = self.action_tx.send(UIAction::Exit);
                return Ok(());
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                self.state.toggle_mode();
                // Notify log buffer of mode change and sync level
                if let Some(ref log_buffer) = self.log_buffer {
                    log_buffer.set_ui_mode(self.state.mode.clone());
                    // Sync log level when switching to log mode
                    if let UIMode::Logs { level, .. } = &self.state.mode {
                        log_buffer.set_display_level(*level);
                    }
                }
                return Ok(());
            }
            _ => {}
        }

        // Mode-specific key handling
        let old_mode = self.state.mode.clone();
        match &self.state.mode {
            UIMode::Chat => {
                self.chat_mode.handle_key(&mut self.state, key, &self.action_tx).await?;
            }
            UIMode::Logs { .. } => {
                self.log_mode.handle_key(&mut self.state, key, &self.action_tx).await?;
                
                // Check if log level changed and notify log buffer
                if let (UIMode::Logs { level: old_level, .. }, UIMode::Logs { level: new_level, .. }) = (&old_mode, &self.state.mode) {
                    if old_level != new_level {
                        if let Some(ref log_buffer) = self.log_buffer {
                            log_buffer.set_display_level(*new_level);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        let mut stdout = stdout();
        
        // Clear screen
        queue!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        
        let (width, height) = self.state.terminal_size;
        
        // Calculate layout dimensions
        let message_area_height = (height as f32 * 0.8) as u16;
        let status_line_row = message_area_height;
        let input_area_row = status_line_row + 1;
        let input_area_height = height - input_area_row;

        // Render based on current mode
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

        // Render status line
        self.render_status_line(&mut stdout, status_line_row, width)?;
        
        // Render input area
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
            UIMode::Chat => {
                format!(
                    " Status: Chat Mode | Peers: {} | F9: Logs | Ctrl+C: Exit",
                    self.state.connected_peers_count
                )
            }
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

        // Truncate status text if too long (Unicode-safe)
        let display_text = if status_text.chars().count() > width as usize {
            status_text.chars().take(width as usize).collect::<String>()
        } else {
            status_text.clone()
        };

        queue!(stdout, Print(&display_text))?;
        
        // Fill rest of line (Unicode-safe with proper width calculation)
        use unicode_width::UnicodeWidthStr;
        let padding = width as usize - UnicodeWidthStr::width(display_text.as_str());
        if padding > 0 {
            queue!(stdout, Print(" ".repeat(padding)))?;
        }

        queue!(stdout, ResetColor)?;
        Ok(())
    }

    fn render_input_area(&self, stdout: &mut impl Write, row: u16, width: u16, height: u16) -> Result<()> {
        queue!(stdout, cursor::MoveTo(0, row))?;

        // Input prompt based on mode
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

        // Render input buffer
        queue!(stdout, Print(&self.state.input_buffer))?;
        
        // Render grayed suggestion if available and we're in chat mode
        if matches!(self.state.mode, UIMode::Chat) {
            if let Some(suggestion) = self.chat_mode.get_current_suggestion() {
                if suggestion.starts_with(&self.state.input_buffer) && suggestion != &self.state.input_buffer {
                    // Use Unicode-safe character-based slicing instead of byte-based slicing
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

        // Show cursor (calculate position accounting for Unicode character widths)
        use unicode_width::UnicodeWidthChar;
        
        let input_display_width: usize = self.state.input_buffer.chars()
            .take(self.state.cursor_pos)
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
            .sum();
        
        let prompt_display_width: usize = prompt.chars()
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
            .sum();
        
        let cursor_x = prompt_display_width + input_display_width;
        if cursor_x < width as usize {
            queue!(stdout, cursor::MoveTo(cursor_x as u16, row))?;
        }

        // Render help text on the last line
        if height > 2 {
            let help_row = row + height - 1;
            queue!(stdout, cursor::MoveTo(0, help_row))?;
            
            let help_text = match &self.state.mode {
                UIMode::Chat => " Tab: complete | ↑↓: history | PgUp/Down: scroll | Ctrl+Home/End: H-scroll | F9: logs",
                UIMode::Logs { .. } => " Tab: complete | ↑↓: scroll | Ctrl+Home/End: H-scroll | F9: chat",
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

impl Drop for TerminalUI {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}