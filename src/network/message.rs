use crate::types::{ChatRequest, EncryptedMessage, Message};
use anyhow::Result;
use libp2p::{kad, PeerId};
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum NetworkResponse {
    MessageSent,
    ConnectedPeers { peers: Vec<PeerId> },
    Error(String),
    MailboxPutResult { success: bool },
    MailboxMessages { messages: Vec<EncryptedMessage> },
    MailboxAckResult { deleted: usize },
}

#[derive(Debug)]
pub enum NetworkCommand {
    SendMessage {
        peer_id: PeerId,
        message: Message,
        response: oneshot::Sender<NetworkResponse>,
    },
    SendChatRequest {
        peer_id: PeerId,
        request: ChatRequest,
        response: oneshot::Sender<NetworkResponse>,
    },
    GetConnectedPeers {
        response: oneshot::Sender<NetworkResponse>,
    },
    MailboxPut {
        peer_id: PeerId,
        recipient: [u8; 32],
        message: EncryptedMessage,
        response: oneshot::Sender<NetworkResponse>,
    },
    MailboxFetch {
        peer_id: PeerId,
        recipient: [u8; 32],
        limit: usize,
        response: oneshot::Sender<NetworkResponse>,
    },
    MailboxAck {
        peer_id: PeerId,
        recipient: [u8; 32],
        msg_ids: Vec<uuid::Uuid>,
        response: oneshot::Sender<NetworkResponse>,
    },
    StartDhtProviderQuery {
        key: kad::RecordKey,
        response: oneshot::Sender<Result<kad::QueryId>>,
    },
}
