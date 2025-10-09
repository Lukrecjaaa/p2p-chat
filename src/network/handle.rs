use anyhow::{anyhow, Result};
use libp2p::{kad, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::types::{EncryptedMessage, Message};

use super::message::{NetworkCommand, NetworkResponse};

#[derive(Clone)]
pub struct NetworkHandle {
    pub(super) command_sender: mpsc::UnboundedSender<NetworkCommand>,
}

impl NetworkHandle {
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

    pub async fn start_dht_provider_query(&self, key: kad::RecordKey) -> Result<kad::QueryId> {
        let (tx, rx) = oneshot::channel();
        self.command_sender
            .send(NetworkCommand::StartDhtProviderQuery { key, response: tx })?;
        rx.await?
    }
}
