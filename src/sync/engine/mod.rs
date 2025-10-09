use crate::cli::UiNotification;
use crate::crypto::Identity;
use crate::network::NetworkHandle;
use crate::storage::{FriendsStore, MessageStore, OutboxStore, SeenTracker};
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

pub struct SyncEngine {
    pub interval: Duration,
    pub discovered_mailboxes: HashSet<PeerId>,
    pub mailbox_performance: HashMap<PeerId, MailboxPerformance>,
    pub backoff_manager: BackoffManager,
    pub pending_dht_queries: HashMap<kad::QueryId, DhtQueryState>,
    pub last_discovery_time: Option<Instant>,
    pub identity: Arc<Identity>,
    pub friends: Arc<dyn FriendsStore + Send + Sync>,
    pub outbox: Arc<dyn OutboxStore + Send + Sync>,
    pub history: Arc<dyn MessageStore + Send + Sync>,
    pub seen: Arc<dyn SeenTracker + Send + Sync>,
    pub network: Option<NetworkHandle>,
    pub ui_notify_tx: mpsc::UnboundedSender<UiNotification>,
}

#[derive(Debug, Clone)]
pub struct DhtQueryState {
    pub key: kad::RecordKey,
    pub started_at: Instant,
    pub received_results: bool,
}

impl SyncEngine {
    pub fn new_with_network(
        interval: Duration,
        identity: Arc<Identity>,
        friends: Arc<dyn FriendsStore + Send + Sync>,
        outbox: Arc<dyn OutboxStore + Send + Sync>,
        history: Arc<dyn MessageStore + Send + Sync>,
        seen: Arc<dyn SeenTracker + Send + Sync>,
        network: NetworkHandle,
        ui_notify_tx: mpsc::UnboundedSender<UiNotification>,
    ) -> Result<(
        Self,
        mpsc::UnboundedSender<SyncEvent>,
        mpsc::UnboundedReceiver<SyncEvent>,
    )> {
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
            network: Some(network),
            ui_notify_tx,
        };
        Ok((engine, event_tx, event_rx))
    }

    pub async fn initial_discovery(&mut self) -> Result<()> {
        debug!("Performing initial mailbox discovery on startup");

        if let Err(e) = self.discover_mailboxes_if_needed(true).await {
            warn!("Initial mailbox discovery failed: {}", e);
        }

        Ok(())
    }

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

        self.cleanup_failing_mailboxes();
        self.cleanup_stale_dht_queries();

        trace!("Sync cycle completed");
        Ok(())
    }

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

                    self.update_mailbox_performance(peer_id, false, Duration::from_millis(2000));

                    if self.should_forget_mailbox(peer_id) {
                        self.forget_failing_mailbox(peer_id);
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
