use crate::cli::UiNotification;
use crate::crypto::Identity;
use crate::mailbox::{make_mailbox_provider_key, make_recipient_mailbox_key};
use crate::network::NetworkHandle;
use crate::storage::{FriendsStore, MessageStore, OutboxStore, SeenTracker};
use crate::sync::backoff::BackoffManager;
use crate::sync::retry::RetryPolicy;
use crate::types::{EncryptedMessage, Message};
use anyhow::Result;
use libp2p::{kad, PeerId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};

const MAX_CONSECUTIVE_FAILURES: u32 = 3;
const MAX_FAILURES_IN_WINDOW: u32 = 5;
const FAILURE_WINDOW_SECONDS: u64 = 60; // 1 minute

pub enum SyncEvent {
    PeerConnected(PeerId),
    PeerConnectionFailed(PeerId),
    DhtQueryResult {
        query_id: kad::QueryId,
        result: DhtQueryResult,
    },
}

pub enum DhtQueryResult {
    ProvidersFound {
        providers: HashSet<PeerId>,
        finished: bool,
    },
    QueryFailed {
        error: String,
    },
}

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

#[derive(Debug, Clone)]
pub struct MailboxPerformance {
    pub success_count: u32,
    pub failure_count: u32,
    pub consecutive_failures: u32,
    pub last_success: Option<std::time::Instant>,
    pub last_failure: Option<std::time::Instant>,
    pub avg_response_time: Duration,
}

impl SyncEngine {
    pub fn new_with_network(
        _interval: Duration,
        identity: Arc<Identity>,
        friends: Arc<dyn FriendsStore + Send + Sync>,
        outbox: Arc<dyn OutboxStore + Send + Sync>,
        history: Arc<dyn MessageStore + Send + Sync>,
        seen: Arc<dyn SeenTracker + Send + Sync>,
        network: NetworkHandle,
        ui_notify_tx: mpsc::UnboundedSender<UiNotification>,
    ) -> Result<(Self, mpsc::UnboundedSender<SyncEvent>, mpsc::UnboundedReceiver<SyncEvent>)> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let engine = Self {
            interval: Duration::from_secs(5), // Much more frequent sync cycles for faster outbox retry
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
    
    /// Perform initial discovery on startup - this should only be called once
    pub async fn initial_discovery(&mut self) -> Result<()> {
        debug!("Performing initial mailbox discovery on startup");
        
        if let Err(e) = self.discover_mailboxes_if_needed(true).await {
            warn!("Initial mailbox discovery failed: {}", e);
        }
        
        Ok(())
    }

    pub async fn sync_cycle(&mut self) -> Result<()> {
        trace!("Starting sync cycle");

        // Smart discovery - only discover if we don't have enough available mailboxes
        if let Err(e) = self.discover_mailboxes_if_needed(false).await {
            error!("Failed to discover mailboxes: {}", e);
        }

        if let Err(e) = self.fetch_from_mailboxes().await {
            error!("Failed to fetch from mailboxes: {}", e);
        }

        if let Err(e) = self.retry_outbox().await {
            error!("Failed to retry outbox: {}", e);
        }

        if let Err(e) = self.seen.cleanup_old(Duration::from_secs(7 * 24 * 60 * 60)).await {
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
                debug!("Peer {} connected, retrying outbox messages and checking for mailboxes", peer_id);
                
                // Discover mailboxes when new peers connect (but smartly)
                self.discover_mailboxes_if_needed(false).await?;
                self.retry_outbox_for_peer(&peer_id).await?;

                if self.discovered_mailboxes.contains(&peer_id) {
                    info!("Connected to known mailbox provider {}, triggering instant fetch.", peer_id);
                    if let Err(e) = self.fetch_from_single_mailbox(peer_id).await {
                        error!("Instant fetch from mailbox {} failed: {}", peer_id, e);
                    }
                }
            }
            SyncEvent::PeerConnectionFailed(peer_id) => {
                // Only track connection failures for discovered mailboxes
                if self.discovered_mailboxes.contains(&peer_id) {
                    debug!("Connection failed to known mailbox {}, tracking failure", peer_id);
                    
                    // Track connection failure as a serious failure (equivalent to multiple request failures)
                    self.update_mailbox_performance(peer_id, false, Duration::from_millis(2000));
                    
                    // Check if we should forget this mailbox due to persistent failures
                    if self.should_forget_mailbox(peer_id) {
                        self.forget_failing_mailbox(peer_id);
                    }
                } else {
                    trace!("Connection failed to peer {} (not a known mailbox)", peer_id);
                }
            }
            SyncEvent::DhtQueryResult { query_id, result } => {
                if let Some(query_state) = self.pending_dht_queries.get_mut(&query_id) {
                    query_state.received_results = true;
                    let key = query_state.key.clone();
                    
                    // Only remove the query if it's finished
                    let should_remove = match &result {
                        DhtQueryResult::ProvidersFound { finished, .. } => *finished,
                        DhtQueryResult::QueryFailed { .. } => true, // Failures are always final
                    };
                    
                    if should_remove {
                        self.pending_dht_queries.remove(&query_id);
                    }
                    
                    self.handle_dht_query_result(key, result).await?;
                } else {
                    debug!("Received DHT query result for unknown query: {:?}", query_id);
                }
            }
        }
        Ok(())
    }

    pub async fn retry_outbox_for_peer(&self, target_peer: &PeerId) -> Result<()> {
        let pending_messages = self.outbox.get_pending().await?;
        
        if pending_messages.is_empty() {
            return Ok(());
        }

        let Some(network) = &self.network else {
            debug!("No network handle available for outbox retry");
            return Ok(());
        };

        debug!("Retrying {} pending messages for peer {}", pending_messages.len(), target_peer);

        for message in pending_messages {
            if message.recipient != *target_peer {
                continue;
            }

            match network.send_message(message.recipient, message.clone()).await {
                Ok(()) => {
                    self.outbox.remove_pending(&message.id).await?;
                    info!("Successfully delivered message {} to {}", message.id, message.recipient);
                }
                Err(e) => {
                    debug!("Failed to deliver message {} to {}: {}", message.id, message.recipient, e);
                }
            }
        }

        Ok(())
    }

    async fn fetch_from_mailboxes(&mut self) -> Result<()> {
        if self.discovered_mailboxes.is_empty() {
            trace!("No mailbox nodes discovered, skipping fetch cycle.");
            return Ok(());
        }

        // Filter out backed-off mailboxes
        let available_mailboxes = self.get_available_mailboxes();
            
        if available_mailboxes.is_empty() {
            trace!("All discovered mailboxes are currently backed off, skipping fetch cycle.");
            return Ok(());
        }

        let mut total_processed = 0;
        for peer_id in available_mailboxes.iter() {
            // Skip if this mailbox was removed during iteration (due to connection failures)
            if !self.discovered_mailboxes.contains(peer_id) {
                debug!("Skipping fetch from mailbox {} - was removed during iteration", peer_id);
                continue;
            }
            
            // Double-check backoff status (may have changed during iteration)
            if !self.backoff_manager.can_attempt(peer_id) {
                debug!("Skipping fetch from backed-off mailbox {}", peer_id);
                continue;
            }
            
            match self.fetch_from_single_mailbox(*peer_id).await {
                Ok(processed_ids) => {
                    total_processed += processed_ids.len();
                }
                Err(e) => {
                    error!("Scheduled fetch from mailbox {} failed: {}", peer_id, e);
                }
            }
        }

        if total_processed > 0 {
            info!("Fetch cycle completed: {} messages processed across all mailboxes", total_processed);
        } else {
            trace!("Fetch cycle completed: no new messages found");
        }

        Ok(())
    }
    
    async fn fetch_from_single_mailbox(&mut self, peer_id: PeerId) -> Result<Vec<uuid::Uuid>> {
        let Some(network) = self.network.clone() else {
            debug!("No network handle available for single mailbox fetch");
            return Ok(vec![]);
        };

        let recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(
            &self.identity.hpke_public_key()
        );

        debug!("Sync: Fetching messages from mailbox {}", peer_id);
        
        let start_time = std::time::Instant::now();
        let retry_policy = RetryPolicy::fast_mailbox();
        
        let fetch_result = retry_policy.retry_with_jitter(|| async {
            network.mailbox_fetch(peer_id, recipient_hash, 100).await
                .map_err(|e| anyhow::anyhow!("Fetch failed: {}", e))
        }).await;

        match fetch_result {
            Ok(messages) => {
                self.update_mailbox_performance(peer_id, true, start_time.elapsed());
                
                if messages.is_empty() {
                    trace!("No messages found in mailbox {}", peer_id);
                    return Ok(vec![]);
                }
                info!("Retrieved {} messages from mailbox {}", messages.len(), peer_id);
                
                match self.process_mailbox_messages(messages).await {
                    Ok(processed_ids) => {
                        if !processed_ids.is_empty() {
                            info!("Successfully processed {} new messages from mailbox {}", processed_ids.len(), peer_id);
                            if let Err(e) = self.acknowledge_mailbox_messages(processed_ids.clone()).await {
                                error!("Failed to ACK messages to mailbox {}: {}", peer_id, e);
                            }
                        }
                        Ok(processed_ids)
                    }
                    Err(e) => {
                        error!("Failed to process messages from mailbox {}: {}", peer_id, e);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                // The retry policy tried multiple times, so this represents multiple failures
                // Update performance to reflect the severity of this failure
                let fast_policy = RetryPolicy::fast_mailbox();
                for _ in 0..fast_policy.max_attempts {
                    self.update_mailbox_performance(peer_id, false, start_time.elapsed() / fast_policy.max_attempts);
                }
                
                if self.should_forget_mailbox(peer_id) {
                    self.forget_failing_mailbox(peer_id);
                }
                
                error!("Failed to fetch from mailbox {} after retries: {}", peer_id, e);
                Err(e)
            }
        }
    }

    pub async fn retry_outbox(&mut self) -> Result<()> {
        let pending_messages = self.outbox.get_pending().await?;
        if pending_messages.is_empty() {
            return Ok(());
        }
    
        let Some(network) = self.network.clone() else {
            debug!("No network handle available for outbox retry");
            return Ok(());
        };
        
        // If we have pending messages but no available mailboxes, trigger discovery
        let available_mailboxes = self.get_available_mailboxes();
            
        if self.discovered_mailboxes.is_empty() {
            debug!("Have {} pending messages but no discovered mailboxes, triggering forced discovery", pending_messages.len());
            if let Err(e) = self.discover_mailboxes_if_needed(true).await {
                warn!("Failed to trigger mailbox discovery for pending messages: {}", e);
            }
        } else if available_mailboxes.is_empty() {
            debug!("Have {} pending messages but all {} mailboxes are backed off", 
                   pending_messages.len(), self.discovered_mailboxes.len());
            // Don't trigger discovery immediately - let the backoff naturally expire
            // But log this so users know why messages aren't being forwarded
        }
    
        debug!("Retrying {} pending messages", pending_messages.len());
    
        for message in pending_messages {
            let should_try_direct = self.backoff_manager.can_attempt(&message.recipient);
            
            let direct_result = if should_try_direct {
                debug!("Attempting direct delivery to peer {}", message.recipient);
                self.backoff_manager.record_attempt(message.recipient);
                network.send_message(message.recipient, message.clone()).await
            } else {
                debug!("Skipping direct delivery attempt to backed-off peer {}", message.recipient);
                Err(anyhow::anyhow!("Peer is backed off"))
            };
            
            match direct_result {
                Ok(()) => {
                    if should_try_direct {
                        self.backoff_manager.record_success(&message.recipient);
                    }
                    self.outbox.remove_pending(&message.id).await?;
                    info!("Successfully delivered message {} directly to {}", message.id, message.recipient);
                }
                Err(e) => {
                    if should_try_direct {
                        self.backoff_manager.record_failure(message.recipient);
                    }
                    debug!("Direct retry for message {} to {} failed: {}. Attempting mailbox forward.", message.id, message.recipient, e);
    
                    if self.discovered_mailboxes.is_empty() {
                        debug!("No mailboxes discovered to forward message {}.", message.id);
                        continue;
                    }
    
                    let Some(friend) = self.friends.get_friend(&message.recipient).await? else {
                        error!("Cannot forward message {}: recipient {} not in friends list.", message.id, message.recipient);
                        continue;
                    };
    
                    let recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(&friend.e2e_public_key);
                    let encrypted_msg = EncryptedMessage {
                        id: message.id,
                        sender: self.identity.peer_id,
                        recipient_hash,
                        encrypted_content: message.content.clone(),
                        timestamp: message.timestamp,
                        sender_pub_key: self.identity.hpke_public_key(),
                    };
    
                    // Use intelligent load balancing instead of pure random selection
                    let candidate_mailboxes = self.get_best_mailbox_providers();
                    
                    if candidate_mailboxes.is_empty() {
                        debug!("No available (non-backed-off) mailboxes to forward message {}.", message.id);
                        continue;
                    }
    
                    let min_replicas = 2;
                    let max_attempts = candidate_mailboxes.len().min(4);
                    let mut forwarded_count = 0;
                    let mut mailboxes_to_forget = Vec::new();
                    
                    for peer_id in candidate_mailboxes.iter().take(max_attempts) {
                        // Skip if this mailbox was removed during iteration (due to connection failures)
                        if !self.discovered_mailboxes.contains(peer_id) {
                            debug!("Skipping mailbox forwarding to {} - was removed during iteration", peer_id);
                            continue;
                        }
                        
                        let start_time = std::time::Instant::now();
                        match network.mailbox_put(*peer_id, recipient_hash, encrypted_msg.clone()).await {
                            Ok(true) => {
                                self.update_mailbox_performance(*peer_id, true, start_time.elapsed());
                                info!("Successfully forwarded pending message {} to mailbox {} ({}/{})", 
                                      message.id, peer_id, forwarded_count + 1, min_replicas);
                                forwarded_count += 1;
                                
                                if forwarded_count >= min_replicas {
                                    break;
                                }
                            }
                            Ok(false) => {
                                self.update_mailbox_performance(*peer_id, false, start_time.elapsed());
                                debug!("Mailbox {} rejected pending message {}", peer_id, message.id);
                                
                                if self.should_forget_mailbox(*peer_id) {
                                    mailboxes_to_forget.push(*peer_id);
                                }
                            }
                            Err(err) => {
                                self.update_mailbox_performance(*peer_id, false, start_time.elapsed());
                                debug!("Failed to forward pending message {} to mailbox {}: {}", message.id, peer_id, err);
                                
                                if self.should_forget_mailbox(*peer_id) {
                                    mailboxes_to_forget.push(*peer_id);
                                }
                            }
                        }
                    }
                    
                    // Forget failing mailboxes after iteration to avoid modifying collection during iteration
                    for mailbox_id in mailboxes_to_forget {
                        self.forget_failing_mailbox(mailbox_id);
                    }
                    
                    let forwarded = forwarded_count > 0;
    
                    if forwarded {
                        self.outbox.remove_pending(&message.id).await?;
                        info!("Removed message {} from outbox after successful mailbox forward.", message.id);
                    } else {
                        debug!("Failed to forward message {} to any mailboxes, will retry later.", message.id);
                    }
                }
            }
        }
    
        Ok(())
    }

    pub async fn discover_mailboxes(&mut self) -> Result<()> {
        self.discover_mailboxes_if_needed(false).await
    }
    
    pub async fn discover_mailboxes_if_needed(&mut self, force: bool) -> Result<()> {
        let current_mailbox_count = self.discovered_mailboxes.len();
        let available_mailbox_count = self.get_available_mailboxes().len();
        
        // Smart discovery logic: only discover if we need to
        if !force {
            // Skip if we have enough available mailboxes
            if available_mailbox_count >= 2 {
                trace!("Have {} available mailboxes, skipping discovery", available_mailbox_count);
                return Ok(());
            }
            
            // Rate limit discovery attempts (minimum 30 seconds between attempts)
            if let Some(last_discovery) = self.last_discovery_time {
                if last_discovery.elapsed() < Duration::from_secs(30) {
                    trace!("Last discovery was {:?} ago, skipping (rate limited)", last_discovery.elapsed());
                    return Ok(());
                }
            }
        }
        
        trace!("Discovering mailbox providers in DHT (currently have: {}, available: {}, force: {})", 
               current_mailbox_count, available_mailbox_count, force);
        
        let Some(network) = &self.network else {
            debug!("No network handle available for mailbox discovery");
            return Ok(());
        };
        
        // Update last discovery time
        self.last_discovery_time = Some(Instant::now());
        
        let general_mailbox_key = make_mailbox_provider_key();
        if let Ok(query_id) = network.start_dht_provider_query(general_mailbox_key.clone()).await {
            let query_state = DhtQueryState {
                key: general_mailbox_key,
                started_at: Instant::now(),
                received_results: false,
            };
            self.pending_dht_queries.insert(query_id, query_state);
            trace!("Started DHT query for general mailbox providers: {:?}", query_id);
        } else {
            error!("Failed to start DHT query for general mailboxes");
        }

        let our_recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(
            &self.identity.hpke_public_key()
        );
        let recipient_mailbox_key = make_recipient_mailbox_key(our_recipient_hash);
        
        if let Ok(query_id) = network.start_dht_provider_query(recipient_mailbox_key.clone()).await {
            let query_state = DhtQueryState {
                key: recipient_mailbox_key,
                started_at: Instant::now(),
                received_results: false,
            };
            self.pending_dht_queries.insert(query_id, query_state);
            trace!("Started DHT query for recipient-specific mailbox providers: {:?}", query_id);
        } else {
            trace!("Failed to start DHT query for recipient-specific mailboxes");
        }
        
        Ok(())
    }

    pub async fn get_emergency_mailboxes(&self) -> Vec<PeerId> {
        let Some(network) = &self.network else {
            return vec![];
        };
        
        match network.get_connected_peers().await {
            Ok(peers) => {
                debug!("Using {} connected peers as emergency mailboxes", peers.len());
                peers
            }
            Err(_) => vec![]
        }
    }

    async fn handle_dht_query_result(&mut self, key: kad::RecordKey, result: DhtQueryResult) -> Result<()> {
        match result {
            DhtQueryResult::ProvidersFound { providers, finished } => {
                let key_str = String::from_utf8_lossy(key.as_ref());
                if !providers.is_empty() {
                    info!("Found {} providers for key {} (finished: {})", providers.len(), key_str, finished);
                }
                
                let mut new_providers = 0;
                for provider in providers {
                    // Skip peers that are currently backed off
                    if !self.backoff_manager.can_attempt(&provider) {
                        if let Some(retry_time) = self.backoff_manager.time_until_retry(&provider) {
                            debug!("Skipping backed-off mailbox {} (retry in {:?})", 
                                   provider, retry_time);
                        }
                        continue;
                    }
                    
                    if self.discovered_mailboxes.insert(provider) {
                        new_providers += 1;
                        info!("Discovered new mailbox provider: {}", provider);
                        
                        // Reset backoff and performance tracking when rediscovering a mailbox
                        self.backoff_manager.record_success(&provider);
                    }
                }
                
                if new_providers > 0 {
                    info!("Added {} new mailbox provider(s) to the pool.", new_providers);
                    
                    if let Err(e) = self.retry_outbox().await {
                        error!("Failed to retry outbox after discovering new mailboxes: {}", e);
                    }
                }
            }
            DhtQueryResult::QueryFailed { error } => {
                let key_str = String::from_utf8_lossy(key.as_ref());
                trace!("DHT query failed for key {}: {}", key_str, error);
            }
        }
        Ok(())
    }

    pub fn get_mailbox_providers(&self) -> &HashSet<PeerId> {
        &self.discovered_mailboxes
    }
    
    pub fn get_available_mailboxes(&self) -> Vec<PeerId> {
        self.discovered_mailboxes
            .iter()
            .filter(|&peer_id| self.backoff_manager.can_attempt(peer_id))
            .cloned()
            .collect()
    }

    pub fn get_best_mailbox_providers(&self) -> Vec<PeerId> {
        let mut providers: Vec<_> = self.get_available_mailboxes(); // Only include non-backed-off mailboxes
        
        providers.sort_by(|a, b| {
            let score_a = self.calculate_mailbox_score(*a);
            let score_b = self.calculate_mailbox_score(*b);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        providers
    }

    fn calculate_mailbox_score(&self, peer_id: PeerId) -> f64 {
        // Base score for unknown mailboxes (moderate priority)
        let mut score = 0.5;
        
        // Factor in performance data if available
        if let Some(perf) = self.mailbox_performance.get(&peer_id) {
            let total_attempts = perf.success_count + perf.failure_count;
            
            if total_attempts > 0 {
                // Success rate (0.0 to 1.0) - most important factor
                let success_rate = perf.success_count as f64 / total_attempts as f64;
                score = success_rate * 0.7; // 70% weight for success rate
                
                // Recency bonus - prefer recently successful mailboxes
                if let Some(last_success) = perf.last_success {
                    let age_hours = last_success.elapsed().as_secs() as f64 / 3600.0;
                    let recency_bonus = (1.0 / (1.0 + age_hours)).min(0.3); // Max 30% bonus
                    score += recency_bonus * 0.2; // 20% weight for recency
                }
                
                // Response time factor - prefer faster mailboxes
                let response_ms = perf.avg_response_time.as_millis() as f64;
                let speed_score = (3000.0 - response_ms.min(3000.0)) / 3000.0; // Scale 0-3s to 1.0-0.0
                score += speed_score * 0.1; // 10% weight for speed
                
                // Consecutive failure penalty
                let failure_penalty = (perf.consecutive_failures as f64 * 0.1).min(0.3); // Max 30% penalty
                score -= failure_penalty;
            }
        }
        
        // Factor in backoff status (should already be filtered out, but double-check)
        if !self.backoff_manager.can_attempt(&peer_id) {
            score *= 0.1; // Heavy penalty for backed-off mailboxes
        }
        
        score.max(0.0).min(1.0) // Clamp to [0, 1]
    }

    fn should_forget_mailbox(&self, peer_id: PeerId) -> bool {
        if let Some(perf) = self.mailbox_performance.get(&peer_id) {
            // Forget if too many consecutive failures
            if perf.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                return true;
            }
            
            // Forget if too many failures in the recent time window
            if let Some(last_failure) = perf.last_failure {
                let time_since_last_failure = last_failure.elapsed().as_secs();
                if time_since_last_failure <= FAILURE_WINDOW_SECONDS && 
                   perf.failure_count >= MAX_FAILURES_IN_WINDOW {
                    return true;
                }
            }
        }
        false
    }

    fn forget_failing_mailbox(&mut self, peer_id: PeerId) {
        if self.discovered_mailboxes.remove(&peer_id) {
            warn!("Temporarily forgetting failing mailbox {} due to persistent failures", peer_id);
            
            // Add to backoff manager instead of blocking at transport level
            self.backoff_manager.record_failure(peer_id);
            
            // Keep performance tracking for when it comes back
            // Don't remove from performance tracking - let it recover naturally
        }
    }

    fn cleanup_failing_mailboxes(&mut self) {
        let mut mailboxes_to_forget = Vec::new();
        
        for peer_id in &self.discovered_mailboxes {
            if self.should_forget_mailbox(*peer_id) {
                mailboxes_to_forget.push(*peer_id);
            }
        }
        
        for peer_id in mailboxes_to_forget {
            self.forget_failing_mailbox(peer_id);
        }
    }
    
    fn cleanup_stale_dht_queries(&mut self) {
        let stale_timeout = Duration::from_secs(60); // 60 seconds timeout for DHT queries
        let mut stale_queries = Vec::new();
        
        for (&query_id, query_state) in &self.pending_dht_queries {
            if query_state.started_at.elapsed() > stale_timeout {
                stale_queries.push(query_id);
            }
        }
        
        for query_id in stale_queries {
            if let Some(query_state) = self.pending_dht_queries.remove(&query_id) {
                warn!("Removing stale DHT query {:?} (age: {:?}, received_results: {})", 
                      query_id, query_state.started_at.elapsed(), query_state.received_results);
            }
        }
        
        // Also cleanup backoff manager
        self.backoff_manager.cleanup_old_entries(Duration::from_secs(3600)); // 1 hour
    }

    pub fn update_mailbox_performance(&mut self, peer_id: PeerId, success: bool, response_time: Duration) {
        let perf = self.mailbox_performance.entry(peer_id).or_insert(MailboxPerformance {
            success_count: 0,
            failure_count: 0,
            consecutive_failures: 0,
            last_success: None,
            last_failure: None,
            avg_response_time: Duration::from_millis(1000),
        });

        if success {
            perf.success_count += 1;
            perf.consecutive_failures = 0; // Reset consecutive failures on success
            perf.last_success = Some(std::time::Instant::now());
            
            // Also update backoff manager
            self.backoff_manager.record_success(&peer_id);
        } else {
            perf.failure_count += 1;
            perf.consecutive_failures += 1;
            perf.last_failure = Some(std::time::Instant::now());
            
            // Also update backoff manager
            self.backoff_manager.record_failure(peer_id);
        }

        let new_weight = 0.3;
        let old_weight = 1.0 - new_weight;
        perf.avg_response_time = Duration::from_millis(
            ((perf.avg_response_time.as_millis() as f64 * old_weight) +
             (response_time.as_millis() as f64 * new_weight)) as u64
        );
    }


    async fn process_mailbox_messages(&self, messages: Vec<EncryptedMessage>) -> Result<Vec<uuid::Uuid>> {
        let mut processed_msg_ids = Vec::new();
        
        for encrypted_msg in messages {
            if self.seen.is_seen(&encrypted_msg.id).await? {
                trace!("Message {} already seen, adding to ACK list", encrypted_msg.id);
                processed_msg_ids.push(encrypted_msg.id);
                continue;
            }

            let message = self.reconstruct_message_from_mailbox(&encrypted_msg).await?;
            
            if let Err(e) = self.history.store_message(message.clone()).await {
                error!("Failed to store mailbox message {} in history: {}", encrypted_msg.id, e);
                continue;
            }
            
            if let Err(e) = self.seen.mark_seen(encrypted_msg.id).await {
                error!("Failed to mark message {} as seen: {}", encrypted_msg.id, e);
            }
            
            if let Err(e) = self.ui_notify_tx.send(UiNotification::NewMessage(message)) {
                error!("Failed to send UI notification for new message {}: {}", encrypted_msg.id, e);
            }
            
            processed_msg_ids.push(encrypted_msg.id);
        }
        
        Ok(processed_msg_ids)
    }

    async fn reconstruct_message_from_mailbox(&self, encrypted_msg: &EncryptedMessage) -> Result<Message> {
        Ok(Message {
            id: encrypted_msg.id,
            sender: encrypted_msg.sender,
            recipient: self.identity.peer_id,
            timestamp: encrypted_msg.timestamp,
            content: encrypted_msg.encrypted_content.clone(),
            nonce: 0,
        })
    }

    async fn acknowledge_mailbox_messages(&self, msg_ids: Vec<uuid::Uuid>) -> Result<()> {
        if msg_ids.is_empty() {
            return Ok(());
        }

        let Some(network) = &self.network else {
            debug!("No network handle available for mailbox ACK");
            return Ok(());
        };
        
        let recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(
            &self.identity.hpke_public_key()
        );
        
        info!("Acknowledging {} messages to {} mailboxes", msg_ids.len(), self.get_mailbox_providers().len());

        let mut total_deleted = 0;
        let mut successful_acks = 0;
        let mut failed_acks = 0;

        let retry_policy = RetryPolicy::fast_mailbox();

        for peer_id in self.get_mailbox_providers().iter() {
            let ack_result = retry_policy.retry_with_jitter(|| async {
                network.mailbox_ack(*peer_id, recipient_hash, msg_ids.clone()).await
                    .map_err(|e| anyhow::anyhow!("ACK failed: {}", e))
            }).await;

            match ack_result {
                Ok(deleted_count) => {
                    successful_acks += 1;
                    total_deleted += deleted_count;
                    if deleted_count > 0 {
                        info!("Mailbox {} confirmed deletion of {} messages", peer_id, deleted_count);
                    } else {
                        trace!("Mailbox {} had no messages to delete", peer_id);
                    }
                }
                Err(e) => {
                    failed_acks += 1;
                    error!("Failed to ACK messages to mailbox {} after retries: {}", peer_id, e);
                }
            }
        }
        
        info!("ACK summary: {} messages deleted across {} mailboxes, {}/{} ACKs successful", 
              total_deleted, successful_acks, successful_acks, successful_acks + failed_acks);

        if failed_acks > 0 {
            warn!("Failed to ACK to {} mailboxes - messages may remain stored", failed_acks);
        }
        
        Ok(())
    }
}