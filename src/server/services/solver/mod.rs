mod types;

pub use types::*;

use crate::server::services::{
    deepseek::DeepSeekService,
    openrouter::OpenRouterService,
};
use anyhow::{anyhow, Result};
use sqlx::PgPool;
use tracing::{debug, info, error};
use std::path::Path;

#[allow(dead_code)]  // These methods will be used in future
pub struct SolverService {
    pool: PgPool,
    openrouter: OpenRouterService,
    deepseek: DeepSeekService,
}

impl SolverService {
    pub fn new(
        pool: PgPool,
        openrouter: OpenRouterService,
        deepseek: DeepSeekService,
    ) -> Self {
        Self {
            pool,
            openrouter,
            deepseek,
        }
    }

    pub async fn create_solver(&self, issue_number: i32, issue_title: String, issue_body: String) -> Result<SolverState> {
        let state = SolverState::new(issue_number, issue_title, issue_body);

        // Store initial state in DB
        sqlx::query!(
            "INSERT INTO solver_states (id, status, issue_number, issue_title, issue_body, files) VALUES ($1, $2, $3, $4, $5, $6)",
            state.id,
            serde_json::to_value(&state.status)?,
            state.issue_number,
            state.issue_title,
            state.issue_body,
            serde_json::to_value(&state.files)?
        )
        .execute(&self.pool)
        .await?;

        info!("Created new solver state: {}", state.id);
        Ok(state)
    }

    pub async fn get_solver(&self, id: &str) -> Result<Option<SolverState>> {
        let record = sqlx::query!(
            "SELECT id, status, issue_number, issue_title, issue_body, files FROM solver_states WHERE id = $1",
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
        }))
    }

    pub async fn update_solver(&self, state: &SolverState) -> Result<()> {
        sqlx::query!(
            "UPDATE solver_states SET status = $1, files = $2 WHERE id = $3",
            serde_json::to_value(&state.status)?,
            serde_json::to_value(&state.files)?,
            state.id
        )
        .execute(&self.pool)
        .await?;

        info!("Updated solver state: {}", state.id);
        Ok(())
    }

    pub async fn start_generating_changes(&self, state: &mut SolverState, repo_dir: &str) -> Result<()> {
        info!("Starting to generate changes for solver {}", state.id);
        state.status = SolverStatus::GeneratingChanges;
        self.update_solver(state).await?;

        // First get DeepSeek's analysis of all files
        let mut file_contents = String::new();
        for file in &state.files {
            let path = Path::new(repo_dir).join(&file.path);
            if let Ok(content) = std::fs::read_to_string(&path) {
                file_contents.push_str(&format!("\nFile: {}\nContent:\n{}\n", file.path, content));
            }
        }

        // Create the prompt for DeepSeek
        let prompt = format!(
            "Analyze these files and suggest specific code changes to implement the following issue:\n\n\
            Issue Title: {}\n\n\
            Issue Description:\n{}\n\n\
            Files to modify:\n{}\n\n\
            Think through each change carefully and explain your reasoning. \
            For each file, explain what needs to be changed and why. \
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
            if let Ok(content) = std::fs::read_to_string(&path) {
                let prompt = format!(
                    "Based on the analysis below, generate specific code changes for this file.\n\n\
                    Analysis:\n{}\n\n\
                    File: {}\n\
                    Content:\n{}\n\n\
                    Generate changes in JSON format with the following rules:\n\
                    1. The 'search' field MUST contain EXACT code that exists in the file\n\
                    2. The 'replace' field must contain the complete new code\n\
                    3. Include a clear explanation in the 'analysis' field\n\
                    4. Verify each search string exists in the file\n\
                    5. Make sure search strings are unique",
                    response,
                    file.path,
                    content
                );

                let changes = self.openrouter.analyze_issue(&prompt).await?;

                // Add changes to file state
                for change in changes.files {
                    file.add_change(
                        change.filepath,  // Use the actual code to search for
                        change.comment.clone(),  // Use the new code to replace with
                        change.comment,  // Keep the analysis
                    );
                }
            }
        }

        // Immediately apply changes without waiting for approval
        self.apply_changes(state, repo_dir).await?;

        Ok(())
    }

    pub async fn apply_changes(&self, state: &mut SolverState, repo_dir: &str) -> Result<()> {
        info!("Applying changes for solver {}", state.id);
        state.status = SolverStatus::ApplyingChanges;
        self.update_solver(state).await?;

        // Apply changes file by file
        for file in &state.files {
            let file_path = Path::new(repo_dir).join(&file.path);
            info!("Applying changes to file: {}", file.path);

            // Read current content
            let mut content = std::fs::read_to_string(&file_path)
                .map_err(|e| anyhow!("Failed to read {}: {}", file.path, e))?;

            // Apply all changes
            for change in &file.changes {
                if let Some(pos) = content.find(&change.search) {
                    info!("Applying change to {}: {}", file.path, change.analysis);
                    content.replace_range(pos..pos + change.search.len(), &change.replace);
                } else {
                    error!("Could not find search string in {}: {}", file.path, change.search);
                }
            }

            // Write back to file
            std::fs::write(&file_path, content)
                .map_err(|e| anyhow!("Failed to write {}: {}", file.path, e))?;
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
