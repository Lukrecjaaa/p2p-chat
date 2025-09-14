use super::{UIState, UIAction};
use super::completers::{ChatCompleter};
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io::Write;
use tokio::sync::mpsc;
use tracing::debug;

pub struct ChatMode {
    input_history: Vec<String>,
    history_index: Option<usize>,
    completer: ChatCompleter,
    current_suggestion: Option<String>,
}

impl ChatMode {
    pub fn new() -> Self {
        Self {
            input_history: Vec::new(),
            history_index: None,
            completer: ChatCompleter::new(Vec::new()),
            current_suggestion: None,
        }
    }
    
    pub fn update_friends(&mut self, friends: Vec<String>) {
        self.completer.update_friends(friends);
    }
    
    pub fn update_discovered_peers(&mut self, peers: Vec<String>) {
        self.completer.update_discovered_peers(peers);
    }

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
                    
                    // Parse and execute command
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
                // Accept suggestion if at end of input
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
                self.navigate_history(state, true);
            }
            KeyCode::Down => {
                self.navigate_history(state, false);
            }
            KeyCode::PageUp => {
                state.scroll_offset = state.scroll_offset.saturating_add(10);
            }
            KeyCode::PageDown => {
                state.scroll_offset = state.scroll_offset.saturating_sub(10);
            }
            KeyCode::Tab => {
                // Cycle through completions
                let suggestions = self.completer.get_suggestions(&state.input_buffer);
                if !suggestions.is_empty() {
                    // For now, just take the first suggestion
                    state.input_buffer = suggestions[0].clone();
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
                if suggestion.starts_with(&state.input_buffer) && suggestion != &state.input_buffer {
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
    
    pub fn get_current_suggestion(&self) -> Option<&str> {
        self.current_suggestion.as_deref()
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

    pub fn render(
        &self,
        stdout: &mut impl Write,
        state: &UIState,
        area: (u16, u16, u16, u16), // x, y, width, height
        node: Option<&crate::cli::commands::Node>, // Add node for decryption
    ) -> Result<()> {
        let (x, y, width, height) = area;
        
        // Combine encrypted messages and chat messages with timestamps
        let mut all_items: Vec<(chrono::DateTime<Local>, String, Color)> = Vec::new();
        
        // Add encrypted messages
        for message in &state.messages {
            let timestamp = DateTime::<Utc>::from_timestamp_millis(message.timestamp)
                .unwrap_or_else(|| Utc::now())
                .with_timezone(&chrono::Local);
            
            // Try to decrypt message if node is available
            let content = if let Some(node) = node {
                // Determine if this is a sent or received message
                let is_sent = message.sender == node.identity.peer_id;
                let other_peer = if is_sent { message.recipient } else { message.sender };
                
                // Get the friend's public key
                match tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(node.friends.get_friend(&other_peer))
                }) {
                    Ok(Some(friend)) => {
                        match node.identity.decrypt_from(&friend.e2e_public_key, &message.content) {
                            Ok(plaintext) => String::from_utf8_lossy(&plaintext).to_string(),
                            Err(_) => "[Decryption Failed]".to_string(),
                        }
                    }
                    Ok(None) => "[Unknown Peer]".to_string(),
                    Err(_) => "[Database Error]".to_string(),
                }
            } else {
                "[Encrypted]".to_string()
            };
            
            let (text, color) = if node.map(|n| message.sender == n.identity.peer_id).unwrap_or(false) {
                (format!("You: {}", content), Color::Green)
            } else {
                // Try to get nickname for the sender, fallback to truncated PeerID
                let sender_display = if let Some(node) = node {
                    match tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(node.friends.get_friend(&message.sender))
                    }) {
                        Ok(Some(friend)) => friend.nickname.unwrap_or_else(|| {
                            // Truncate PeerID to first 8 characters for readability (Unicode-safe)
                            let peer_str = message.sender.to_string();
                            if peer_str.chars().count() > 8 {
                                format!("{}...", peer_str.chars().take(8).collect::<String>())
                            } else {
                                peer_str
                            }
                        }),
                        _ => {
                            // Truncate PeerID to first 8 characters for readability (Unicode-safe)
                            let peer_str = message.sender.to_string();
                            if peer_str.chars().count() > 8 {
                                format!("{}...", peer_str.chars().take(8).collect::<String>())
                            } else {
                                peer_str
                            }
                        }
                    }
                } else {
                    message.sender.to_string()
                };
                
                (format!("{}: {}", sender_display, content), Color::Cyan)
            };
            
            all_items.push((timestamp, text, color));
        }
        
        // Add chat messages (command results, system messages)
        for (timestamp, chat_msg) in &state.chat_messages {
            let local_timestamp = timestamp.with_timezone(&chrono::Local);
            // Split multi-line messages into separate lines
            for line in chat_msg.lines() {
                all_items.push((local_timestamp, line.to_string(), Color::White));
            }
        }
        
        // Sort by timestamp
        all_items.sort_by_key(|(timestamp, _, _)| *timestamp);
        
        // Calculate visible range
        let total_items = all_items.len();
        let visible_lines = height as usize;
        
        let start_idx = if total_items > visible_lines {
            if state.scroll_offset >= total_items {
                0
            } else {
                total_items.saturating_sub(visible_lines + state.scroll_offset)
            }
        } else {
            0
        };
        
        let end_idx = (start_idx + visible_lines).min(total_items);
        
        // Render items
        for (line_idx, item_idx) in (start_idx..end_idx).enumerate() {
            if let Some((timestamp, text, color)) = all_items.get(item_idx) {
                queue!(stdout, cursor::MoveTo(x, y + line_idx as u16))?;
                
                // Check if this is a history output (marked with special prefix) to skip showing timestamp
                let full_text = if text.starts_with("__HISTORY_OUTPUT__") {
                    // Remove the marker prefix and don't show timestamp
                    text.strip_prefix("__HISTORY_OUTPUT__").unwrap_or(text).to_string()
                } else {
                    let time_str = timestamp.format("%H:%M:%S").to_string();
                    format!("[{}] {}", time_str, text)
                };
                
                // Apply horizontal scrolling (Unicode-safe)
                let display_text = if state.horizontal_scroll_offset < full_text.chars().count() {
                    full_text.chars().skip(state.horizontal_scroll_offset).collect::<String>()
                } else {
                    String::new()
                };
                
                queue!(
                    stdout,
                    SetForegroundColor(*color),
                    Print(display_text),
                    ResetColor
                )?;
            }
        }

        // Show scroll indicator if there are more items
        if state.scroll_offset > 0 {
            queue!(
                stdout,
                cursor::MoveTo(width - 10, y),
                SetForegroundColor(Color::Yellow),
                Print(format!("↑ +{} more", state.scroll_offset)),
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