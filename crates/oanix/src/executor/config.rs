//! Configuration types for executors.

use std::time::Duration;

/// Retry policy for network operations.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (0 = no retries)
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier (e.g., 2.0 for exponential backoff)
    pub backoff_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_factor: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Create a policy with no retries.
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 0,
            ..Default::default()
        }
    }

    /// Create a policy with fixed delay between retries.
    pub fn fixed(max_attempts: u32, delay: Duration) -> Self {
        Self {
            max_attempts,
            initial_delay: delay,
            max_delay: delay,
            backoff_factor: 1.0,
        }
    }

    /// Create a policy with exponential backoff.
    pub fn exponential(max_attempts: u32, initial: Duration, max: Duration) -> Self {
        Self {
            max_attempts,
            initial_delay: initial,
            max_delay: max,
            backoff_factor: 2.0,
        }
    }

    /// Calculate delay for a given attempt number (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return self.initial_delay;
        }

        let multiplier = self.backoff_factor.powi(attempt as i32);
        let delay_ms = (self.initial_delay.as_millis() as f64 * multiplier) as u64;
        let delay = Duration::from_millis(delay_ms);

        std::cmp::min(delay, self.max_delay)
    }
}

/// Configuration for all executors.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// HTTP request timeout
    pub http_timeout: Duration,
    /// HTTP retry policy
    pub http_retry: RetryPolicy,
    /// WebSocket connect timeout
    pub ws_connect_timeout: Duration,
    /// WebSocket ping interval (for keepalive)
    pub ws_ping_interval: Duration,
    /// WebSocket reconnect policy
    pub ws_reconnect: RetryPolicy,
    /// Nostr relay subscription timeout
    pub nostr_sub_timeout: Duration,
    /// Nostr relay reconnect policy
    pub nostr_reconnect: RetryPolicy,
    /// Polling interval for checking pending requests
    pub poll_interval: Duration,
    /// Maximum concurrent HTTP requests
    pub http_max_concurrent: usize,
    /// Maximum concurrent WebSocket connections
    pub ws_max_concurrent: usize,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            http_timeout: Duration::from_secs(30),
            http_retry: RetryPolicy::exponential(
                3,
                Duration::from_millis(100),
                Duration::from_secs(5),
            ),
            ws_connect_timeout: Duration::from_secs(10),
            ws_ping_interval: Duration::from_secs(30),
            ws_reconnect: RetryPolicy::exponential(
                5,
                Duration::from_secs(1),
                Duration::from_secs(60),
            ),
            nostr_sub_timeout: Duration::from_secs(30),
            nostr_reconnect: RetryPolicy::exponential(
                5,
                Duration::from_secs(1),
                Duration::from_secs(60),
            ),
            poll_interval: Duration::from_millis(50),
            http_max_concurrent: 10,
            ws_max_concurrent: 50,
        }
    }
}

impl ExecutorConfig {
    /// Create a new config builder.
    pub fn builder() -> ExecutorConfigBuilder {
        ExecutorConfigBuilder::default()
    }
}

/// Builder for ExecutorConfig.
#[derive(Default)]
pub struct ExecutorConfigBuilder {
    config: ExecutorConfig,
}

impl ExecutorConfigBuilder {
    /// Set HTTP request timeout.
    pub fn http_timeout(mut self, timeout: Duration) -> Self {
        self.config.http_timeout = timeout;
        self
    }

    /// Set HTTP retry policy.
    pub fn http_retry(mut self, policy: RetryPolicy) -> Self {
        self.config.http_retry = policy;
        self
    }

    /// Set WebSocket connect timeout.
    pub fn ws_connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.ws_connect_timeout = timeout;
        self
    }

    /// Set WebSocket ping interval.
    pub fn ws_ping_interval(mut self, interval: Duration) -> Self {
        self.config.ws_ping_interval = interval;
        self
    }

    /// Set WebSocket reconnect policy.
    pub fn ws_reconnect(mut self, policy: RetryPolicy) -> Self {
        self.config.ws_reconnect = policy;
        self
    }

    /// Set Nostr subscription timeout.
    pub fn nostr_sub_timeout(mut self, timeout: Duration) -> Self {
        self.config.nostr_sub_timeout = timeout;
        self
    }

    /// Set polling interval.
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.config.poll_interval = interval;
        self
    }

    /// Set maximum concurrent HTTP requests.
    pub fn http_max_concurrent(mut self, max: usize) -> Self {
        self.config.http_max_concurrent = max;
        self
    }

    /// Set maximum concurrent WebSocket connections.
    pub fn ws_max_concurrent(mut self, max: usize) -> Self {
        self.config.ws_max_concurrent = max;
        self
    }

    /// Build the config.
    pub fn build(self) -> ExecutorConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.backoff_factor, 2.0);
    }

    #[test]
    fn test_retry_policy_no_retry() {
        let policy = RetryPolicy::no_retry();
        assert_eq!(policy.max_attempts, 0);
    }

    #[test]
    fn test_retry_delay_calculation() {
        let policy =
            RetryPolicy::exponential(3, Duration::from_millis(100), Duration::from_secs(10));

        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(800));
    }

    #[test]
    fn test_retry_delay_respects_max() {
        let policy =
            RetryPolicy::exponential(10, Duration::from_millis(100), Duration::from_millis(500));

        // After a few attempts, should cap at max_delay
        assert_eq!(policy.delay_for_attempt(10), Duration::from_millis(500));
    }

    #[test]
    fn test_config_builder() {
        let config = ExecutorConfig::builder()
            .http_timeout(Duration::from_secs(60))
            .poll_interval(Duration::from_millis(100))
            .build();

        assert_eq!(config.http_timeout, Duration::from_secs(60));
        assert_eq!(config.poll_interval, Duration::from_millis(100));
    }

    #[test]
    fn test_default_config() {
        let config = ExecutorConfig::default();
        assert_eq!(config.http_timeout, Duration::from_secs(30));
        assert_eq!(config.ws_ping_interval, Duration::from_secs(30));
        assert_eq!(config.poll_interval, Duration::from_millis(50));
    }
}
