use crate::server::services::gateway::{
    types::{ChatRequest, GatewayMetadata, Message},
    Gateway,
};
use crate::server::services::ollama::types::*;
use anyhow::{anyhow, Result};
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use std::pin::Pin;
use tokio_stream::wrappers::ReceiverStream;

pub struct OllamaService {
    config: OllamaConfig,
    client: Client,
}

impl OllamaService {
    pub fn new() -> Self {
        Self {
            config: OllamaConfig::default(),
            client: Client::new(),
        }
    }

    pub fn with_config(base_url: &str, model: &str) -> Self {
        Self {
            config: OllamaConfig {
                base_url: base_url.to_string(),
                model: model.to_string(),
            },
            client: Client::new(),
        }
    }

    async fn make_request(&self, prompt: String, stream: bool) -> Result<reqwest::Response> {
        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            stream,
            temperature: 0.7,
            max_tokens: None,
        };

        let response = self.client
            .post(format!("{}/api/chat", self.config.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            return Err(anyhow!(error.error));
        }

        Ok(response)
    }

    async fn process_stream_chunk(chunk: &[u8]) -> Result<Option<String>> {
        if chunk.is_empty() {
            return Ok(None);
        }

        let response: OllamaChatResponse = serde_json::from_slice(chunk)?;
        if response.done {
            return Ok(None);
        }

        Ok(Some(response.message.content))
    }
}

#[async_trait::async_trait]
impl Gateway for OllamaService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "ollama".to_string(),
            openai_compatible: false,
            supported_features: vec![
                "chat".to_string(),
                "streaming".to_string(),
            ],
            default_model: self.config.model.clone(),
            available_models: vec![self.config.model.clone()],
        }
    }

    async fn chat(&self, prompt: String, _use_reasoner: bool) -> Result<(String, Option<String>)> {
        let response = self.make_request(prompt, false).await?;
        let chat_response: OllamaChatResponse = response.json().await?;
        Ok((chat_response.message.content, None))
    }

    async fn chat_stream(
        &self,
        prompt: String,
        _use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let response = self.make_request(prompt, true).await?;
        let stream = response.bytes_stream();

        let content_stream = stream.map(|chunk_result| {
            chunk_result
                .map_err(|e| anyhow!(e))
                .and_then(|chunk| async move {
                    if let Some(content) = Self::process_stream_chunk(&chunk).await? {
                        Ok(content)
                    } else {
                        Err(anyhow!("Stream ended"))
                    }
                }.boxed().await)
        });

        Ok(Box::pin(content_stream))
    }
}