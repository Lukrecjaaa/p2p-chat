use crate::cli::commands::Node;
use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error};

#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    NewMessage {
        id: String,
        sender: String,
        recipient: String,
        timestamp: i64,
    },
}

pub struct WebSocketState {
    pub node: Arc<Node>,
    pub broadcast_tx: broadcast::Sender<WebSocketMessage>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebSocketState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<WebSocketState>) {
    let (sender, mut receiver) = socket.split();

    // Subscribe to broadcast channel
    let mut broadcast_rx = state.broadcast_tx.subscribe();

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

    // Handle incoming WebSocket messages (ping/pong, close)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(axum::extract::ws::Message::Close(_)) => {
                debug!("WebSocket client disconnected");
                break;
            }
            Ok(axum::extract::ws::Message::Ping(_)) => {
                // Can't respond to ping since sender is moved
                // The client will handle reconnection if needed
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    send_task.abort();
}
