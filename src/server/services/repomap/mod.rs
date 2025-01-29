use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

pub struct RepomapService {
    client: Client,
    base_url: String,
}

impl RepomapService {
    pub fn new() -> Self {
        let base_url = std::env::var("REPOMAP_API_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());

        Self {
            client: Client::new(),
            base_url,
        }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    pub async fn generate_repomap(&self, repo_url: &str) -> Result<Value> {
        let response = self
            .client
            .post(&format!("{}/repomap/generate", self.base_url))
            .json(&serde_json::json!({
                "repo_url": repo_url,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow::anyhow!("Repomap API error: {}", error));
        }

        Ok(response.json().await?)
    }
}