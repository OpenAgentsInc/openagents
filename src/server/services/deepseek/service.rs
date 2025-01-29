use crate::server::services::gateway::{Gateway, GatewayMetadata};
use crate::server::services::StreamUpdate;
use anyhow::{anyhow, Result};
use reqwest::Client;
use tokio::sync::mpsc;

pub struct DeepSeekService {
    client: Client,
    api_key: String,
    base_url: String,
}

impl DeepSeekService {
    pub fn new(api_key: String) -> Result<Self> {
        let base_url = std::env::var("DEEPSEEK_API_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com/v1".to_string());

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url,
        })
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            api_key,
            base_url,
        })
    }

    async fn make_request(&self, prompt: String) -> Result<String> {
        if std::env::var("DEEPSEEK_TEST_MODE").is_ok() {
            return Ok(prompt);
        }

        let response = self
            .client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": "deepseek-chat",
                "messages": [{
                    "role": "user",
                    "content": prompt
                }]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow!("DeepSeek API error: {}", error));
        }

        let response: serde_json::Value = response.json().await?;
        let content = response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("No content in response"))?
            .to_string();

        Ok(content)
    }
}

#[async_trait::async_trait]
impl Gateway for DeepSeekService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "DeepSeek".to_string(),
            openai_compatible: true,
            supported_features: vec!["chat".to_string()],
            default_model: "deepseek-chat".to_string(),
            available_models: vec!["deepseek-chat".to_string()],
        }
    }

    async fn chat(&self, prompt: String, _use_reasoner: bool) -> Result<(String, Option<String>)> {
        let content = self.make_request(prompt).await?;
        Ok((content, None))
    }

    async fn chat_stream(&self, prompt: String, _use_reasoner: bool) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let service = self.clone();

        tokio::spawn(async move {
            match service.make_request(prompt).await {
                Ok(content) => {
                    let _ = tx.send(StreamUpdate::Content(content)).await;
                    let _ = tx.send(StreamUpdate::Done).await;
                }
                Err(e) => {
                    let _ = tx.send(StreamUpdate::Error(e.to_string())).await;
                }
            }
        });

        rx
    }
}

impl Clone for DeepSeekService {
    fn clone(&self) -> Self {
        Self {
            client: Client::new(),
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
        }
    }
}