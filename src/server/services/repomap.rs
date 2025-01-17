use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, error};

#[derive(Debug, Clone)]
pub struct RepomapService {
    client: Client,
    api_key: String,
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
        }
    }

    pub async fn generate_repomap(&self, repo_url: String) -> Result<RepomapResponse> {
        info!("Making request to aider service for repo: {}", repo_url);
        
        let response = self.client
            .post("https://aider.openagents.com/api/v1/repomap/generate")
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