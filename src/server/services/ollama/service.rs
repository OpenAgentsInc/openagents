use crate::server::services::gateway::{Gateway, GatewayMetadata, StreamUpdate};
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
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            stream,
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

        let response: ChatResponse = serde_json::from_slice(chunk)?;
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
            description: "Local model execution via Ollama".to_string(),
            model: Some(self.config.model.clone()),
        }
    }

    async fn chat(&self, prompt: String, _use_reasoner: bool) -> Result<(String, Option<String>)> {
        let response = self.make_request(prompt, false).await?;
        let chat_response: ChatResponse = response.json().await?;
        Ok((chat_response.message.content, None))
    }

    async fn chat_stream(&self, prompt: String, _use_reasoner: bool) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let config = self.config.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            let request = ChatRequest {
                model: config.model,
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                }],
                stream: true,
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