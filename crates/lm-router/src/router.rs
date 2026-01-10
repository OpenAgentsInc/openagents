//! Main LM router implementation.

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::backend::{LmBackend, LmResponse};
use crate::error::{Error, Result};
use crate::usage::{UsageReport, UsageTracker};

/// Unified LM router supporting multiple backends.
///
/// The router manages multiple backends and routes requests based on model name.
/// It also tracks usage across all backends for cost/token reporting.
#[derive(Clone)]
pub struct LmRouter {
    /// Registered backends by name.
    backends: HashMap<String, Arc<dyn LmBackend>>,
    /// Model to backend routing.
    model_routing: HashMap<String, String>,
    /// Default backend for unrouted models.
    default_backend: Option<String>,
    /// Usage tracker.
    usage_tracker: UsageTracker,
}

impl std::fmt::Debug for LmRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LmRouter")
            .field("backend_count", &self.backends.len())
            .field("model_routing_count", &self.model_routing.len())
            .field("default_backend", &self.default_backend)
            .field("usage_tracker", &self.usage_tracker)
            .finish()
    }
}

impl LmRouter {
    /// Create a new router builder.
    pub fn builder() -> LmRouterBuilder {
        LmRouterBuilder::new()
    }

    /// Complete a prompt with the specified model.
    ///
    /// Routes the request to the appropriate backend based on the model name.
    pub async fn complete(&self, model: &str, prompt: &str, max_tokens: usize) -> Result<LmResponse> {
        let backend = self.get_backend_for_model(model)?;

        debug!(
            model = model,
            backend = backend.name(),
            prompt_len = prompt.len(),
            "Routing completion request"
        );

        let start = web_time::Instant::now();
        let mut response = backend.complete(model, prompt, max_tokens).await?;
        let latency_ms = start.elapsed().as_millis() as u64;

        response.latency_ms = latency_ms;
        self.usage_tracker
            .record(model, &response.usage, latency_ms);

        debug!(
            model = model,
            tokens = response.usage.total_tokens,
            latency_ms = latency_ms,
            "Completion finished"
        );

        Ok(response)
    }

    /// Get usage report across all backends.
    pub fn usage_report(&self) -> UsageReport {
        self.usage_tracker.report()
    }

    /// Reset usage tracking.
    pub fn reset_usage(&self) {
        self.usage_tracker.reset();
    }

    /// Check health of all backends.
    pub async fn health_check(&self) -> HashMap<String, bool> {
        let mut results = HashMap::new();
        for (name, backend) in &self.backends {
            let healthy = backend.health_check().await;
            results.insert(name.clone(), healthy);
        }
        results
    }

    /// Get the backend for a specific model.
    fn get_backend_for_model(&self, model: &str) -> Result<&Arc<dyn LmBackend>> {
        // Check explicit routing first
        if let Some(backend_name) = self.model_routing.get(model) {
            if let Some(backend) = self.backends.get(backend_name) {
                return Ok(backend);
            }
        }

        // Check if any backend supports this model
        for backend in self.backends.values() {
            if backend.supports_model(model) {
                return Ok(backend);
            }
        }

        // Fall back to default backend
        if let Some(default_name) = &self.default_backend {
            if let Some(backend) = self.backends.get(default_name) {
                return Ok(backend);
            }
        }

        Err(Error::BackendNotFound(model.to_string()))
    }

    /// Get a list of all available models.
    pub fn available_models(&self) -> Vec<String> {
        let mut models = Vec::new();
        for backend in self.backends.values() {
            models.extend(backend.supported_models());
        }
        models.sort();
        models.dedup();
        models
    }

    /// Add a model routing rule.
    pub fn add_route(&mut self, model: impl Into<String>, backend: impl Into<String>) {
        self.model_routing.insert(model.into(), backend.into());
    }
}

/// Builder for constructing an LmRouter.
pub struct LmRouterBuilder {
    backends: HashMap<String, Arc<dyn LmBackend>>,
    model_routing: HashMap<String, String>,
    default_backend: Option<String>,
}

impl Default for LmRouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LmRouterBuilder {
    /// Create a new router builder.
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            model_routing: HashMap::new(),
            default_backend: None,
        }
    }

    /// Add a backend.
    pub fn add_backend(mut self, backend: impl LmBackend + 'static) -> Self {
        let name = backend.name().to_string();
        info!(backend = name, "Adding backend to router");
        self.backends.insert(name, Arc::new(backend));
        self
    }

    /// Add a backend with a custom name.
    pub fn add_backend_as(
        mut self,
        name: impl Into<String>,
        backend: impl LmBackend + 'static,
    ) -> Self {
        let name = name.into();
        info!(backend = name, "Adding backend to router");
        self.backends.insert(name, Arc::new(backend));
        self
    }

    /// Add a model routing rule.
    pub fn route_model(mut self, model: impl Into<String>, backend: impl Into<String>) -> Self {
        self.model_routing.insert(model.into(), backend.into());
        self
    }

    /// Set the default backend for unrouted models.
    pub fn default_backend(mut self, backend: impl Into<String>) -> Self {
        self.default_backend = Some(backend.into());
        self
    }

    /// Build the router.
    pub fn build(self) -> LmRouter {
        if self.backends.is_empty() {
            warn!("Building router with no backends");
        }

        LmRouter {
            backends: self.backends,
            model_routing: self.model_routing,
            default_backend: self.default_backend,
            usage_tracker: UsageTracker::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mock::MockBackend;

    #[tokio::test]
    async fn test_router_with_mock_backend() {
        let mock = MockBackend::new()
            .with_model("test-model")
            .with_response("Hello, world!");

        let router = LmRouter::builder()
            .add_backend(mock)
            .default_backend("mock")
            .build();

        let response = router.complete("test-model", "Say hello", 100).await.unwrap();
        assert_eq!(response.text, "Hello, world!");
    }

    #[tokio::test]
    async fn test_router_model_routing() {
        let mock1 = MockBackend::new()
            .with_name("backend-1")
            .with_model("model-a")
            .with_response("Response from backend 1");

        let mock2 = MockBackend::new()
            .with_name("backend-2")
            .with_model("model-b")
            .with_response("Response from backend 2");

        let router = LmRouter::builder()
            .add_backend(mock1)
            .add_backend(mock2)
            .build();

        let resp1 = router.complete("model-a", "test", 100).await.unwrap();
        assert_eq!(resp1.text, "Response from backend 1");

        let resp2 = router.complete("model-b", "test", 100).await.unwrap();
        assert_eq!(resp2.text, "Response from backend 2");
    }

    #[tokio::test]
    async fn test_usage_tracking() {
        let mock = MockBackend::new()
            .with_model("test-model")
            .with_response("Response");

        let router = LmRouter::builder()
            .add_backend(mock)
            .default_backend("mock")
            .build();

        router.complete("test-model", "prompt 1", 100).await.unwrap();
        router.complete("test-model", "prompt 2", 100).await.unwrap();

        let report = router.usage_report();
        assert_eq!(report.total_calls, 2);
    }
}
