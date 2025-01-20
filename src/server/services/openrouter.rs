use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct OpenRouterService {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub output: String,
}

impl OpenRouterService {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))  // 2 minute timeout
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            base_url: "https://openrouter.ai/api/v1".to_string(),
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))  // 2 minute timeout
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub async fn inference(&self, prompt: String) -> Result<InferenceResponse> {
        info!("Making inference request to OpenRouter");
        info!("Sending prompt to OpenRouter: {}", prompt);

        let request_body = serde_json::json!({
            "model": "deepseek/deepseek-chat",
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        });

        info!(
            "Request body: {}",
            serde_json::to_string_pretty(&request_body)?
        );

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://openagents.com")
            .header("X-Title", "OpenAgents")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        info!("OpenRouter response status: {}", status);
        info!("OpenRouter response body: {}", response_text);

        if !status.is_success() {
            error!("OpenRouter API error ({}): {}", status, response_text);
            return Err(anyhow::anyhow!(
                "OpenRouter API error ({}): {}",
                status,
                response_text
            ));
        }

        let response_json: serde_json::Value = serde_json::from_str(&response_text)?;
        let output = response_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?
            .to_string();

        info!("Extracted output: {}", output);

        Ok(InferenceResponse { output })
    }
}