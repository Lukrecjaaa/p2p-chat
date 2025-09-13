use crate::cli::UiNotification;
use crate::crypto::Identity;
use crate::mailbox::{make_mailbox_provider_key, make_recipient_mailbox_key};
use crate::network::NetworkHandle;
use crate::storage::{FriendsStore, MessageStore, OutboxStore, SeenTracker};
use crate::sync::retry::RetryPolicy;
use crate::types::{EncryptedMessage, Message};
use anyhow::Result;
use libp2p::{kad, PeerId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};

pub enum SyncEvent {
    PeerConnected(PeerId),
    DhtQueryResult {
        query_id: kad::QueryId,
        result: DhtQueryResult,
    },
}

pub enum DhtQueryResult {
    ProvidersFound {
        providers: HashSet<PeerId>,
    },
    QueryFailed {
        error: String,
    },
}

pub struct SyncEngine {
    pub interval: Duration,
    pub discovered_mailboxes: HashSet<PeerId>,
    pub mailbox_performance: HashMap<PeerId, MailboxPerformance>,
    pub pending_dht_queries: HashMap<kad::QueryId, kad::RecordKey>,
    pub identity: Arc<Identity>,
    pub friends: Arc<dyn FriendsStore + Send + Sync>,
    pub outbox: Arc<dyn OutboxStore + Send + Sync>,
    pub history: Arc<dyn MessageStore + Send + Sync>,
    pub seen: Arc<dyn SeenTracker + Send + Sync>,
    pub network: Option<NetworkHandle>,
    pub ui_notify_tx: mpsc::UnboundedSender<UiNotification>,
}

#[derive(Debug, Clone)]
pub struct MailboxPerformance {
    pub success_count: u32,
    pub failure_count: u32,
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
            pending_dht_queries: HashMap::new(),
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

    pub async fn sync_cycle(&mut self) -> Result<()> {
        trace!("Starting sync cycle");

        if let Err(e) = self.discover_mailboxes().await {
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

        trace!("Sync cycle completed");
        Ok(())
    }

    pub async fn handle_event(&mut self, event: SyncEvent) -> Result<()> {
        match event {
            SyncEvent::PeerConnected(peer_id) => {
                debug!("Peer {} connected, retrying outbox messages and checking for mailboxes", peer_id);
                self.discover_mailboxes().await?;
                self.retry_outbox_for_peer(&peer_id).await?;

                if self.discovered_mailboxes.contains(&peer_id) {
                    info!("Connected to known mailbox provider {}, triggering instant fetch.", peer_id);
                    if let Err(e) = self.fetch_from_single_mailbox(peer_id).await {
                        error!("Instant fetch from mailbox {} failed: {}", peer_id, e);
                    }
                }
            }
            SyncEvent::DhtQueryResult { query_id, result } => {
                if let Some(key) = self.pending_dht_queries.remove(&query_id) {
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

    async fn fetch_from_mailboxes(&self) -> Result<()> {
        if self.discovered_mailboxes.is_empty() {
            trace!("No mailbox nodes to fetch from, skipping fetch cycle.");
            return Ok(());
        }

        let mut total_processed = 0;

        for peer_id in self.get_mailbox_providers().iter() {
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
    
    async fn fetch_from_single_mailbox(&self, peer_id: PeerId) -> Result<Vec<uuid::Uuid>> {
        let Some(network) = &self.network else {
            debug!("No network handle available for single mailbox fetch");
            return Ok(vec![]);
        };

        let recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(
            &self.identity.hpke_public_key()
        );

        debug!("Sync: Fetching messages from mailbox {}", peer_id);
        
        let retry_policy = RetryPolicy::fast_mailbox();
        
        let fetch_result = retry_policy.retry_with_jitter(|| async {
            network.mailbox_fetch(peer_id, recipient_hash, 100).await
                .map_err(|e| anyhow::anyhow!("Fetch failed: {}", e))
        }).await;

        match fetch_result {
            Ok(messages) => {
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
                error!("Failed to fetch from mailbox {} after retries: {}", peer_id, e);
                Err(e)
            }
        }
    }

    pub async fn retry_outbox(&self) -> Result<()> {
        let pending_messages = self.outbox.get_pending().await?;
        if pending_messages.is_empty() {
            return Ok(());
        }
    
        let Some(network) = &self.network else {
            debug!("No network handle available for outbox retry");
            return Ok(());
        };
    
        debug!("Retrying {} pending messages", pending_messages.len());
    
        for message in pending_messages {
            match network.send_message(message.recipient, message.clone()).await {
                Ok(()) => {
                    self.outbox.remove_pending(&message.id).await?;
                    info!("Successfully delivered message {} directly to {}", message.id, message.recipient);
                }
                Err(e) => {
                    debug!("Direct retry for message {} to {} failed: {}. Attempting mailbox forward.", message.id, message.recipient, e);
    
                    if self.discovered_mailboxes.is_empty() {
                        debug!("No mailboxes available to forward message {}.", message.id);
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
    
                    use rand::seq::SliceRandom;
                    let mut candidate_mailboxes: Vec<_> = self.discovered_mailboxes.iter().cloned().collect();
                    candidate_mailboxes.shuffle(&mut rand::thread_rng());
    
                    let min_replicas = 2;
                    let max_attempts = candidate_mailboxes.len().min(4);
                    let mut forwarded_count = 0;
                    
                    for peer_id in candidate_mailboxes.iter().take(max_attempts) {
                        match network.mailbox_put(*peer_id, recipient_hash, encrypted_msg.clone()).await {
                            Ok(true) => {
                                info!("Successfully forwarded pending message {} to mailbox {} ({}/{})", 
                                      message.id, peer_id, forwarded_count + 1, min_replicas);
                                forwarded_count += 1;
                                
                                if forwarded_count >= min_replicas {
                                    break;
                                }
                            }
                            Ok(false) => debug!("Mailbox {} rejected pending message {}", peer_id, message.id),
                            Err(err) => debug!("Failed to forward pending message {} to mailbox {}: {}", message.id, peer_id, err),
                        }
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
        let current_mailbox_count = self.discovered_mailboxes.len();
        trace!("Discovering mailbox providers in DHT (currently have: {})", current_mailbox_count);
        
        let Some(network) = &self.network else {
            debug!("No network handle available for mailbox discovery");
            return Ok(());
        };
        
        let general_mailbox_key = make_mailbox_provider_key();
        if let Ok(query_id) = network.start_dht_provider_query(general_mailbox_key.clone()).await {
            self.pending_dht_queries.insert(query_id, general_mailbox_key);
            trace!("Started DHT query for general mailbox providers: {:?}", query_id);
        } else {
            error!("Failed to start DHT query for general mailboxes");
        }

        let our_recipient_hash = crate::crypto::StorageEncryption::derive_recipient_hash(
            &self.identity.hpke_public_key()
        );
        let recipient_mailbox_key = make_recipient_mailbox_key(our_recipient_hash);
        
        if let Ok(query_id) = network.start_dht_provider_query(recipient_mailbox_key.clone()).await {
            self.pending_dht_queries.insert(query_id, recipient_mailbox_key);
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
            DhtQueryResult::ProvidersFound { providers, .. } => {
                let key_str = String::from_utf8_lossy(key.as_ref());
                if !providers.is_empty() {
                    info!("Found {} providers for key {}", providers.len(), key_str);
                }
                
                let mut new_providers = 0;
                for provider in providers {
                    if self.discovered_mailboxes.insert(provider) {
                        new_providers += 1;
                        info!("Discovered new mailbox provider: {}", provider);
                    }
                }
                
                if new_providers > 0 {
                    info!("Added {} new mailbox provider(s) to the pool.", new_providers);
                    
                    if let Err(e) = self.retry_outbox().await {
                        error!("Failed to retry outbox after discovering new mailboxes: {}", e);
                    }
                }
            }
            DhtQueryResult::QueryFailed { error, .. } => {
                let key_str = String::from_utf8_lossy(key.as_ref());
                trace!("DHT query failed for key {}: {}", key_str, error);
            }
        }
        Ok(())
    }

    pub fn get_mailbox_providers(&self) -> &HashSet<PeerId> {
        &self.discovered_mailboxes
    }

    pub fn get_best_mailbox_providers(&self) -> Vec<PeerId> {
        let mut providers: Vec<_> = self.discovered_mailboxes.iter().cloned().collect();
        
        providers.sort_by(|a, b| {
            let score_a = self.calculate_mailbox_score(*a);
            let score_b = self.calculate_mailbox_score(*b);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        providers
    }

    fn calculate_mailbox_score(&self, peer_id: PeerId) -> f64 {
        if let Some(perf) = self.mailbox_performance.get(&peer_id) {
            let total_attempts = perf.success_count + perf.failure_count;
            if total_attempts == 0 {
                return 0.5;
            }
            
            let success_rate = perf.success_count as f64 / total_attempts as f64;
            let recency_bonus = match perf.last_success {
                Some(last) => {
                    let age = last.elapsed().as_secs() as f64;
                    (1.0 / (1.0 + age / 3600.0)) * 0.2
                }
                None => 0.0,
            };
            
            let response_time_penalty = (perf.avg_response_time.as_millis() as f64 / 1000.0) * 0.1;
            
            success_rate + recency_bonus - response_time_penalty
        } else {
            0.5
        }
    }

    pub fn update_mailbox_performance(&mut self, peer_id: PeerId, success: bool, response_time: Duration) {
        let perf = self.mailbox_performance.entry(peer_id).or_insert(MailboxPerformance {
            success_count: 0,
            failure_count: 0,
            last_success: None,
            last_failure: None,
            avg_response_time: Duration::from_millis(1000),
        });

        if success {
            perf.success_count += 1;
            perf.last_success = Some(std::time::Instant::now());
        } else {
            perf.failure_count += 1;
            perf.last_failure = Some(std::time::Instant::now());
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