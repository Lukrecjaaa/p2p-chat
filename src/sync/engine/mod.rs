//! This module contains the core synchronization engine of the application.
//!
//! The `SyncEngine` is responsible for discovering mailbox providers, fetching
//! and processing messages from mailboxes, retrying failed message deliveries,
//! and maintaining the reliability of mailbox interactions.
use crate::cli::UiNotification;
use crate::crypto::Identity;
use crate::network::NetworkHandle;
use crate::storage::{FriendsStore, KnownMailboxesStore, MessageStore, OutboxStore, SeenTracker};
use crate::sync::backoff::BackoffManager;
use anyhow::Result;
use libp2p::{kad, PeerId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};

mod discovery;
mod events;
mod mailbox;
mod outbox;
mod performance;

pub use events::{DhtQueryResult, SyncEvent};
use performance::MailboxPerformance;

/// The core synchronization engine.
///
/// This struct manages the discovery of mailbox providers, fetching and processing
/// messages, and retrying message deliveries.
pub struct SyncEngine {
    /// The interval at which the synchronization cycle runs.
    pub interval: Duration,
    /// A set of discovered mailbox `PeerId`s.
    pub discovered_mailboxes: HashSet<PeerId>,
    /// Performance metrics for each discovered mailbox.
    pub mailbox_performance: HashMap<PeerId, MailboxPerformance>,
    /// Manages backoff for failing peers.
    pub backoff_manager: BackoffManager,
    /// Stores the state of pending DHT queries.
    pub pending_dht_queries: HashMap<kad::QueryId, DhtQueryState>,
    /// The `Instant` of the last mailbox discovery.
    pub last_discovery_time: Option<Instant>,
    /// The local node's identity.
    pub identity: Arc<Identity>,
    /// The store for managing friends.
    pub friends: Arc<dyn FriendsStore + Send + Sync>,
    /// The store for managing outgoing messages.
    pub outbox: Arc<dyn OutboxStore + Send + Sync>,
    /// The store for managing message history.
    pub history: Arc<dyn MessageStore + Send + Sync>,
    /// The tracker for seen messages.
    pub seen: Arc<dyn SeenTracker + Send + Sync>,
    /// The store for known mailbox providers.
    pub known_mailboxes: Arc<dyn KnownMailboxesStore + Send + Sync>,
    /// The network handle for communicating with the `NetworkLayer`.
    pub network: Option<NetworkHandle>,
    /// Sender for UI notifications.
    pub ui_notify_tx: mpsc::UnboundedSender<UiNotification>,
    /// Sender for web UI notifications.
    pub web_notify_tx: Option<mpsc::UnboundedSender<UiNotification>>,
}

/// A collection of storage traits used by the `SyncEngine`.
#[derive(Clone)]
pub struct SyncStores {
    /// The friends store.
    pub friends: Arc<dyn FriendsStore + Send + Sync>,
    /// The outbox store.
    pub outbox: Arc<dyn OutboxStore + Send + Sync>,
    /// The message history store.
    pub history: Arc<dyn MessageStore + Send + Sync>,
    /// The seen messages tracker.
    pub seen: Arc<dyn SeenTracker + Send + Sync>,
    /// The known mailboxes store.
    pub known_mailboxes: Arc<dyn KnownMailboxesStore + Send + Sync>,
}

impl SyncStores {
    /// Creates a new `SyncStores` instance.
    pub fn new(
        friends: Arc<dyn FriendsStore + Send + Sync>,
        outbox: Arc<dyn OutboxStore + Send + Sync>,
        history: Arc<dyn MessageStore + Send + Sync>,
        seen: Arc<dyn SeenTracker + Send + Sync>,
        known_mailboxes: Arc<dyn KnownMailboxesStore + Send + Sync>,
    ) -> Self {
        Self {
            friends,
            outbox,
            history,
            seen,
            known_mailboxes,
        }
    }
}

/// Represents the state of a pending Kademlia DHT query.
#[derive(Debug, Clone)]
pub struct DhtQueryState {
    /// The key being queried.
    pub key: kad::RecordKey,
    /// The `Instant` when the query was started.
    pub started_at: Instant,
    /// Whether any results have been received for this query.
    pub received_results: bool,
}

impl SyncEngine {
    /// Creates a new `SyncEngine` with a network handle.
    ///
    /// # Arguments
    ///
    /// * `interval` - The interval for the synchronization cycle.
    /// * `identity` - The local node's identity.
    /// * `stores` - A collection of storage implementations.
    /// * `network` - The network handle.
    /// * `ui_notify_tx` - Sender for UI notifications.
    /// * `web_notify_tx` - Sender for web UI notifications.
    ///
    /// # Returns
    ///
    /// A `Result` containing a tuple of the `SyncEngine` instance, an event sender,
    /// and an event receiver.
    pub fn new_with_network(
        interval: Duration,
        identity: Arc<Identity>,
        stores: SyncStores,
        network: NetworkHandle,
        ui_notify_tx: mpsc::UnboundedSender<UiNotification>,
        web_notify_tx: Option<mpsc::UnboundedSender<UiNotification>>,
    ) -> Result<(
        Self,
        mpsc::UnboundedSender<SyncEvent>,
        mpsc::UnboundedReceiver<SyncEvent>,
    )> {
        let SyncStores {
            friends,
            outbox,
            history,
            seen,
            known_mailboxes,
        } = stores;
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let engine = Self {
            interval: if interval.is_zero() {
                Duration::from_secs(5)
            } else {
                interval
            },
            discovered_mailboxes: HashSet::new(),
            mailbox_performance: HashMap::new(),
            backoff_manager: BackoffManager::new(),
            pending_dht_queries: HashMap::new(),
            last_discovery_time: None,
            identity,
            friends,
            outbox,
            history,
            seen,
            known_mailboxes,
            network: Some(network),
            ui_notify_tx,
            web_notify_tx,
        };
        Ok((engine, event_tx, event_rx))
    }

    /// Performs an initial discovery of mailbox providers on startup.
    ///
    /// # Errors
    ///
    /// Returns an error if the discovery process fails.
    pub async fn initial_discovery(&mut self) -> Result<()> {
        debug!("Performing initial mailbox discovery on startup");

        if let Err(e) = self.discover_mailboxes_if_needed(true).await {
            warn!("Initial mailbox discovery failed: {}", e);
        }

        Ok(())
    }

    /// Runs a single synchronization cycle.
    ///
    /// This includes discovering mailboxes, fetching messages, retrying outbox
    /// messages, and cleaning up old data.
    ///
    /// # Errors
    ///
    /// Returns an error if any part of the synchronization cycle fails.
    pub async fn sync_cycle(&mut self) -> Result<()> {
        trace!("Starting sync cycle");

        if let Err(e) = self.discover_mailboxes_if_needed(false).await {
            error!("Failed to discover mailboxes: {}", e);
        }

        if let Err(e) = self.fetch_from_mailboxes().await {
            error!("Failed to fetch from mailboxes: {}", e);
        }

        if let Err(e) = self.retry_outbox().await {
            error!("Failed to retry outbox: {}", e);
        }

        if let Err(e) = self
            .seen
            .cleanup_old(Duration::from_secs(7 * 24 * 60 * 60))
            .await
        {
            error!("Failed to cleanup seen entries: {}", e);
        }

        self.cleanup_failing_mailboxes().await;
        self.cleanup_stale_dht_queries();

        trace!("Sync cycle completed");
        Ok(())
    }

    /// Handles an incoming `SyncEvent`.
    ///
    /// This function processes various events related to peer connections and
    /// DHT query results, triggering appropriate synchronization actions.
    ///
    /// # Arguments
    ///
    /// * `event` - The `SyncEvent` to handle.
    ///
    /// # Errors
    ///
    /// Returns an error if handling the event fails.
    pub async fn handle_event(&mut self, event: SyncEvent) -> Result<()> {
        match event {
            SyncEvent::PeerConnected(peer_id) => {
                debug!(
                    "Peer {} connected, retrying outbox messages and checking for mailboxes",
                    peer_id
                );

                self.discover_mailboxes_if_needed(false).await?;
                self.retry_outbox_for_peer(&peer_id).await?;

                if self.discovered_mailboxes.contains(&peer_id) {
                    info!(
                        "Connected to known mailbox provider {}, triggering instant fetch.",
                        peer_id
                    );
                    if let Err(e) = self.fetch_from_single_mailbox(peer_id).await {
                        error!("Instant fetch from mailbox {} failed: {}", peer_id, e);
                    }
                }
            }
            SyncEvent::PeerConnectionFailed(peer_id) => {
                if self.discovered_mailboxes.contains(&peer_id) {
                    debug!(
                        "Connection failed to known mailbox {}, tracking failure",
                        peer_id
                    );

                    self.update_mailbox_performance(peer_id, false, Duration::from_millis(2000)).await;

                    if self.should_forget_mailbox(peer_id) {
                        self.forget_failing_mailbox(peer_id).await;
                    }
                } else {
                    trace!(
                        "Connection failed to peer {} (not a known mailbox)",
                        peer_id
                    );
                }
            }
            SyncEvent::DhtQueryResult { query_id, result } => {
                if let Some(query_state) = self.pending_dht_queries.get_mut(&query_id) {
                    query_state.received_results = true;
                    let key = query_state.key.clone();

                    let should_remove = match &result {
                        DhtQueryResult::ProvidersFound { finished, .. } => *finished,
                        DhtQueryResult::QueryFailed { .. } => true,
                    };

                    if should_remove {
                        self.pending_dht_queries.remove(&query_id);
                    }

                    self.handle_dht_query_result(key, result).await?;
                } else {
                    debug!(
                        "Received DHT query result for unknown query: {:?}",
                        query_id
                    );
                }
            }
        }
        Ok(())
    }
}
