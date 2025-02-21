use anyhow::Result;
use reqwest::{Client, ClientBuilder};
use std::pin::Pin;
use std::time::Duration;
use tokio_stream::Stream;

use crate::server::services::gateway::{Gateway, GatewayMetadata};
use super::{GroqConfig, GroqError};

#[derive(Debug, Clone)]
pub struct GroqService {
    client: Client,
    api_key: String,
    base_url: String,
}

impl GroqService {
    pub fn new(api_key: String) -> Self {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(180)) // 3 minutes timeout
            .build()
            .expect("Failed to create HTTP client");

        let base_url = std::env::var("GROQ_API_URL")
            .unwrap_or_else(|_| "https://api.groq.com/v1".to_string());

        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub fn with_config(config: GroqConfig) -> Self {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(config.timeout_secs.unwrap_or(180)))
            .build()
            .expect("Failed to create HTTP client");

        let base_url = config.base_url
            .unwrap_or_else(|| "https://api.groq.com/v1".to_string());

        Self {
            client,
            api_key: config.api_key,
            base_url,
        }
    }
}

#[async_trait::async_trait]
impl Gateway for GroqService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "Groq".to_string(),
            openai_compatible: true,
            supported_features: vec![
                "chat".to_string(),
                "streaming".to_string(),
            ],
            default_model: "mixtral-8x7b-32768".to_string(),
            available_models: vec![
                "llama-3.1-8b-instant".to_string(),
                "llama-3.3-70b-versatile".to_string(),
                "mixtral-8x7b-32768".to_string(),
            ],
        }
    }

    async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)> {
        let response = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.metadata().default_model,
                "messages": [{
                    "role": "user",
                    "content": prompt
                }],
                "temperature": if use_reasoner { 0.0 } else { 0.7 },
                "stream": false
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(GroqError::RequestFailed(error).into());
        }

        let json: serde_json::Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| GroqError::ParseError("Failed to parse response content".to_string()))?
            .to_string();

        Ok((content, None))
    }

    async fn chat_stream(
        &self,
        prompt: String,
        use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // TODO: Implement streaming using SSE
        todo!("Implement streaming support")
    }
}