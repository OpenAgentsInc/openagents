//! Automatic fallback from local inference to marketplace swarm compute
//!
//! Provides seamless transition between local and remote compute resources.
//! When local inference fails or is unavailable, jobs are automatically
//! submitted to marketplace providers.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

// Local inference integration
// TEMP: Uncomment when local-inference crate is integrated
// use local_inference::{LocalModelBackend, CompletionRequest, CompletionResponse};

// FIXME: ComputeConsumer not yet implemented
// use super::consumer::ComputeConsumer;
use super::events::ComputeJobRequest;

/// Fallback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Enable automatic fallback to marketplace
    pub enabled: bool,
    /// Maximum price willing to pay in millisats
    pub max_price_msats: Option<u64>,
    /// Timeout for local inference before fallback (seconds)
    pub local_timeout_secs: u64,
    /// Force local-only (never fallback)
    pub force_local: bool,
    /// Force swarm-only (skip local)
    pub force_swarm: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_price_msats: Some(1000), // Default max 1000 msats per job
            local_timeout_secs: 30,
            force_local: false,
            force_swarm: false,
        }
    }
}

/// Fallback strategy result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FallbackResult {
    /// Used local inference successfully
    Local { response: String, duration_ms: u64 },
    /// Fell back to marketplace
    Swarm {
        job_id: String,
        provider: String,
        cost_msats: u64,
        duration_ms: u64,
    },
    /// Both failed
    Failed {
        local_error: String,
        swarm_error: Option<String>,
    },
}

/// Metrics for fallback usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FallbackMetrics {
    /// Total local successes
    pub local_success: u64,
    /// Total local failures
    pub local_failure: u64,
    /// Total swarm fallbacks
    pub swarm_fallback: u64,
    /// Total swarm failures
    pub swarm_failure: u64,
    /// Total cost in millisats
    pub total_cost_msats: u64,
}

impl FallbackMetrics {
    /// Record a local success
    pub fn record_local_success(&mut self) {
        self.local_success += 1;
    }

    /// Record a local failure that triggered fallback
    pub fn record_local_failure(&mut self) {
        self.local_failure += 1;
    }

    /// Record a successful swarm fallback
    pub fn record_swarm_fallback(&mut self, cost_msats: u64) {
        self.swarm_fallback += 1;
        self.total_cost_msats += cost_msats;
    }

    /// Record a swarm failure
    pub fn record_swarm_failure(&mut self) {
        self.swarm_failure += 1;
    }

    /// Get fallback rate (percentage of requests that fell back)
    pub fn fallback_rate(&self) -> f64 {
        let total = self.local_success + self.local_failure;
        if total == 0 {
            return 0.0;
        }
        (self.swarm_fallback as f64 / total as f64) * 100.0
    }

    /// Get local success rate
    pub fn local_success_rate(&self) -> f64 {
        let total = self.local_success + self.local_failure;
        if total == 0 {
            return 0.0;
        }
        (self.local_success as f64 / total as f64) * 100.0
    }
}

/// Compute fallback manager
///
/// Manages automatic fallback from local inference to marketplace swarm.
pub struct FallbackManager {
    config: Arc<RwLock<FallbackConfig>>,
    // FIXME: ComputeConsumer not yet implemented
    // consumer: Option<Arc<ComputeConsumer>>,
    metrics: Arc<RwLock<FallbackMetrics>>,
    // TEMP: Add when local-inference is integrated
    // local_backend: Option<Arc<dyn LocalModelBackend>>,
}

impl FallbackManager {
    /// Create a new fallback manager
    pub fn new(config: FallbackConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            // consumer: None,
            metrics: Arc::new(RwLock::new(FallbackMetrics::default())),
            // local_backend: None,
        }
    }

    /* FIXME: ComputeConsumer not yet implemented
    /// Set the marketplace consumer for swarm fallback
    pub fn with_consumer(mut self, consumer: Arc<ComputeConsumer>) -> Self {
        self.consumer = Some(consumer);
        self
    }
    */

    // TEMP: Uncomment when local-inference is integrated
    // /// Set the local inference backend
    // pub fn with_local_backend(mut self, backend: Arc<dyn LocalModelBackend>) -> Self {
    //     self.local_backend = Some(backend);
    //     self
    // }

    /// Update fallback configuration
    pub async fn update_config(&self, config: FallbackConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
    }

    /// Get current configuration
    pub async fn get_config(&self) -> FallbackConfig {
        self.config.read().await.clone()
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> FallbackMetrics {
        self.metrics.read().await.clone()
    }

    /// Execute inference with automatic fallback
    ///
    /// Tries local inference first, falls back to marketplace on failure.
    ///
    /// # Arguments
    /// * `model` - Model identifier
    /// * `prompt` - Input prompt
    ///
    /// # Returns
    /// Result with response text and information about which backend was used
    pub async fn execute_with_fallback(&self, model: &str, prompt: &str) -> Result<FallbackResult> {
        let config = self.config.read().await.clone();
        let start_time = std::time::Instant::now();

        // Force swarm-only mode
        if config.force_swarm {
            return self.execute_swarm(model, prompt, start_time).await;
        }

        // Try local inference first (unless force_local is set and we skip on failure)
        let local_result = self.try_local_inference(model, prompt, &config).await;

        match local_result {
            Ok(response) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                self.metrics.write().await.record_local_success();
                Ok(FallbackResult::Local {
                    response,
                    duration_ms,
                })
            }
            Err(local_error) => {
                self.metrics.write().await.record_local_failure();

                // If force_local, don't fallback
                if config.force_local {
                    return Ok(FallbackResult::Failed {
                        local_error: local_error.to_string(),
                        swarm_error: None,
                    });
                }

                // If fallback disabled, fail
                if !config.enabled {
                    return Ok(FallbackResult::Failed {
                        local_error: local_error.to_string(),
                        swarm_error: Some("Fallback disabled".to_string()),
                    });
                }

                // Try marketplace fallback
                eprintln!(
                    "⚠ Local inference failed: {} - falling back to marketplace",
                    local_error
                );

                self.execute_swarm(model, prompt, start_time).await
            }
        }
    }

    /// Try local inference
    async fn try_local_inference(
        &self,
        _model: &str,
        _prompt: &str,
        config: &FallbackConfig,
    ) -> Result<String> {
        // TEMP: Until local-inference is integrated
        // Check if we have a local backend
        // if self.local_backend.is_none() {
        //     return Err(anyhow::anyhow!("No local inference backend configured"));
        // }

        // For now, simulate local inference failure to demonstrate fallback
        Err(anyhow::anyhow!(
            "Local inference not available - local-inference crate integration pending. \
             Timeout: {}s",
            config.local_timeout_secs
        ))

        // When local-inference is ready:
        /*
        let backend = self.local_backend.as_ref().unwrap();

        // Check if backend is ready
        if !backend.is_ready().await {
            return Err(anyhow::anyhow!("Local backend not ready"));
        }

        // Execute with timeout
        let request = CompletionRequest::new(model, prompt);
        let timeout_duration = std::time::Duration::from_secs(config.local_timeout_secs);

        match tokio::time::timeout(timeout_duration, backend.complete(request)).await {
            Ok(Ok(response)) => Ok(response.text),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err(anyhow::anyhow!("Local inference timeout after {}s", config.local_timeout_secs)),
        }
        */
    }

    /// Execute via marketplace swarm
    async fn execute_swarm(
        &self,
        _model: &str,
        prompt: &str,
        start_time: std::time::Instant,
    ) -> Result<FallbackResult> {
        // FIXME: ComputeConsumer not yet implemented
        // let consumer = self
        //     .consumer
        //     .as_ref()
        //     .ok_or_else(|| anyhow::anyhow!("No marketplace consumer configured"))?;

        let config = self.config.read().await;

        // Create compute job request
        let _request = ComputeJobRequest::text_generation(prompt);

        // Submit to marketplace
        // TEMP: Actual implementation would use consumer.submit_job()
        // For now, return a stub showing the flow

        let job_id = format!("job-{}", uuid::Uuid::new_v4());
        let provider = "swarm-provider-1";
        let cost_msats = 500; // Example cost

        // Check price limit
        if let Some(max_price) = config.max_price_msats {
            if cost_msats > max_price {
                self.metrics.write().await.record_swarm_failure();
                return Err(anyhow::anyhow!(
                    "Price {} msats exceeds max {} msats",
                    cost_msats,
                    max_price
                ));
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        self.metrics.write().await.record_swarm_fallback(cost_msats);

        Ok(FallbackResult::Swarm {
            job_id,
            provider: provider.to_string(),
            cost_msats,
            duration_ms,
        })

        // When marketplace consumer is fully integrated:
        /*
        let handle = consumer.submit_job(request).await?;

        // Wait for result
        let result = handle.wait_for_result().await?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let cost_msats = result.cost_msats.unwrap_or(0);

        // Check price limit
        if let Some(max_price) = config.max_price_msats {
            if cost_msats > max_price {
                self.metrics.write().await.record_swarm_failure();
                return Err(anyhow::anyhow!(
                    "Price {} msats exceeds max {} msats",
                    cost_msats,
                    max_price
                ));
            }
        }

        self.metrics.write().await.record_swarm_fallback(cost_msats);

        Ok(FallbackResult::Swarm {
            job_id: result.job_id,
            provider: result.provider,
            cost_msats,
            duration_ms,
        })
        */
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_config_defaults() {
        let config = FallbackConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_price_msats, Some(1000));
        assert_eq!(config.local_timeout_secs, 30);
        assert!(!config.force_local);
        assert!(!config.force_swarm);
    }

    #[test]
    fn test_fallback_metrics() {
        let mut metrics = FallbackMetrics::default();

        // Record some activity
        metrics.record_local_success();
        metrics.record_local_success();
        metrics.record_local_failure();
        metrics.record_swarm_fallback(500);

        assert_eq!(metrics.local_success, 2);
        assert_eq!(metrics.local_failure, 1);
        assert_eq!(metrics.swarm_fallback, 1);
        assert_eq!(metrics.total_cost_msats, 500);

        // Check rates
        assert!((metrics.local_success_rate() - 66.67).abs() < 0.1);
        assert!((metrics.fallback_rate() - 33.33).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_fallback_manager_config() {
        let config = FallbackConfig {
            enabled: false,
            max_price_msats: Some(2000),
            ..Default::default()
        };

        let manager = FallbackManager::new(config.clone());

        let retrieved = manager.get_config().await;
        assert!(!retrieved.enabled);
        assert_eq!(retrieved.max_price_msats, Some(2000));
    }

    #[tokio::test]
    async fn test_execute_with_force_local() {
        let config = FallbackConfig {
            force_local: true,
            ..Default::default()
        };

        let manager = FallbackManager::new(config);

        // Should fail with local error, no fallback
        let result = manager.execute_with_fallback("llama3", "test").await;
        assert!(result.is_ok());

        if let Ok(FallbackResult::Failed {
            local_error,
            swarm_error,
        }) = result
        {
            assert!(local_error.contains("Local inference not available"));
            assert!(swarm_error.is_none());
        } else {
            panic!("Expected Failed result");
        }
    }

    #[tokio::test]
    async fn test_execute_with_fallback_disabled() {
        let config = FallbackConfig {
            enabled: false,
            ..Default::default()
        };

        let manager = FallbackManager::new(config);

        let result = manager.execute_with_fallback("llama3", "test").await;
        assert!(result.is_ok());

        if let Ok(FallbackResult::Failed { swarm_error, .. }) = result {
            assert_eq!(swarm_error.as_deref(), Some("Fallback disabled"));
        } else {
            panic!("Expected Failed result with fallback disabled");
        }
    }
}
