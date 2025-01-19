use crate::server::services::{
    solver_ws::{SolverStage, SolverUpdate},
    GitHubService, OpenRouterService, RepomapService,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

#[derive(Debug, Clone)]
pub struct SolverService {
    repomap_service: Arc<RepomapService>,
    openrouter_service: Arc<OpenRouterService>,
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
        let openrouter_api_key =
            std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
        let github_token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");

        Self {
            repomap_service: Arc::new(RepomapService::new(aider_api_key)),
            openrouter_service: Arc::new(OpenRouterService::new(openrouter_api_key)),
            github_service: Arc::new(GitHubService::new(github_token)),
        }
    }

    pub async fn solve_issue_with_ws(
        &self,
        issue_url: String,
        update_tx: broadcast::Sender<SolverUpdate>,
    ) -> Result<SolverResponse> {
        info!("Processing issue: {}", issue_url);

        // Send initial progress update
        let _ = update_tx.send(SolverUpdate::Progress {
            stage: SolverStage::Init,
            message: "Starting solver analysis".into(),
            data: Some(serde_json::json!({
                "issue_url": issue_url
            })),
        });

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
            let err = anyhow::anyhow!("Invalid GitHub URL format");
            let _ = update_tx.send(SolverUpdate::Error {
                message: "Invalid GitHub URL format".into(),
                details: None,
            });
            return Err(err);
        };

        info!("Extracted repo URL: {}", repo_url);

        // Send repomap progress update
        let _ = update_tx.send(SolverUpdate::Progress {
            stage: SolverStage::Repomap,
            message: "Generating repository map".into(),
            data: Some(serde_json::json!({
                "repo_url": repo_url
            })),
        });

        // Generate repomap
        match self
            .repomap_service
            .generate_repomap(repo_url.clone())
            .await
        {
            Ok(repomap_response) => {
                // Get issue details from GitHub
                let (owner, repo, issue_number) = GitHubService::parse_issue_url(&issue_url)?;
                let issue = self
                    .github_service
                    .get_issue(&owner, &repo, issue_number)
                    .await?;

                // Send analysis progress update
                let _ = update_tx.send(SolverUpdate::Progress {
                    stage: SolverStage::Analysis,
                    message: "Analyzing repository structure".into(),
                    data: Some(serde_json::json!({
                        "repomap": repomap_response.repo_map
                    })),
                });

                // First, ask for relevant files
                let files_prompt = format!(
                    "Given this GitHub repository map:\n\n{}\n\n\
                    And this GitHub issue:\nTitle: {}\nDescription: {}\n\n\
                    Based on the repository structure and issue description, return a list of file paths that would be most relevant to review for solving this issue.\n\
                    Format your response as a markdown list with one file per line, starting each line with a hyphen (-).",
                    repomap_response.repo_map,
                    issue.title,
                    issue.body
                );

                match self.openrouter_service.inference(files_prompt).await {
                    Ok(files_response) => {
                        info!("Files response: {}", files_response.output);

                        // Parse the response as a markdown list
                        let files: Vec<String> = files_response
                            .output
                            .lines()
                            .filter(|line| line.trim().starts_with("- "))
                            .map(|line| line.trim().trim_start_matches("- ").trim().to_string())
                            .collect();

                        info!("Parsed files: {:?}", files);

                        // Send solution progress update
                        let _ = update_tx.send(SolverUpdate::Progress {
                            stage: SolverStage::Solution,
                            message: "Generating solution".into(),
                            data: Some(serde_json::json!({
                                "files": files
                            })),
                        });

                        // Create solution prompt with files list
                        let solution_prompt = format!(
                            "Given this GitHub repository map:\n\n{}\n\n\
                             And these relevant files:\n{}\n\n\
                             For this GitHub issue:\nTitle: {}\nDescription: {}\n\n\
                             Analyze the codebase and propose a solution to the issue.",
                            repomap_response.repo_map,
                            files.join("\n"),
                            issue.title,
                            issue.body
                        );

                        // Get solution from OpenRouter
                        match self.openrouter_service.inference(solution_prompt).await {
                            Ok(inference_response) => {
                                // Send PR progress update
                                let _ = update_tx.send(SolverUpdate::Progress {
                                    stage: SolverStage::PR,
                                    message: "Preparing solution".into(),
                                    data: Some(serde_json::json!({
                                        "solution": inference_response.output
                                    })),
                                });

                                let solution = format!(
                                    "<div class='space-y-4'>\
                                     <div class='text-sm text-gray-400'>Relevant files:</div>\
                                     <div class='max-w-4xl overflow-x-auto'>\
                                     <pre class='text-xs whitespace-pre-wrap break-words overflow-hidden'><code>{}</code></pre>\
                                     </div>\
                                     <div class='text-sm text-gray-400'>Proposed solution:</div>\
                                     <div class='max-w-4xl overflow-x-auto'>\
                                     <pre class='text-xs whitespace-pre-wrap break-words overflow-hidden'><code>{}</code></pre>\
                                     </div>\
                                     </div>",
                                    html_escape::encode_text(&files.join("\n")),
                                    html_escape::encode_text(&inference_response.output)
                                );

                                // Send complete update
                                let _ = update_tx.send(SolverUpdate::Complete {
                                    result: serde_json::json!({
                                        "solution": solution,
                                        "files": files,
                                        "analysis": inference_response.output
                                    }),
                                });

                                Ok(SolverResponse { solution })
                            }
                            Err(e) => {
                                let _ = update_tx.send(SolverUpdate::Error {
                                    message: "Error getting solution".into(),
                                    details: Some(e.to_string()),
                                });

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
                        let _ = update_tx.send(SolverUpdate::Error {
                            message: "Error identifying relevant files".into(),
                            details: Some(e.to_string()),
                        });

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
                let _ = update_tx.send(SolverUpdate::Error {
                    message: "Error generating repository map".into(),
                    details: Some(e.to_string()),
                });

                // Return a more user-friendly error message
                Ok(SolverResponse {
                    solution: format!("<div class='text-red-500'>Error: {}</div>", e),
                })
            }
        }
    }

    pub async fn solve_issue(&self, issue_url: String) -> Result<SolverResponse> {
        // Create a temporary broadcast channel for this request
        let (tx, _) = broadcast::channel(100);
        self.solve_issue_with_ws(issue_url, tx).await
    }
}