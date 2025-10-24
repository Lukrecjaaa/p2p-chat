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

#[derive(RustEmbed)]
#[folder = "web-ui/dist"]
struct Assets;

pub async fn start_server(node: Arc<Node>, port: u16, mut ui_notify_rx: mpsc::UnboundedReceiver<UiNotification>) -> Result<()> {
    let (broadcast_tx, _) = broadcast::channel::<WebSocketMessage>(100);

    let ws_state = Arc::new(WebSocketState {
        node: node.clone(),
        broadcast_tx: broadcast_tx.clone(),
    });

    // Spawn task to forward UI notifications to broadcast channel
    tokio::spawn(async move {
        while let Some(notification) = ui_notify_rx.recv().await {
            match notification {
                UiNotification::NewMessage(msg) => {
                    let ws_msg = WebSocketMessage::NewMessage {
                        id: msg.id.to_string(),
                        sender: msg.sender.to_string(),
                        recipient: msg.recipient.to_string(),
                        timestamp: msg.timestamp,
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
        .route("/api/peers/online", get(api::get_online_peers))
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

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if path.is_empty() || path == "index.html" {
        return serve_file("index.html");
    }

    // Try to serve the file, if not found serve index.html for client-side routing
    if Assets::get(path).is_some() {
        serve_file(path)
    } else {
        serve_file("index.html")
    }
}

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
