//! Shared LM configuration helpers for the Adjutant agent.

use crate::ai_server::AiServerConfig;
use dsrs::LM;

pub fn load_ai_gateway_config() -> Result<AiServerConfig, String> {
    let config = AiServerConfig::from_env()?;
    config.validate()?;
    Ok(config)
}

pub async fn build_dsrs_lm_from_env() -> Result<LM, String> {
    let config = load_ai_gateway_config()?;
    build_dsrs_lm(&config).await
}

pub async fn build_dsrs_lm(config: &AiServerConfig) -> Result<LM, String> {
    let server_url = config.server_url();
    let base_url = format!("{}/v1", server_url);

    LM::builder()
        .base_url(base_url)
        .api_key(config.api_key.clone())
        .model(config.default_model.clone())
        .build()
        .await
        .map_err(|e| format!("Failed to build AI Gateway LM: {}", e))
}
