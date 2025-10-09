use std::time::{Duration, Instant};

pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;
pub const MAX_FAILURES_IN_WINDOW: u32 = 5;
pub const FAILURE_WINDOW_SECONDS: u64 = 60; // 1 minute

#[derive(Debug, Clone)]
pub struct MailboxPerformance {
    pub success_count: u32,
    pub failure_count: u32,
    pub consecutive_failures: u32,
    pub last_success: Option<Instant>,
    pub last_failure: Option<Instant>,
    pub avg_response_time: Duration,
}

impl MailboxPerformance {
    pub fn new() -> Self {
        Self {
            success_count: 0,
            failure_count: 0,
            consecutive_failures: 0,
            last_success: None,
            last_failure: None,
            avg_response_time: Duration::from_millis(1000),
        }
    }
}
