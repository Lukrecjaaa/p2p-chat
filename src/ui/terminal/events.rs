use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::{UIAction, UIEvent, UIMode};

use super::TerminalUI;

impl TerminalUI {
    pub(super) async fn handle_event(&mut self, event: UIEvent) -> Result<()> {
        match event {
            UIEvent::NewMessage(msg) => {
                self.state.add_message(msg);
            }
            UIEvent::ChatMessage(msg) => {
                self.state.add_chat_message(msg);
            }
            UIEvent::HistoryOutput(msg) => {
                self.state.add_history_output(msg);
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
        match (key.code, key.modifiers) {
            (KeyCode::F(9), _) => {
                self.state.toggle_mode();
                if let Some(ref log_buffer) = self.log_buffer {
                    log_buffer.set_ui_mode(self.state.mode.clone());
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
                if let Some(ref log_buffer) = self.log_buffer {
                    log_buffer.set_ui_mode(self.state.mode.clone());
                    if let UIMode::Logs { level, .. } = &self.state.mode {
                        log_buffer.set_display_level(*level);
                    }
                }
                return Ok(());
            }
            _ => {}
        }

        let old_mode = self.state.mode.clone();
        match &self.state.mode {
            UIMode::Chat => {
                self.chat_mode
                    .handle_key(&mut self.state, key, &self.action_tx)
                    .await?;
            }
            UIMode::Logs { .. } => {
                self.log_mode
                    .handle_key(&mut self.state, key, &self.action_tx)
                    .await?;

                if let (
                    UIMode::Logs {
                        level: old_level, ..
                    },
                    UIMode::Logs {
                        level: new_level, ..
                    },
                ) = (&old_mode, &self.state.mode)
                {
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
}
