use crate::server::services::gateway::{
    types::{ChatRequest, GatewayMetadata, Message},
    Gateway,
};
use crate::server::services::StreamUpdate;
use crate::server::services::ollama::types::*;
use anyhow::{anyhow, Result};
use reqwest::Client;
use tokio::sync::mpsc;
use futures_util::StreamExt;

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

    async fn chat_stream(&self, prompt: String, _use_reasoner: bool) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let config = self.config.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            let request = ChatRequest {
                model: config.model,
                messages: vec![Message {
                    role: "user".to_string(),
                    content: prompt,
                }],
                stream: true,
                temperature: 0.7,
                max_tokens: None,
            };

            match client
                .post(format!("{}/api/chat", config.base_url))
                .json(&request)
                .send()
                .await
            {
                Ok(response) => {
                    let mut stream = response.bytes_stream();

                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                match Self::process_stream_chunk(&chunk).await {
                                    Ok(Some(content)) => {
                                        if tx.send(StreamUpdate::Content(content)).await.is_err() {
                                            break;
                                        }
                                    }
                                    Ok(None) => break,
                                    Err(e) => {
                                        let _ = tx.send(StreamUpdate::Error(e.to_string())).await;
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(StreamUpdate::Error(e.to_string())).await;
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(StreamUpdate::Error(e.to_string())).await;
                }
            }

            let _ = tx.send(StreamUpdate::Done).await;
        });

        rx
    }
}