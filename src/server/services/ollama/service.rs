use super::config::OllamaConfig;
use crate::server::services::gateway::Gateway;
use crate::server::services::deepseek::StreamUpdate;
use anyhow::Result;
use tokio::sync::mpsc;

pub struct OllamaService {
    config: OllamaConfig,
}

impl OllamaService {
    pub fn new() -> Self {
        Self {
            config: OllamaConfig::global().clone(),
        }
    }

    pub fn with_config(base_url: &str, model: &str) -> Self {
        Self {
            config: OllamaConfig {
                base_url: base_url.to_string(),
                model: model.to_string(),
            },
        }
    }
}

#[async_trait::async_trait]
impl Gateway for OllamaService {
    fn metadata(&self) -> crate::server::services::gateway::GatewayMetadata {
        crate::server::services::gateway::GatewayMetadata {
            name: "ollama".to_string(),
            openai_compatible: false,
            supported_features: vec!["chat".to_string(), "streaming".to_string()],
            default_model: self.config.model.clone(),
            available_models: vec![self.config.model.clone()],
        }
    }

    async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)> {
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
        use_reasoner: bool,
    ) -> Result<mpsc::Receiver<StreamUpdate>> {
        let (tx, rx) = mpsc::channel(100);
        let client = reqwest::Client::new();
        let config = self.config.clone();

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
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if let Some(content) = json["message"]["content"].as_str() {
                                            let _ = tx.send(StreamUpdate::Content(content.to_string())).await;
                                        }
                                        if json["done"].as_bool().unwrap_or(false) {
                                            let _ = tx.send(StreamUpdate::Done).await;
                                            break;
                                        }
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
        });

        Ok(rx)
    }
}