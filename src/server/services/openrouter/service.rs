use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures::StreamExt;
use reqwest::{Client, ClientBuilder};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::server::services::{
    gateway::Gateway,
    StreamUpdate,
};

use super::types::{
    OpenRouterConfig, OpenRouterError, OpenRouterMessage, OpenRouterRequest, OpenRouterResponse,
    OpenRouterStreamResponse,
};

/// OpenRouter service implementation
#[derive(Debug)]
pub struct OpenRouterService {
    client: Client,
    api_key: String,
    base_url: String,
    config: OpenRouterConfig,
    conversation_history: Vec<OpenRouterMessage>,
}

impl OpenRouterService {
    pub fn new() -> Result<Self> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(180)) // 3 minutes timeout
            .build()
            .expect("Failed to create HTTP client");

        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| anyhow!("OPENROUTER_API_KEY not found in environment"))?;

        let base_url = std::env::var("OPENROUTER_API_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

        info!("Using OpenRouter API URL: {}", base_url);

        Ok(Self {
            client,
            api_key,
            base_url,
            config: OpenRouterConfig::default(),
            conversation_history: Vec::new(),
        })
    }

    pub fn with_config(config: OpenRouterConfig) -> Result<Self> {
        let mut service = Self::new()?;
        service.config = config;
        Ok(service)
    }

    fn is_test_mode(&self) -> bool {
        self.api_key == "test-key" || std::env::var("OPENROUTER_TEST_MODE").is_ok()
    }

    fn get_model(&self, use_reasoner: bool) -> String {
        if use_reasoner {
            "anthropic/claude-2".to_string()
        } else {
            "openai/gpt-3.5-turbo".to_string()
        }
    }

    fn prepare_messages(&mut self, prompt: String) -> Vec<OpenRouterMessage> {
        let user_message = OpenRouterMessage {
            role: "user".to_string(),
            content: prompt,
            name: None,
        };

        // Add to history and return full conversation
        self.conversation_history.push(user_message.clone());
        self.conversation_history.clone()
    }

    fn update_history(&mut self, response: &OpenRouterMessage) {
        self.conversation_history.push(response.clone());
    }

    async fn make_request(&self, request: OpenRouterRequest) -> Result<OpenRouterResponse> {
        if self.is_test_mode() {
            // Return mock response for testing
            return Ok(OpenRouterResponse {
                id: "test-id".to_string(),
                model: request.model,
                choices: vec![super::types::OpenRouterChoice {
                    message: OpenRouterMessage {
                        role: "assistant".to_string(),
                        content: request.messages.last().unwrap().content.clone(),
                        name: None,
                    },
                    finish_reason: Some("stop".to_string()),
                    index: 0,
                }],
                usage: Some(super::types::OpenRouterUsage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
            });
        }

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://openagents.com")
            .header("X-Title", "OpenAgents")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error: OpenRouterError = response.json().await?;
            return Err(anyhow!(
                "OpenRouter API error: {} ({})",
                error.error.message,
                error.error.r#type
            ));
        }

        Ok(response.json().await?)
    }

    async fn process_stream_chunk(
        chunk: Bytes,
        buffer: &mut String,
        tx: &mpsc::Sender<StreamUpdate>,
    ) -> Result<bool> {
        let chunk_str = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_str);

        let mut done = false;
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            *buffer = buffer[pos + 1..].to_string();

            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    let _ = tx.send(StreamUpdate::Done).await;
                    done = true;
                    break;
                }

                if let Ok(response) = serde_json::from_str::<OpenRouterStreamResponse>(data) {
                    if let Some(choice) = response.choices.first() {
                        if let Some(content) = &choice.delta.content {
                            let _ = tx.send(StreamUpdate::Content(content.to_string())).await;
                        }
                        if choice.finish_reason.is_some() {
                            let _ = tx.send(StreamUpdate::Done).await;
                            done = true;
                            break;
                        }
                    }
                }
            }
        }

        Ok(done)
    }
}

#[async_trait::async_trait]
impl Gateway for OpenRouterService {
    fn metadata(&self) -> crate::server::services::gateway::types::GatewayMetadata {
        crate::server::services::gateway::types::GatewayMetadata {
            name: "OpenRouter".to_string(),
            openai_compatible: true,
            supported_features: vec![
                "chat".to_string(),
                "streaming".to_string(),
            ],
            default_model: "openai/gpt-3.5-turbo".to_string(),
            available_models: vec![
                "openai/gpt-3.5-turbo".to_string(),
                "openai/gpt-4".to_string(),
                "anthropic/claude-2".to_string(),
            ],
        }
    }

    async fn chat(&mut self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)> {
        let messages = self.prepare_messages(prompt);

        let request = OpenRouterRequest {
            model: self.get_model(use_reasoner),
            messages,
            stream: false,
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
            top_p: self.config.top_p,
            frequency_penalty: self.config.frequency_penalty,
            presence_penalty: self.config.presence_penalty,
            stop: self.config.stop.clone(),
        };

        let response = self.make_request(request).await?;
        
        if let Some(choice) = response.choices.first() {
            self.update_history(&choice.message);
            Ok((choice.message.content.clone(), None))
        } else {
            Err(anyhow!("No response from model"))
        }
    }

    async fn chat_stream(&mut self, prompt: String, use_reasoner: bool) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        
        if self.is_test_mode() {
            // Return mock stream for testing
            tokio::spawn(async move {
                let _ = tx.send(StreamUpdate::Content(prompt)).await;
                if use_reasoner {
                    let _ = tx.send(StreamUpdate::Reasoning("Test reasoning".to_string())).await;
                }
                let _ = tx.send(StreamUpdate::Done).await;
            });
            return rx;
        }

        let messages = self.prepare_messages(prompt);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();
        let model = self.get_model(use_reasoner);
        let config = self.config.clone();

        tokio::spawn(async move {
            let request = OpenRouterRequest {
                model,
                messages,
                stream: true,
                temperature: config.temperature,
                max_tokens: config.max_tokens,
                top_p: config.top_p,
                frequency_penalty: config.frequency_penalty,
                presence_penalty: config.presence_penalty,
                stop: config.stop,
            };

            let url = format!("{}/chat/completions", base_url);
            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("HTTP-Referer", "https://openagents.com")
                .header("X-Title", "OpenAgents")
                .json(&request)
                .send()
                .await;

            match response {
                Ok(response) => {
                    if !response.status().is_success() {
                        if let Ok(error) = response.json::<OpenRouterError>().await {
                            error!(
                                "OpenRouter API error: {} ({})",
                                error.error.message, error.error.r#type
                            );
                        }
                        return;
                    }

                    let mut stream = response.bytes_stream();
                    let mut buffer = String::new();

                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                if let Ok(true) =
                                    Self::process_stream_chunk(chunk, &mut buffer, &tx).await
                                {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Stream error: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Request error: {}", e);
                }
            }
        });

        rx
    }
}