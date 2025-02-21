use anyhow::{Context, Result};
use futures::stream::Stream;
use reqwest::{Client, ClientBuilder};
use serde_json::{json, Value};
use std::pin::Pin;
use std::time::Duration;
use tokio_stream::StreamExt;

use super::error::GroqError;
use super::types::{ChatCompletion, StreamResponse};
use crate::server::services::gateway::{types::GatewayMetadata, Gateway};

#[derive(Debug, Clone)]
pub struct GroqService {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
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
            model: "deepseek-r1-distill-qwen-32b".to_string(), // Default to reasoning-capable model
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
            model: "deepseek-r1-distill-qwen-32b".to_string(),
        }
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    pub async fn chat_with_history(
        &self,
        messages: Vec<Value>,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>)> {
        let mut request = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": if use_reasoner { 0.0 } else { 0.7 },
            "stream": false,
        });

        // Add reasoning_format if using a model that supports it
        if self.model.starts_with("deepseek-r1") {
            request["reasoning_format"] =
                serde_json::json!(if use_reasoner { "parsed" } else { "hidden" });
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
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
            .ok_or_else(|| GroqError::ParseError("No choices in response".to_string()))?;

        // Return both content and reasoning
        Ok((
            content.message.content.clone(),
            content.message.reasoning.clone(),
        ))
    }

    pub async fn chat_with_history_stream(
        &self,
        messages: Vec<Value>,
        use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let mut request = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": if use_reasoner { 0.0 } else { 0.7 },
            "stream": true,
        });

        // Add reasoning_format if using a model that supports it
        if self.model.starts_with("deepseek-r1") {
            request["reasoning_format"] =
                serde_json::json!(if use_reasoner { "parsed" } else { "hidden" });
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Groq API")?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(GroqError::RequestFailed(error).into());
        }

        let stream = response
            .bytes_stream()
            .map(|chunk| -> Result<String> {
                let chunk = chunk.context("Failed to read chunk")?;
                let text = String::from_utf8_lossy(&chunk);

                // Parse SSE data
                if let Some(data) = text
                    .lines()
                    .filter(|line| line.starts_with("data: "))
                    .map(|line| line.trim_start_matches("data: "))
                    .find(|line| *line != "[DONE]")
                {
                    let stream_response: StreamResponse =
                        serde_json::from_str(data).context("Failed to parse stream response")?;

                    if let Some(choice) = stream_response.choices.first() {
                        if let Some(content) = &choice.delta.content {
                            return Ok(serde_json::to_string(&json!({
                                "content": content,
                                "reasoning": null
                            }))?);
                        }
                        if let Some(reasoning) = &choice.delta.reasoning {
                            return Ok(serde_json::to_string(&json!({
                                "content": null,
                                "reasoning": reasoning
                            }))?);
                        }
                    }
                }
                Ok(String::new())
            })
            .filter(|result| {
                if let Ok(content) = result {
                    !content.is_empty()
                } else {
                    true
                }
            });

        Ok(Box::pin(stream))
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
                "reasoning".to_string(),
            ],
            default_model: self.model.clone(),
            available_models: vec![
                "llama-3.1-8b-instant".to_string(),
                "llama-3.3-70b-versatile".to_string(),
                "deepseek-r1-distill-qwen-32b".to_string(),
                "deepseek-r1-distill-llama-70b".to_string(),
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
        prompt: String,
        use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": prompt
        })];

        self.chat_with_history_stream(messages, use_reasoner).await
    }
}
