use reqwest::{Client, ClientBuilder};
use std::time::Duration;
use anyhow::{Result, Context};
use std::pin::Pin;
use tokio_stream::Stream;

use crate::server::services::gateway::{Gateway, types::GatewayMetadata};
use super::types::ChatCompletion;
use super::error::GroqError;

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
            .unwrap_or_else(|_| "https://api.groq.com/openai/v1".to_string());

        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(180)) // 3 minutes timeout
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
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
            .await
            .context("Failed to send request to Groq API")?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(GroqError::RequestFailed(error).into());
        }

        let completion: ChatCompletion = response.json().await
            .context("Failed to parse Groq API response")?;

        let content = completion.choices.first()
            .ok_or_else(|| GroqError::ParseError("No choices in response".to_string()))?
            .message.content.clone();

        Ok((content, None))
    }

    async fn chat_stream(
        &self,
        _prompt: String,
        _use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // TODO: Implement streaming using SSE
        todo!("Implement streaming")
    }
}