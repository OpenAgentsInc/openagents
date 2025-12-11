//! Test helpers and utilities for executor E2E tests

use oanix::executor::{ExecutorConfig, ExecutorManager, RetryPolicy};
use oanix::services::{HttpFs, RequestState, WsFs, WsState};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "nostr")]
use oanix::services::NostrFs;

/// Test fixture with pre-configured executor and services
///
/// IMPORTANT: This fixture creates its own tokio runtime via ExecutorManager.
/// Tests using this fixture should NOT use `#[tokio::test]` - instead use `#[test]`
/// and call `fixture.block_on(async { ... })` for async operations.
pub struct ExecutorTestFixture {
    pub http_fs: Arc<HttpFs>,
    pub ws_fs: Arc<WsFs>,
    #[cfg(feature = "nostr")]
    pub nostr_fs: Arc<NostrFs>,
    pub executor: ExecutorManager,
}

impl ExecutorTestFixture {
    /// Create a new test fixture with the given configuration
    pub fn new(config: ExecutorConfig) -> Self {
        let http_fs = Arc::new(HttpFs::new());
        let ws_fs = Arc::new(WsFs::new());

        #[cfg(feature = "nostr")]
        let nostr_fs = Arc::new(NostrFs::generate().expect("Failed to generate NostrFs"));

        let mut executor = ExecutorManager::new(config).expect("Failed to create ExecutorManager");
        executor.attach_http(Arc::clone(&http_fs));
        executor.attach_ws(Arc::clone(&ws_fs));

        #[cfg(feature = "nostr")]
        executor.attach_nostr(Arc::clone(&nostr_fs));

        Self {
            http_fs,
            ws_fs,
            #[cfg(feature = "nostr")]
            nostr_fs,
            executor,
        }
    }

    /// Start the executor
    pub fn start(&mut self) -> Result<(), oanix::executor::ExecutorError> {
        self.executor.start()
    }

    /// Shutdown the executor (consumes self)
    pub fn shutdown(self) -> Result<(), oanix::executor::ExecutorError> {
        self.executor.shutdown()
    }

    /// Run an async block on the executor's runtime
    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.executor.block_on(future)
    }
}

/// Configuration presets for different test scenarios

/// Fast test config with short timeouts and no retries
pub fn fast_test_config() -> ExecutorConfig {
    ExecutorConfig::builder()
        .poll_interval(Duration::from_millis(10))
        .http_timeout(Duration::from_secs(5))
        .http_retry(RetryPolicy::no_retry())
        .ws_connect_timeout(Duration::from_secs(2))
        .ws_ping_interval(Duration::from_secs(5))
        .build()
}

/// Config for testing retry behavior
pub fn retry_test_config() -> ExecutorConfig {
    ExecutorConfig::builder()
        .poll_interval(Duration::from_millis(10))
        .http_timeout(Duration::from_secs(5))
        .http_retry(RetryPolicy::fixed(3, Duration::from_millis(50)))
        .ws_connect_timeout(Duration::from_secs(2))
        .build()
}

/// Config with very short timeout for timeout testing
pub fn timeout_test_config() -> ExecutorConfig {
    ExecutorConfig::builder()
        .poll_interval(Duration::from_millis(10))
        .http_timeout(Duration::from_millis(100))
        .http_retry(RetryPolicy::no_retry())
        .ws_connect_timeout(Duration::from_millis(500))
        .build()
}

/// Wait for a condition to become true, with timeout
pub async fn wait_for<F>(timeout: Duration, mut condition: F) -> bool
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

/// Wait for an HTTP response to appear in HttpFs
pub async fn wait_for_response(http_fs: &HttpFs, request_id: &str, timeout: Duration) -> bool {
    wait_for(timeout, || http_fs.get_response(request_id).is_some()).await
}

/// Wait for an HTTP request to fail
pub async fn wait_for_failure(http_fs: &HttpFs, request_id: &str, timeout: Duration) -> bool {
    wait_for(timeout, || {
        http_fs.get_state(request_id) == Some(RequestState::Failed)
    })
    .await
}

/// Wait for an HTTP request to complete (success or failure)
pub async fn wait_for_completion(http_fs: &HttpFs, request_id: &str, timeout: Duration) -> bool {
    wait_for(timeout, || {
        matches!(
            http_fs.get_state(request_id),
            Some(RequestState::Completed) | Some(RequestState::Failed)
        )
    })
    .await
}

/// Wait for a WebSocket connection to reach a specific state
pub async fn wait_for_ws_state(
    ws_fs: &WsFs,
    conn_id: &str,
    state: WsState,
    timeout: Duration,
) -> bool {
    wait_for(timeout, || {
        ws_fs
            .get_connection(conn_id)
            .map(|info| info.state == state)
            .unwrap_or(false)
    })
    .await
}

/// Wait for a WebSocket message to be available
pub async fn wait_for_ws_message(ws_fs: &WsFs, conn_id: &str, timeout: Duration) -> bool {
    wait_for(timeout, || {
        ws_fs
            .get_connection(conn_id)
            .map(|info| info.inbox_count > 0)
            .unwrap_or(false)
    })
    .await
}

#[cfg(feature = "nostr")]
/// Wait for a Nostr event to be sent to at least one relay
pub async fn wait_for_nostr_sent(nostr_fs: &NostrFs, event_id: &str, timeout: Duration) -> bool {
    wait_for(timeout, || !nostr_fs.sent_to(event_id).is_empty()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_config_has_short_timeouts() {
        let config = fast_test_config();
        assert!(config.http_timeout < Duration::from_secs(10));
        assert!(config.poll_interval < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_wait_for_returns_true_when_condition_met() {
        let result = wait_for(Duration::from_secs(1), || true).await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_wait_for_returns_false_on_timeout() {
        let result = wait_for(Duration::from_millis(50), || false).await;
        assert!(!result);
    }
}
