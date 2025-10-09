use std::sync::Arc;

use anyhow::Result;
use chrono::{Duration, Utc};
use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::cli::commands::Node;
use crate::logging::LogBuffer;
use crate::types::Message;
use crate::ui::{ChatMode, LogMode, UIAction, UIEvent, UIState};

pub struct TerminalUI {
    pub(super) state: UIState,
    pub(super) chat_mode: ChatMode,
    pub(super) log_mode: LogMode,
    pub(super) event_rx: mpsc::UnboundedReceiver<UIEvent>,
    pub(super) action_tx: mpsc::UnboundedSender<UIAction>,
    pub(super) node: Option<Arc<Node>>,
    pub(super) log_buffer: Option<Arc<LogBuffer>>,
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

    pub fn set_node(&mut self, node: Arc<Node>) {
        self.node = Some(node);
    }

    pub fn set_log_buffer(&mut self, log_buffer: Arc<LogBuffer>) {
        self.log_buffer = Some(log_buffer);
    }

    pub fn update_friends(&mut self, friends: Vec<String>) {
        self.chat_mode.update_friends(friends);
    }

    pub fn update_discovered_peers(&mut self, peers: Vec<String>) {
        self.chat_mode.update_discovered_peers(peers);
    }

    pub fn preload_messages(&mut self, messages: Vec<Message>) {
        let count = messages.len();
        let earliest_timestamp = messages
            .iter()
            .filter_map(|msg| chrono::DateTime::<Utc>::from_timestamp_millis(msg.timestamp))
            .min();

        self.state.replace_messages(messages);

        if count > 0 {
            if let Some(earliest) = earliest_timestamp {
                let header_ts = earliest
                    .checked_sub_signed(Duration::milliseconds(1))
                    .unwrap_or(earliest);
                self.state.chat_messages.push((
                    header_ts,
                    format!(
                        "__HISTORY_OUTPUT__History: last {} message{}",
                        count,
                        if count == 1 { "" } else { "s" }
                    ),
                ));
            }
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.initialize_terminal()?;

        debug!("Starting terminal UI loop");

        loop {
            if let Some(event) = self.event_rx.recv().await {
                if let Err(e) = self.handle_event(event).await {
                    error!("Error handling UI event: {}", e);
                }
            }

            self.render()?;
        }
    }
}
