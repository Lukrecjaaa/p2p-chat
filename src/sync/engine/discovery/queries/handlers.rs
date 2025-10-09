use crate::sync::engine::events::DhtQueryResult;
use crate::sync::engine::SyncEngine;
use anyhow::Result;
use libp2p::kad;
use tracing::{debug, error, info, trace};

impl SyncEngine {
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
}
