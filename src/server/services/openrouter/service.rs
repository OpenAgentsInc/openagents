use crate::server::services::gateway::{Gateway, GatewayMetadata, StreamUpdate};
use crate::server::services::openrouter::types::{OpenRouterConfig, OpenRouterRequest, OpenRouterResponse};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::json;
use tokio::sync::mpsc;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Clone)]
pub struct OpenRouterService {
    client: Client,
    config: OpenRouterConfig,
}

impl OpenRouterService {
    pub fn new(api_key: String) -> Result<Self> {
        let config = OpenRouterConfig {
            api_key,
            ..Default::default()
        };

        let client = Client::new();

        Ok(Self { client, config })
    }

    pub fn with_config(config: OpenRouterConfig) -> Result<Self> {
        let client = Client::new();
        Ok(Self { client, config })
    }

    pub fn is_test_mode() -> bool {
        std::env::var("OPENROUTER_TEST_MODE").is_ok()
    }

    fn get_model(&self) -> &str {
        &self.config.model
    }

    fn prepare_messages(&self, prompt: String) -> Vec<serde_json::Value> {
        vec![json!({
            "role": "user",
            "content": prompt
        })]
    }

    fn update_history(&self, _response: &str) {
        // TODO: Implement conversation history
    }

    async fn make_request(&self, prompt: String) -> Result<OpenRouterResponse> {
        if Self::is_test_mode() {
            return Ok(OpenRouterResponse {
                id: "test".to_string(),
                choices: vec![serde_json::from_value(json!({
                    "message": {
                        "content": crate::server::services::openrouter::test_responses::get_file_list_test_response()
                    }
                })).unwrap()],
            });
        }

        let request = OpenRouterRequest {
            model: self.get_model().to_string(),
            messages: self.prepare_messages(prompt),
        };

        let response = self
            .client
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("HTTP-Referer", "https://openagents.com")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow!("OpenRouter API error: {}", error));
        }

        let response: OpenRouterResponse = response.json().await?;
        Ok(response)
    }

    fn process_stream_chunk(&self, chunk: &str) -> Result<Option<String>> {
        if chunk.is_empty() {
            return Ok(None);
        }

        let response: OpenRouterResponse = serde_json::from_str(chunk)?;
        if let Some(choice) = response.choices.first() {
            if let Some(content) = &choice.message.content {
                return Ok(Some(content.clone()));
            }
        }
        Ok(None)
    }
}

#[async_trait::async_trait]
impl Gateway for OpenRouterService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "OpenRouter".to_string(),
            model: self.get_model().to_string(),
        }
    }

    async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)> {
        let response = self.make_request(prompt).await?;
        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| anyhow!("No content in response"))?;

        self.update_history(&content);

        if use_reasoner {
            Ok((content, None))
        } else {
            Ok((content, None))
        }
    }

    async fn chat_stream(&self, prompt: String, use_reasoner: bool) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let service = self.clone();

        tokio::spawn(async move {
            match service.make_request(prompt).await {
                Ok(response) => {
                    if let Some(choice) = response.choices.first() {
                        if let Some(content) = &choice.message.content {
                            let _ = tx.send(StreamUpdate::Content(content.clone())).await;
                            if use_reasoner {
                                let _ = tx.send(StreamUpdate::ReasoningContent(content.clone())).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(StreamUpdate::Error(e.to_string())).await;
                }
            }
        });

        rx
    }
}