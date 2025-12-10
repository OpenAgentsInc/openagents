//! Retry logic with exponential backoff for LLM requests

use crate::{LlmError, LlmResult};
use std::future::Future;
use std::time::Duration;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay between retries (in milliseconds)
    pub base_delay_ms: u64,
    /// Maximum delay between retries (in milliseconds)
    pub max_delay_ms: u64,
    /// Jitter factor (0.0 - 1.0) to add randomness to delays
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            jitter_factor: 0.1,
        }
    }
}

impl RetryConfig {
    /// Create a new retry config
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            ..Default::default()
        }
    }

    /// Load configuration from environment variables
    ///
    /// Reads:
    /// - `LLM_RETRY_ATTEMPTS` (default: 3)
    /// - `LLM_RETRY_BASE_MS` (default: 1000)
    /// - `LLM_RETRY_MAX_MS` (default: 30000)
    pub fn from_env() -> Self {
        let max_attempts = std::env::var("LLM_RETRY_ATTEMPTS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);

        let base_delay_ms = std::env::var("LLM_RETRY_BASE_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);

        let max_delay_ms = std::env::var("LLM_RETRY_MAX_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30000);

        Self {
            max_attempts,
            base_delay_ms,
            max_delay_ms,
            ..Default::default()
        }
    }

    /// Set base delay
    pub fn base_delay(mut self, ms: u64) -> Self {
        self.base_delay_ms = ms;
        self
    }

    /// Set max delay
    pub fn max_delay(mut self, ms: u64) -> Self {
        self.max_delay_ms = ms;
        self
    }

    /// Set jitter factor
    pub fn jitter(mut self, factor: f64) -> Self {
        self.jitter_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Calculate delay for a given attempt (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        // Exponential backoff: base * 2^attempt
        let base_delay = self.base_delay_ms as f64 * 2.0_f64.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay_ms as f64);

        // Add jitter: delay * (1 - jitter/2 + random * jitter)
        // For determinism in non-random contexts, use a simple pattern
        let jitter_amount = if self.jitter_factor > 0.0 {
            let jitter_range = capped_delay * self.jitter_factor;
            // Simple pseudo-random based on attempt number
            let pseudo_random = ((attempt as f64 * 0.618033988749895) % 1.0) - 0.5;
            jitter_range * pseudo_random
        } else {
            0.0
        };

        Duration::from_millis((capped_delay + jitter_amount).max(0.0) as u64)
    }
}

/// Determines if an error is retryable
pub fn is_retryable_error(error: &LlmError) -> bool {
    match error {
        // Rate limits - always retry
        LlmError::RateLimitError(_) => true,

        // Timeouts - network issue, retry
        LlmError::TimeoutError(_) => true,

        // Network errors - usually transient
        LlmError::NetworkError(msg) => {
            // Check for specific non-retryable network errors
            !msg.contains("name resolution") && !msg.contains("invalid URL")
        }

        // Stream errors - might be recoverable
        LlmError::StreamError(_) => true,

        // Provider errors - check for specific retryable conditions
        LlmError::ProviderError { message, .. } => {
            message.contains("overloaded")
                || message.contains("capacity")
                || message.contains("temporarily")
                || message.contains("503")
                || message.contains("502")
                || message.contains("500")
        }

        // Authentication - never retry (won't help)
        LlmError::AuthenticationError(_) => false,

        // Invalid request - won't help to retry
        LlmError::InvalidRequest(_) => false,

        // Model not found - won't help to retry
        LlmError::ModelNotFound(_) => false,

        // Context length - won't help to retry
        LlmError::ContextLengthExceeded(_) => false,

        // Content filtered - won't help to retry
        LlmError::ContentFiltered(_) => false,

        // Tool errors - application-level, don't retry
        LlmError::ToolError(_) => false,

        // Serialization - won't help to retry
        LlmError::SerializationError(_) => false,

        // Configuration - won't help to retry
        LlmError::ConfigurationError(_) => false,

        // Unknown - be conservative, don't retry
        LlmError::Unknown(_) => false,
    }
}

/// Retry an async operation with exponential backoff
///
/// # Example
///
/// ```ignore
/// use llm::{retry_with_backoff, RetryConfig, is_retryable_error};
///
/// let result = retry_with_backoff(
///     || async { client.chat(&messages, None).await },
///     RetryConfig::default(),
///     is_retryable_error,
/// ).await;
/// ```
pub async fn retry_with_backoff<T, F, Fut>(
    operation: F,
    config: RetryConfig,
    should_retry: fn(&LlmError) -> bool,
) -> LlmResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = LlmResult<T>>,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                // Check if we should retry
                if !should_retry(&error) || attempt + 1 >= config.max_attempts {
                    return Err(error);
                }

                // Log retry attempt (in practice, this would use tracing)
                tracing::warn!(
                    attempt = attempt + 1,
                    max_attempts = config.max_attempts,
                    error = %error,
                    "Retrying LLM request after error"
                );

                // Wait before retrying
                let delay = config.delay_for_attempt(attempt);
                tokio::time::sleep(delay).await;

                last_error = Some(error);
            }
        }
    }

    // Should not reach here, but handle it anyway
    Err(last_error.unwrap_or_else(|| LlmError::Unknown("Retry exhausted".to_string())))
}

/// A wrapper that adds retry capability to any LlmProvider
pub struct RetryingProvider<P> {
    inner: P,
    config: RetryConfig,
}

impl<P> RetryingProvider<P> {
    /// Wrap a provider with retry logic
    pub fn new(inner: P, config: RetryConfig) -> Self {
        Self { inner, config }
    }

    /// Get the retry configuration
    pub fn config(&self) -> &RetryConfig {
        &self.config
    }

    /// Get the inner provider
    pub fn inner(&self) -> &P {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
    }

    #[test]
    fn test_delay_calculation() {
        let config = RetryConfig::default().jitter(0.0); // No jitter for deterministic test

        // First attempt: 1000ms
        let delay0 = config.delay_for_attempt(0);
        assert_eq!(delay0, Duration::from_millis(1000));

        // Second attempt: 2000ms
        let delay1 = config.delay_for_attempt(1);
        assert_eq!(delay1, Duration::from_millis(2000));

        // Third attempt: 4000ms
        let delay2 = config.delay_for_attempt(2);
        assert_eq!(delay2, Duration::from_millis(4000));

        // Should cap at max_delay
        let delay10 = config.delay_for_attempt(10);
        assert_eq!(delay10, Duration::from_millis(30000));
    }

    #[test]
    fn test_is_retryable_error() {
        // Retryable errors
        assert!(is_retryable_error(&LlmError::RateLimitError(
            "too many requests".to_string()
        )));
        assert!(is_retryable_error(&LlmError::TimeoutError(
            "request timed out".to_string()
        )));
        assert!(is_retryable_error(&LlmError::NetworkError(
            "connection reset".to_string()
        )));

        // Non-retryable errors
        assert!(!is_retryable_error(&LlmError::AuthenticationError(
            "invalid key".to_string()
        )));
        assert!(!is_retryable_error(&LlmError::InvalidRequest(
            "bad params".to_string()
        )));
        assert!(!is_retryable_error(&LlmError::ModelNotFound(
            "gpt-5".to_string()
        )));
        assert!(!is_retryable_error(&LlmError::ContextLengthExceeded(
            "too long".to_string()
        )));
    }

    #[test]
    fn test_provider_error_retryable() {
        // Retryable provider errors
        assert!(is_retryable_error(&LlmError::ProviderError {
            provider: "openai".to_string(),
            message: "Server overloaded".to_string(),
        }));
        assert!(is_retryable_error(&LlmError::ProviderError {
            provider: "anthropic".to_string(),
            message: "503 Service Unavailable".to_string(),
        }));

        // Non-retryable provider errors
        assert!(!is_retryable_error(&LlmError::ProviderError {
            provider: "openai".to_string(),
            message: "Invalid model specified".to_string(),
        }));
    }

    #[tokio::test]
    async fn test_retry_success_first_try() {
        let config = RetryConfig::new(3);
        let result = retry_with_backoff(
            || async { Ok::<_, LlmError>(42) },
            config,
            is_retryable_error,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_gives_up_on_non_retryable() {
        let config = RetryConfig::new(3);
        let result = retry_with_backoff(
            || async { Err::<i32, _>(LlmError::AuthenticationError("bad key".to_string())) },
            config,
            is_retryable_error,
        )
        .await;

        assert!(matches!(result, Err(LlmError::AuthenticationError(_))));
    }
}
