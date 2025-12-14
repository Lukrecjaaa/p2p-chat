//! This module contains logic for ranking mailbox providers based on their
//! perceived performance and reliability.
use std::cmp::Ordering;

use libp2p::PeerId;

use super::super::SyncEngine;

impl SyncEngine {
    /// Ranks a given set of candidate mailbox providers.
    ///
    /// Providers are filtered by whether they can be attempted (not backed off)
    /// and then sorted by their calculated score in descending order.
    ///
    /// # Arguments
    ///
    /// * `candidates` - An iterator over `PeerId`s of potential mailbox providers.
    ///
    /// # Returns
    ///
    /// A `Vec` of `PeerId`s, ranked from best to worst.
    pub(super) fn rank_mailboxes<I>(&self, candidates: I) -> Vec<PeerId>
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
            score_b.partial_cmp(&score_a).unwrap_or(Ordering::Equal)
        });

        providers
    }

    /// Calculates a performance score for a single mailbox provider.
    ///
    /// The score takes into account:
    /// - Success rate (70% weight)
    /// - Recency of last success (20% weight, with bonus for recent success)
    /// - Average response time (10% weight, bonus for faster responses)
    /// - Penalty for consecutive failures
    /// - A significant penalty if the peer is currently in backoff.
    ///
    /// The score is clamped between 0.0 and 1.0.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The `PeerId` of the mailbox provider.
    ///
    /// # Returns
    ///
    /// A `f64` representing the performance score of the mailbox.
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

        score.clamp(0.0, 1.0)
    }
}
