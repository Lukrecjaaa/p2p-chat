//! This module contains the rendering logic for the chat UI mode.
use super::super::UIState;
use super::ChatMode;
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use crossterm::{
    cursor, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io::Write;

impl ChatMode {
    /// Renders the chat interface, including messages and suggestions.
    ///
    /// This function draws the chat history, input buffer, and any active
    /// suggestions to the terminal. It handles scrolling and message formatting.
    ///
    /// # Arguments
    ///
    /// * `stdout` - A mutable reference to the output stream.
    /// * `state` - The current UI state.
    /// * `area` - The (x, y, width, height) coordinates of the rendering area.
    /// * `node` - An optional reference to the application's `Node` for message decryption and friend information.
    ///
    /// # Errors
    ///
    /// This function returns an error if writing to the output stream fails.
    pub fn render(
        &self,
        stdout: &mut impl Write,
        state: &UIState,
        area: (u16, u16, u16, u16),
        node: Option<&crate::cli::commands::Node>,
    ) -> Result<()> {
        let (x, y, width, height) = area;

        let mut all_items: Vec<(DateTime<Utc>, DateTime<Local>, String, Color)> = Vec::new();

        // Process stored messages
        for entry in &state.messages {
            let message = &entry.message;
            let message_timestamp =
                DateTime::<Utc>::from_timestamp_millis(message.timestamp).unwrap_or_else(Utc::now);
            let display_timestamp = message_timestamp.with_timezone(&Local);

            let content = if let Some(node) = node {
                let is_sent = message.sender == node.identity.peer_id;
                let other_peer = if is_sent {
                    message.recipient
                } else {
                    message.sender
                };

                // Decrypt message content if a node is provided
                match tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(node.friends.get_friend(&other_peer))
                }) {
                    Ok(Some(friend)) => {
                        // Attempt to decrypt the message content.
                        match node
                            .identity
                            .decrypt_from(&friend.e2e_public_key, &message.content)
                        {
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

            let (text, color) = if node
                .map(|n| message.sender == n.identity.peer_id)
                .unwrap_or(false)
            {
                (format!("You: {}", content), Color::Green)
            } else {
                let sender_display = if let Some(node) = node {
                    match tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(node.friends.get_friend(&message.sender))
                    }) {
                        Ok(Some(friend)) => friend.nickname.unwrap_or_else(|| {
                            let peer_str = message.sender.to_string();
                            if peer_str.chars().count() > 8 {
                                format!("{}...", peer_str.chars().take(8).collect::<String>())
                            } else {
                                peer_str
                            }
                        }),
                        _ => {
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

            all_items.push((entry.received_at, display_timestamp, text, color));
        }

        // Add chat messages from UI state (e.g., system messages)
        for (timestamp, chat_msg) in &state.chat_messages {
            let local_timestamp = timestamp.with_timezone(&Local);
            for line in chat_msg.lines() {
                all_items.push((*timestamp, local_timestamp, line.to_string(), Color::White));
            }
        }

        // Sort all items chronologically for display
        all_items.sort_by_key(|(ordering, _, _, _)| *ordering);

        let total_items = all_items.len();
        let visible_lines = height as usize;

        // Calculate visible range based on scroll offset
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

        // Render visible items
        for (line_idx, item_idx) in (start_idx..end_idx).enumerate() {
            if let Some((_, timestamp, text, color)) = all_items.get(item_idx) {
                queue!(stdout, cursor::MoveTo(x, y + line_idx as u16))?;

                let full_text = if text.starts_with("__HISTORY_OUTPUT__") {
                    // Special handling for history output without timestamp
                    text.trim_start_matches("__HISTORY_OUTPUT__").to_string()
                } else {
                    let time_str = timestamp.format("%H:%M:%S").to_string();
                    format!("[{}] {}", time_str, text)
                };

                let display_text = if state.horizontal_scroll_offset < full_text.chars().count() {
                    full_text
                        .chars()
                        .skip(state.horizontal_scroll_offset)
                        .collect::<String>()
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

        // Render scroll indicators
        if state.scroll_offset > 0 {
            queue!(
                stdout,
                cursor::MoveTo(x + width - 10, y),
                SetForegroundColor(Color::Yellow),
                Print(format!("↑ +{} more", state.scroll_offset)),
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
