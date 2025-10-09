use std::time::{Duration, Instant};

use anyhow::Result;
use libp2p::kad;
use tracing::{debug, error, info, trace};

use crate::crypto::StorageEncryption;
use crate::mailbox::{make_mailbox_provider_key, make_recipient_mailbox_key};

use super::super::events::DhtQueryResult;
use super::super::{DhtQueryState, SyncEngine};

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

    pub(crate) fn cleanup_stale_dht_queries(&mut self) {
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
}
