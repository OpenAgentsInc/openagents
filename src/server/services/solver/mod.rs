mod types;

pub use types::*;

use crate::server::services::{deepseek::DeepSeekService, openrouter::OpenRouterService};
use anyhow::Result;
use sqlx::PgPool;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info};

#[allow(dead_code)] // These methods will be used in future
pub struct SolverService {
    pool: PgPool,
    openrouter: OpenRouterService,
    deepseek: DeepSeekService,
}

impl SolverService {
    pub fn new(pool: PgPool, openrouter: OpenRouterService, deepseek: DeepSeekService) -> Self {
        Self {
            pool,
            openrouter,
            deepseek,
        }
    }

    pub async fn create_solver(
        &self,
        issue_number: i32,
        issue_title: String,
        issue_body: String,
    ) -> Result<SolverState> {
        let state = SolverState::new(issue_number, issue_title, issue_body);

        // Store initial state in DB
        sqlx::query!(
            "INSERT INTO solver_states (id, status, issue_number, issue_title, issue_body, files, repo_path) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            state.id,
            serde_json::to_value(&state.status)?,
            state.issue_number,
            state.issue_title,
            state.issue_body,
            serde_json::to_value(&state.files)?,
            state.repo_path
        )
        .execute(&self.pool)
        .await?;

        info!("Created new solver state: {}", state.id);
        Ok(state)
    }

    pub async fn get_solver(&self, id: &str) -> Result<Option<SolverState>> {
        let record = sqlx::query!(
            "SELECT id, status, issue_number, issue_title, issue_body, files, repo_path FROM solver_states WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(record.map(|r| SolverState {
            id: r.id,
            status: serde_json::from_value(r.status).unwrap(),
            issue_number: r.issue_number,
            issue_title: r.issue_title,
            issue_body: r.issue_body,
            files: serde_json::from_value(r.files).unwrap(),
            repo_path: r.repo_path,
        }))
    }

    pub async fn update_solver(&self, state: &SolverState) -> Result<()> {
        sqlx::query!(
            "UPDATE solver_states SET status = $1, files = $2, repo_path = $3 WHERE id = $4",
            serde_json::to_value(&state.status)?,
            serde_json::to_value(&state.files)?,
            state.repo_path,
            state.id
        )
        .execute(&self.pool)
        .await?;

        info!("Updated solver state: {}", state.id);
        Ok(())
    }

    pub async fn start_generating_changes(
        &self,
        state: &mut SolverState,
        repo_dir: &str,
    ) -> Result<()> {
        info!("Starting to generate changes for solver {}", state.id);
        state.status = SolverStatus::GeneratingChanges;
        state.set_repo_path(repo_dir.to_string());
        self.update_solver(state).await?;

        // First get DeepSeek's analysis of all files
        let mut file_contents = String::new();
        for file in &state.files {
            let path = Path::new(repo_dir).join(&file.path);
            file_contents.push_str(&format!("\nFile: {}\n", file.path));

            // Check if file exists
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    file_contents.push_str(&format!("Content:\n{}\n", content));
                } else {
                    file_contents.push_str("Content: [Error reading file]\n");
                }
            } else {
                file_contents.push_str("Content: [New file to be created]\n");
            }
        }

        // Create the prompt for DeepSeek
        let prompt = format!(
            "Analyze these files and suggest specific code changes to implement the following issue:\n\n\
            Issue Title: {}\n\n\
            Issue Description:\n{}\n\n\
            Files to modify or create:\n{}\n\n\
            Think through each change carefully and explain your reasoning. \
            For each file, explain what needs to be changed and why. \
            For new files that don't exist yet, provide the complete initial Rust code content. \
            Show your step-by-step thinking process.\n\n\
            After your analysis, provide a summary of all proposed changes.",
            state.issue_title,
            state.issue_body,
            file_contents
        );

        // Get analysis from DeepSeek
        let mut response = String::new();
        let mut rx = self.deepseek.chat_stream(prompt, false).await;

        while let Some(update) = rx.recv().await {
            match update {
                crate::server::services::StreamUpdate::Content(content) => {
                    let content = content.to_string();
                    debug!("DeepSeek chunk: {}", content);
                    response.push_str(&content);
                }
                crate::server::services::StreamUpdate::Done => {
                    debug!("DeepSeek stream complete");
                    break;
                }
                _ => {
                    debug!("Unexpected stream update");
                }
            }
        }

        // Now use OpenRouter to generate specific changes for each file
        for file in &mut state.files {
            let path = Path::new(repo_dir).join(&file.path);
            let file_exists = path.exists();

            let prompt = format!(
                "Based on the analysis below, generate specific Rust code changes for this file.\n\n\
                Analysis:\n{}\n\n\
                File: {}\n\
                Status: {}\n\n\
                Generate code changes following the schema exactly.",
                response,
                file.path,
                if file_exists {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        format!("Existing file content:\n{}", content)
                    } else {
                        "Error reading existing file".to_string()
                    }
                } else {
                    "New file to be created".to_string()
                }
            );

            let request_body = serde_json::json!({
                "model": "google/gemini-2.0-flash-lite-preview-02-05:free",
                "messages": [{
                    "role": "user",
                    "content": prompt
                }],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "code_changes",
                        "strict": true,
                        "schema": {
                            "type": "object",
                            "properties": {
                                "files": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "search": {
                                                "type": "string",
                                                "description": "For existing files: exact code to replace. For new files: empty string"
                                            },
                                            "replace": {
                                                "type": "string",
                                                "description": "For existing files: new code to replace search string. For new files: complete file content"
                                            },
                                            "comment": {
                                                "type": "string",
                                                "description": "Explanation of what this change does"
                                            }
                                        },
                                        "required": ["search", "replace", "comment"],
                                        "additionalProperties": false
                                    }
                                }
                            },
                            "required": ["files"],
                            "additionalProperties": false
                        }
                    }
                }
            });

            let changes = self
                .openrouter
                .analyze_issue_with_schema(&prompt, request_body)
                .await?;

            info!("Generated changes for file: {}", file.path);
            info!("Change summary:");
            for (i, change) in changes.files.iter().enumerate() {
                info!("  Change #{}", i + 1);
                info!(
                    "    Type: {}",
                    if change.search.is_empty() {
                        "New File Creation"
                    } else {
                        "File Modification"
                    }
                );
                info!("    Description: {}", change.comment);
                if !change.search.is_empty() {
                    debug!("    Replacing: \n{}", change.search);
                    debug!("    With: \n{}", change.replace);
                }
            }

            // Apply changes to file
            for change in changes.files {
                if change.search.is_empty() {
                    // This is a new file
                    info!("📝 Creating new file: {}", file.path);
                    info!("   Reason: {}", change.comment);
                    if let Some(parent) = path.parent() {
                        info!("   Creating parent directories: {:?}", parent);
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&path, &change.replace)?;
                    info!("✅ Successfully created new file");
                } else {
                    // This is an existing file
                    info!("🔄 Modifying file: {}", file.path);
                    info!("   Change description: {}", change.comment);
                    if path.exists() {
                        let content = fs::read_to_string(&path)?;
                        if let Some(start) = content.find(&change.search) {
                            let mut new_content = content.clone();
                            new_content
                                .replace_range(start..start + change.search.len(), &change.replace);
                            fs::write(&path, new_content)?;
                            info!("✅ Successfully applied change to file");
                        } else {
                            error!("❌ Could not find search string in {}", file.path);
                            error!("   Search string: {}", change.search);
                            error!("   This change was skipped");
                        }
                    } else {
                        error!("❌ File does not exist: {}", file.path);
                    }
                }
            }
        }

        info!("🎉 All changes have been applied successfully");
        info!("📊 Summary of changes:");
        for file in &state.files {
            info!("   - {}: {}", file.path, file.reason);
        }

        state.status = SolverStatus::Complete;
        self.update_solver(state).await?;

        Ok(())
    }

    // Keep these methods but mark as unused for future use
    #[allow(dead_code)]
    pub async fn approve_change(&self, state: &mut SolverState, change_id: &str) -> Result<()> {
        info!("Approving change {} for solver {}", change_id, state.id);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn reject_change(&self, state: &mut SolverState, change_id: &str) -> Result<()> {
        info!("Rejecting change {} for solver {}", change_id, state.id);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn check_all_changes_reviewed(&self, _state: &SolverState) -> bool {
        true
    }
}
