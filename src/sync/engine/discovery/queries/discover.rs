use crate::crypto::StorageEncryption;
use crate::mailbox::{make_mailbox_provider_key, make_recipient_mailbox_key};
use crate::sync::engine::{DhtQueryState, SyncEngine};
use anyhow::Result;
use std::time::{Duration, Instant};
use tracing::{debug, error, trace};

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
}
