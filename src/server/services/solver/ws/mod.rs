use super::SolverResponse;
use crate::server::services::{
    solver_ws::{SolverStage, SolverUpdate},
    GitHubService,
    github::types::Issue,
};
use anyhow::Result;
use tokio::sync::broadcast;
use tracing::info;

mod files_analysis;
mod solution_generation;
mod url_parsing;
mod html_formatting;

pub(crate) use files_analysis::*;
pub(crate) use solution_generation::*;
pub(crate) use url_parsing::*;
pub(crate) use html_formatting::*;

impl super::SolverService {
    pub(crate) async fn solve_issue_with_ws(
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

        // Parse URL and get repo info
        let repo_url = parse_repo_url(&issue_url, &update_tx)?;
        info!("Extracted repo URL: {}", repo_url);

        // Generate repomap and process
        match self.process_repomap(&repo_url, &issue_url, update_tx.clone()).await {
            Ok(solution) => Ok(SolverResponse { solution }),
            Err(e) => {
                let _ = update_tx.send(SolverUpdate::Error {
                    message: "Error generating repository map".into(),
                    details: Some(e.to_string()),
                });

                Ok(SolverResponse {
                    solution: format!("<div class='text-red-500'>Error: {}</div>", e),
                })
            }
        }
    }

    async fn process_repomap(
        &self,
        repo_url: &str,
        issue_url: &str,
        update_tx: broadcast::Sender<SolverUpdate>,
    ) -> Result<String> {
        // Send repomap progress update
        let _ = update_tx.send(SolverUpdate::Progress {
            stage: SolverStage::Repomap,
            message: "Generating repository map".into(),
            data: Some(serde_json::json!({
                "repo_url": repo_url
            })),
        });

        // Generate repomap
        let repomap_response = self.repomap_service.generate_repomap(repo_url.to_string()).await?;

        // Get issue details
        let (owner, repo, issue_number) = GitHubService::parse_issue_url(issue_url)?;
        let issue = self.github_service.get_issue(&owner, &repo, issue_number).await?;

        // Send analysis progress update
        let _ = update_tx.send(SolverUpdate::Progress {
            stage: SolverStage::Analysis,
            message: "Analyzing repository structure".into(),
            data: Some(serde_json::json!({
                "repomap": repomap_response.repo_map
            })),
        });

        // Analyze files
        let (files, files_reasoning) = self
            .analyze_files(&repomap_response.repo_map, &issue, update_tx.clone())
            .await?;

        // Generate solution
        let (solution_text, solution_reasoning) = self
            .generate_solution(&repomap_response.repo_map, &files, &issue, update_tx.clone())
            .await?;

        // Format final HTML
        let solution = format_solution_html(&files_reasoning, &files, &solution_reasoning, &solution_text);

        // Send complete update
        let _ = update_tx.send(SolverUpdate::Complete {
            result: serde_json::json!({
                "solution": solution,
                "files": files,
                "analysis": solution_text,
                "files_reasoning": files_reasoning,
                "solution_reasoning": solution_reasoning
            }),
        });

        Ok(solution)
    }
}
