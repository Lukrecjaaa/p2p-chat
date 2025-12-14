//! This module contains the runtime logic for the `NetworkLayer`.
use std::time::Duration;

use anyhow::Result;
use futures::StreamExt;
use tokio::select;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::types::Message;

use super::NetworkLayer;

impl NetworkLayer {
    /// Periodically cleans up the list of blocked peers.
    fn cleanup_blocked_peers(&mut self) {
        let block_duration = Duration::from_secs(600);
        let mut expired_peers = Vec::new();

        for (&peer_id, &blocked_time) in &self.blocked_peers {
            if blocked_time.elapsed() > block_duration {
                expired_peers.push(peer_id);
            }
        }

        for peer_id in expired_peers {
            info!("Unblocking peer {} after timeout", peer_id);
            self.blocked_peers.remove(&peer_id);
        }
    }

    /// Runs the main event loop for the `NetworkLayer`.
    ///
    /// This function listens for events from the `libp2p` `Swarm` and for
    /// commands from other parts of the application. It also periodically
    /// cleans up the list of blocked peers.
    ///
    /// # Arguments
    ///
    /// * `incoming_messages` - The sender for incoming chat messages.
    ///
    /// # Errors
    ///
    /// This function will return an error if the event loop fails.
    pub async fn run(&mut self, incoming_messages: mpsc::UnboundedSender<Message>) -> Result<()> {
        info!("Starting network event loop");

        let mut cleanup_timer = tokio::time::interval(Duration::from_secs(300));

        loop {
            select! {
                event = self.swarm.select_next_some() => {
                    if let Err(e) = self.handle_swarm_event(event, &incoming_messages).await {
                        error!("Error handling swarm event: {}", e);
                    }
                }

                command = self.command_receiver.recv() => {
                    match command {
                        Some(cmd) => {
                            if let Err(e) = self.handle_command(cmd).await {
                                error!("Error handling command: {}", e);
                            }
                        }
                        None => {
                            info!("Command channel closed, shutting down network layer");
                            break;
                        }
                    }
                }

                _ = cleanup_timer.tick() => {
                    self.cleanup_blocked_peers();
                }
            }
        }

        Ok(())
    }
}
