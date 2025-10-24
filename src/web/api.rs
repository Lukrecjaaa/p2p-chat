use crate::cli::commands::Node;
use crate::types::{Friend, Message};
use axum::{
    extract::{Path, State},
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
}

#[derive(Deserialize)]
pub struct SendMessageRequest {
    content: String,
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

    match node
        .history
        .get_history(&node.identity.peer_id, &peer_id, 1000)
        .await
    {
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
        timestamp: chrono::Utc::now().timestamp(),
        content: encrypted_content,
        nonce: rand::random(),
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

    // Try direct send
    if let Err(e) = node.network.send_message(peer_id, message.clone()).await {
        tracing::debug!("Direct send failed, will retry via sync: {}", e);
    }

    (StatusCode::OK, Json(serde_json::json!({ "id": message.id }))).into_response()
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
