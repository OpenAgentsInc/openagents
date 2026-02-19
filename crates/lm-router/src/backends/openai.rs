//! OpenAI backend for direct API access.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::backend::{LmBackend, LmResponse};
use crate::error::{Error, Result};
use crate::usage::LmUsage;

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: usize,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

/// OpenAI API backend.
pub struct OpenAiBackend {
    client: Client,
    api_key: String,
    models: Vec<String>,
}

impl OpenAiBackend {
    /// Create a new OpenAI backend.
    ///
    /// Uses OPENAI_API_KEY environment variable.
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            Error::RequestFailed("OPENAI_API_KEY environment variable not set".to_string())
        })?;

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e: reqwest::Error| Error::RequestFailed(e.to_string()))?;

        Ok(Self {
            client,
            api_key,
            models: vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
        })
    }

    /// Create with a specific API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e: reqwest::Error| Error::RequestFailed(e.to_string()))?;

        Ok(Self {
            client,
            api_key: api_key.into(),
            models: vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
        })
    }
}

#[async_trait]
impl LmBackend for OpenAiBackend {
    fn name(&self) -> &str {
        "openai"
    }

    fn supported_models(&self) -> Vec<String> {
        self.models.clone()
    }

    async fn complete(&self, model: &str, prompt: &str, max_tokens: usize) -> Result<LmResponse> {
        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens,
            temperature: 0.0,
        };

        let http_response: reqwest::Response = self
            .client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e: reqwest::Error| Error::RequestFailed(format!("HTTP error: {}", e)))?;

        if !http_response.status().is_success() {
            let status = http_response.status();
            let error_text: String = http_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "OpenAI API error {}: {}",
                status, error_text
            )));
        }

        let response_text: String = http_response.text().await.map_err(|e: reqwest::Error| {
            Error::RequestFailed(format!("Failed to read response: {}", e))
        })?;

        let chat_response: ChatResponse = serde_json::from_str(&response_text)?;

        let text = chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let usage = chat_response
            .usage
            .map(|u| LmUsage::new(u.prompt_tokens, u.completion_tokens))
            .unwrap_or_else(|| LmUsage::new(prompt.len() / 4, text.len() / 4));

        Ok(LmResponse::new(text, model, usage))
    }

    async fn health_check(&self) -> bool {
        // Simple check - just verify we have an API key
        !self.api_key.is_empty()
    }
}
