use crate::server::services::gateway::{types::GatewayMetadata, Gateway};
use anyhow::Result;
use futures_util::{Stream, StreamExt};
use serde_json::Value;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info};

pub struct OllamaService {
    config: super::config::OllamaConfig,
}

impl Default for OllamaService {
    fn default() -> Self {
        Self {
            config: super::config::OllamaConfig::global().clone(),
        }
    }
}

impl OllamaService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(base_url: &str, model: &str) -> Self {
        Self {
            config: super::config::OllamaConfig {
                base_url: base_url.to_string(),
                model: model.to_string(),
            },
        }
    }

    pub async fn chat_structured<T>(&self, prompt: String, format: Value) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let client = reqwest::Client::new();
        let request_body = serde_json::json!({
            "model": self.config.model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "stream": false,
            "format": format,
            "options": {
                "temperature": 0
            }
        });

        info!(
            "Sending request to Ollama",
            // serde_json::to_string_pretty(&request_body)?
        );

        let response = client
            .post(format!("{}/api/chat", self.config.base_url))
            .json(&request_body)
            .send()
            .await?;

        let response_text = response.text().await?;
        info!("Raw response from Ollama: {}", response_text);

        let response_json: Value = serde_json::from_str(&response_text)?;
        info!(
            "Parsed JSON response: {}",
            serde_json::to_string_pretty(&response_json)?
        );

        let content = response_json["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid response format from Ollama - content not found in message"
                )
            })?;

        info!("Extracted content: {}", content);

        serde_json::from_str(content).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse structured response: {} - Raw content: {}",
                e,
                content
            )
        })
    }
}

#[async_trait::async_trait]
impl Gateway for OllamaService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "ollama".to_string(),
            openai_compatible: false,
            supported_features: vec!["chat".to_string(), "streaming".to_string()],
            default_model: self.config.model.clone(),
            available_models: vec![self.config.model.clone()],
        }
    }

    async fn chat(&self, prompt: String, _use_reasoner: bool) -> Result<(String, Option<String>)> {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/chat", self.config.base_url))
            .json(&serde_json::json!({
                "model": self.config.model,
                "messages": [{
                    "role": "user",
                    "content": prompt
                }],
                "stream": false
            }))
            .send()
            .await?;

        let response_json = response.json::<serde_json::Value>().await?;
        let content = response_json["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format from Ollama"))?;

        Ok((content.to_string(), None))
    }

    async fn chat_stream(
        &self,
        prompt: String,
        _use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let (tx, rx) = mpsc::channel(100);
        let client = reqwest::Client::new();
        let config = self.config.clone();

        debug!("Starting Ollama chat stream with URL: {}", config.base_url);

        tokio::spawn(async move {
            let response = client
                .post(format!("{}/api/chat", config.base_url))
                .json(&serde_json::json!({
                    "model": config.model,
                    "messages": [{
                        "role": "user",
                        "content": prompt
                    }],
                    "stream": true
                }))
                .send()
                .await;

            match response {
                Ok(response) => {
                    debug!("Got response from Ollama, starting stream");
                    let mut stream = response.bytes_stream();
                    let mut full_response = String::new();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                    debug!("Received chunk: {}", text);
                                    if let Ok(json) =
                                        serde_json::from_str::<serde_json::Value>(&text)
                                    {
                                        if let Some(content) = json["message"]["content"].as_str() {
                                            debug!("Extracted content: {}", content);
                                            let _ = tx.send(Ok(content.to_string())).await;
                                            full_response.push_str(content);
                                        }
                                        if json["done"].as_bool().unwrap_or(false) {
                                            debug!("Stream done, full response: {}", full_response);
                                            break;
                                        }
                                    } else {
                                        error!("Failed to parse JSON from chunk: {}", text);
                                    }
                                } else {
                                    error!("Failed to parse UTF-8 from chunk");
                                }
                            }
                            Err(e) => {
                                error!("Error in stream chunk: {}", e);
                                let _ = tx.send(Err(anyhow::anyhow!(e))).await;
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to connect to Ollama: {}", e);
                    let _ = tx.send(Err(anyhow::anyhow!(e))).await;
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
