//! This module defines data structures and constants for tracking the performance
//! of mailbox providers.
use std::time::{Duration, Instant};

/// The maximum number of consecutive failures before a mailbox is considered unreliable.
pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;
/// The maximum number of failures within a specified time window before a mailbox is considered unreliable.
pub const MAX_FAILURES_IN_WINDOW: u32 = 5;
/// The duration (in seconds) for the failure window.
pub const FAILURE_WINDOW_SECONDS: u64 = 60; // 1 minute

/// Represents the performance metrics of a mailbox provider.
#[derive(Debug, Clone)]
pub struct MailboxPerformance {
    /// The total count of successful interactions.
    pub success_count: u32,
    /// The total count of failed interactions.
    pub failure_count: u32,
    /// The number of consecutive failed interactions.
    pub consecutive_failures: u32,
    /// The `Instant` of the last successful interaction.
    pub last_success: Option<Instant>,
    /// The `Instant` of the last failed interaction.
    pub last_failure: Option<Instant>,
    /// The exponentially-weighted moving average response time.
    pub avg_response_time: Duration,
}

impl MailboxPerformance {
    /// Creates a new `MailboxPerformance` instance with default values.
    pub fn new() -> Self {
        Self {
            success_count: 0,
            failure_count: 0,
            consecutive_failures: 0,
            last_success: None,
            last_failure: None,
            avg_response_time: Duration::from_millis(1000), // Default to 1 second
        }
    }
}
