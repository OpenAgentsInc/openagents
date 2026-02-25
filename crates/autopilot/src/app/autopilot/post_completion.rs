//! Post-completion hook system for autopilot.
//!
//! Handles verification, issue lifecycle, and auto-continuation after task completion.

use std::path::PathBuf;

use autopilot_core::dspy_verify::{VerificationInput, VerificationPipeline, VerificationVerdict};
use tokio::process::Command;

/// Event sent to trigger post-completion processing.
#[derive(Debug, Clone)]
pub(crate) struct PostCompletionEvent {
    /// Issue UUID being worked on
    pub(crate) issue_id: String,
    /// Issue number for display
    pub(crate) issue_number: i32,
    /// Issue title for retry prompts
    pub(crate) issue_title: String,
    /// Issue description for verification
    pub(crate) issue_description: Option<String>,
    /// Workspace root path
    pub(crate) workspace_root: PathBuf,
    /// Summary of the task from assistant's response
    pub(crate) task_summary: String,
    /// Current retry count
    pub(crate) retry_count: u8,
    /// Whether to auto-start next issue
    pub(crate) autopilot_continuous: bool,
}

/// Information about the next issue to work on.
#[derive(Debug, Clone)]
pub(crate) struct NextIssueInfo {
    pub(crate) id: String,
    pub(crate) number: i32,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
}

/// Result from post-completion processing.
#[derive(Debug, Clone)]
pub(crate) enum PostCompletionResult {
    /// Verification passed, issue marked complete
    Success {
        issue_number: i32,
        next_issue: Option<NextIssueInfo>,
    },
    /// Verification failed, should retry
    RetryNeeded {
        issue_number: i32,
        issue_title: String,
        reason: String,
    },
    /// Retry exhausted, moving to next issue
    MovingToNext {
        failed_issue_number: i32,
        reason: String,
        next_issue: Option<NextIssueInfo>,
    },
    /// No more issues available
    NoMoreIssues,
    /// Error during processing
    Error(String),
}

/// Post-completion hook processor.
pub(crate) struct PostCompletionHook {
    pipeline: VerificationPipeline,
}

impl Default for PostCompletionHook {
    fn default() -> Self {
        Self::new()
    }
}

impl PostCompletionHook {
    pub(crate) fn new() -> Self {
        Self {
            pipeline: VerificationPipeline::new(),
        }
    }

    /// Process a completion event.
    pub(crate) async fn process(&self, event: PostCompletionEvent) -> PostCompletionResult {
        tracing::info!(
            issue_number = event.issue_number,
            retry_count = event.retry_count,
            continuous = event.autopilot_continuous,
            "Post-completion: starting verification"
        );

        // 1. Run DSPy verification
        let verification_result: anyhow::Result<autopilot_core::dspy_verify::VerificationResult> =
            self.verify_requirements(&event).await;

        match verification_result {
            Ok(result) => {
                tracing::info!(
                    verdict = ?result.verdict,
                    confidence = result.confidence,
                    "Post-completion: verification complete"
                );

                match result.verdict {
                    VerificationVerdict::Pass => {
                        // Mark issue as complete
                        if let Err(e) =
                            self.complete_issue_in_db(&event.issue_id, &event.workspace_root)
                        {
                            tracing::error!(error = %e, "Post-completion: failed to mark issue complete");
                            return PostCompletionResult::Error(format!(
                                "Failed to mark issue complete: {}",
                                e
                            ));
                        }

                        tracing::info!(
                            issue_number = event.issue_number,
                            "Post-completion: issue marked complete"
                        );

                        // Get next issue if in continuous mode
                        let next_issue = if event.autopilot_continuous {
                            self.get_next_issue(&event.workspace_root).ok().flatten()
                        } else {
                            None
                        };

                        PostCompletionResult::Success {
                            issue_number: event.issue_number,
                            next_issue,
                        }
                    }
                    VerificationVerdict::Fail | VerificationVerdict::Retry => {
                        let reason = result.explanation;

                        if event.retry_count < 1 {
                            tracing::info!(
                                issue_number = event.issue_number,
                                "Post-completion: verification failed, will retry"
                            );
                            PostCompletionResult::RetryNeeded {
                                issue_number: event.issue_number,
                                issue_title: event.issue_title,
                                reason,
                            }
                        } else {
                            tracing::info!(
                                issue_number = event.issue_number,
                                "Post-completion: max retries reached, moving to next"
                            );
                            self.handle_verification_failure(&event, &reason)
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Post-completion: verification error");
                // On verification error, allow retry
                if event.retry_count < 1 {
                    PostCompletionResult::RetryNeeded {
                        issue_number: event.issue_number,
                        issue_title: event.issue_title,
                        reason: format!("Verification error: {}", e),
                    }
                } else {
                    self.handle_verification_failure(&event, &e.to_string())
                }
            }
        }
    }

    async fn verify_requirements(
        &self,
        event: &PostCompletionEvent,
    ) -> anyhow::Result<autopilot_core::dspy_verify::VerificationResult> {
        // Extract requirements from issue description
        let requirements = self.extract_requirements(&event.issue_description, &event.issue_title);

        // Get git diff for code changes
        let code_changes = self.get_git_diff(&event.workspace_root).await;

        let input = VerificationInput {
            requirements,
            solution_summary: event.task_summary.clone(),
            code_changes,
            build_output: String::new(), // Could run cargo check here
            test_output: String::new(),  // Could run tests here
        };

        self.pipeline.verify(&input).await
    }

    fn extract_requirements(&self, description: &Option<String>, title: &str) -> Vec<String> {
        // Start with title as primary requirement
        let mut requirements = vec![title.to_string()];

        // Parse description for additional requirements
        if let Some(desc) = description {
            // Look for bullet points or numbered items
            for line in desc.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('-') || trimmed.starts_with('*') || trimmed.starts_with("•")
                {
                    let req = trimmed
                        .trim_start_matches('-')
                        .trim_start_matches('*')
                        .trim_start_matches('•')
                        .trim();
                    if !req.is_empty() {
                        requirements.push(req.to_string());
                    }
                } else if trimmed
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
                {
                    // Numbered item like "1. Do something"
                    if let Some(idx) = trimmed.find('.') {
                        let req = trimmed[idx + 1..].trim();
                        if !req.is_empty() {
                            requirements.push(req.to_string());
                        }
                    }
                }
            }
        }

        requirements
    }

    async fn get_git_diff(&self, workspace_root: &PathBuf) -> String {
        // Get staged + unstaged changes
        let output = Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(workspace_root)
            .output()
            .await;

        match output {
            Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to get git diff");
                String::new()
            }
        }
    }

    fn complete_issue_in_db(&self, issue_id: &str, workspace_root: &PathBuf) -> anyhow::Result<()> {
        let db_path = workspace_root.join(".openagents").join("autopilot.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        issues::issue::complete_issue(&conn, issue_id)?;
        Ok(())
    }

    fn get_next_issue(&self, workspace_root: &PathBuf) -> anyhow::Result<Option<NextIssueInfo>> {
        let db_path = workspace_root.join(".openagents").join("autopilot.db");
        let conn = rusqlite::Connection::open(&db_path)?;

        if let Some(issue) = issues::issue::get_next_ready_issue(&conn, Some("codex"))? {
            Ok(Some(NextIssueInfo {
                id: issue.id,
                number: issue.number,
                title: issue.title,
                description: issue.description,
            }))
        } else {
            Ok(None)
        }
    }

    fn handle_verification_failure(
        &self,
        event: &PostCompletionEvent,
        reason: &str,
    ) -> PostCompletionResult {
        // Unclaim the issue so it can be picked up later
        let db_path = event
            .workspace_root
            .join(".openagents")
            .join("autopilot.db");
        if let Ok(conn) = rusqlite::Connection::open(&db_path) {
            let _ = issues::issue::unclaim_issue(&conn, &event.issue_id);
        }

        // Get next issue if in autopilot mode
        let next_issue = if event.autopilot_continuous {
            self.get_next_issue(&event.workspace_root).ok().flatten()
        } else {
            None
        };

        PostCompletionResult::MovingToNext {
            failed_issue_number: event.issue_number,
            reason: reason.to_string(),
            next_issue,
        }
    }
}
