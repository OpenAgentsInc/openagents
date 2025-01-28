use anyhow::Result;
use reqwest::{Client, ClientBuilder};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;

use crate::server::services::{
    gateway::{Gateway, GatewayMetadata},
    StreamUpdate,
};

use super::types::OpenRouterRequest;

/// OpenRouter service implementation
pub struct OpenRouterService {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenRouterService {
    pub fn new(api_key: String) -> Self {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(180)) // 3 minutes timeout
            .build()
            .expect("Failed to create HTTP client");

        let base_url = std::env::var("OPENROUTER_API_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

        info!("Using OpenRouter API URL: {}", base_url);

        Self {
            client,
            api_key,
            base_url,
        }
    }
}

#[async_trait::async_trait]
impl Gateway for OpenRouterService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "OpenRouter".to_string(),
            openai_compatible: true,
            supported_features: vec![
                "chat".to_string(),
                "streaming".to_string(),
            ],
            default_model: "openai/gpt-3.5-turbo".to_string(),
            available_models: vec![
                "openai/gpt-3.5-turbo".to_string(),
                "openai/gpt-4".to_string(),
                "anthropic/claude-2".to_string(),
            ],
        }
    }

    async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)> {
        // For initial implementation, we'll just return the prompt as-is
        // This allows us to test the interface without actual API calls
        Ok((prompt, if use_reasoner { Some("Reasoning".to_string()) } else { None }))
    }

    async fn chat_stream(&self, prompt: String, use_reasoner: bool) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        
        // For initial implementation, just send the prompt as a single message
        // This allows us to test the interface without actual API calls
        tokio::spawn(async move {
            let _ = tx.send(StreamUpdate::Content(prompt)).await;
            if use_reasoner {
                let _ = tx.send(StreamUpdate::Reasoning("Test reasoning".to_string())).await;
            }
            let _ = tx.send(StreamUpdate::Done).await;
        });

        rx
    }
}