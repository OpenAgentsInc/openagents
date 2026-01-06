//! FM Bridge backend for Apple Foundation Models.

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::backend::{LmBackend, LmResponse};
use crate::error::Result;
use crate::usage::LmUsage;

/// Backend for Apple Foundation Models via FM Bridge.
pub struct FmBridgeBackend {
    client: fm_bridge::FMClient,
    models: Vec<String>,
}

impl FmBridgeBackend {
    /// Create a new FM Bridge backend with default settings.
    pub fn new() -> Result<Self> {
        let client = fm_bridge::FMClient::new()?;
        Ok(Self {
            client,
            models: vec!["apple-fm".to_string()],
        })
    }

    /// Create a new FM Bridge backend with a custom URL.
    pub fn with_url(url: impl Into<String>) -> Result<Self> {
        let client = fm_bridge::FMClientBuilder::new()
            .base_url(url)
            .build()?;
        Ok(Self {
            client,
            models: vec!["apple-fm".to_string()],
        })
    }

    /// Create from an existing FMClient.
    pub fn from_client(client: fm_bridge::FMClient) -> Self {
        Self {
            client,
            models: vec!["apple-fm".to_string()],
        }
    }

    /// Set the supported models.
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }

    /// Add a supported model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.models.push(model.into());
        self
    }

    /// Get a reference to the underlying client.
    pub fn client(&self) -> &fm_bridge::FMClient {
        &self.client
    }
}

impl Default for FmBridgeBackend {
    fn default() -> Self {
        Self::new().expect("Failed to create default FmBridgeBackend")
    }
}

#[async_trait]
impl LmBackend for FmBridgeBackend {
    fn name(&self) -> &str {
        "fm-bridge"
    }

    fn supported_models(&self) -> Vec<String> {
        self.models.clone()
    }

    async fn complete(&self, model: &str, prompt: &str, max_tokens: usize) -> Result<LmResponse> {
        debug!(
            model = model,
            prompt_len = prompt.len(),
            max_tokens = max_tokens,
            "FM Bridge completion request"
        );

        let options = fm_bridge::CompletionOptions {
            max_tokens: Some(max_tokens as u32),
            ..Default::default()
        };

        let response = self.client.complete(prompt, Some(options)).await?;

        // Extract the response text
        let text = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        // Extract usage if available
        let usage = if let Some(u) = response.usage {
            LmUsage::new(
                u.prompt_tokens.unwrap_or(0) as usize,
                u.completion_tokens.unwrap_or(0) as usize,
            )
        } else {
            // Estimate if not provided
            let prompt_tokens = prompt.len() / 4;
            let completion_tokens = text.len() / 4;
            LmUsage::new(prompt_tokens, completion_tokens)
        };

        // Convert FinishReason enum to string
        let finish_reason = response
            .choices
            .first()
            .and_then(|c| c.finish_reason)
            .map(|r| match r {
                fm_bridge::FinishReason::Stop => "stop".to_string(),
                fm_bridge::FinishReason::Length => "length".to_string(),
                fm_bridge::FinishReason::ToolCalls => "tool_calls".to_string(),
            });

        let mut resp = LmResponse::new(text, model, usage);
        if let Some(reason) = finish_reason {
            resp = resp.with_finish_reason(reason);
        }
        Ok(resp)
    }

    async fn health_check(&self) -> bool {
        match self.client.health().await {
            Ok(healthy) => healthy,
            Err(e) => {
                warn!(error = %e, "FM Bridge health check failed");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_models() {
        let backend = FmBridgeBackend::new()
            .unwrap()
            .with_model("custom-model");

        let models = backend.supported_models();
        assert!(models.contains(&"apple-fm".to_string()));
        assert!(models.contains(&"custom-model".to_string()));
    }
}
