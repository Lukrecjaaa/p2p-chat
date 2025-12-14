//! This module defines the `NetworkHandle`, which is the main API for
//! interacting with the `NetworkLayer` from other parts of the application.
use anyhow::{anyhow, Result};
use libp2p::{kad, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::types::{ChatRequest, EncryptedMessage, Message};

use super::message::{NetworkCommand, NetworkResponse};

/// A handle for interacting with the `NetworkLayer`.
///
/// This struct provides a thread-safe way to send commands to the `NetworkLayer`
/// and receive responses.
#[derive(Clone)]
pub struct NetworkHandle {
    pub(super) command_sender: mpsc::UnboundedSender<NetworkCommand>,
}

impl NetworkHandle {
    /// Sends a chat message to a peer.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the recipient.
    /// * `message` - The message to send.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be sent.
    pub async fn send_message(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::SendMessage {
            peer_id,
            message,
            response: tx,
        })?;

        match rx.await? {
            NetworkResponse::MessageSent => Ok(()),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Sends a chat request to a peer.
    ///
    /// This can be used for things like sending delivery confirmations or read receipts.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the recipient.
    /// * `request` - The chat request to send.
    ///
    /// # Errors
    ///
    /// This function will return an error if the request cannot be sent.
    pub async fn send_chat_request(&self, peer_id: PeerId, request: ChatRequest) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::SendChatRequest {
            peer_id,
            request,
            response: tx,
        })?;

        match rx.await? {
            NetworkResponse::MessageSent => Ok(()),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Gets the list of connected peers.
    ///
    /// # Errors
    ///
    /// This function will return an error if the list of peers cannot be retrieved.
    pub async fn get_connected_peers(&self) -> Result<Vec<PeerId>> {
        let (tx, rx) = oneshot::channel();
        self.command_sender
            .send(NetworkCommand::GetConnectedPeers { response: tx })?;

        match rx.await? {
            NetworkResponse::ConnectedPeers { peers } => Ok(peers),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Puts a message into a mailbox.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the mailbox node.
    /// * `recipient` - The hash of the recipient's public key.
    /// * `message` - The encrypted message to store.
    ///
    /// # Errors
    ///
    /// This function will return an error if the message cannot be stored.
    pub async fn mailbox_put(
        &self,
        peer_id: PeerId,
        recipient: [u8; 32],
        message: EncryptedMessage,
    ) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxPut {
            peer_id,
            recipient,
            message,
            response: tx,
        })?;
        match rx.await? {
            NetworkResponse::MailboxPutResult { success } => Ok(success),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Fetches messages from a mailbox.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the mailbox node.
    /// * `recipient` - The hash of the recipient's public key.
    /// * `limit` - The maximum number of messages to fetch.
    ///
    /// # Errors
    ///
    /// This function will return an error if the messages cannot be fetched.
    pub async fn mailbox_fetch(
        &self,
        peer_id: PeerId,
        recipient: [u8; 32],
        limit: usize,
    ) -> Result<Vec<EncryptedMessage>> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxFetch {
            peer_id,
            recipient,
            limit,
            response: tx,
        })?;
        match rx.await? {
            NetworkResponse::MailboxMessages { messages } => Ok(messages),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Acknowledges the receipt of messages from a mailbox.
    ///
    /// This will delete the acknowledged messages from the mailbox.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the mailbox node.
    /// * `recipient` - The hash of the recipient's public key.
    /// * `msg_ids` - The IDs of the messages to acknowledge.
    ///
    /// # Errors
    ///
    /// This function will return an error if the messages cannot be acknowledged.
    pub async fn mailbox_ack(
        &self,
        peer_id: PeerId,
        recipient: [u8; 32],
        msg_ids: Vec<uuid::Uuid>,
    ) -> Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.command_sender.send(NetworkCommand::MailboxAck {
            peer_id,
            recipient,
            msg_ids,
            response: tx,
        })?;
        match rx.await? {
            NetworkResponse::MailboxAckResult { deleted } => Ok(deleted),
            NetworkResponse::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Starts a Kademlia DHT query to find providers for a key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to find providers for.
    ///
    /// # Errors
    ///
    /// This function will return an error if the query cannot be started.
    pub async fn start_dht_provider_query(&self, key: kad::RecordKey) -> Result<kad::QueryId> {
        let (tx, rx) = oneshot::channel();
        self.command_sender
            .send(NetworkCommand::StartDhtProviderQuery { key, response: tx })?;
        rx.await?
    }
}
