use crate::sync::engine::SyncEngine;
use libp2p::kad;
use std::time::Duration;
use tracing::trace;

impl SyncEngine {
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

    pub(crate) fn has_pending_query_for(&self, key: &kad::RecordKey) -> bool {
        self.pending_dht_queries
            .values()
            .any(|state| state.key == *key)
    }
}
