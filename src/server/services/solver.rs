use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::info;
use crate::server::services::RepomapService;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SolverService {
    client: Client,
    repomap_service: Arc<RepomapService>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SolverResponse {
    pub solution: String,
}

impl SolverService {
    pub fn new() -> Self {
        let api_key = std::env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
        Self {
            client: Client::new(),
            repomap_service: Arc::new(RepomapService::new(api_key)),
        }
    }

    pub async fn solve_issue(&self, issue_url: String) -> Result<SolverResponse> {
        info!("Processing issue: {}", issue_url);
        
        // Extract repo URL from issue URL
        // Example: https://github.com/username/repo/issues/1 -> https://github.com/username/repo
        let repo_url = if issue_url.contains("/issues/") {
            issue_url
                .split("/issues/")
                .next()
                .unwrap_or(&issue_url)
                .to_string()
        } else if issue_url.contains("github.com") {
            // If it's already a repo URL, use it directly
            issue_url.trim_end_matches('/').to_string()
        } else {
            return Err(anyhow::anyhow!("Invalid GitHub URL format"));
        };

        info!("Extracted repo URL: {}", repo_url);
        
        // Generate repomap
        match self.repomap_service.generate_repomap(repo_url).await {
            Ok(repomap_response) => {
                // Take first 200 characters of the repomap
                let preview = repomap_response.repo_map
                    .chars()
                    .take(200)
                    .collect::<String>();
                
                Ok(SolverResponse {
                    solution: format!("<pre><code>{}</code></pre>", preview),
                })
            }
            Err(e) => {
                // Return a more user-friendly error message
                Ok(SolverResponse {
                    solution: format!(
                        "<div class='text-red-500'>Error: {}</div>", 
                        e
                    ),
                })
            }
        }
    }
}
