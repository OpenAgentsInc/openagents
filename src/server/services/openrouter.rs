use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::info;

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
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://openrouter.ai/api/v1".to_string(),
        }
    }

    pub async fn inference(&self, prompt: String) -> Result<InferenceResponse> {
        info!("Making inference request to OpenRouter");

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://openagents.com")
            .json(&serde_json::json!({
                "model": "anthropic/claude-2",
                "messages": [{
                    "role": "user",
                    "content": prompt
                }]
            }))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!(
                "OpenRouter API error ({}): {}", 
                status,
                error_text
            ));
        }

        let response_json: serde_json::Value = response.json().await?;
        let output = response_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?
            .to_string();

        Ok(InferenceResponse { output })
    }
}
