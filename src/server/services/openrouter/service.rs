use crate::server::services::gateway::types::{ChatMessage, ChatResponse, ChatStreamResponse};
use crate::server::services::gateway::Gateway;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use tokio_stream::Stream;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct OpenRouterService {
    client: Client,
    api_key: String,
    config: OpenRouterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    pub model: String,
    pub use_reasoner: bool,
    pub test_mode: bool,
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            model: "deepseek/deepseek-chat".to_string(),
            use_reasoner: false,
            test_mode: false,
        }
    }
}

impl OpenRouterService {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            config: OpenRouterConfig::default(),
        }
    }

    pub fn with_config(api_key: String, config: OpenRouterConfig) -> Self {
        Self {
            client: Client::new(),
            api_key,
            config,
        }
    }

    pub fn is_test_mode(&self) -> bool {
        self.config.test_mode
    }

    fn get_model(&self) -> String {
        // Always use Claude for now
        "anthropic/claude-3.5-haiku".to_string()
    }

    fn prepare_messages(&self, messages: &[ChatMessage]) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect()
    }

    fn update_history(&self, history: &mut Vec<ChatMessage>, response: &str) {
        history.push(ChatMessage {
            role: "assistant".to_string(),
            content: response.to_string(),
        });
    }

    async fn make_request(
        &self,
        messages: &[ChatMessage],
        stream: bool,
    ) -> Result<reqwest::Response> {
        let model = self.get_model();
        debug!("Using model: {}", model);

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header(
                "HTTP-Referer",
                "https://github.com/OpenAgentsInc/openagents",
            )
            .json(&serde_json::json!({
                "model": model,
                "messages": self.prepare_messages(messages),
                "stream": stream
            }))
            .send()
            .await?;

        Ok(response)
    }

    fn process_stream_chunk(&self, chunk: &str) -> Result<Option<String>> {
        if chunk.is_empty() || chunk == "[DONE]" {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(chunk)?;
        let content = value["choices"][0]["delta"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if content.is_empty() {
            Ok(None)
        } else {
            Ok(Some(content))
        }
    }
}

#[async_trait::async_trait]
impl Gateway for OpenRouterService {
    async fn metadata(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "name": "OpenRouter",
            "version": "1.0.0",
            "models": ["deepseek/deepseek-chat", "anthropic/claude-3.5-haiku"]
        }))
    }

    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse> {
        // Return test response if in test mode
        if self.is_test_mode() {
            return Ok(ChatResponse {
                content: "Test response".to_string(),
            });
        }

        let response = self.make_request(messages, false).await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            warn!("OpenRouter API error: {}", error);
            return Err(anyhow!("OpenRouter API error: {}", error));
        }

        let json: Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid response format"))?
            .to_string();

        Ok(ChatResponse { content })
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatStreamResponse>> + Send>>> {
        // Return test stream if in test mode
        if self.is_test_mode() {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                tx.send(Ok(ChatStreamResponse {
                    content: "Test response".to_string(),
                }))
                .await
                .ok();
            });
            return Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)));
        }

        let response = self.make_request(messages, true).await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            warn!("OpenRouter API error: {}", error);
            return Err(anyhow!("OpenRouter API error: {}", error));
        }

        let mut stream = response.bytes_stream();
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let service = self.clone();

        tokio::spawn(async move {
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let chunk_str = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&chunk_str);

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim();
                            buffer = buffer[pos + 1..].to_string();

                            if !line.is_empty() && line.starts_with("data: ") {
                                let data = &line["data: ".len()..];
                                if let Ok(Some(content)) = service.process_stream_chunk(data) {
                                    tx.send(Ok(ChatStreamResponse { content }))
                                        .await
                                        .ok();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tx.send(Err(anyhow!("Stream error: {}", e))).await.ok();
                        break;
                    }
                }
            }
        });

        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}