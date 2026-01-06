//! Mock backend for testing.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

use async_trait::async_trait;

use crate::backend::{LmBackend, LmResponse};
use crate::error::Result;
use crate::usage::LmUsage;

/// A mock backend for testing.
///
/// Can be configured with:
/// - Fixed responses
/// - Controllable latency
/// - Request counting
pub struct MockBackend {
    name: String,
    models: Vec<String>,
    responses: RwLock<Vec<String>>,
    response_index: AtomicUsize,
    call_count: AtomicUsize,
    healthy: bool,
}

impl MockBackend {
    /// Create a new mock backend with default settings.
    pub fn new() -> Self {
        Self {
            name: "mock".to_string(),
            models: vec!["mock-model".to_string()],
            responses: RwLock::new(vec!["Mock response".to_string()]),
            response_index: AtomicUsize::new(0),
            call_count: AtomicUsize::new(0),
            healthy: true,
        }
    }

    /// Set the backend name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a supported model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.models.push(model.into());
        self
    }

    /// Set the models (replaces existing).
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }

    /// Set a single fixed response.
    pub fn with_response(self, response: impl Into<String>) -> Self {
        self.with_responses(vec![response.into()])
    }

    /// Set multiple responses (cycles through them).
    pub fn with_responses(self, responses: Vec<String>) -> Self {
        *self.responses.write().unwrap() = responses;
        self
    }

    /// Set whether the backend reports as healthy.
    pub fn with_healthy(mut self, healthy: bool) -> Self {
        self.healthy = healthy;
        self
    }

    /// Get the number of calls made.
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::Relaxed)
    }

    /// Reset the call count.
    pub fn reset_count(&self) {
        self.call_count.store(0, Ordering::Relaxed);
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LmBackend for MockBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn supported_models(&self) -> Vec<String> {
        self.models.clone()
    }

    async fn complete(&self, model: &str, prompt: &str, _max_tokens: usize) -> Result<LmResponse> {
        self.call_count.fetch_add(1, Ordering::Relaxed);

        let responses = self.responses.read().unwrap();
        let index = self.response_index.fetch_add(1, Ordering::Relaxed) % responses.len();
        let text = responses[index].clone();

        // Estimate tokens (rough approximation)
        let prompt_tokens = prompt.len() / 4;
        let completion_tokens = text.len() / 4;

        Ok(LmResponse::new(
            text,
            model,
            LmUsage::new(prompt_tokens, completion_tokens),
        ))
    }

    async fn health_check(&self) -> bool {
        self.healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_backend() {
        let backend = MockBackend::new()
            .with_model("test-model")
            .with_response("Test response");

        let response = backend.complete("test-model", "Hello", 100).await.unwrap();
        assert_eq!(response.text, "Test response");
        assert_eq!(backend.call_count(), 1);
    }

    #[tokio::test]
    async fn test_mock_cycling_responses() {
        let backend = MockBackend::new()
            .with_responses(vec!["First".into(), "Second".into(), "Third".into()]);

        let r1 = backend.complete("mock-model", "a", 100).await.unwrap();
        let r2 = backend.complete("mock-model", "b", 100).await.unwrap();
        let r3 = backend.complete("mock-model", "c", 100).await.unwrap();
        let r4 = backend.complete("mock-model", "d", 100).await.unwrap();

        assert_eq!(r1.text, "First");
        assert_eq!(r2.text, "Second");
        assert_eq!(r3.text, "Third");
        assert_eq!(r4.text, "First"); // Cycles back
    }
}
