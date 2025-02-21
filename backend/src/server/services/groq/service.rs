use anyhow::{Context, Result};
use reqwest::{Client, ClientBuilder};
use serde_json::Value;
use std::pin::Pin;
use std::time::Duration;
use tokio_stream::Stream;

use super::error::GroqError;
use super::types::ChatCompletion;
use crate::server::services::gateway::{types::GatewayMetadata, Gateway};

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

    async fn chat_with_history(
        &self,
        messages: Vec<Value>,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>)> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.metadata().default_model,
                "messages": messages,
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

        let completion: ChatCompletion = response
            .json()
            .await
            .context("Failed to parse Groq API response")?;

        let content = completion
            .choices
            .first()
            .ok_or_else(|| GroqError::ParseError("No choices in response".to_string()))?
            .message
            .content
            .clone();

        Ok((content, None))
    }
}

#[async_trait::async_trait]
impl Gateway for GroqService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "Groq".to_string(),
            openai_compatible: true,
            supported_features: vec!["chat".to_string(), "streaming".to_string()],
            default_model: "llama-3.1-8b-instant".to_string(),
            available_models: vec![
                "llama-3.1-8b-instant".to_string(),
                "llama-3.3-70b-versatile".to_string(),
            ],
        }
    }

    async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)> {
        // Convert single prompt into messages format
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": prompt
        })];

        self.chat_with_history(messages, use_reasoner).await
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
