//! Retry logic with exponential backoff for Nostr operations

use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier (exponential growth factor)
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

/// Retry a fallible async operation with exponential backoff
pub async fn retry_with_backoff<F, Fut, T, E>(
    config: &RetryConfig,
    operation_name: &str,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempts = 0;
    let mut backoff = config.initial_backoff;

    loop {
        attempts += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if attempts >= config.max_attempts {
                    warn!(
                        "{} failed after {} attempts: {}",
                        operation_name, attempts, err
                    );
                    return Err(err);
                }

                warn!(
                    "{} attempt {}/{} failed: {}. Retrying in {:?}...",
                    operation_name, attempts, config.max_attempts, err, backoff
                );

                sleep(backoff).await;

                // Exponential backoff with cap
                backoff = Duration::from_secs_f64(
                    (backoff.as_secs_f64() * config.backoff_multiplier)
                        .min(config.max_backoff.as_secs_f64()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        use std::sync::atomic::{AtomicU32, Ordering};

        let attempt = AtomicU32::new(0);
        let config = RetryConfig::default();

        let result = retry_with_backoff(&config, "test_op", || {
            let current = attempt.fetch_add(1, Ordering::SeqCst) + 1;
            async move {
                if current == 1 {
                    Err("first attempt fails")
                } else {
                    Ok("success")
                }
            }
        })
        .await;

        assert_eq!(result, Ok("success"));
        assert_eq!(attempt.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_fails_after_max_attempts() {
        let config = RetryConfig {
            max_attempts: 2,
            initial_backoff: Duration::from_millis(10),
            ..Default::default()
        };

        let result = retry_with_backoff(&config, "test_op", || async {
            Err::<(), _>("always fails")
        })
        .await;

        assert_eq!(result, Err("always fails"));
    }
}
