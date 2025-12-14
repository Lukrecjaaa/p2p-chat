//! This module handles WebSocket connections for the web UI.
use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error};

/// Represents messages that can be sent over the WebSocket to the web UI.
#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    /// A new chat message has been received or sent.
    NewMessage {
        id: String,
        sender: String,
        recipient: String,
        content: String,
        timestamp: i64,
        nonce: u64,
        delivery_status: String,
    },
    /// A peer has connected to the network.
    PeerConnected {
        peer_id: String,
    },
    /// A peer has disconnected from the network.
    PeerDisconnected {
        peer_id: String,
    },
    /// The delivery status of a message has been updated.
    DeliveryStatusUpdate {
        message_id: String,
        new_status: String,
    },
}

/// The state shared across WebSocket connections.
pub struct WebSocketState {
    /// A broadcast sender for distributing messages to all connected WebSocket clients.
    pub broadcast_tx: broadcast::Sender<WebSocketMessage>,
}

/// Handles the WebSocket upgrade request.
///
/// This function is an Axum handler that takes a `WebSocketUpgrade` and
/// a `WebSocketState`, then upgrades the connection to a WebSocket and
/// spawns a task to handle the socket.
///
/// # Arguments
///
/// * `ws` - The `WebSocketUpgrade` extractor.
/// * `State(state)` - The shared `WebSocketState`.
///
/// # Returns
///
/// An Axum `Response`.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebSocketState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handles a single WebSocket connection.
///
/// This asynchronous function manages sending messages from a broadcast channel
/// to the client and handles incoming messages from the client (e.g., pings, close).
///
/// # Arguments
///
/// * `socket` - The established `WebSocket` connection.
/// * `state` - The shared `WebSocketState`.
async fn handle_socket(socket: WebSocket, state: Arc<WebSocketState>) {
    let (sender, mut receiver) = socket.split();

    // Subscribe to broadcast channel.
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    // Task for sending messages from broadcast channel to WebSocket client.
    let send_task = tokio::spawn(async move {
        let mut sender = sender;
        loop {
            match broadcast_rx.recv().await {
                Ok(msg) => {
                    let json = match serde_json::to_string(&msg) {
                        Ok(j) => j,
                        Err(e) => {
                            error!("Failed to serialize WebSocket message: {}", e);
                            continue;
                        }
                    };

                    if sender
                        .send(axum::extract::ws::Message::Text(json))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    error!("WebSocket lagged by {} messages", n);
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Handle incoming WebSocket messages (ping/pong, close).
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(axum::extract::ws::Message::Close(_)) => {
                debug!("WebSocket client disconnected");
                break;
            }
            Ok(axum::extract::ws::Message::Ping(_)) => {
                // Currently, pings are acknowledged implicitly by tokio-tungstenite.
                // Explicit Pong response is not needed here.
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Abort the send task when the receive loop ends (client disconnected or error).
    send_task.abort();
}
