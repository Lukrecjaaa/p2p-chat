//! This module contains the web server implementation for the application.
//!
//! It sets up an Axum server to serve both static web UI assets and a REST API,
//! including WebSocket communication.
mod api;
mod websocket;

use crate::cli::commands::{Node, UiNotification};
use anyhow::Result;
use axum::{
    http::{header, StatusCode, Uri},
    response::Response,
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::CorsLayer;
use tracing::info;
use websocket::{WebSocketMessage, WebSocketState};

/// Embeds the web UI static assets into the binary.
#[derive(RustEmbed)]
#[folder = "web-ui/dist"]
struct Assets;

/// Starts the web server.
///
/// This function initializes an Axum server, sets up API and WebSocket routes,
/// serves static web UI assets, and forwards UI notifications to connected
/// WebSocket clients.
///
/// # Arguments
///
/// * `node` - A shared reference to the application's core `Node`.
/// * `port` - The port to bind the web server to.
/// * `ui_notify_rx` - Receiver for UI notifications from the core application.
///
/// # Errors
///
/// Returns an error if the server fails to bind or run.
pub async fn start_server(
    node: Arc<Node>,
    port: u16,
    mut ui_notify_rx: mpsc::UnboundedReceiver<UiNotification>,
) -> Result<()> {
    let (broadcast_tx, _) = broadcast::channel::<WebSocketMessage>(100);

    let ws_state = Arc::new(WebSocketState {
        broadcast_tx: broadcast_tx.clone(),
    });

    // Spawn task to forward UI notifications to broadcast channel.
    let node_clone = node.clone();
    tokio::spawn(async move {
        while let Some(notification) = ui_notify_rx.recv().await {
            match notification {
                UiNotification::NewMessage(msg) => {
                    // Decrypt message content before broadcasting.
                    let other_peer = if msg.sender == node_clone.identity.peer_id {
                        &msg.recipient
                    } else {
                        &msg.sender
                    };

                    let content = match node_clone.friends.get_friend(other_peer).await {
                        Ok(Some(friend)) => {
                            match node_clone
                                .identity
                                .decrypt_from(&friend.e2e_public_key, &msg.content)
                            {
                                Ok(plaintext) => match String::from_utf8(plaintext) {
                                    Ok(s) => s,
                                    Err(_) => continue, // Skip non-UTF8 messages
                                },
                                Err(_) => continue, // Skip decryption failures
                            }
                        }
                        _ => continue, // Skip if friend not found
                    };

                    let ws_msg = WebSocketMessage::NewMessage {
                        id: msg.id.to_string(),
                        sender: msg.sender.to_string(),
                        recipient: msg.recipient.to_string(),
                        content,
                        timestamp: msg.timestamp,
                        nonce: msg.nonce,
                        delivery_status: format!("{:?}", msg.delivery_status),
                    };
                    let _ = broadcast_tx.send(ws_msg);
                }
                UiNotification::PeerConnected(peer_id) => {
                    let ws_msg = WebSocketMessage::PeerConnected {
                        peer_id: peer_id.to_string(),
                    };
                    let _ = broadcast_tx.send(ws_msg);
                }
                UiNotification::PeerDisconnected(peer_id) => {
                    let ws_msg = WebSocketMessage::PeerDisconnected {
                        peer_id: peer_id.to_string(),
                    };
                    let _ = broadcast_tx.send(ws_msg);
                }
                UiNotification::DeliveryStatusUpdate { message_id, new_status } => {
                    let ws_msg = WebSocketMessage::DeliveryStatusUpdate {
                        message_id: message_id.to_string(),
                        new_status: format!("{:?}", new_status),
                    };
                    let _ = broadcast_tx.send(ws_msg);
                }
            }
        }
    });

    let api_router = Router::new()
        .route("/api/me", get(api::get_me))
        .route("/api/friends", get(api::list_friends).post(api::add_friend))
        .route("/api/conversations", get(api::list_conversations))
        .route("/api/conversations/:peer_id/messages", get(api::get_messages))
        .route("/api/conversations/:peer_id/messages", axum::routing::post(api::send_message))
        .route("/api/messages/:msg_id/read", axum::routing::post(api::mark_message_read))
        .route("/api/peers/online", get(api::get_online_peers))
        .route("/api/system/status", get(api::get_system_status))
        .with_state(node);

    let ws_router = Router::new()
        .route("/ws", get(websocket::ws_handler))
        .with_state(ws_state);

    let app = Router::new()
        .merge(api_router)
        .merge(ws_router)
        .fallback(static_handler)
        .layer(CorsLayer::permissive());

    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Web server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Handles requests for static web UI assets.
///
/// This function attempts to serve the requested file from the embedded assets.
/// If the path is empty or "index.html", it serves "index.html".
/// If a file is not found, it falls back to serving "index.html" for client-side routing.
///
/// # Arguments
///
/// * `uri` - The `Uri` of the incoming request.
///
/// # Returns
///
/// An Axum `Response` containing the static file or a 404 error.
async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if path.is_empty() || path == "index.html" {
        return serve_file("index.html");
    }

    // Try to serve the file, if not found serve index.html for client-side routing.
    if Assets::get(path).is_some() {
        serve_file(path)
    } else {
        serve_file("index.html")
    }
}

/// Serves a static file from the embedded assets.
///
/// # Arguments
///
/// * `path` - The path to the file within the embedded assets.
///
/// # Returns
///
/// An Axum `Response` containing the file content or a 404 error.
fn serve_file(path: &str) -> Response {
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let body = content.data.into_owned();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(body.into())
                .unwrap()
        }
        None => {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain")
                .body("404 Not Found".into())
                .unwrap()
        }
    }
}
