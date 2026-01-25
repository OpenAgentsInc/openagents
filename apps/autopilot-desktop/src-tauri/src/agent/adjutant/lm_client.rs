//! LM Client for connecting DSPy to local AI Gateway
//!
//! Provides a dsrs::LM implementation that talks to the local bun server
//! which proxies requests to Vercel AI Gateway.

use std::sync::Arc;
use std::collections::HashMap;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use crate::ai_server::AiServerConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspyPredictRequest {
    pub signature_type: String,
    pub inputs: HashMap<String, serde_json::Value>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspyPredictResponse {
    pub prediction: String,
    pub model: String,
    pub signature_type: String,
    pub usage: Option<TokenUsage>,
}

/// Local AI LM implementation for DSPy
pub struct LocalAiLM {
    base_url: String,
    api_key: String,
    client: Client,
    default_model: String,
}

impl LocalAiLM {
    pub fn new(base_url: String, api_key: String, default_model: String) -> Self {
        Self {
            base_url,
            api_key,
            client: Client::new(),
            default_model,
        }
    }

    pub fn from_config(config: &AiServerConfig) -> Self {
        Self::new(
            config.server_url(),
            "local-api-key".to_string(), // Local server doesn't need real auth
            config.default_model.clone(),
        )
    }

    /// Send a chat completion request to the local AI server
    pub async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow!("Request failed: {}", e))?;

        if response.status().is_success() {
            let completion: ChatCompletionResponse = response
                .json()
                .await
                .map_err(|e| anyhow!("Failed to parse response: {}", e))?;
            Ok(completion)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow!("AI server error {}: {}", status, error_text))
        }
    }

    /// Send a DSPy prediction request to the local AI server
    pub async fn dspy_predict(&self, request: DspyPredictRequest) -> Result<DspyPredictResponse> {
        let url = format!("{}/dspy/predict", self.base_url);
        
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow!("DSPy predict request failed: {}", e))?;

        if response.status().is_success() {
            let prediction: DspyPredictResponse = response
                .json()
                .await
                .map_err(|e| anyhow!("Failed to parse DSPy response: {}", e))?;
            Ok(prediction)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow!("DSPy prediction error {}: {}", status, error_text))
        }
    }

    /// Convert a generic prompt to chat messages
    fn prompt_to_messages(&self, prompt: &str, system_prompt: Option<&str>) -> Vec<ChatMessage> {
        let mut messages = Vec::new();
        
        if let Some(system) = system_prompt {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: system.to_string(),
            });
        }
        
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });
        
        messages
    }
}

// Note: LocalAiLM is a wrapper around the local AI server
// In a full integration, we'd either:
// 1. Use the dsrs LM struct directly with proper configuration, or
// 2. Create a custom LM client that wraps our local server
// 
// For now, LocalAiLM serves as a client to our local AI Gateway

impl Clone for LocalAiLM {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            client: self.client.clone(),
            default_model: self.default_model.clone(),
        }
    }
}

/// Create a LocalAiLM from environment or default configuration
pub fn create_local_ai_lm() -> Result<Arc<LocalAiLM>> {
    match AiServerConfig::from_env() {
        Ok(config) => {
            config.validate()
                .map_err(|e| anyhow!("Invalid AI server config: {}", e))?;
            Ok(Arc::new(LocalAiLM::from_config(&config)))
        }
        Err(e) => {
            Err(anyhow!("Failed to create LocalAiLM: {}", e))
        }
    }
}

/// Create a LocalAiLM with specific configuration
pub fn create_local_ai_lm_with_config(
    base_url: &str,
    model: &str,
) -> Arc<LocalAiLM> {
    Arc::new(LocalAiLM::new(
        base_url.to_string(),
        "local-api-key".to_string(),
        model.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_to_messages() {
        let lm = LocalAiLM::new(
            "http://localhost:3001".to_string(),
            "test-key".to_string(),
            "test-model".to_string(),
        );

        let messages = lm.prompt_to_messages("Hello", Some("You are helpful"));
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "You are helpful");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, "Hello");
    }

    #[test]
    fn test_prompt_to_messages_no_system() {
        let lm = LocalAiLM::new(
            "http://localhost:3001".to_string(),
            "test-key".to_string(),
            "test-model".to_string(),
        );

        let messages = lm.prompt_to_messages("Hello", None);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello");
    }
}
