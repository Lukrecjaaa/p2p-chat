//! This module defines the state of the `NetworkLayer`.
use std::collections::HashMap;
use std::sync::Arc;

use libp2p::{request_response::OutboundRequestId, swarm::Swarm, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::cli::commands::UiNotification;
use crate::storage::SledMailboxStore;
use crate::sync::SyncEvent;

use super::super::behaviour::P2PBehaviour;
use super::super::message::{NetworkCommand, NetworkResponse};

/// The state of the network layer.
///
/// This struct holds all the necessary components for the network layer to
/// function, such as the `libp2p` `Swarm`, channels for communication with
/// other parts of the application, and storage.
pub struct NetworkLayer {
    /// The `libp2p` `Swarm`.
    pub(crate) swarm: Swarm<P2PBehaviour>,
    /// The receiver for network commands.
    pub(crate) command_receiver: mpsc::UnboundedReceiver<NetworkCommand>,
    /// A map of pending outbound requests.
    pub(crate) pending_requests: HashMap<OutboundRequestId, oneshot::Sender<NetworkResponse>>,
    /// The sender for synchronization events.
    pub(crate) sync_event_tx: Option<mpsc::UnboundedSender<SyncEvent>>,
    /// The sender for UI notifications.
    pub(crate) ui_notify_tx: Option<mpsc::UnboundedSender<UiNotification>>,
    /// The storage for the mailbox.
    pub(crate) mailbox_storage: Option<Arc<SledMailboxStore>>,
    /// A map of peers that are currently blocked.
    pub(crate) blocked_peers: HashMap<PeerId, std::time::Instant>,
}
