use std::time::{Duration, Instant};
use std::collections::HashMap;
use libp2p::PeerId;
use rand::Rng;

const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(300); // 5 minutes max
const BACKOFF_MULTIPLIER: f64 = 2.0;
const JITTER_RANGE: f64 = 0.1; // 10% jitter

#[derive(Debug, Clone)]
pub struct BackoffEntry {
    pub attempt_count: u32,
    pub last_attempt: Instant,
    pub next_attempt_after: Duration,
}

impl BackoffEntry {
    pub fn new() -> Self {
        Self {
            attempt_count: 0,
            last_attempt: Instant::now(),
            next_attempt_after: MIN_BACKOFF,
        }
    }
    
    pub fn can_retry(&self) -> bool {
        self.last_attempt.elapsed() >= self.next_attempt_after
    }
    
    pub fn time_until_retry(&self) -> Duration {
        self.next_attempt_after.saturating_sub(self.last_attempt.elapsed())
    }
    
    pub fn record_attempt(&mut self) {
        self.attempt_count += 1;
        self.last_attempt = Instant::now();
        
        // Calculate next backoff with exponential growth
        let base_backoff = MIN_BACKOFF.as_secs_f64() * BACKOFF_MULTIPLIER.powi(self.attempt_count as i32 - 1);
        let clamped_backoff = base_backoff.min(MAX_BACKOFF.as_secs_f64());
        
        // Add jitter to prevent thundering herd
        let mut rng = rand::thread_rng();
        let jitter_factor = 1.0 + rng.gen_range(-JITTER_RANGE..JITTER_RANGE);
        let final_backoff = clamped_backoff * jitter_factor;
        
        self.next_attempt_after = Duration::from_secs_f64(final_backoff);
    }
    
    pub fn record_success(&mut self) {
        // Reset backoff on success
        self.attempt_count = 0;
        self.next_attempt_after = MIN_BACKOFF;
    }
    
    pub fn should_give_up(&self) -> bool {
        // Give up after 10 attempts or 5 minutes of backoff
        self.attempt_count >= 10 || self.next_attempt_after >= MAX_BACKOFF
    }
}

#[derive(Debug)]
pub struct BackoffManager {
    entries: HashMap<PeerId, BackoffEntry>,
}

impl BackoffManager {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
    
    pub fn can_attempt(&self, peer_id: &PeerId) -> bool {
        match self.entries.get(peer_id) {
            Some(entry) => entry.can_retry() && !entry.should_give_up(),
            None => true, // First attempt is always allowed
        }
    }
    
    pub fn time_until_retry(&self, peer_id: &PeerId) -> Option<Duration> {
        self.entries.get(peer_id).map(|entry| entry.time_until_retry())
    }
    
    pub fn record_attempt(&mut self, peer_id: PeerId) {
        let entry = self.entries.entry(peer_id).or_insert_with(BackoffEntry::new);
        entry.record_attempt();
    }
    
    pub fn record_success(&mut self, peer_id: &PeerId) {
        if let Some(entry) = self.entries.get_mut(peer_id) {
            entry.record_success();
        }
    }
    
    pub fn record_failure(&mut self, peer_id: PeerId) {
        // Same as record_attempt for now, but could be extended with different logic
        self.record_attempt(peer_id);
    }
    
    pub fn cleanup_old_entries(&mut self, max_age: Duration) {
        let cutoff = Instant::now() - max_age;
        self.entries.retain(|_, entry| entry.last_attempt >= cutoff);
    }
}


impl Default for BackoffManager {
    fn default() -> Self {
        Self::new()
    }
}