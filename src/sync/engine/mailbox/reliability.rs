use std::time::{Duration, Instant};

use libp2p::PeerId;
use tracing::warn;

use super::super::performance::MailboxPerformance;
use super::super::SyncEngine;

impl SyncEngine {
    pub(crate) fn update_mailbox_performance(
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
        } else {
            perf.failure_count += 1;
            perf.consecutive_failures += 1;
            perf.last_failure = Some(Instant::now());
            self.backoff_manager.record_failure(peer_id);
        }

        let new_weight = 0.3;
        let old_weight = 1.0 - new_weight;
        perf.avg_response_time = Duration::from_millis(
            ((perf.avg_response_time.as_millis() as f64 * old_weight)
                + (response_time.as_millis() as f64 * new_weight)) as u64,
        );
    }

    pub(crate) fn forget_failing_mailbox(&mut self, peer_id: PeerId) {
        if self.discovered_mailboxes.remove(&peer_id) {
            warn!(
                "Temporarily forgetting failing mailbox {} due to persistent failures",
                peer_id
            );
            self.backoff_manager.record_failure(peer_id);
        }
    }

    pub(crate) fn cleanup_failing_mailboxes(&mut self) {
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
}
