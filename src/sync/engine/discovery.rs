use super::events::DhtQueryResult;
use super::performance::{
    FAILURE_WINDOW_SECONDS, MAX_CONSECUTIVE_FAILURES, MAX_FAILURES_IN_WINDOW,
};
use super::{DhtQueryState, SyncEngine};
use crate::crypto::StorageEncryption;
use crate::mailbox::{make_mailbox_provider_key, make_recipient_mailbox_key};
use anyhow::Result;
use libp2p::{kad, PeerId};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, trace};

impl SyncEngine {
    pub async fn discover_mailboxes(&mut self) -> Result<()> {
        self.discover_mailboxes_if_needed(false).await
    }

    pub async fn discover_mailboxes_if_needed(&mut self, force: bool) -> Result<()> {
        let current_mailbox_count = self.discovered_mailboxes.len();
        let available_mailbox_count = self.get_available_mailboxes().len();

        if !force {
            if available_mailbox_count >= 2 {
                trace!(
                    "Have {} available mailboxes, skipping discovery",
                    available_mailbox_count
                );
                return Ok(());
            }

            if let Some(last_discovery) = self.last_discovery_time {
                if last_discovery.elapsed() < Duration::from_secs(30) {
                    trace!(
                        "Last discovery was {:?} ago, skipping (rate limited)",
                        last_discovery.elapsed()
                    );
                    return Ok(());
                }
            }
        }

        trace!(
            "Discovering mailbox providers in DHT (currently have: {}, available: {}, force: {})",
            current_mailbox_count,
            available_mailbox_count,
            force
        );

        let Some(network) = &self.network else {
            debug!("No network handle available for mailbox discovery");
            return Ok(());
        };

        self.last_discovery_time = Some(Instant::now());

        let general_mailbox_key = make_mailbox_provider_key();
        if !self.has_pending_query_for(&general_mailbox_key) {
            match network
                .start_dht_provider_query(general_mailbox_key.clone())
                .await
            {
                Ok(query_id) => {
                    let query_state = DhtQueryState {
                        key: general_mailbox_key,
                        started_at: Instant::now(),
                        received_results: false,
                    };
                    self.pending_dht_queries.insert(query_id, query_state);
                    trace!(
                        "Started DHT query for general mailbox providers: {:?}",
                        query_id
                    );
                }
                Err(e) => {
                    error!("Failed to start DHT query for general mailboxes: {}", e);
                }
            }
        } else {
            trace!("Skipping DHT query for general mailbox providers; query already pending");
        }

        let our_recipient_hash =
            StorageEncryption::derive_recipient_hash(&self.identity.hpke_public_key());
        let recipient_mailbox_key = make_recipient_mailbox_key(our_recipient_hash);

        if !self.has_pending_query_for(&recipient_mailbox_key) {
            match network
                .start_dht_provider_query(recipient_mailbox_key.clone())
                .await
            {
                Ok(query_id) => {
                    let query_state = DhtQueryState {
                        key: recipient_mailbox_key,
                        started_at: Instant::now(),
                        received_results: false,
                    };
                    self.pending_dht_queries.insert(query_id, query_state);
                    trace!(
                        "Started DHT query for recipient-specific mailbox providers: {:?}",
                        query_id
                    );
                }
                Err(e) => {
                    trace!(
                        "Failed to start DHT query for recipient-specific mailboxes: {}",
                        e
                    );
                }
            }
        } else {
            trace!("Skipping DHT query for recipient-specific mailboxes; query already pending");
        }

        Ok(())
    }

    pub async fn handle_dht_query_result(
        &mut self,
        key: kad::RecordKey,
        result: DhtQueryResult,
    ) -> Result<()> {
        match result {
            DhtQueryResult::ProvidersFound {
                providers,
                finished,
            } => {
                let key_str = String::from_utf8_lossy(key.as_ref());
                if !providers.is_empty() {
                    info!(
                        "Found {} providers for key {} (finished: {})",
                        providers.len(),
                        key_str,
                        finished
                    );
                }

                let mut new_providers = 0;
                for provider in providers {
                    if !self.backoff_manager.can_attempt(&provider) {
                        if let Some(retry_time) = self.backoff_manager.time_until_retry(&provider) {
                            debug!(
                                "Skipping backed-off mailbox {} (retry in {:?})",
                                provider, retry_time
                            );
                        }
                        continue;
                    }

                    if self.discovered_mailboxes.insert(provider) {
                        new_providers += 1;
                        info!("Discovered new mailbox provider: {}", provider);
                        self.backoff_manager.record_success(&provider);
                    }
                }

                if new_providers > 0 {
                    info!(
                        "Added {} new mailbox provider(s) to the pool.",
                        new_providers
                    );

                    if let Err(e) = self.retry_outbox().await {
                        error!(
                            "Failed to retry outbox after discovering new mailboxes: {}",
                            e
                        );
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
        self.rank_mailboxes(self.discovered_mailboxes.iter().cloned())
    }

    pub fn rank_mailboxes_subset(&self, providers: &HashSet<PeerId>) -> Vec<PeerId> {
        self.rank_mailboxes(providers.iter().cloned())
    }

    fn rank_mailboxes<I>(&self, candidates: I) -> Vec<PeerId>
    where
        I: IntoIterator<Item = PeerId>,
    {
        let mut providers: Vec<_> = candidates
            .into_iter()
            .filter(|peer| self.backoff_manager.can_attempt(peer))
            .collect();

        providers.sort_by(|a, b| {
            let score_a = self.calculate_mailbox_score(*a);
            let score_b = self.calculate_mailbox_score(*b);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        providers
    }

    pub async fn get_emergency_mailboxes(&self) -> Vec<PeerId> {
        let Some(network) = &self.network else {
            return vec![];
        };

        match network.get_connected_peers().await {
            Ok(peers) => peers
                .into_iter()
                .filter(|peer| self.discovered_mailboxes.contains(peer))
                .collect(),
            Err(_) => vec![],
        }
    }

    pub(super) fn cleanup_stale_dht_queries(&mut self) {
        let stale_timeout = Duration::from_secs(60);
        let mut stale_queries = Vec::new();

        for (&query_id, query_state) in &self.pending_dht_queries {
            if query_state.started_at.elapsed() > stale_timeout {
                stale_queries.push(query_id);
            }
        }

        for query_id in stale_queries {
            if let Some(query_state) = self.pending_dht_queries.remove(&query_id) {
                trace!(
                    "Removing stale DHT query {:?} (age: {:?}, received_results: {})",
                    query_id,
                    query_state.started_at.elapsed(),
                    query_state.received_results
                );
            }
        }

        self.backoff_manager
            .cleanup_old_entries(Duration::from_secs(3600));
    }

    fn has_pending_query_for(&self, key: &kad::RecordKey) -> bool {
        self.pending_dht_queries
            .values()
            .any(|state| state.key == *key)
    }

    fn calculate_mailbox_score(&self, peer_id: PeerId) -> f64 {
        let mut score = 0.5;

        if let Some(perf) = self.mailbox_performance.get(&peer_id) {
            let total_attempts = perf.success_count + perf.failure_count;

            if total_attempts > 0 {
                let success_rate = perf.success_count as f64 / total_attempts as f64;
                score = success_rate * 0.7;

                if let Some(last_success) = perf.last_success {
                    let age_hours = last_success.elapsed().as_secs() as f64 / 3600.0;
                    let recency_bonus = (1.0 / (1.0 + age_hours)).min(0.3);
                    score += recency_bonus * 0.2;
                }

                let response_ms = perf.avg_response_time.as_millis() as f64;
                let speed_score = (3000.0 - response_ms.min(3000.0)) / 3000.0;
                score += speed_score * 0.1;

                let failure_penalty = (perf.consecutive_failures as f64 * 0.1).min(0.3);
                score -= failure_penalty;
            }
        }

        if !self.backoff_manager.can_attempt(&peer_id) {
            score *= 0.1;
        }

        score.max(0.0).min(1.0)
    }

    pub(super) fn should_forget_mailbox(&self, peer_id: PeerId) -> bool {
        if let Some(perf) = self.mailbox_performance.get(&peer_id) {
            if perf.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                return true;
            }

            if let Some(last_failure) = perf.last_failure {
                let time_since_last_failure = last_failure.elapsed().as_secs();
                if time_since_last_failure <= FAILURE_WINDOW_SECONDS
                    && perf.failure_count >= MAX_FAILURES_IN_WINDOW
                {
                    return true;
                }
            }
        }
        false
    }
}
