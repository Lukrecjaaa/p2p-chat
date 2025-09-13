use super::{UIEvent, UIAction, TerminalUI};
use crate::cli::commands::{Node, UiNotification, MailboxDeliveryResult};
use crate::logging::{LogBuffer, TUILogCollector};
use crate::types::Message;
use anyhow::Result;
use base64::Engine;
use crossterm::event::{self, Event};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use std::str::FromStr;

pub async fn run_tui(
    node: Arc<Node>,
    mut ui_notify_rx: tokio::sync::mpsc::UnboundedReceiver<UiNotification>,
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
    
    // Initialize terminal UI
    let mut terminal_ui = TerminalUI::new(ui_event_rx, ui_action_tx.clone());
    terminal_ui.set_node(node.clone());
    terminal_ui.set_log_buffer(log_buffer.clone());
    
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
            if let Err(e) = handle_ui_action(action, &node_clone, ui_event_tx_actions.clone()).await {
                error!("Failed to dispatch UI action: {}", e);
            }
        }
    });
    
    // Spawn UI notification handler (for incoming messages)
    let ui_event_tx_notifications = ui_event_tx.clone();
    tokio::spawn(async move {
        while let Some(notification) = ui_notify_rx.recv().await {
            match notification {
                UiNotification::NewMessage(message) => {
                    if let Err(e) = ui_event_tx_notifications.send(UIEvent::NewMessage(message)) {
                        debug!("Failed to send new message event: {}", e);
                        break;
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
    
    info!("TUI initialized with {} friends for autocompletion", friends.len());
    
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

async fn handle_ui_action(
    action: UIAction,
    node: &Arc<Node>,
    ui_sender: mpsc::UnboundedSender<UIEvent>,
) -> Result<()> {
    // Check for exit command first, as it needs to be handled synchronously to stop the process.
    if let UIAction::ExecuteCommand(ref command) = action {
        if command.trim() == "exit" {
            let _ = ui_sender.send(UIEvent::ChatMessage("👋 Goodbye!".to_string()));
            tokio::time::sleep(Duration::from_millis(50)).await; // Brief pause for rendering
            std::process::exit(0);
        }
    } else if matches!(action, UIAction::Exit) {
        let _ = ui_sender.send(UIEvent::ChatMessage("👋 Goodbye!".to_string()));
        tokio::time::sleep(Duration::from_millis(50)).await;
        std::process::exit(0);
    }

    // For all other commands, spawn a task to execute them in the background.
    // This prevents the command from blocking the main UI action loop.
    let node_clone = node.clone();
    let ui_sender_clone = ui_sender.clone();
    tokio::spawn(async move {
        let cmd_to_run = match action {
            UIAction::SendMessage(recipient, message) => {
                format!("send {} {}", recipient, message)
            }
            UIAction::ExecuteCommand(command) => command,
            // Exit is handled synchronously above, so we just return here.
            UIAction::Exit => return,
        };
        
        debug!("Executing command in background: '{}'", cmd_to_run);
        if let Err(e) = execute_chat_command(&cmd_to_run, &node_clone, ui_sender_clone.clone()).await {
            // Report any errors from the command execution back to the UI.
            let _ = ui_sender_clone.send(UIEvent::ChatMessage(format!("❌ Error: {}", e)));
        }
    });

    Ok(())
}

// This replicates the command handling from the old CLI
async fn execute_chat_command(
    cmd_line: &str, 
    node: &Arc<Node>, 
    ui_sender: mpsc::UnboundedSender<UIEvent>
) -> Result<()> {
    let parts: Vec<&str> = cmd_line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    match parts[0] {
        "send" => {
            if parts.len() < 3 {
                let _ = ui_sender.send(UIEvent::ChatMessage("Usage: send <peer_id_or_nickname> <message...>".to_string()));
                return Ok(());
            }
            let destination = parts[1];
            let message = parts[2..].join(" ");

            let recipient_peer_id = match resolve_peer_id(destination, node).await {
                Ok(id) => id,
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ {}", e)));
                    return Ok(());
                }
            };

            let friend = match node.friends.get_friend(&recipient_peer_id).await? {
                Some(f) => f,
                None => {
                    let _ = ui_sender.send(UIEvent::ChatMessage("❌ Friend not found. Add them first with 'friend' command.".to_string()));
                    return Ok(());
                }
            };

            let encrypted_content = match node.identity.encrypt_for(&friend.e2e_public_key, message.as_bytes()) {
                Ok(content) => content,
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ Encryption failed: {}", e)));
                    return Ok(());
                }
            };

            let msg = Message {
                id: uuid::Uuid::new_v4(),
                sender: node.identity.peer_id,
                recipient: recipient_peer_id,
                timestamp: chrono::Utc::now().timestamp(),
                content: encrypted_content,
                nonce: rand::random(),
            };

            node.history.store_message(msg.clone()).await?;
            node.outbox.add_pending(msg.clone()).await?;

            match node.network.send_message(recipient_peer_id, msg.clone()).await {
                Ok(()) => {
                    node.outbox.remove_pending(&msg.id).await?;
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("✅ Message sent directly to {}", destination)));
                },
                Err(_) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("⚠️ {} is offline. Attempting mailbox delivery...", destination)));
                    
                    // Try mailbox forwarding - first trigger discovery if needed
                    let providers = {
                        let mut sync_engine = node.sync_engine.lock().await;
                        let current_providers = sync_engine.get_mailbox_providers().clone();
                        
                        // If we have no mailboxes, trigger immediate discovery
                        if current_providers.is_empty() {
                            debug!("No known mailboxes, triggering discovery");
                            if let Err(e) = sync_engine.discover_mailboxes().await {
                                debug!("Mailbox discovery failed: {}", e);
                            }
                            // Get potentially updated providers
                            sync_engine.get_mailbox_providers().clone()
                        } else {
                            current_providers
                        }
                    };
                    
                    if !providers.is_empty() {
                        match node.forward_to_mailboxes(&msg, &friend, &providers).await {
                            Ok(MailboxDeliveryResult::Success(count)) => {
                                node.outbox.remove_pending(&msg.id).await?;
                                let _ = ui_sender.send(UIEvent::ChatMessage(format!("📬 Message stored in {} network mailbox(es) for {}", count, destination)));
                            }
                            Ok(MailboxDeliveryResult::Failure) => {
                                let _ = ui_sender.send(UIEvent::ChatMessage(format!("⚠️ Mailbox delivery failed. Message queued for retry when {} comes online", destination)));
                            }
                            Err(e) => {
                                let _ = ui_sender.send(UIEvent::ChatMessage(format!("⚠️ Mailbox error: {}. Message queued for retry", e)));
                            }
                        }
                    } else {
                        // Try emergency mailboxes using connected peers
                        let emergency_providers = {
                            let sync_engine = node.sync_engine.lock().await;
                            sync_engine.get_emergency_mailboxes().await
                        };
                        
                        if !emergency_providers.is_empty() {
                            let emergency_set: std::collections::HashSet<libp2p::PeerId> = emergency_providers.into_iter().collect();
                            match node.forward_to_mailboxes(&msg, &friend, &emergency_set).await {
                                Ok(MailboxDeliveryResult::Success(count)) => {
                                    node.outbox.remove_pending(&msg.id).await?;
                                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("📦 Message stored using emergency relay in {} mailbox(es) for {}", count, destination)));
                                }
                                Ok(MailboxDeliveryResult::Failure) => {
                                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("⚠️ Emergency relay failed. Message queued for when {} comes online", destination)));
                                }
                                Err(e) => {
                                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("⚠️ Emergency relay error: {}. Message queued for retry", e)));
                                }
                            }
                        } else {
                            let _ = ui_sender.send(UIEvent::ChatMessage(format!("⚠️ No mailboxes or connected peers available. Message queued for when {} comes online", destination)));
                        }
                    }
                }
            }
        }
        "friend" => {
            if !(3..=4).contains(&parts.len()) {
                let _ = ui_sender.send(UIEvent::ChatMessage("Usage: friend <peer_id> <e2e_key> [nickname]".to_string()));
                return Ok(());
            }
            
            let peer_id = match libp2p::PeerId::from_str(parts[1]) {
                Ok(id) => id,
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ Invalid peer ID: {}", e)));
                    return Ok(());
                }
            };
            
            let e2e_public_key = match base64::prelude::BASE64_STANDARD.decode(parts[2]) {
                Ok(key) => key,
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ Invalid base64 key: {}", e)));
                    return Ok(());
                }
            };
            
            let nickname = parts.get(3).map(|s| s.to_string());
            
            let friend = crate::types::Friend { 
                peer_id, 
                e2e_public_key, 
                nickname: nickname.clone()
            };
            
            match node.friends.add_friend(friend).await {
                Ok(()) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("✅ Added friend: {} ({})", peer_id, nickname.unwrap_or_else(|| "no nickname".to_string()))));
                }
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ Failed to add friend: {}", e)));
                }
            }
        }
        "friends" => {
            match node.friends.list_friends().await {
                Ok(friends) => {
                    if friends.is_empty() {
                        let _ = ui_sender.send(UIEvent::ChatMessage("No friends added yet.".to_string()));
                    } else {
                        let mut output = format!("Friends ({}):", friends.len());
                        for friend in friends {
                            let nickname = friend.nickname
                                .as_deref()
                                .unwrap_or("(no nickname)");
                            output.push_str(&format!("\n  {} - {}", friend.peer_id, nickname));
                        }
                        let _ = ui_sender.send(UIEvent::ChatMessage(output));
                    }
                }
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ Failed to list friends: {}", e)));
                }
            }
        }
        "history" => {
            if parts.len() < 2 || parts.len() > 3 {
                let _ = ui_sender.send(UIEvent::ChatMessage("Usage: history <peer_id_or_nickname> [message_count]".to_string()));
                return Ok(());
            }
            
            let peer_id = match resolve_peer_id(parts[1], node).await {
                Ok(id) => id,
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ {}", e)));
                    return Ok(());
                }
            };
            
            // Parse optional message count parameter, default to 20
            let limit = if parts.len() == 3 {
                match parts[2].parse::<usize>() {
                    Ok(count) => {
                        if count == 0 {
                            let _ = ui_sender.send(UIEvent::ChatMessage("❌ Message count must be greater than 0".to_string()));
                            return Ok(());
                        } else if count > 1000 {
                            let _ = ui_sender.send(UIEvent::ChatMessage("❌ Message count cannot exceed 1000".to_string()));
                            return Ok(());
                        } else {
                            count
                        }
                    }
                    Err(_) => {
                        let _ = ui_sender.send(UIEvent::ChatMessage("❌ Invalid message count. Must be a number between 1 and 1000".to_string()));
                        return Ok(());
                    }
                }
            } else {
                20  // default
            };
            match node.history.get_history(&node.identity.peer_id, &peer_id, limit).await {
                Ok(messages) => {
                    if messages.is_empty() {
                        let _ = ui_sender.send(UIEvent::ChatMessage(format!("No message history with {}", peer_id)));
                    } else {
                        let mut output = format!("Message history with {} (last {} messages):", peer_id, messages.len());
                        for msg in messages {
                            let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp(msg.timestamp, 0)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                .unwrap_or_else(|| "Invalid timestamp".to_string());
                            
                            let direction = if msg.sender == node.identity.peer_id { "→" } else { "←" };
                            
                            // Try to decrypt message content
                            let other_partys_pub_key = if msg.sender == node.identity.peer_id {
                                node.friends.get_friend(&msg.recipient).await.ok().flatten().map(|f| f.e2e_public_key)
                            } else {
                                node.friends.get_friend(&msg.sender).await.ok().flatten().map(|f| f.e2e_public_key)
                            };
                            
                            let content = if let Some(pub_key) = other_partys_pub_key {
                                match node.identity.decrypt_from(&pub_key, &msg.content) {
                                    Ok(plaintext) => String::from_utf8_lossy(&plaintext).to_string(),
                                    Err(_) => "[Decryption Failed]".to_string(),
                                }
                            } else {
                                "[Cannot decrypt - unknown peer]".to_string()
                            };
                            
                            output.push_str(&format!("\n  {} [{}] {}", direction, timestamp, content));
                        }
                        let _ = ui_sender.send(UIEvent::ChatMessage(output));
                    }
                }
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ Failed to get message history: {}", e)));
                }
            }
        }
        "peers" => {
            match node.network.get_connected_peers().await {
                Ok(peers) => {
                    let mailboxes = {
                        let sync_engine = node.sync_engine.lock().await;
                        sync_engine.get_mailbox_providers().clone()
                    };

                    let _ = ui_sender.send(UIEvent::UpdatePeersCount(peers.len()));
                    let peer_strings: Vec<String> = peers.iter().map(|p| p.to_string()).collect();
                    let _ = ui_sender.send(UIEvent::UpdateDiscoveredPeers(peer_strings));
                    
                    if peers.is_empty() {
                        let _ = ui_sender.send(UIEvent::ChatMessage("No peers currently connected.".to_string()));
                    } else {
                        let mut output = format!("Connected peers ({}):", peers.len());
                        for peer in &peers {
                            if let Ok(Some(friend)) = node.friends.get_friend(peer).await {
                                let nickname = friend.nickname
                                    .as_deref()
                                    .unwrap_or("(no nickname)");
                                output.push_str(&format!("\n  {} - {} (👥 Friend)", peer, nickname));
                            } else if mailboxes.contains(peer) {
                                output.push_str(&format!("\n  {} - 📬 Mailbox", peer));
                            } else {
                                output.push_str(&format!("\n  {} - 🌐 Peer", peer));
                            }
                        }
                        let _ = ui_sender.send(UIEvent::ChatMessage(output));
                    }
                }
                Err(e) => {
                    let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ Failed to get peer list: {}", e)));
                }
            }
        }
        "info" => {
            let output = format!("Your Identity:\n  Peer ID: {}\n  E2E Public Key: {}", 
                node.identity.peer_id,
                base64::prelude::BASE64_STANDARD.encode(&node.identity.hpke_public_key()));
            let _ = ui_sender.send(UIEvent::ChatMessage(output));
        }
        "check" => {
            let _ = ui_sender.send(UIEvent::ChatMessage("✅ Mailbox discovery runs automatically every few seconds.".to_string()));
        }
        "help" => {
            let help_text = "Available commands:\n  friend <peer_id> <e2e_key> [nickname] - Add a friend and optionally assign a nickname\n  friends                     - List all friends\n  send <peer_id_or_nickname> <message>    - Send a message\n  history <peer_id_or_nickname> [count] - Show message history (default: 20, max: 1000)\n  peers                       - Show connected peers\n  info                        - Show your identity\n  check                       - Check for new messages in mailboxes\n  help                        - Show this help\n  exit                        - Exit the application";
            let _ = ui_sender.send(UIEvent::ChatMessage(help_text.to_string()));
        }
        "exit" => {
            // This case is handled synchronously in `handle_ui_action`
        }
        _ => {
            let _ = ui_sender.send(UIEvent::ChatMessage(format!("❌ Unknown command: {}. Type 'help' for available commands.", parts[0])));
        }
    }

    Ok(())
}

async fn resolve_peer_id(destination: &str, node: &Arc<Node>) -> Result<libp2p::PeerId> {
    use std::str::FromStr;
    
    if let Ok(peer_id) = libp2p::PeerId::from_str(destination) {
        Ok(peer_id)
    } else {
        let friends = node.friends.list_friends().await?;
        friends
            .into_iter()
            .find(|f| f.nickname.as_deref() == Some(destination))
            .map(|f| f.peer_id)
            .ok_or_else(|| anyhow::anyhow!("Peer not found by ID or nickname: '{}'", destination))
    }
}