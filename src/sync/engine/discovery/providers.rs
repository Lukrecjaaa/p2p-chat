//! This module contains helper functions for `SyncEngine` related to managing
//! and ranking mailbox providers.
use std::collections::HashSet;

use libp2p::PeerId;

use super::super::SyncEngine;

impl SyncEngine {
    /// Returns a reference to the set of discovered mailbox providers.
    pub fn get_mailbox_providers(&self) -> &HashSet<PeerId> {
        &self.discovered_mailboxes
    }

    /// Returns a ranked list of available mailbox providers.
    ///
    /// The ranking is based on performance metrics stored in the `SyncEngine`.
    pub fn get_available_mailboxes(&self) -> Vec<PeerId> {
        self.rank_mailboxes(self.discovered_mailboxes.iter().cloned())
    }

    /// Returns a ranked list of a subset of mailbox providers.
    ///
    /// # Arguments
    ///
    /// * `providers` - The subset of `PeerId`s to rank.
    pub fn rank_mailboxes_subset(&self, providers: &HashSet<PeerId>) -> Vec<PeerId> {
        self.rank_mailboxes(providers.iter().cloned())
    }

    /// Asynchronously retrieves a list of "emergency" mailboxes.
    ///
    /// These are connected peers that are also known mailbox providers.
    ///
    /// # Returns
    ///
    /// A `Vec` of `PeerId`s representing the emergency mailboxes.
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

    /// Determines if a mailbox should be forgotten due to poor performance.
    ///
    /// This is based on consecutive failures or too many failures within a time window.
    pub(crate) fn should_forget_mailbox(&self, peer_id: PeerId) -> bool {
        use super::super::performance::{
            FAILURE_WINDOW_SECONDS, MAX_CONSECUTIVE_FAILURES, MAX_FAILURES_IN_WINDOW,
        };

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
