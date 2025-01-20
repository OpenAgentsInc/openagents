pub mod ws;

use crate::server::services::{DeepSeekService, GitHubService, RepomapService};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct SolverService {
    repomap_service: Arc<RepomapService>,
    deepseek_service: Arc<DeepSeekService>,
    github_service: Arc<GitHubService>,
}

impl Default for SolverService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SolverResponse {
    pub solution: String,
}

impl SolverService {
    pub fn new() -> Self {
        let aider_api_key = std::env::var("AIDER_API_KEY").expect("AIDER_API_KEY must be set");
        let deepseek_api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set");
        let github_token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");

        Self {
            repomap_service: Arc::new(RepomapService::new(aider_api_key)),
            deepseek_service: Arc::new(DeepSeekService::new(deepseek_api_key)),
            github_service: Arc::new(GitHubService::new(github_token)),
        }
    }

    pub async fn solve_issue(&self, issue_url: String) -> Result<SolverResponse> {
        // Create a temporary broadcast channel for this request
        let (tx, _) = broadcast::channel(100);
        self.solve_issue_with_ws(issue_url, tx).await
    }
}