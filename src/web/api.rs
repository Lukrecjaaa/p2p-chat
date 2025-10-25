use crate::cli::commands::Node;
use crate::types::{DeliveryStatus, Friend, Message};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::prelude::*;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
pub struct IdentityResponse {
    peer_id: String,
    hpke_public_key: String,
}

#[derive(Serialize)]
pub struct FriendResponse {
    peer_id: String,
    e2e_public_key: String,
    nickname: Option<String>,
    online: bool,
}

#[derive(Deserialize)]
pub struct AddFriendRequest {
    peer_id: String,
    e2e_public_key: String,
    nickname: Option<String>,
}

#[derive(Serialize)]
pub struct MessageResponse {
    id: String,
    sender: String,
    recipient: String,
    content: String,
    timestamp: i64,
    nonce: u64,
    delivery_status: String,
}

#[derive(Deserialize)]
pub struct SendMessageRequest {
    content: String,
}

#[derive(Deserialize)]
pub struct GetMessagesQuery {
    #[serde(default)]
    mode: MessageQueryMode,
    #[serde(default = "default_limit")]
    limit: usize,
    before_id: Option<String>,
    after_id: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum MessageQueryMode {
    #[default]
    Latest,
    Before,
    After,
}

fn default_limit() -> usize {
    50
}

#[derive(Serialize)]
pub struct ConversationResponse {
    peer_id: String,
    nickname: Option<String>,
    last_message: Option<MessageResponse>,
    online: bool,
}

pub async fn get_me(State(node): State<Arc<Node>>) -> impl IntoResponse {
    let response = IdentityResponse {
        peer_id: node.identity.peer_id.to_string(),
        hpke_public_key: BASE64_STANDARD.encode(node.identity.hpke_public_key()),
    };
    Json(response)
}

pub async fn list_friends(State(node): State<Arc<Node>>) -> impl IntoResponse {
    match node.friends.list_friends().await {
        Ok(friends) => {
            let online_peers = node
                .network
                .get_connected_peers()
                .await
                .unwrap_or_default();

            let response: Vec<FriendResponse> = friends
                .into_iter()
                .map(|f| FriendResponse {
                    online: online_peers.contains(&f.peer_id),
                    peer_id: f.peer_id.to_string(),
                    e2e_public_key: BASE64_STANDARD.encode(&f.e2e_public_key),
                    nickname: f.nickname,
                })
                .collect();

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list friends: {}", e),
        )
            .into_response(),
    }
}

pub async fn add_friend(
    State(node): State<Arc<Node>>,
    Json(req): Json<AddFriendRequest>,
) -> impl IntoResponse {
    let peer_id = match PeerId::from_str(&req.peer_id) {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid peer ID: {}", e),
            )
                .into_response()
        }
    };

    let e2e_public_key = match BASE64_STANDARD.decode(&req.e2e_public_key) {
        Ok(key) => key,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid public key: {}", e),
            )
                .into_response()
        }
    };

    let friend = Friend {
        peer_id,
        e2e_public_key,
        nickname: req.nickname,
    };

    match node.friends.add_friend(friend).await {
        Ok(_) => (StatusCode::CREATED, "Friend added").into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to add friend: {}", e),
        )
            .into_response(),
    }
}

pub async fn list_conversations(State(node): State<Arc<Node>>) -> impl IntoResponse {
    let friends = match node.friends.list_friends().await {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list friends: {}", e),
            )
                .into_response()
        }
    };

    let online_peers = node
        .network
        .get_connected_peers()
        .await
        .unwrap_or_default();

    let mut conversations = Vec::new();

    for friend in friends {
        let messages = node
            .history
            .get_history(&node.identity.peer_id, &friend.peer_id, 1)
            .await
            .unwrap_or_default();

        let mut last_message = None;
        if let Some(msg) = messages.last() {
            if let Some(content) = decrypt_message_content(msg, &node).await {
                last_message = Some(MessageResponse {
                    id: msg.id.to_string(),
                    sender: msg.sender.to_string(),
                    recipient: msg.recipient.to_string(),
                    content,
                    timestamp: msg.timestamp,
                    nonce: msg.nonce,
                    delivery_status: format!("{:?}", msg.delivery_status),
                });
            }
        }

        conversations.push(ConversationResponse {
            peer_id: friend.peer_id.to_string(),
            nickname: friend.nickname,
            last_message,
            online: online_peers.contains(&friend.peer_id),
        });
    }

    // Sort by last message timestamp
    conversations.sort_by(|a, b| {
        let a_ts = a.last_message.as_ref().map(|m| m.timestamp).unwrap_or(0);
        let b_ts = b.last_message.as_ref().map(|m| m.timestamp).unwrap_or(0);
        b_ts.cmp(&a_ts)
    });

    Json(conversations).into_response()
}

pub async fn get_messages(
    State(node): State<Arc<Node>>,
    Path(peer_id_str): Path<String>,
    Query(query): Query<GetMessagesQuery>,
) -> impl IntoResponse {
    let peer_id = match PeerId::from_str(&peer_id_str) {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid peer ID: {}", e),
            )
                .into_response()
        }
    };

    let messages_result = match query.mode {
        MessageQueryMode::Latest => {
            node.history
                .get_history(&node.identity.peer_id, &peer_id, query.limit)
                .await
        }
        MessageQueryMode::Before => {
            let before_id = match &query.before_id {
                Some(id_str) => match Uuid::from_str(id_str) {
                    Ok(id) => id,
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            format!("Invalid before_id: {}", e),
                        )
                            .into_response()
                    }
                },
                None => {
                    return (StatusCode::BAD_REQUEST, "before_id is required for mode=before")
                        .into_response()
                }
            };
            node.history
                .get_messages_before(&node.identity.peer_id, &peer_id, &before_id, query.limit)
                .await
        }
        MessageQueryMode::After => {
            let after_id = match &query.after_id {
                Some(id_str) => match Uuid::from_str(id_str) {
                    Ok(id) => id,
                    Err(e) => {
                        return (StatusCode::BAD_REQUEST, format!("Invalid after_id: {}", e))
                            .into_response()
                    }
                },
                None => {
                    return (StatusCode::BAD_REQUEST, "after_id is required for mode=after")
                        .into_response()
                }
            };
            node.history
                .get_messages_after(&node.identity.peer_id, &peer_id, &after_id, query.limit)
                .await
        }
    };

    match messages_result {
        Ok(messages) => {
            let mut response = Vec::new();
            for msg in messages.iter() {
                if let Some(content) = decrypt_message_content(msg, &node).await {
                    response.push(MessageResponse {
                        id: msg.id.to_string(),
                        sender: msg.sender.to_string(),
                        recipient: msg.recipient.to_string(),
                        content,
                        timestamp: msg.timestamp,
                        nonce: msg.nonce,
                        delivery_status: format!("{:?}", msg.delivery_status),
                    });
                }
            }

            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get messages: {}", e),
        )
            .into_response(),
    }
}

pub async fn send_message(
    State(node): State<Arc<Node>>,
    Path(peer_id_str): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> impl IntoResponse {
    let peer_id = match PeerId::from_str(&peer_id_str) {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid peer ID: {}", e),
            )
                .into_response()
        }
    };

    let friend = match node.friends.get_friend(&peer_id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Friend not found").into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get friend: {}", e),
            )
                .into_response()
        }
    };

    let encrypted_content = match node
        .identity
        .encrypt_for(&friend.e2e_public_key, req.content.as_bytes())
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to encrypt message: {}", e),
            )
                .into_response()
        }
    };

    let message = Message {
        id: Uuid::new_v4(),
        sender: node.identity.peer_id,
        recipient: peer_id,
        timestamp: chrono::Utc::now().timestamp_millis(),
        content: encrypted_content,
        nonce: rand::random(),
        delivery_status: DeliveryStatus::Sent,
    };

    if let Err(e) = node.history.store_message(message.clone()).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to store message: {}", e),
        )
            .into_response();
    }

    if let Err(e) = node.outbox.add_pending(message.clone()).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to add message to outbox: {}", e),
        )
            .into_response();
    }

    // Try direct send in background (delivery confirmation will update status)
    let network_clone = node.network.clone();
    let msg_clone = message.clone();
    tokio::spawn(async move {
        if let Err(e) = network_clone.send_message(peer_id, msg_clone).await {
            tracing::debug!("Direct send failed, will retry via sync: {}", e);
        }
    });

    (StatusCode::OK, Json(serde_json::json!({ "id": message.id }))).into_response()
}

pub async fn mark_message_read(
    State(node): State<Arc<Node>>,
    Path(msg_id_str): Path<String>,
) -> impl IntoResponse {
    let msg_id = match Uuid::from_str(&msg_id_str) {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid message ID: {}", e),
            )
                .into_response()
        }
    };

    // Get the message to find the sender
    let message = match node.history.get_message_by_id(&msg_id).await {
        Ok(Some(msg)) => msg,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Message not found").into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", e),
            )
                .into_response()
        }
    };

    // Only mark as read if we're the recipient
    if message.recipient != node.identity.peer_id {
        return (
            StatusCode::BAD_REQUEST,
            "Can only mark received messages as read",
        )
            .into_response();
    }

    // Update local status to Read
    if let Err(e) = node
        .history
        .update_delivery_status(&msg_id, DeliveryStatus::Read)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update status: {}", e),
        )
            .into_response();
    }

    // Send read receipt to sender
    let receipt = crate::types::ReadReceipt {
        message_id: msg_id,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let read_request = crate::types::ChatRequest::ReadReceipt { receipt };

    // Best effort - don't wait for result
    let network_clone = node.network.clone();
    let sender = message.sender;
    tokio::spawn(async move {
        if let Err(e) = network_clone.send_chat_request(sender, read_request).await {
            tracing::debug!("Failed to send read receipt: {}", e);
        }
    });

    StatusCode::OK.into_response()
}

pub async fn get_online_peers(State(node): State<Arc<Node>>) -> impl IntoResponse {
    match node.network.get_connected_peers().await {
        Ok(peers) => {
            let peer_ids: Vec<String> = peers.iter().map(|p| p.to_string()).collect();
            Json(peer_ids).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get online peers: {}", e),
        )
            .into_response(),
    }
}

#[derive(Serialize)]
pub struct SystemStatus {
    connected_peers: usize,
    known_mailboxes: usize,
    pending_messages: usize,
}

pub async fn get_system_status(State(node): State<Arc<Node>>) -> impl IntoResponse {
    let connected_peers = node
        .network
        .get_connected_peers()
        .await
        .unwrap_or_default()
        .len();

    let known_mailboxes = {
        let sync_engine = node.sync_engine.lock().await;
        sync_engine.get_mailbox_providers().len()
    };

    let pending_messages = node.outbox.count_pending().await.unwrap_or(0);

    Json(SystemStatus {
        connected_peers,
        known_mailboxes,
        pending_messages,
    })
    .into_response()
}

async fn decrypt_message_content(msg: &Message, node: &Node) -> Option<String> {
    // Determine which peer's public key to use for decryption
    let other_peer = if msg.sender == node.identity.peer_id {
        &msg.recipient
    } else {
        &msg.sender
    };

    // Get the friend's public key
    let friend = node.friends.get_friend(other_peer).await.ok()??;

    // Decrypt using the friend's public key
    let plaintext = node
        .identity
        .decrypt_from(&friend.e2e_public_key, &msg.content)
        .ok()?;

    String::from_utf8(plaintext).ok()
}
