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
pub use dspy_rs::{
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
pub use dspy_rs::Signature;

// Re-export macros (these use dspy_rs internally so we just re-export)
pub use dspy_rs::{example, prediction, sign, field, hashmap};

use std::sync::Arc;
use anyhow::Result;

/// Configure DSPy to use a specific LM configuration.
///
/// This sets up the global DSRs LM context to use either:
/// - OpenAI-compatible API (via `base_url`)
/// - Direct provider (via model string like "openai:gpt-4o-mini")
///
/// # Arguments
///
/// * `model` - Model identifier (e.g., "openai:gpt-4o-mini", "anthropic:claude-3-sonnet")
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
