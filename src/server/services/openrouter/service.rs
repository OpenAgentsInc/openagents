use crate::server::services::gateway::{Gateway, GatewayMetadata};
use crate::server::services::openrouter::types::{OpenRouterConfig, OpenRouterMessage, OpenRouterRequest, OpenRouterResponse};
use anyhow::{anyhow, Result};
use reqwest::Client;
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

    fn prepare_messages(&self, prompt: String) -> Vec<OpenRouterMessage> {
        vec![OpenRouterMessage {
            role: "user".to_string(),
            content: prompt,
        }]
    }

    async fn make_request(&self, prompt: String) -> Result<OpenRouterResponse> {
        if Self::is_test_mode() {
            return Ok(OpenRouterResponse {
                id: "test".to_string(),
                model: self.get_model().to_string(),
                choices: vec![crate::server::services::openrouter::types::OpenRouterChoice {
                    message: OpenRouterMessage {
                        role: "assistant".to_string(),
                        content: crate::server::services::openrouter::test_responses::get_file_list_test_response(),
                    },
                    finish_reason: Some("stop".to_string()),
                    index: 0,
                }],
                usage: Some(crate::server::services::openrouter::types::OpenRouterUsage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
            });
        }

        let request = OpenRouterRequest {
            model: self.get_model().to_string(),
            messages: self.prepare_messages(prompt),
            temperature: Some(self.config.temperature),
            max_tokens: self.config.max_tokens,
            top_p: self.config.top_p,
            frequency_penalty: self.config.frequency_penalty,
            presence_penalty: self.config.presence_penalty,
            stop: self.config.stop.clone(),
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
}

#[async_trait::async_trait]
impl Gateway for OpenRouterService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "OpenRouter".to_string(),
            openai_compatible: true,
            supported_features: vec!["chat".to_string()],
            default_model: self.get_model().to_string(),
            available_models: vec![
                "deepseek/deepseek-r1-distill-llama-70b".to_string(),
            ],
        }
    }

    async fn chat(&self, prompt: String, _use_reasoner: bool) -> Result<(String, Option<String>)> {
        let response = self.make_request(prompt).await?;
        let content = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow!("No content in response"))?;

        Ok((content, None))
    }

    async fn chat_stream(&self, prompt: String, _use_reasoner: bool) -> mpsc::Receiver<crate::server::services::StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let service = self.clone();

        tokio::spawn(async move {
            match service.make_request(prompt).await {
                Ok(response) => {
                    if let Some(choice) = response.choices.first() {
                        let _ = tx.send(crate::server::services::StreamUpdate::Content(choice.message.content.clone())).await;
                    }
                }
                Err(e) => {
                    let _ = tx.send(crate::server::services::StreamUpdate::Error(e.to_string())).await;
                }
            }
        });

        rx
    }
}