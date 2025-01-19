use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;
use crate::server::services::{RepomapService, OpenRouterService};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SolverService {
    repomap_service: Arc<RepomapService>,
    openrouter_service: Arc<OpenRouterService>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SolverResponse {
    pub solution: String,
}

impl SolverService {
    pub fn new() -> Self {
        let aider_api_key = std::env::var("AIDER_API_KEY").expect("AIDER_API_KEY must be set");
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
        
        Self {
            repomap_service: Arc::new(RepomapService::new(aider_api_key)),
            openrouter_service: Arc::new(OpenRouterService::new(openrouter_api_key)),
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
        match self.repomap_service.generate_repomap(repo_url.clone()).await {
            Ok(repomap_response) => {
                // Create prompt for OpenRouter
                let prompt = format!(
                    "Given this GitHub repository map:\n\n{}\n\nAnd this issue URL: {}\n\nAnalyze the codebase and propose a solution to the issue.",
                    repomap_response.repo_map,
                    issue_url
                );

                // Get solution from OpenRouter
                match self.openrouter_service.inference(prompt).await {
                    Ok(inference_response) => {
                        Ok(SolverResponse {
                            solution: format!("<pre><code>{}</code></pre>", inference_response.output),
                        })
                    }
                    Err(e) => {
                        Ok(SolverResponse {
                            solution: format!(
                                "<div class='text-red-500'>Error getting solution: {}</div>",
                                e
                            ),
                        })
                    }
                }
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
