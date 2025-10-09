use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub id: Uuid,
    pub sender: PeerId,
    pub recipient: PeerId,
    pub timestamp: i64,
    pub content: Vec<u8>, // Encrypted
    pub nonce: u64,       // For ordering
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Friend {
    pub peer_id: PeerId,
    pub e2e_public_key: Vec<u8>,
    pub nickname: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedMessage {
    pub id: Uuid,
    pub sender: PeerId,
    pub recipient_hash: [u8; 32],
    pub encrypted_content: Vec<u8>,
    pub timestamp: i64,
    #[serde(default)]
    pub nonce: u64,
    pub sender_pub_key: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ChatRequest {
    SendMessage { message: Message },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ChatResponse {
    MessageResult {
        success: bool,
        message_id: Option<Uuid>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MailboxRequest {
    Put {
        recipient: [u8; 32],
        message: EncryptedMessage,
    },
    Fetch {
        recipient: [u8; 32],
        limit: usize,
    },
    Ack {
        recipient: [u8; 32],
        msg_ids: Vec<Uuid>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MailboxResponse {
    PutResult { success: bool },
    Messages { items: Vec<EncryptedMessage> },
    AckResult { deleted: usize },
}
