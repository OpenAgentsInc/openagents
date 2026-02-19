//! Ollama backend for local LLM inference.
//!
//! Connects to Ollama's OpenAI-compatible API at localhost:11434.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

use crate::backend::{LmBackend, LmResponse};
use crate::error::{Error, Result};
use crate::usage::LmUsage;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    options: Option<ChatOptions>,
}

#[derive(Debug, Serialize)]
struct ChatOptions {
    num_predict: Option<usize>,
    temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: MessageContent,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    eval_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    content: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

/// Ollama backend for local inference.
pub struct OllamaBackend {
    client: Client,
    base_url: String,
    models: Vec<String>,
}

impl OllamaBackend {
    /// Create a new Ollama backend with default URL (localhost:11434).
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // Longer timeout for local inference
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: DEFAULT_OLLAMA_URL.to_string(),
            models: Vec::new(),
        }
    }

    /// Create with a custom base URL.
    pub fn with_url(url: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: url.into(),
            models: Vec::new(),
        }
    }

    /// Detect available models from Ollama.
    pub async fn detect_models(&mut self) -> Result<()> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::RequestFailed(format!("Failed to connect to Ollama: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::RequestFailed("Ollama not available".to_string()));
        }

        let tags: TagsResponse = response
            .json()
            .await
            .map_err(|e| Error::RequestFailed(format!("Failed to parse Ollama response: {}", e)))?;

        self.models = tags.models.into_iter().map(|m| m.name).collect();
        debug!(models = ?self.models, "Detected Ollama models");

        Ok(())
    }

    /// Check if Ollama is running and accessible.
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.client.get(&url).send().await.is_ok()
    }

    /// Set specific models to advertise.
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }
}

impl Default for OllamaBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LmBackend for OllamaBackend {
    fn name(&self) -> &str {
        "ollama"
    }

    fn supported_models(&self) -> Vec<String> {
        self.models.clone()
    }

    fn supports_model(&self, model: &str) -> bool {
        // Support any model if we haven't detected specific ones yet,
        // or check our list
        if self.models.is_empty() {
            true // Let Ollama decide
        } else {
            self.models
                .iter()
                .any(|m| m == model || m.starts_with(&format!("{}:", model)))
        }
    }

    async fn complete(&self, model: &str, prompt: &str, max_tokens: usize) -> Result<LmResponse> {
        let url = format!("{}/api/chat", self.base_url);

        debug!(
            model = model,
            prompt_len = prompt.len(),
            max_tokens = max_tokens,
            "Ollama completion request"
        );

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            stream: false,
            options: Some(ChatOptions {
                num_predict: Some(max_tokens),
                temperature: Some(0.0),
            }),
        };

        let http_response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::RequestFailed(format!("Ollama request failed: {}", e)))?;

        if !http_response.status().is_success() {
            let status = http_response.status();
            let error_text = http_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Ollama API error {}: {}",
                status, error_text
            )));
        }

        let chat_response: ChatResponse = http_response
            .json()
            .await
            .map_err(|e| Error::RequestFailed(format!("Failed to parse Ollama response: {}", e)))?;

        let text = chat_response.message.content;

        // Use actual token counts if available, otherwise estimate
        let usage = LmUsage::new(
            chat_response.prompt_eval_count.unwrap_or(prompt.len() / 4),
            chat_response.eval_count.unwrap_or(text.len() / 4),
        );

        debug!(
            model = model,
            response_len = text.len(),
            "Ollama completion finished"
        );

        Ok(LmResponse::new(text, model, usage))
    }

    async fn health_check(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(e) => {
                warn!(error = %e, "Ollama health check failed");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_url() {
        let backend = OllamaBackend::new();
        assert_eq!(backend.base_url, DEFAULT_OLLAMA_URL);
    }

    #[test]
    fn test_custom_url() {
        let backend = OllamaBackend::with_url("http://custom:11434");
        assert_eq!(backend.base_url, "http://custom:11434");
    }

    #[test]
    fn test_supports_model_empty() {
        let backend = OllamaBackend::new();
        // With no models detected, accepts any
        assert!(backend.supports_model("anything"));
    }

    #[test]
    fn test_supports_model_specific() {
        let backend = OllamaBackend::new().with_models(vec!["llama3.2:latest".to_string()]);
        assert!(backend.supports_model("llama3.2:latest"));
        assert!(!backend.supports_model("gpt-4"));
    }
}
