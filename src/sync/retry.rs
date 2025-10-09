use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;

pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            max_delay,
        }
    }

    /// Create a fast retry policy for responsive mailbox operations
    pub fn fast_mailbox() -> Self {
        Self::new(4, Duration::from_millis(50), Duration::from_millis(500))
    }

    pub fn exponential_backoff(&self, attempt: u32) -> Duration {
        let delay = self.base_delay.as_millis() as u64 * 2_u64.pow(attempt);
        Duration::from_millis(delay.min(self.max_delay.as_millis() as u64))
    }

    /// Add jitter to prevent thundering herd
    pub fn exponential_backoff_with_jitter(&self, attempt: u32) -> Duration {
        let base_delay = self.exponential_backoff(attempt);
        let jitter_ms = rand::random::<u64>() % (base_delay.as_millis() as u64 / 4 + 1);
        Duration::from_millis(base_delay.as_millis() as u64 + jitter_ms)
    }

    /// Retry with jitter to avoid thundering herd problems
    pub async fn retry_with_jitter<F, T, Fut>(&self, mut op: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        for attempt in 0..self.max_attempts {
            match op().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt + 1 >= self.max_attempts {
                        return Err(e);
                    }
                    let delay = self.exponential_backoff_with_jitter(attempt);
                    sleep(delay).await;
                }
            }
        }
        unreachable!()
    }
}
