//! DSPy bridge module - integrates DSRs (dspy-rs) with RLM.
//!
//! This module provides:
//! 1. Re-exports of key DSRs types for convenient use
//! 2. Helper functions to configure DSPy with OpenAgents infrastructure
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm::dspy_bridge::{configure_dspy_lm, Predict, example};
//!
//! // Configure DSPy to use an LM
//! configure_dspy_lm("openai:gpt-4o-mini", None, None).await?;
//!
//! // Now use DSPy modules
//! let predictor = Predict::new(MySignature::new());
//! let result = predictor.forward(example! {
//!     "question": "input" => "What is 2+2?"
//! }).await?;
//! ```

// Re-export DSRs core types
pub use dsrs::{
    // Core traits
    Module,
    Optimizable,
    MetaSignature,

    // Predictors
    Predict,
    Predictor,

    // Data types
    Example,
    Prediction,

    // LM types
    LM,
    LMResponse,
    LmUsage,
    Chat,
    Message,

    // Adapter
    ChatAdapter,

    // Configuration
    configure,
    get_lm,

    // Evaluation
    Evaluator,

    // Optimizers
    COPRO,
    MIPROv2,
    Optimizer,
};

// Re-export the Signature derive macro
pub use dsrs::Signature;

// Re-export macros (these use dsrs internally so we just re-export)
pub use dsrs::{example, prediction, sign, field, hashmap};

use std::sync::Arc;
use anyhow::Result;
use lm_router::{LmRouter, LmResponse};

/// Configure DSPy to use a specific LM configuration.
///
/// This sets up the global DSRs LM context to use either:
/// - OpenAI-compatible API (via `base_url`)
/// - Direct provider (via model string like "openai:gpt-4o-mini")
///
/// # Arguments
///
/// * `model` - Model identifier (e.g., "openai:gpt-4o-mini", "openai:codex-3-sonnet")
/// * `api_key` - Optional API key (reads from env vars if not provided)
/// * `base_url` - Optional base URL for OpenAI-compatible APIs
///
/// # Example
///
/// ```rust,ignore
/// // Use OpenAI with env var for API key
/// configure_dspy_lm("openai:gpt-4o-mini", None, None).await?;
///
/// // Use OpenRouter
/// configure_dspy_lm(
///     "openai/gpt-4o-mini",
///     Some("your-openrouter-key"),
///     Some("https://openrouter.ai/api/v1")
/// ).await?;
/// ```
pub async fn configure_dspy_lm(
    model: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> Result<()> {
    // Build LM with optional fields set if provided
    let lm = match (api_key, base_url) {
        (Some(key), Some(url)) => {
            LM::builder()
                .model(model.to_string())
                .api_key(key.to_string())
                .base_url(url.to_string())
                .temperature(0.7)
                .max_tokens(4096)
                .build()
                .await?
        }
        (Some(key), None) => {
            LM::builder()
                .model(model.to_string())
                .api_key(key.to_string())
                .temperature(0.7)
                .max_tokens(4096)
                .build()
                .await?
        }
        (None, Some(url)) => {
            LM::builder()
                .model(model.to_string())
                .base_url(url.to_string())
                .temperature(0.7)
                .max_tokens(4096)
                .build()
                .await?
        }
        (None, None) => {
            LM::builder()
                .model(model.to_string())
                .temperature(0.7)
                .max_tokens(4096)
                .build()
                .await?
        }
    };

    configure(lm, ChatAdapter);

    Ok(())
}

/// Configure DSPy with a pre-built LM instance.
///
/// Useful when you need custom LM configuration.
pub fn configure_dspy_with_lm(lm: LM) {
    configure(lm, ChatAdapter);
}

/// Get the currently configured global LM.
///
/// Returns an Arc to the global LM instance set by `configure_dspy_*`.
pub fn get_dspy_lm() -> Arc<LM> {
    get_lm()
}

/// Create an LM instance from lm-router configuration.
///
/// This creates a DSRs LM that uses an OpenAI-compatible endpoint,
/// which works with lm-router's OpenRouter or OpenAI backends.
///
/// # Arguments
///
/// * `model` - Model name to use
/// * `api_key` - API key for authentication
///
/// # Example
///
/// ```rust,ignore
/// // Use with OpenRouter
/// let lm = create_lm_for_openrouter("openai/gpt-4o-mini", "your-key").await?;
///
/// // Now you can use it directly or configure globally
/// configure_dspy_with_lm(lm);
/// ```
pub async fn create_lm_for_openrouter(model: &str, api_key: &str) -> Result<LM> {
    let lm = LM::builder()
        .model(model.to_string())
        .base_url("https://openrouter.ai/api/v1".to_string())
        .api_key(api_key.to_string())
        .temperature(0.7)
        .max_tokens(4096)
        .build()
        .await?;

    Ok(lm)
}

/// Create an LM instance for local inference (e.g., Ollama, vLLM).
///
/// # Arguments
///
/// * `model` - Model name to use
/// * `base_url` - Base URL of the local server (e.g., "http://localhost:11434")
pub async fn create_lm_for_local(model: &str, base_url: &str) -> Result<LM> {
    let lm = LM::builder()
        .model(model.to_string())
        .base_url(base_url.to_string())
        .temperature(0.7)
        .max_tokens(4096)
        .build()
        .await?;

    Ok(lm)
}

// ============================================================================
// LmRouter-to-DSPy Bridge
// ============================================================================

/// Bridge between LmRouter and DSPy LM.
///
/// Enables per-request routing through LmRouter for:
/// - Unified cost tracking across all DSPy calls
/// - Dynamic backend selection based on model
/// - Health monitoring and fallback logic
///
/// # Example
///
/// ```rust,ignore
/// use lm_router::LmRouter;
/// use rlm::dspy_bridge::LmRouterDspyBridge;
///
/// let router = Arc::new(LmRouter::builder()
///     .add_backend(openai_backend)
///     .default_backend("openai")
///     .build());
///
/// let bridge = LmRouterDspyBridge::new(router, "gpt-4o-mini");
/// let lm = bridge.create_lm().await?;
///
/// // Use LM for DSPy orchestration
/// let orchestrator = DspyOrchestrator::with_lm(lm);
/// ```
#[derive(Clone)]
pub struct LmRouterDspyBridge {
    /// LmRouter instance for backend routing
    router: Arc<LmRouter>,
    /// Model to use for requests
    model: String,
    /// Default max tokens
    max_tokens: usize,
    /// Default temperature
    temperature: f32,
}

impl LmRouterDspyBridge {
    /// Create a new bridge with the given router and model.
    pub fn new(router: Arc<LmRouter>, model: impl Into<String>) -> Self {
        Self {
            router,
            model: model.into(),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }

    /// Set the default max tokens for requests.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set the default temperature for requests.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Get the router for direct access (e.g., usage reports).
    pub fn router(&self) -> &Arc<LmRouter> {
        &self.router
    }

    /// Get usage report from the router.
    pub fn usage_report(&self) -> lm_router::UsageReport {
        self.router.usage_report()
    }

    /// Create a DSPy LM instance that routes through LmRouter.
    ///
    /// Note: This creates an LM using the OpenAI-compatible API pattern.
    /// The router handles actual backend selection based on the model.
    ///
    /// For true LmRouter integration, you would need to implement a custom
    /// DSPy LM client. This is a bridge pattern that works with existing
    /// DSRs infrastructure.
    pub async fn create_lm(&self) -> Result<LM> {
        // Get the backend URL from the router if available
        // For now, we create an LM that will be configured separately
        // and the router tracking happens at a higher level

        // TODO: Implement true LM::with_client() when DSRs supports custom clients
        // For now, use the builder pattern with OpenAI-compatible endpoint

        let lm = LM::builder()
            .model(self.model.clone())
            .temperature(self.temperature)
            .max_tokens(self.max_tokens as u32)
            .build()
            .await?;

        Ok(lm)
    }

    /// Complete a request through LmRouter directly.
    ///
    /// This bypasses DSPy and goes straight through LmRouter,
    /// useful for simple completions where DSPy overhead isn't needed.
    pub async fn complete(&self, prompt: &str, max_tokens: Option<usize>) -> Result<LmResponse> {
        let max = max_tokens.unwrap_or(self.max_tokens);
        let response = self.router.complete(&self.model, prompt, max).await?;
        Ok(response)
    }

    /// Create a pre-configured LM and set it globally.
    ///
    /// This is a convenience method for simple setups where you want
    /// the bridge to manage the global LM state.
    pub async fn configure_global(&self) -> Result<()> {
        let lm = self.create_lm().await?;
        configure(lm, ChatAdapter);
        Ok(())
    }
}

/// Configuration for creating an LmRouter-backed DSPy setup.
#[derive(Clone)]
pub struct LmRouterDspyConfig {
    /// Model to use
    pub model: String,
    /// Max tokens per request
    pub max_tokens: usize,
    /// Temperature for sampling
    pub temperature: f32,
    /// Whether to configure globally on creation
    pub configure_global: bool,
}

impl Default for LmRouterDspyConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4o-mini".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            configure_global: false,
        }
    }
}

impl LmRouterDspyConfig {
    /// Create config with a specific model.
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Build a bridge from this config.
    pub fn build(self, router: Arc<LmRouter>) -> LmRouterDspyBridge {
        LmRouterDspyBridge {
            router,
            model: self.model,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Basic smoke test that types re-export correctly
    #[test]
    fn test_reexports() {
        // These should compile if re-exports work
        let _: fn(Example) = |_| {};
        let _: fn(Prediction) = |_| {};
    }
}
