use super::{TerminalUI, UIAction, UIEvent};
use crate::cli::commands::{Node, UiNotification};
use crate::logging::{LogBuffer, TUILogCollector};
use anyhow::Result;
use crossterm::event::{self, Event};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

mod actions;
use actions::handle_ui_action;

pub async fn run_tui(
    node: Arc<Node>,
    mut ui_notify_rx: tokio::sync::mpsc::UnboundedReceiver<UiNotification>,
    web_port: u16,
) -> Result<()> {
    info!("🚀 Starting P2P Messenger TUI");

    // Initialize log buffer and collector
    let log_buffer = Arc::new(LogBuffer::new(10000));

    // Set up TUI log collector - only when in log mode
    if let Err(e) = TUILogCollector::init_subscriber(log_buffer.clone()) {
        debug!("Failed to initialize TUI log collector: {}", e);
    }

    // Create channels for UI communication
    let (ui_event_tx, ui_event_rx) = mpsc::unbounded_channel::<UIEvent>();
    let (ui_action_tx, mut ui_action_rx) = mpsc::unbounded_channel::<UIAction>();

    // Connect log buffer to UI events
    log_buffer.set_ui_sender(ui_event_tx.clone());

    // Send web UI notification to chat
    let _ = ui_event_tx.send(UIEvent::ChatMessage(format!(
        "🌐 Web UI available at: http://127.0.0.1:{}",
        web_port
    )));

    // Initialize terminal UI
    let mut terminal_ui = TerminalUI::new(ui_event_rx, ui_action_tx.clone());
    terminal_ui.set_node(node.clone());
    terminal_ui.set_log_buffer(log_buffer.clone());

    const INITIAL_HISTORY_LIMIT: usize = 10;
    if let Ok(initial_messages) = node
        .history
        .get_recent_messages(&node.identity.peer_id, INITIAL_HISTORY_LIMIT)
        .await
    {
        terminal_ui.preload_messages(initial_messages);
    }

    // Load friends for autocompletion
    let friends = match node.friends.list_friends().await {
        Ok(friends_list) => friends_list
            .into_iter()
            .filter_map(|f| f.nickname.or_else(|| Some(f.peer_id.to_string())))
            .collect(),
        Err(e) => {
            debug!("Failed to load friends for autocompletion: {}", e);
            Vec::new()
        }
    };

    terminal_ui.update_friends(friends);

    // Spawn terminal event handler
    let ui_event_tx_clone = ui_event_tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                match event::read() {
                    Ok(Event::Key(key_event)) => {
                        if let Err(e) = ui_event_tx_clone.send(UIEvent::KeyPress(key_event)) {
                            debug!("Failed to send key event: {}", e);
                            break;
                        }
                    }
                    Ok(Event::Resize(width, height)) => {
                        if let Err(e) = ui_event_tx_clone.send(UIEvent::Resize(width, height)) {
                            debug!("Failed to send resize event: {}", e);
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    // Spawn UI action handler
    let node_clone = node.clone();
    let ui_event_tx_actions = ui_event_tx.clone();
    tokio::spawn(async move {
        while let Some(action) = ui_action_rx.recv().await {
            if let Err(e) = handle_ui_action(action, &node_clone, ui_event_tx_actions.clone()).await
            {
                error!("Failed to dispatch UI action: {}", e);
            }
        }
    });

    // Spawn UI notification handler (for incoming messages and peer events)
    let ui_event_tx_notifications = ui_event_tx.clone();
    let node_for_notifications = node.clone();
    tokio::spawn(async move {
        while let Some(notification) = ui_notify_rx.recv().await {
            match notification {
                UiNotification::NewMessage(message) => {
                    if let Err(e) = ui_event_tx_notifications.send(UIEvent::NewMessage(message)) {
                        debug!("Failed to send new message event: {}", e);
                        break;
                    }
                }
                UiNotification::PeerConnected(_) | UiNotification::PeerDisconnected(_) => {
                    // Update peers count immediately
                    if let Ok(peers) = node_for_notifications.network.get_connected_peers().await {
                        let _ = ui_event_tx_notifications.send(UIEvent::UpdatePeersCount(peers.len()));
                        let peer_strings: Vec<String> =
                            peers.iter().map(|p| p.to_string()).collect();
                        let _ =
                            ui_event_tx_notifications.send(UIEvent::UpdateDiscoveredPeers(peer_strings));
                    }
                }
            }
        }
    });

    // Spawn periodic peers updater (for autocomplete)
    let ui_event_tx_peers = ui_event_tx.clone();
    let node_peers = node.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            match node_peers.network.get_connected_peers().await {
                Ok(peers) => {
                    let _ = ui_event_tx_peers.send(UIEvent::UpdatePeersCount(peers.len()));
                    let peer_strings: Vec<String> = peers.iter().map(|p| p.to_string()).collect();
                    let _ = ui_event_tx_peers.send(UIEvent::UpdateDiscoveredPeers(peer_strings));
                }
                Err(_) => {
                    // Ignore errors, will try again next interval
                }
            }
        }
    });

    // Load initial friends for autocompletion
    let friends = match node.friends.list_friends().await {
        Ok(friends_list) => friends_list
            .into_iter()
            .filter_map(|f| f.nickname.or_else(|| Some(f.peer_id.to_string())))
            .collect(),
        Err(e) => {
            debug!("Failed to load friends for autocompletion: {}", e);
            Vec::new()
        }
    };

    info!(
        "TUI initialized with {} friends for autocompletion",
        friends.len()
    );

    // Update initial peers count and discovered peers
    match node.network.get_connected_peers().await {
        Ok(peers) => {
            let _ = ui_event_tx.send(UIEvent::UpdatePeersCount(peers.len()));
            let peer_strings: Vec<String> = peers.iter().map(|p| p.to_string()).collect();
            let _ = ui_event_tx.send(UIEvent::UpdateDiscoveredPeers(peer_strings));
        }
        Err(_) => {
            // If we can't get peers count, default to 0 which is already set
        }
    }

    // Run the terminal UI
    terminal_ui.run().await
}
