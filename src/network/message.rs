//! This module defines the messages that are sent to and from the `NetworkLayer`.
use crate::types::{ChatRequest, EncryptedMessage, Message};
use anyhow::Result;
use libp2p::{kad, PeerId};
use tokio::sync::oneshot;

/// A response from the `NetworkLayer`.
#[derive(Debug)]
pub enum NetworkResponse {
    /// A message was successfully sent.
    MessageSent,
    /// A list of connected peers.
    ConnectedPeers {
        /// The list of connected peers.
        peers: Vec<PeerId>,
    },
    /// An error occurred.
    Error(String),
    /// The result of a mailbox `put` operation.
    MailboxPutResult {
        /// Whether the operation was successful.
        success: bool,
    },
    /// A list of messages fetched from a mailbox.
    MailboxMessages {
        /// The list of fetched messages.
        messages: Vec<EncryptedMessage>,
    },
    /// The result of a mailbox `ack` operation.
    MailboxAckResult {
        /// The number of messages that were deleted.
        deleted: usize,
    },
}

/// A command to be sent to the `NetworkLayer`.
#[derive(Debug)]
pub enum NetworkCommand {
    /// Send a chat message to a peer.
    SendMessage {
        /// The `PeerId` of the recipient.
        peer_id: PeerId,
        /// The message to send.
        message: Message,
        /// The channel to send the response on.
        response: oneshot::Sender<NetworkResponse>,
    },
    /// Send a chat request to a peer.
    SendChatRequest {
        /// The `PeerId` of the recipient.
        peer_id: PeerId,
        /// The request to send.
        request: ChatRequest,
        /// The channel to send the response on.
        response: oneshot::Sender<NetworkResponse>,
    },
    /// Get the list of connected peers.
    GetConnectedPeers {
        /// The channel to send the response on.
        response: oneshot::Sender<NetworkResponse>,
    },
    /// Put a message into a mailbox.
    MailboxPut {
        /// The `PeerId` of the mailbox node.
        peer_id: PeerId,
        /// The hash of the recipient's public key.
        recipient: [u8; 32],
        /// The encrypted message to store.
        message: EncryptedMessage,
        /// The channel to send the response on.
        response: oneshot::Sender<NetworkResponse>,
    },
    /// Fetch messages from a mailbox.
    MailboxFetch {
        /// The `PeerId` of the mailbox node.
        peer_id: PeerId,
        /// The hash of the recipient's public key.
        recipient: [u8; 32],
        /// The maximum number of messages to fetch.
        limit: usize,
        /// The channel to send the response on.
        response: oneshot::Sender<NetworkResponse>,
    },
    /// Acknowledge the receipt of messages from a mailbox.
    MailboxAck {
        /// The `PeerId` of the mailbox node.
        peer_id: PeerId,
        /// The hash of the recipient's public key.
        recipient: [u8; 32],
        /// The IDs of the messages to acknowledge.
        msg_ids: Vec<uuid::Uuid>,
        /// The channel to send the response on.
        response: oneshot::Sender<NetworkResponse>,
    },
    /// Start a Kademlia DHT query to find providers for a key.
    StartDhtProviderQuery {
        /// The key to find providers for.
        key: kad::RecordKey,
        /// The channel to send the response on.
        response: oneshot::Sender<Result<kad::QueryId>>,
    },
}
