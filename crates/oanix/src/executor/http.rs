//! HTTP executor that processes HttpFs pending requests.

use crate::services::{HttpFs, HttpMethod, HttpRequest, HttpResponse};
use crate::executor::{ExecutorConfig, ExecutorError};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::{sleep, timeout};

/// HTTP executor that polls HttpFs for pending requests and executes them.
///
/// The executor runs in a loop, periodically checking for pending requests
/// in the attached HttpFs. When requests are found, they are executed using
/// reqwest with configurable timeouts and retry policies.
pub struct HttpExecutor {
    /// The HttpFs to poll for pending requests
    http_fs: Arc<HttpFs>,
    /// HTTP client for making requests
    client: Client,
    /// Configuration
    config: ExecutorConfig,
    /// Shutdown signal receiver
    shutdown_rx: broadcast::Receiver<()>,
}

impl HttpExecutor {
    /// Create a new HTTP executor.
    pub fn new(
        http_fs: Arc<HttpFs>,
        config: ExecutorConfig,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        let client = Client::builder()
            .timeout(config.http_timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http_fs,
            client,
            config,
            shutdown_rx,
        }
    }

    /// Run the executor loop.
    ///
    /// This will poll the HttpFs for pending requests and execute them
    /// until a shutdown signal is received.
    pub async fn run(mut self) {
        tracing::info!("HttpExecutor started");

        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    tracing::info!("HttpExecutor shutting down");
                    break;
                }
                _ = sleep(self.config.poll_interval) => {
                    self.process_pending().await;
                }
            }
        }

        tracing::info!("HttpExecutor stopped");
    }

    /// Process all pending requests.
    async fn process_pending(&self) {
        let pending_ids = self.http_fs.list_pending();

        for req_id in pending_ids {
            if let Some(request) = self.http_fs.take_pending(&req_id) {
                self.execute_with_retry(request).await;
            }
        }
    }

    /// Execute a request with retry logic.
    async fn execute_with_retry(&self, request: HttpRequest) {
        let req_id = request.id.clone();
        let policy = &self.config.http_retry;

        for attempt in 0..=policy.max_attempts {
            match self.execute_request(&request).await {
                Ok(response) => {
                    self.http_fs.complete_request(response);
                    return;
                }
                Err(e) => {
                    if attempt < policy.max_attempts && Self::is_retryable(&e) {
                        let delay = policy.delay_for_attempt(attempt);
                        tracing::warn!(
                            "HTTP request {} failed (attempt {}/{}): {}, retrying in {:?}",
                            req_id,
                            attempt + 1,
                            policy.max_attempts + 1,
                            e,
                            delay
                        );
                        sleep(delay).await;
                    } else {
                        tracing::error!("HTTP request {} failed: {}", req_id, e);
                        self.http_fs.fail_request(&req_id, e.to_string());
                        return;
                    }
                }
            }
        }
    }

    /// Execute a single HTTP request.
    async fn execute_request(&self, request: &HttpRequest) -> Result<HttpResponse, ExecutorError> {
        let start = std::time::Instant::now();

        // Build the request
        let mut req_builder = match request.method {
            HttpMethod::Get => self.client.get(&request.url),
            HttpMethod::Post => self.client.post(&request.url),
            HttpMethod::Put => self.client.put(&request.url),
            HttpMethod::Patch => self.client.patch(&request.url),
            HttpMethod::Delete => self.client.delete(&request.url),
            HttpMethod::Head => self.client.head(&request.url),
            HttpMethod::Options => self.client.request(reqwest::Method::OPTIONS, &request.url),
        };

        // Add headers
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        // Add body if present
        if let Some(body) = &request.body {
            req_builder = req_builder.body(body.clone());
        }

        // Apply request-specific timeout if set
        let request_timeout = request
            .timeout_secs
            .map(Duration::from_secs)
            .unwrap_or(self.config.http_timeout);

        // Execute with timeout
        let response = timeout(request_timeout, req_builder.send())
            .await
            .map_err(|_| {
                ExecutorError::Timeout(format!(
                    "Request timed out after {:?}",
                    request_timeout
                ))
            })?
            .map_err(ExecutorError::from)?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let status = response.status().as_u16();
        let status_text = response
            .status()
            .canonical_reason()
            .unwrap_or("Unknown")
            .to_string();

        // Collect response headers
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();

        // Read body
        let body = response.text().await.map_err(ExecutorError::from)?;

        let completed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(HttpResponse {
            request_id: request.id.clone(),
            status,
            status_text,
            headers,
            body,
            duration_ms,
            completed_at,
        })
    }

    /// Check if an error is retryable.
    fn is_retryable(error: &ExecutorError) -> bool {
        matches!(
            error,
            ExecutorError::Timeout(_) | ExecutorError::Connection(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_executor_creation() {
        let http_fs = Arc::new(HttpFs::new());
        let config = ExecutorConfig::default();
        let (tx, rx) = broadcast::channel(1);

        let _executor = HttpExecutor::new(http_fs, config, rx);
        // Executor was created successfully

        drop(tx);
    }

    #[tokio::test]
    async fn test_is_retryable() {
        assert!(HttpExecutor::is_retryable(&ExecutorError::Timeout(
            "test".to_string()
        )));
        assert!(HttpExecutor::is_retryable(&ExecutorError::Connection(
            "test".to_string()
        )));
        assert!(!HttpExecutor::is_retryable(&ExecutorError::Protocol(
            "test".to_string()
        )));
    }
}
