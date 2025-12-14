//! This module defines common data structures and types used throughout the p2p-chat application.
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents the delivery status of a message.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum DeliveryStatus {
    /// The message is currently being sent.
    Sending,
    /// The message has been sent (e.g., to the network or outbox).
    Sent,
    /// The message has been delivered to the recipient or a mailbox.
    Delivered,
    /// The message has been read by the recipient (future feature).
    Read,
}

impl Default for DeliveryStatus {
    /// Returns the default delivery status, which is `Sending`.
    fn default() -> Self {
        DeliveryStatus::Sending
    }
}

/// Represents a chat message.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    /// The unique identifier for the message.
    pub id: Uuid,
    /// The Peer ID of the sender.
    pub sender: PeerId,
    /// The Peer ID of the recipient.
    pub recipient: PeerId,
    /// The timestamp when the message was created (milliseconds since epoch).
    pub timestamp: i64,
    /// The encrypted content of the message.
    pub content: Vec<u8>,
    /// A random nonce used for ordering or cryptographic purposes.
    pub nonce: u64,
    /// The current delivery status of the message.
    #[serde(default)]
    pub delivery_status: DeliveryStatus,
}

/// Represents a friend in the application.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Friend {
    /// The Peer ID of the friend.
    pub peer_id: PeerId,
    /// The E2E public key of the friend.
    pub e2e_public_key: Vec<u8>,
    /// An optional nickname for the friend.
    pub nickname: Option<String>,
}

/// Represents an encrypted message stored in a mailbox.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedMessage {
    /// The unique identifier for the message.
    pub id: Uuid,
    /// The Peer ID of the original sender.
    pub sender: PeerId,
    /// The cryptographic hash of the recipient's public key, used for mailbox lookup.
    pub recipient_hash: [u8; 32],
    /// The encrypted content of the message.
    pub encrypted_content: Vec<u8>,
    /// The timestamp when the message was created (milliseconds since epoch).
    pub timestamp: i64,
    /// A random nonce used for ordering or cryptographic purposes.
    #[serde(default)]
    pub nonce: u64,
    /// The sender's E2E public key.
    pub sender_pub_key: Vec<u8>,
}

/// Represents a delivery confirmation for a message.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeliveryConfirmation {
    /// The ID of the original message being confirmed.
    pub original_message_id: Uuid,
    /// The timestamp when the confirmation was generated.
    pub timestamp: i64,
}

/// Represents a read receipt for a message.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadReceipt {
    /// The ID of the message that was read.
    pub message_id: Uuid,
    /// The timestamp when the message was read.
    pub timestamp: i64,
}

/// Represents a request in the chat protocol.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ChatRequest {
    /// Request to send a chat message.
    SendMessage {
        /// The message to send.
        message: Message,
    },
    /// Request to send a delivery confirmation.
    DeliveryConfirmation {
        /// The delivery confirmation details.
        confirmation: DeliveryConfirmation,
    },
    /// Request to send a read receipt.
    ReadReceipt {
        /// The read receipt details.
        receipt: ReadReceipt,
    },
}

/// Represents a response in the chat protocol.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ChatResponse {
    /// Response to a message sending request.
    MessageResult {
        /// Whether the message operation was successful.
        success: bool,
        /// The ID of the message, if successful.
        message_id: Option<Uuid>,
    },
}

/// Represents a request to a mailbox node.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MailboxRequest {
    /// Request to put an encrypted message into the mailbox.
    Put {
        /// The cryptographic hash of the recipient's public key.
        recipient: [u8; 32],
        /// The encrypted message to store.
        message: EncryptedMessage,
    },
    /// Request to fetch encrypted messages for a recipient.
    Fetch {
        /// The cryptographic hash of the recipient's public key.
        recipient: [u8; 32],
        /// The maximum number of messages to fetch.
        limit: usize,
    },
    /// Request to acknowledge and delete messages from the mailbox.
    Ack {
        /// The cryptographic hash of the recipient's public key.
        recipient: [u8; 32],
        /// The IDs of the messages to acknowledge and delete.
        msg_ids: Vec<Uuid>,
    },
}

/// Represents a response from a mailbox node.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MailboxResponse {
    /// Response to a `Put` request.
    PutResult {
        /// Whether the put operation was successful.
        success: bool,
    },
    /// Response containing fetched messages.
    Messages {
        /// A vector of encrypted messages.
        items: Vec<EncryptedMessage>,
    },
    /// Response to an `Ack` request.
    AckResult {
        /// The number of messages successfully deleted.
        deleted: usize,
    },
}
