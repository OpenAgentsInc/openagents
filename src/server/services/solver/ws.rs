use super::SolverResponse;
use crate::server::services::{
    solver_ws::{SolverStage, SolverUpdate},
    GitHubService,
};
use anyhow::Result;
use tokio::sync::broadcast;
use tracing::{error, info};

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

                // First, ask for relevant files using reasoning
                let files_prompt = format!(
                    "Given this GitHub repository map:\n\n{}\n\n\
                    And this GitHub issue:\nTitle: {}\nDescription: {}\n\n\
                    Based on the repository structure and issue description, analyze which files would be most relevant to review for solving this issue.\n\
                    Consider:\n\
                    1. Files that would need to be modified\n\
                    2. Related files for context\n\
                    3. Test files that would need updating\n\
                    4. Configuration files if relevant\n\n\
                    Format your final answer as a markdown list with one file per line, starting each line with a hyphen (-).",
                    repomap_response.repo_map,
                    issue.title,
                    issue.body
                );

                match self.deepseek_service.chat(files_prompt, true).await {
                    Ok((files_list, files_reasoning)) => {
                        info!("Files response: {}", files_list);
                        if let Some(reasoning) = &files_reasoning {
                            info!("Files reasoning: {}", reasoning);
                        }

                        // Parse the response as a markdown list
                        let files: Vec<String> = files_list
                            .lines()
                            .filter(|line| line.trim().starts_with("- "))
                            .map(|line| line.trim().trim_start_matches("- ").trim().to_string())
                            .collect();

                        info!("Parsed files: {:?}", files);

                        // Send solution progress update with reasoning
                        let _ = update_tx.send(SolverUpdate::Progress {
                            stage: SolverStage::Solution,
                            message: "Analyzing solution approach".into(),
                            data: Some(serde_json::json!({
                                "files": files,
                                "reasoning": files_reasoning.unwrap_or_else(|| "No reasoning provided".into())
                            })),
                        });

                        // Create solution prompt with files list
                        let solution_prompt = format!(
                            "Given this GitHub repository map:\n\n{}\n\n\
                             And these relevant files:\n{}\n\n\
                             For this GitHub issue:\nTitle: {}\nDescription: {}\n\n\
                             Analyze and provide a detailed solution including:\n\
                             1. Specific code changes needed (with file paths)\n\
                             2. Any new files that need to be created\n\
                             3. Step-by-step implementation instructions\n\
                             4. Potential risks or considerations\n\
                             Format the response in markdown with code blocks for any code changes.",
                            repomap_response.repo_map,
                            files.join("\n"),
                            issue.title,
                            issue.body
                        );

                        // Get solution from DeepSeek with reasoning
                        match self.deepseek_service.chat(solution_prompt, true).await {
                            Ok((solution_text, solution_reasoning)) => {
                                // Send PR progress update with reasoning
                                let _ = update_tx.send(SolverUpdate::Progress {
                                    stage: SolverStage::PR,
                                    message: "Preparing solution".into(),
                                    data: Some(serde_json::json!({
                                        "solution": solution_text,
                                        "reasoning": solution_reasoning.unwrap_or_else(|| "No reasoning provided".into())
                                    })),
                                });

                                let solution = format!(
                                    r#"<div class='space-y-4'>
                                    <div class='bg-gray-800 rounded-lg p-4 mb-4'>
                                        <div class='text-sm text-yellow-400 mb-2'>File Selection Reasoning:</div>
                                        <div class='text-xs text-gray-300 whitespace-pre-wrap'>{}</div>
                                    </div>
                                    <div class='text-sm text-gray-400'>Relevant files:</div>
                                    <div class='max-w-4xl overflow-x-auto'>
                                        <pre class='text-xs whitespace-pre-wrap break-words overflow-hidden'><code>{}</code></pre>
                                    </div>
                                    <div class='bg-gray-800 rounded-lg p-4 mb-4'>
                                        <div class='text-sm text-yellow-400 mb-2'>Solution Reasoning:</div>
                                        <div class='text-xs text-gray-300 whitespace-pre-wrap'>{}</div>
                                    </div>
                                    <div class='text-sm text-gray-400'>Proposed solution:</div>
                                    <div class='max-w-4xl overflow-x-auto'>
                                        <pre class='text-xs whitespace-pre-wrap break-words overflow-hidden'><code>{}</code></pre>
                                    </div>
                                    </div>"#,
                                    html_escape::encode_text(&files_reasoning.unwrap_or_else(|| "No reasoning provided".into())),
                                    html_escape::encode_text(&files.join("\n")),
                                    html_escape::encode_text(&solution_reasoning.unwrap_or_else(|| "No reasoning provided".into())),
                                    html_escape::encode_text(&solution_text)
                                );

                                // Send complete update
                                let _ = update_tx.send(SolverUpdate::Complete {
                                    result: serde_json::json!({
                                        "solution": solution,
                                        "files": files,
                                        "analysis": solution_text,
                                        "files_reasoning": files_reasoning.unwrap_or_else(|| "No reasoning provided".into()),
                                        "solution_reasoning": solution_reasoning.unwrap_or_else(|| "No reasoning provided".into())
                                    }),
                                });

                                Ok(SolverResponse { solution })
                            }
                            Err(e) => {
                                error!("DeepSeek inference error: {}", e);
                                let _ = update_tx.send(SolverUpdate::Error {
                                    message: "Error getting solution".into(),
                                    details: Some(format!("DeepSeek error: {}. This could be due to a timeout or service issue. Please try again.", e)),
                                });

                                Ok(SolverResponse {
                                    solution: format!(
                                        "<div class='text-red-500'>Error getting solution: {}. Please try again.</div>",
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
}