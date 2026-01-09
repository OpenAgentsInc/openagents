//! Cerebras LM configuration for dsrs.
//!
//! Configures dsrs to use Cerebras models via their OpenAI-compatible API.

use anyhow::Result;
use dsrs::{ChatAdapter, LM, configure};
use std::sync::Arc;

/// Models for tiered execution
pub const PLANNING_MODEL: &str = "zai-glm-4.7";
pub const EXECUTION_MODEL: &str = "qwen-3-32b";

/// Cerebras API endpoint (OpenAI-compatible)
const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

/// Create an LM configured for Cerebras.
///
/// Uses the OpenAI-compatible API with Cerebras base URL.
pub async fn create_cerebras_lm(model: &str) -> Result<LM> {
    let api_key = std::env::var("CEREBRAS_API_KEY")
        .map_err(|_| anyhow::anyhow!("CEREBRAS_API_KEY not set"))?;

    LM::builder()
        .base_url(CEREBRAS_BASE_URL.to_string())
        .api_key(api_key)
        .model(model.to_string())
        .temperature(0.7)
        .max_tokens(4000)
        .build()
        .await
}

/// Create planning LM (GLM 4.7 - smart model for strategic decisions)
pub async fn create_planning_lm() -> Result<LM> {
    create_cerebras_lm(PLANNING_MODEL).await
}

/// Create execution LM (Qwen-3-32B - cost-effective for tactical work)
pub async fn create_execution_lm() -> Result<LM> {
    create_cerebras_lm(EXECUTION_MODEL).await
}

/// Configure global dsrs settings for Cerebras.
///
/// Call this once at startup to set the default LM.
pub async fn configure_cerebras_dsrs() -> Result<()> {
    let lm = create_planning_lm().await?;
    configure(lm, ChatAdapter);
    Ok(())
}

/// Create an LM from environment, supporting multiple providers.
///
/// Checks CEREBRAS_API_KEY first, then falls back to OPENAI_API_KEY.
pub async fn create_lm_from_env(model: &str) -> Result<LM> {
    // Try Cerebras first
    if std::env::var("CEREBRAS_API_KEY").is_ok() {
        return create_cerebras_lm(model).await;
    }

    // Fall back to OpenAI-compatible (model should be in provider:model format)
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return LM::builder()
            .model(format!("openai:{}", model))
            .build()
            .await;
    }

    Err(anyhow::anyhow!(
        "No API key found. Set CEREBRAS_API_KEY or OPENAI_API_KEY"
    ))
}

/// Get an Arc-wrapped LM for use with forward_with_config.
pub async fn get_planning_lm() -> Result<Arc<LM>> {
    Ok(Arc::new(create_planning_lm().await?))
}

/// Get an Arc-wrapped execution LM.
pub async fn get_execution_lm() -> Result<Arc<LM>> {
    Ok(Arc::new(create_execution_lm().await?))
}
