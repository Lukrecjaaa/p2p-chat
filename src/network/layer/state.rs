use std::collections::HashMap;
use std::sync::Arc;

use libp2p::{request_response::OutboundRequestId, swarm::Swarm, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::cli::commands::UiNotification;
use crate::storage::SledMailboxStore;
use crate::sync::SyncEvent;

use super::super::behaviour::P2PBehaviour;
use super::super::message::{NetworkCommand, NetworkResponse};

pub struct NetworkLayer {
    pub(crate) swarm: Swarm<P2PBehaviour>,
    pub(crate) command_receiver: mpsc::UnboundedReceiver<NetworkCommand>,
    pub(crate) pending_requests: HashMap<OutboundRequestId, oneshot::Sender<NetworkResponse>>,
    pub(crate) sync_event_tx: Option<mpsc::UnboundedSender<SyncEvent>>,
    pub(crate) ui_notify_tx: Option<mpsc::UnboundedSender<UiNotification>>,
    pub(crate) mailbox_storage: Option<Arc<SledMailboxStore>>,
    pub(crate) blocked_peers: HashMap<PeerId, std::time::Instant>,
}
