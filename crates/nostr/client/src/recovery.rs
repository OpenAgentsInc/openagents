//! Network error recovery and resilience patterns for relay connections
//!
//! Provides circuit breaker pattern, exponential backoff with jitter, health checks,
//! and graceful degradation for robust production deployments.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected immediately
    Open,
    /// Circuit is half-open, allowing test requests
    HalfOpen,
}

/// Circuit breaker for fault tolerance
///
/// Prevents cascading failures by stopping requests to failing services.
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Current state
    state: Arc<RwLock<CircuitState>>,
    /// Failure count in current window
    failures: Arc<RwLock<u32>>,
    /// Success count in current window
    successes: Arc<RwLock<u32>>,
    /// Failure threshold before opening circuit
    failure_threshold: u32,
    /// Success threshold before closing circuit (when half-open)
    success_threshold: u32,
    /// Time to wait before attempting half-open
    timeout: Duration,
    /// Last state change timestamp
    last_change: Arc<RwLock<Instant>>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, 2, Duration::from_secs(30))
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    ///
    /// # Arguments
    /// * `failure_threshold` - Number of failures before opening (default: 5)
    /// * `success_threshold` - Number of successes to close when half-open (default: 2)
    /// * `timeout` - Time to wait before half-open attempt (default: 30s)
    pub fn new(failure_threshold: u32, success_threshold: u32, timeout: Duration) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failures: Arc::new(RwLock::new(0)),
            successes: Arc::new(RwLock::new(0)),
            failure_threshold,
            success_threshold,
            timeout,
            last_change: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Get current circuit state
    pub async fn state(&self) -> CircuitState {
        let state = *self.state.read().await;

        // Check if open circuit should transition to half-open
        if state == CircuitState::Open {
            let elapsed = self.last_change.read().await.elapsed();
            if elapsed >= self.timeout {
                return CircuitState::HalfOpen;
            }
        }

        state
    }

    /// Check if a request is allowed
    pub async fn is_allowed(&self) -> bool {
        let state = self.state().await;

        // Update state if timeout expired
        if state == CircuitState::HalfOpen {
            let mut current_state = self.state.write().await;
            if *current_state == CircuitState::Open {
                *current_state = CircuitState::HalfOpen;
                *self.last_change.write().await = Instant::now();
            }
        }

        matches!(state, CircuitState::Closed | CircuitState::HalfOpen)
    }

    /// Record a successful request
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => {
                // Reset failure count on success
                *self.failures.write().await = 0;
            }
            CircuitState::HalfOpen => {
                let mut successes = self.successes.write().await;
                *successes += 1;

                // Transition to closed if threshold met
                if *successes >= self.success_threshold {
                    *state = CircuitState::Closed;
                    *successes = 0;
                    *self.failures.write().await = 0;
                    *self.last_change.write().await = Instant::now();
                }
            }
            CircuitState::Open => {
                // Should not happen, but reset if it does
                *state = CircuitState::Closed;
                *self.failures.write().await = 0;
                *self.successes.write().await = 0;
            }
        }
    }

    /// Record a failed request
    pub async fn record_failure(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => {
                let mut failures = self.failures.write().await;
                *failures += 1;

                // Open circuit if threshold met
                if *failures >= self.failure_threshold {
                    *state = CircuitState::Open;
                    *self.last_change.write().await = Instant::now();
                }
            }
            CircuitState::HalfOpen => {
                // Failure in half-open means back to open
                *state = CircuitState::Open;
                *self.successes.write().await = 0;
                *self.last_change.write().await = Instant::now();
            }
            CircuitState::Open => {
                // Already open, no action needed
            }
        }
    }

    /// Reset the circuit breaker to closed state
    pub async fn reset(&self) {
        *self.state.write().await = CircuitState::Closed;
        *self.failures.write().await = 0;
        *self.successes.write().await = 0;
        *self.last_change.write().await = Instant::now();
    }

    /// Get failure count
    pub async fn failure_count(&self) -> u32 {
        *self.failures.read().await
    }
}

pub use openagents_utils::backoff::ExponentialBackoff;

/// Health check metrics for a relay connection
#[derive(Debug, Clone)]
pub struct HealthMetrics {
    /// Connection URL
    pub url: String,
    /// Last successful ping timestamp
    pub last_ping: Option<Instant>,
    /// Ping round-trip time (ms)
    pub ping_rtt_ms: Option<u32>,
    /// Total successful messages
    pub successful_messages: u64,
    /// Total failed messages
    pub failed_messages: u64,
    /// Circuit breaker state
    pub circuit_state: CircuitState,
    /// Current backoff attempt
    pub backoff_attempt: u32,
    /// Connection uptime duration
    pub uptime: Option<Duration>,
    /// Last error message
    pub last_error: Option<String>,
}

impl HealthMetrics {
    /// Create new health metrics
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            last_ping: None,
            ping_rtt_ms: None,
            successful_messages: 0,
            failed_messages: 0,
            circuit_state: CircuitState::Closed,
            backoff_attempt: 0,
            uptime: None,
            last_error: None,
        }
    }

    /// Calculate success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f32 {
        let total = self.successful_messages + self.failed_messages;
        if total == 0 {
            return 1.0;
        }
        self.successful_messages as f32 / total as f32
    }

    /// Check if connection is healthy
    ///
    /// Healthy = success rate > 80% AND (last ping < 60s OR no pings yet)
    pub fn is_healthy(&self) -> bool {
        let rate_ok = self.success_rate() > 0.8;
        let ping_ok = self
            .last_ping
            .map(|t| t.elapsed() < Duration::from_secs(60))
            .unwrap_or(true); // No ping yet is OK

        rate_ok && ping_ok && self.circuit_state == CircuitState::Closed
    }

    /// Get health status as string
    pub fn status(&self) -> &'static str {
        if self.is_healthy() {
            "healthy"
        } else if self.circuit_state == CircuitState::Open {
            "circuit_open"
        } else if self.success_rate() < 0.5 {
            "degraded"
        } else {
            "unhealthy"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let cb = CircuitBreaker::new(3, 2, Duration::from_millis(100));

        // Should be closed initially
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.is_allowed().await);

        // Record failures
        cb.record_failure().await;
        assert_eq!(cb.failure_count().await, 1);
        cb.record_failure().await;
        assert_eq!(cb.failure_count().await, 2);
        cb.record_failure().await;

        // Should be open now
        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(!cb.is_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_open_to_half_open() {
        let cb = CircuitBreaker::new(2, 2, Duration::from_millis(50));

        // Trigger open
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(60)).await;

        // Should transition to half-open
        assert_eq!(cb.state().await, CircuitState::HalfOpen);
        assert!(cb.is_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_to_closed() {
        let cb = CircuitBreaker::new(2, 2, Duration::from_millis(50));

        // Get to half-open state
        cb.record_failure().await;
        cb.record_failure().await;
        sleep(Duration::from_millis(60)).await;
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // Record successes
        cb.record_success().await;
        cb.record_success().await;

        // Should be closed now
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let cb = CircuitBreaker::new(2, 2, Duration::from_millis(100));

        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        cb.reset().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert_eq!(cb.failure_count().await, 0);
    }

    #[test]
    fn test_exponential_backoff_progression() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(10), 0);

        // First delay should be in range [0, 100ms]
        let delay1 = backoff.next_delay().unwrap();
        assert!(delay1.as_millis() <= 100);

        // Second delay should be in range [0, 200ms]
        let delay2 = backoff.next_delay().unwrap();
        assert!(delay2.as_millis() <= 200);

        // Third delay should be in range [0, 400ms]
        let delay3 = backoff.next_delay().unwrap();
        assert!(delay3.as_millis() <= 400);
    }

    #[test]
    fn test_exponential_backoff_max_attempts() {
        let mut backoff = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(10),
            3, // Max 3 attempts
        );

        assert!(backoff.next_delay().is_some());
        assert!(backoff.next_delay().is_some());
        assert!(backoff.next_delay().is_some());
        assert!(backoff.next_delay().is_none()); // Exhausted
        assert!(backoff.is_exhausted());
    }

    #[test]
    fn test_exponential_backoff_cap() {
        let mut backoff = ExponentialBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(5), // Max 5 seconds
            0,
        );

        // Even after many attempts, should cap at max_delay
        for _ in 0..10 {
            let delay = backoff.next_delay().unwrap();
            assert!(delay.as_millis() <= 5000);
        }
    }

    #[test]
    fn test_exponential_backoff_reset() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(10), 0);

        backoff.next_delay();
        backoff.next_delay();
        assert_eq!(backoff.attempt(), 2);

        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
    }

    #[test]
    fn test_health_metrics_success_rate() {
        let mut metrics = HealthMetrics::new("wss://relay.example.com");

        // Empty state
        assert_eq!(metrics.success_rate(), 1.0);

        // Add some data
        metrics.successful_messages = 8;
        metrics.failed_messages = 2;
        assert!((metrics.success_rate() - 0.8).abs() < 0.01);

        // All failures
        metrics.successful_messages = 0;
        metrics.failed_messages = 10;
        assert_eq!(metrics.success_rate(), 0.0);
    }

    #[test]
    fn test_health_metrics_is_healthy() {
        let mut metrics = HealthMetrics::new("wss://relay.example.com");

        // Healthy: high success rate, no ping yet
        metrics.successful_messages = 90;
        metrics.failed_messages = 10;
        assert!(metrics.is_healthy());

        // Unhealthy: low success rate
        metrics.successful_messages = 50;
        metrics.failed_messages = 50;
        assert!(!metrics.is_healthy());

        // Unhealthy: circuit open
        metrics.successful_messages = 90;
        metrics.failed_messages = 10;
        metrics.circuit_state = CircuitState::Open;
        assert!(!metrics.is_healthy());
    }

    #[test]
    fn test_health_metrics_status() {
        let mut metrics = HealthMetrics::new("wss://relay.example.com");

        metrics.successful_messages = 95;
        metrics.failed_messages = 5;
        assert_eq!(metrics.status(), "healthy");

        metrics.circuit_state = CircuitState::Open;
        assert_eq!(metrics.status(), "circuit_open");

        metrics.circuit_state = CircuitState::Closed;
        metrics.successful_messages = 30;
        metrics.failed_messages = 70;
        assert_eq!(metrics.status(), "degraded");
    }
}
