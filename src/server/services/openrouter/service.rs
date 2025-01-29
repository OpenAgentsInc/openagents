use crate::server::services::gateway::Gateway;
use crate::server::services::gateway::types::GatewayMetadata;
use anyhow::{anyhow, Result};
use futures::StreamExt;
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

    fn prepare_messages(&self, prompt: &str) -> Vec<Value> {
        vec![serde_json::json!({
            "role": "user",
            "content": prompt
        })]
    }

    async fn make_request(
        &self,
        prompt: &str,
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
                "messages": self.prepare_messages(prompt),
                "stream": stream
            }))
            .send()
            .await?;

        Ok(response)
    }

    fn process_stream_chunk(chunk: &[u8]) -> Result<Option<String>> {
        if chunk.is_empty() {
            return Ok(None);
        }

        let chunk_str = String::from_utf8_lossy(chunk);
        if chunk_str == "[DONE]" {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(&chunk_str)?;
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
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "OpenRouter".to_string(),
            openai_compatible: true,
            supported_features: vec!["chat".to_string(), "streaming".to_string()],
            default_model: "anthropic/claude-3.5-haiku".to_string(),
            available_models: vec![
                "anthropic/claude-3.5-haiku".to_string(),
                "deepseek/deepseek-chat".to_string(),
            ],
        }
    }

    async fn chat(&self, prompt: String, _use_reasoner: bool) -> Result<(String, Option<String>)> {
        // Return test response if in test mode
        if self.is_test_mode() {
            return Ok(("Test response".to_string(), None));
        }

        let response = self.make_request(&prompt, false).await?;

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

        Ok((content, None))
    }

    async fn chat_stream(
        &self,
        prompt: String,
        _use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // Return test stream if in test mode
        if self.is_test_mode() {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                tx.send(Ok("Test response".to_string()))
                    .await
                    .ok();
            });
            return Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)));
        }

        let response = self.make_request(&prompt, true).await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            warn!("OpenRouter API error: {}", error);
            return Err(anyhow!("OpenRouter API error: {}", error));
        }

        let mut stream = response.bytes_stream();
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            let mut buffer = Vec::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        buffer.extend_from_slice(&chunk);

                        // Process complete messages
                        while let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
                            let message = buffer[..pos].to_vec();
                            buffer = buffer[pos + 2..].to_vec();

                            if let Ok(Some(content)) = Self::process_stream_chunk(&message) {
                                tx.send(Ok(content))
                                    .await
                                    .ok();
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