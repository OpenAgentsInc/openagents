use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone)]
pub struct RepomapService {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepomapResponse {
    pub repo_map: String,
    pub metadata: serde_json::Value,
}

impl RepomapService {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://aider.openagents.com".to_string(),
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
        }
    }

    pub async fn generate_repomap(&self, repo_url: String) -> Result<RepomapResponse> {
        info!("Making request to aider service for repo: {}", repo_url);

        let url = format!("{}/api/v1/repomap/generate", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-API-Key", &self.api_key)
            .json(&serde_json::json!({
                "repo_url": repo_url
            }))
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        info!("Received response with status: {}", status);
        info!("Response body: {}", text);

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(anyhow::anyhow!(
                "Authentication failed - please check your AIDER_API_KEY environment variable"
            ));
        } else if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Aider service error ({}): {}",
                status,
                text
            ));
        }

        serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Failed to parse repomap response: {}", e))
    }
}
