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

        let repomap: RepomapResponse = serde_json::from_str(&text)?;
        Ok(repomap)
    }
}
