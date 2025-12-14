//! This module provides a flexible retry mechanism with exponential backoff and jitter.
use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;

/// Defines a policy for retrying failed operations.
pub struct RetryPolicy {
    /// The maximum number of attempts to make.
    pub max_attempts: u32,
    /// The base delay between retries.
    pub base_delay: Duration,
    /// The maximum delay between retries.
    pub max_delay: Duration,
}

impl RetryPolicy {
    /// Creates a new `RetryPolicy`.
    ///
    /// # Arguments
    ///
    /// * `max_attempts` - The maximum number of attempts.
    /// * `base_delay` - The initial delay before the first retry.
    /// * `max_delay` - The maximum allowed delay between retries.
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            max_delay,
        }
    }

    /// Creates a fast retry policy suitable for responsive mailbox operations.
    pub fn fast_mailbox() -> Self {
        Self::new(4, Duration::from_millis(50), Duration::from_millis(500))
    }

    /// Calculates an exponential backoff delay based on the attempt number.
    ///
    /// The delay increases exponentially with each attempt, up to `max_delay`.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The current attempt number (0-indexed).
    pub fn exponential_backoff(&self, attempt: u32) -> Duration {
        let delay = self.base_delay.as_millis() as u64 * 2_u64.pow(attempt);
        Duration::from_millis(delay.min(self.max_delay.as_millis() as u64))
    }

    /// Calculates an exponential backoff delay with added jitter.
    ///
    /// Jitter helps to prevent "thundering herd" problems when many clients
    /// retry simultaneously.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The current attempt number (0-indexed).
    pub fn exponential_backoff_with_jitter(&self, attempt: u32) -> Duration {
        let base_delay = self.exponential_backoff(attempt);
        let jitter_ms = rand::random::<u64>() % (base_delay.as_millis() as u64 / 4 + 1);
        Duration::from_millis(base_delay.as_millis() as u64 + jitter_ms)
    }

    /// Retries an asynchronous operation using the defined retry policy.
    ///
    /// The operation `op` will be retried `max_attempts` times, with exponential
    /// backoff and jitter between attempts.
    ///
    /// # Arguments
    ///
    /// * `op` - A closure that returns a `Future` representing the operation to retry.
    ///
    /// # Returns
    ///
    /// The `Result` of the operation if successful, or the last error encountered.
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
        unreachable!() // This should not be reachable as either Ok is returned or Err is returned after max_attempts
    }
}
