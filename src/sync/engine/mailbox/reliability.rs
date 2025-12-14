//! This module contains logic for tracking and managing the reliability
//! of interactions with mailbox providers.
use std::time::{Duration, Instant};

use libp2p::PeerId;
use tracing::{error, warn};

use super::super::performance::MailboxPerformance;
use super::super::SyncEngine;

impl SyncEngine {
    /// Updates the performance metrics for a specific mailbox.
    ///
    /// Records whether an interaction was successful or a failure, updates
    /// success/failure counts, last seen timestamps, and average response time.
    /// It also interacts with the `BackoffManager` and persistent storage.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the mailbox.
    /// * `success` - `true` if the interaction was successful, `false` otherwise.
    /// * `response_time` - The duration of the interaction.
    pub(crate) async fn update_mailbox_performance(
        &mut self,
        peer_id: PeerId,
        success: bool,
        response_time: Duration,
    ) {
        let perf = self
            .mailbox_performance
            .entry(peer_id)
            .or_insert_with(MailboxPerformance::new);

        if success {
            perf.success_count += 1;
            perf.consecutive_failures = 0;
            perf.last_success = Some(Instant::now());
            self.backoff_manager.record_success(&peer_id);

            // Update database cache on success.
            if let Err(e) = self.known_mailboxes.increment_success(&peer_id).await {
                error!("Failed to update mailbox {} success in database: {}", peer_id, e);
            }
        } else {
            perf.failure_count += 1;
            perf.consecutive_failures += 1;
            perf.last_failure = Some(Instant::now());
            self.backoff_manager.record_failure(peer_id);

            // Update database cache on failure.
            if let Err(e) = self.known_mailboxes.increment_failure(&peer_id).await {
                error!("Failed to update mailbox {} failure in database: {}", peer_id, e);
            }
        }

        let new_weight = 0.3;
        let old_weight = 1.0 - new_weight;
        perf.avg_response_time = Duration::from_millis(
            ((perf.avg_response_time.as_millis() as f64 * old_weight)
                + (response_time.as_millis() as f64 * new_weight)) as u64,
        );
    }

    /// Temporarily forgets a failing mailbox.
    ///
    /// If a mailbox is persistently failing, it is removed from the list of
    /// discovered mailboxes and its failure is recorded in the `BackoffManager`
    /// and persistent storage.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the mailbox to forget.
    pub(crate) async fn forget_failing_mailbox(&mut self, peer_id: PeerId) {
        if self.discovered_mailboxes.remove(&peer_id) {
            warn!(
                "Temporarily forgetting failing mailbox {} due to persistent failures",
                peer_id
            );
            self.backoff_manager.record_failure(peer_id);

            // Remove from database cache.
            if let Err(e) = self.known_mailboxes.remove_mailbox(&peer_id).await {
                error!("Failed to remove mailbox {} from database: {}", peer_id, e);
            }
        }
    }

    /// Cleans up failing mailboxes.
    ///
    /// Iterates through all discovered mailboxes and calls `forget_failing_mailbox`
    /// for any that meet the criteria for being forgotten.
    pub(crate) async fn cleanup_failing_mailboxes(&mut self) {
        let mut mailboxes_to_forget = Vec::new();

        for peer_id in &self.discovered_mailboxes {
            if self.should_forget_mailbox(*peer_id) {
                mailboxes_to_forget.push(*peer_id);
            }
        }

        for peer_id in mailboxes_to_forget {
            self.forget_failing_mailbox(peer_id).await;
        }
    }
}
