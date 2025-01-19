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
                // First, ask for relevant files
                let files_prompt = format!(
                    "Given this GitHub repository map:\n\n{}\n\nAnd this issue URL: {}\n\n\
                    Based on the repository structure, return ONLY an array of file paths that would be most relevant to review for solving this issue.\n\
                    Return ONLY the JSON array, no other text.",
                    repomap_response.repo_map,
                    issue_url
                );

                match self.openrouter_service.inference(files_prompt).await {
                    Ok(files_response) => {
                        // Parse the response as a markdown list
                        let files: Vec<String> = files_response.output
                            .lines()
                            .filter(|line| line.starts_with("- "))
                            .map(|line| line.trim_start_matches("- ").to_string())
                            .collect();

                        // Create solution prompt with files list
                        let solution_prompt = format!(
                            "Given this GitHub repository map:\n\n{}\n\n\
                            And these relevant files:\n{}\n\n\
                            For this issue URL: {}\n\n\
                            Analyze the codebase and propose a solution to the issue.",
                            repomap_response.repo_map,
                            files.join("\n"),
                            issue_url
                        );

                        // Get solution from OpenRouter
                        match self.openrouter_service.inference(solution_prompt).await {
                            Ok(inference_response) => {
                                Ok(SolverResponse {
                                    solution: format!(
                                        "<div class='space-y-4'>\
                                        <div class='text-sm text-gray-400'>Relevant files:</div>\
                                        <pre><code>{}</code></pre>\
                                        <div class='text-sm text-gray-400'>Proposed solution:</div>\
                                        <pre><code>{}</code></pre>\
                                        </div>",
                                        files.join("\n"),
                                        inference_response.output
                                    ),
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
                        Ok(SolverResponse {
                            solution: format!(
                                "<div class='text-red-500'>Error identifying relevant files: {}</div>",
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
