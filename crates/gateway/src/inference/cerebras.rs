use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use crate::error::{GatewayError, Result};
use crate::inference::types::{ChatRequest, ChatResponse};
use crate::traits::{Gateway, InferenceGateway};
use crate::types::{Capability, GatewayHealth, ModelInfo, ModelPricing};

/// Default Cerebras API endpoint
const DEFAULT_ENDPOINT: &str = "https://api.cerebras.ai/v1";

/// Default request timeout in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Cerebras API error response
#[derive(Debug, Deserialize)]
struct ApiError {
    error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    message: String,
    #[serde(default)]
    code: Option<String>,
}

/// Gateway implementation for Cerebras inference API
pub struct CerebrasGateway {
    client: Client,
    api_key: String,
    endpoint: String,
}

impl CerebrasGateway {
    /// Create a new CerebrasGateway with explicit configuration
    pub fn new(api_key: impl Into<String>, endpoint: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(GatewayError::Http)?;

        Ok(Self {
            client,
            api_key: api_key.into(),
            endpoint: endpoint.into(),
        })
    }

    /// Create a CerebrasGateway from environment variables
    ///
    /// Reads:
    /// - `CEREBRAS_API_KEY` (required)
    /// - `CEREBRAS_ENDPOINT` (optional, defaults to https://api.cerebras.ai/v1)
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("CEREBRAS_API_KEY")
            .map_err(|_| GatewayError::NotConfigured("CEREBRAS_API_KEY not set".into()))?;

        let endpoint = std::env::var("CEREBRAS_ENDPOINT")
            .unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());

        Self::new(api_key, endpoint)
    }

    /// Get the list of known Cerebras models with their info
    fn known_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "zai-glm-4.7".into(),
                name: "Z.ai GLM 4.7".into(),
                provider: "cerebras".into(),
                context_length: 131_072,
                capabilities: vec![
                    Capability::ChatCompletion,
                    Capability::Streaming,
                    Capability::FunctionCalling,
                    Capability::Reasoning,
                ],
                pricing: Some(ModelPricing {
                    input_per_million: 2.25,
                    output_per_million: 2.75,
                }),
            },
            ModelInfo {
                id: "llama-3.3-70b".into(),
                name: "Llama 3.3 70B".into(),
                provider: "cerebras".into(),
                context_length: 128_000,
                capabilities: vec![
                    Capability::ChatCompletion,
                    Capability::Streaming,
                ],
                pricing: Some(ModelPricing {
                    input_per_million: 0.85,
                    output_per_million: 1.20,
                }),
            },
            ModelInfo {
                id: "llama3.1-8b".into(),
                name: "Llama 3.1 8B".into(),
                provider: "cerebras".into(),
                context_length: 128_000,
                capabilities: vec![
                    Capability::ChatCompletion,
                    Capability::Streaming,
                ],
                pricing: Some(ModelPricing {
                    input_per_million: 0.10,
                    output_per_million: 0.10,
                }),
            },
        ]
    }
}

impl Gateway for CerebrasGateway {
    fn gateway_type(&self) -> &str {
        "inference"
    }

    fn provider(&self) -> &str {
        "cerebras"
    }

    fn name(&self) -> &str {
        "Cerebras Cloud"
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::ChatCompletion,
            Capability::Streaming,
            Capability::FunctionCalling,
            Capability::Reasoning,
        ]
    }
}

#[async_trait]
impl InferenceGateway for CerebrasGateway {
    async fn models(&self) -> Result<Vec<ModelInfo>> {
        // Return known models - Cerebras doesn't have a models endpoint
        Ok(Self::known_models())
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/chat/completions", self.endpoint);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            let chat_response: ChatResponse = response.json().await?;
            Ok(chat_response)
        } else if status.as_u16() == 429 {
            Err(GatewayError::RateLimited)
        } else {
            // Try to parse error response
            let error_text = response.text().await.unwrap_or_default();
            let message = if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                api_error.error.message
            } else {
                error_text
            };

            Err(GatewayError::Api {
                status: status.as_u16(),
                message,
            })
        }
    }

    async fn health(&self) -> GatewayHealth {
        let start = Instant::now();
        let now = chrono::Utc::now().timestamp();

        // Try to list models as a health check
        match self.models().await {
            Ok(_) => GatewayHealth {
                available: true,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                error: None,
                last_check: now,
            },
            Err(e) => GatewayHealth {
                available: false,
                latency_ms: None,
                error: Some(e.to_string()),
                last_check: now,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_models() {
        let models = CerebrasGateway::known_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "zai-glm-4.7"));
    }

    #[test]
    fn test_from_env_missing_key() {
        // Temporarily unset the key (unsafe in Rust 2024)
        // SAFETY: This test runs in isolation
        unsafe {
            std::env::remove_var("CEREBRAS_API_KEY");
        }
        let result = CerebrasGateway::from_env();
        assert!(result.is_err());
    }
}
