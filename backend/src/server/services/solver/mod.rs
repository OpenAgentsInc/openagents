pub mod types;

pub use types::*;

use crate::server::services::deepseek::StreamUpdate;
use crate::server::services::{
    gateway::Gateway, github_issue::GitHubService, openrouter::OpenRouterService,
};
use anyhow::Result;
use axum::extract::ws::Message;
use chrono::Utc;
use futures_util::StreamExt;
use git2::Repository;
use sqlx::PgPool;
use std::fs;
use std::path::Path;
use tempfile;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};

#[derive(Clone)]
pub struct SolverService {
    pool: PgPool,
    openrouter: OpenRouterService,
}

impl SolverService {
    pub fn new(pool: PgPool, openrouter: OpenRouterService) -> Self {
        Self { pool, openrouter }
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
        &mut self,
        state: &mut SolverState,
        repo_dir: &str,
        tx: Option<UnboundedSender<String>>,
    ) -> Result<()> {
        info!("🚀 Starting to generate changes for solver {}", state.id);
        state.status = SolverStatus::GeneratingChanges;
        state.set_repo_path(repo_dir.to_string());
        self.update_solver(state).await?;

        // Gather file contents for analysis
        let mut file_contents = String::new();
        info!("📂 Gathering file contents for analysis...");
        if let Some(tx) = &tx {
            let _ = tx.send("Gathering file contents for analysis...".to_string());
        }
        for file in &state.files {
            info!("   Reading file: {}", file.path);
            let path = Path::new(repo_dir).join(&file.path);
            file_contents.push_str(&format!("\nFile: {}\n", file.path));
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    info!("   ✅ Successfully read {}", file.path);
                    file_contents.push_str(&format!("Content:\n{}\n", content));
                    if let Some(tx) = &tx {
                        let _ = tx.send(format!("✅ Read file: {}", file.path));
                    }
                } else {
                    error!("   ❌ Error reading {}", file.path);
                    file_contents.push_str("Content: [Error reading file]\n");
                    if let Some(tx) = &tx {
                        let _ = tx.send(format!("❌ Error reading file: {}", file.path));
                    }
                }
            } else {
                info!("   🆕 File will be created: {}", file.path);
                file_contents.push_str("Content: [New file to be created]\n");
                if let Some(tx) = &tx {
                    let _ = tx.send(format!("🆕 Will create new file: {}", file.path));
                }
            }
        }

        info!("📝 Creating analysis prompt...");
        if let Some(tx) = &tx {
            let _ = tx.send("Creating analysis prompt...".to_string());
        }
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

        // Start analysis stream
        info!("🤖 Starting OpenRouter analysis stream...");
        if let Some(tx) = &tx {
            let _ = tx.send("Starting OpenRouter analysis...".to_string());
        }
        let mut response = String::new();
        let mut stream = match self.openrouter.chat_stream(prompt, true).await {
            Ok(stream) => {
                info!("✅ Successfully started OpenRouter stream");
                if let Some(tx) = &tx {
                    let _ = tx.send("✅ Connected to OpenRouter stream".to_string());
                }
                stream
            }
            Err(e) => {
                error!("❌ Failed to start OpenRouter stream: {}", e);
                if let Some(tx) = &tx {
                    let error_msg = if e.to_string().contains("Rate limit exceeded") {
                        "❌ Error: OpenRouter API rate limit exceeded. Please try again later or upgrade your API plan.".to_string()
                    } else {
                        format!("❌ Error: {}", e)
                    };
                    let _ = tx.send(error_msg);
                }
                return Err(anyhow::anyhow!("Failed to start OpenRouter stream: {}", e));
            }
        };

        let mut chunk_count = 0;
        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => {
                    chunk_count += 1;
                    debug!("📨 Received chunk #{}: {}", chunk_count, chunk);
                    response.push_str(&chunk);
                    if let Some(tx) = &tx {
                        let _ = tx.send(format!("CHUNK #{}: {}\n", chunk_count, chunk));
                    }
                }
                Err(e) => {
                    error!("❌ Error receiving chunk: {}", e);
                    if let Some(tx) = &tx {
                        let error_msg = if e.to_string().contains("Rate limit exceeded") {
                            "❌ Error: OpenRouter API rate limit exceeded. Please try again later or upgrade your API plan.".to_string()
                        } else {
                            format!("❌ Error receiving chunk: {}", e)
                        };
                        let _ = tx.send(error_msg);
                    }
                    return Err(anyhow::anyhow!("Error receiving chunk: {}", e));
                }
            }
        }

        if let Some(tx) = &tx {
            let _ = tx.send(format!(
                "\n=== COMPLETE RAW RESPONSE ===\n{}\n=== END RESPONSE ===\n",
                response
            ));
        }
        info!(
            "✅ Analysis stream complete - received {} chunks",
            chunk_count
        );
        if let Some(tx) = &tx {
            let _ = tx.send(format!(
                "\n✅ Analysis complete - received {} chunks",
                chunk_count
            ));
        }

        info!("🔄 Generating changes for each file...");
        if let Some(tx) = &tx {
            let _ = tx.send("\n🔄 Generating specific changes for each file...".to_string());
        }

        // Process each file to generate changes
        for file in &mut state.files {
            info!("   Processing file: {}", file.path);
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
                    if let Ok(content) = fs::read_to_string(&path) {
                        format!("Existing file content:\n{}", content)
                    } else {
                        "Error reading existing file".to_string()
                    }
                } else {
                    "New file to be created".to_string()
                }
            );

            info!("   🔍 Analyzing changes needed for {}", file.path);
            let request_body = serde_json::json!({
                "model": "deepseek/deepseek-r1-distill-llama-70b:free",
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

            let changes = match self
                .openrouter
                .analyze_issue_with_schema(&prompt, request_body)
                .await
            {
                Ok(changes) => {
                    info!("   ✅ Successfully generated changes for {}", file.path);
                    changes
                }
                Err(e) => {
                    error!("   ❌ Failed to generate changes for {}: {}", file.path, e);
                    continue;
                }
            };

            info!("   📊 Change summary for {}:", file.path);
            for (i, change) in changes.files.iter().enumerate() {
                info!("     Change #{}", i + 1);
                info!(
                    "       Type: {}",
                    if change.search.is_empty() {
                        "New File Creation"
                    } else {
                        "File Modification"
                    }
                );
                info!("       Description: {}", change.comment);
                if !change.search.is_empty() {
                    debug!("       Replacing:\n{}", change.search);
                    debug!("       With:\n{}", change.replace);
                }
            }

            // Apply changes
            for change in changes.files {
                if change.search.is_empty() {
                    info!("   📝 Creating new file: {}", file.path);
                    info!("      Reason: {}", change.comment);
                    info!("      Content to write:\n{}", change.replace);
                    if let Some(parent) = path.parent() {
                        info!("      Creating parent directories: {:?}", parent);
                        if let Err(e) = fs::create_dir_all(parent) {
                            error!("      ❌ Failed to create directories: {}", e);
                            continue;
                        }
                    }
                    if let Err(e) = fs::write(&path, &change.replace) {
                        error!("      ❌ Failed to write file: {}", e);
                        continue;
                    }
                    info!("      ✅ Successfully created new file");
                    if let Some(tx) = &tx {
                        let _ = tx.send(format!(
                            "\n=== NEW FILE: {} ===\nReason: {}\nContent:\n{}\n=== END FILE ===\n",
                            file.path, change.comment, change.replace
                        ));
                    }
                } else {
                    info!("   🔄 Modifying file: {}", file.path);
                    info!("      Change description: {}", change.comment);
                    info!("      Replacing:\n{}", change.search);
                    info!("      With:\n{}", change.replace);
                    if path.exists() {
                        match fs::read_to_string(&path) {
                            Ok(content) => {
                                if let Some(start) = content.find(&change.search) {
                                    let mut new_content = content.clone();
                                    new_content.replace_range(
                                        start..start + change.search.len(),
                                        &change.replace,
                                    );
                                    if let Err(e) = fs::write(&path, new_content) {
                                        error!("      ❌ Failed to write changes: {}", e);
                                        continue;
                                    }
                                    info!("      ✅ Successfully applied change to file");
                                    if let Some(tx) = &tx {
                                        let _ = tx.send(format!(
                                            "\n=== MODIFYING FILE: {} ===\nReason: {}\nReplacing:\n{}\nWith:\n{}\n=== END CHANGE ===\n",
                                            file.path, change.comment, change.search, change.replace
                                        ));
                                    }
                                } else {
                                    error!(
                                        "      ❌ Could not find search string in {}",
                                        file.path
                                    );
                                    error!("      Search string: {}", change.search);
                                    error!("      This change was skipped");
                                    if let Some(tx) = &tx {
                                        let _ = tx.send(format!(
                                            "\n=== ERROR: Failed to modify {} ===\nCould not find text to replace:\n{}\n=== END ERROR ===\n",
                                            file.path, change.search
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                error!("      ❌ Failed to read file: {}", e);
                                continue;
                            }
                        }
                    } else {
                        error!("      ❌ File does not exist: {}", file.path);
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

    pub async fn analyze_issue(
        &mut self,
        prompt: String,
    ) -> tokio::sync::mpsc::UnboundedReceiver<StreamUpdate> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut stream = match self.openrouter.chat_stream(prompt, true).await {
            Ok(stream) => stream,
            Err(e) => {
                error!("Failed to start OpenRouter stream: {}", e);
                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                let _ = tx.send(StreamUpdate::Done);
                return rx;
            }
        };
        tokio::spawn(async move {
            let mut is_reasoning = true;
            while let Some(result) = stream.next().await {
                match result {
                    Ok(chunk) => {
                        if chunk.contains("Final Changes:") {
                            is_reasoning = false;
                            continue;
                        }
                        let update = if is_reasoning {
                            StreamUpdate::Reasoning(chunk)
                        } else {
                            StreamUpdate::Content(chunk)
                        };
                        if tx.send(update).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error receiving chunk: {}", e);
                        break;
                    }
                }
            }
            let _ = tx.send(StreamUpdate::Done);
        });
        rx
    }

    pub async fn solve_demo_repo(&mut self, ws_tx: UnboundedSender<Message>) -> Result<()> {
        let mut state = SolverState::new(
            579,
            "Add repomap caching by branch+sha".to_string(),
            "Add caching for repository maps to avoid regenerating them unnecessarily. Cache key should be based on repository branch and commit SHA.".to_string(),
        );
        state.status = SolverStatus::Analyzing;
        ws_tx
            .send(Message::Text(format!(
                "Status Update: analyzing_files, progress: 0.0, timestamp: {}",
                Utc::now().to_rfc3339()
            )))
            .map_err(|e| anyhow::anyhow!("Failed to send status message: {}", e))?;
        state.add_file(
            "src/server/services/repomap/mod.rs".to_string(),
            0.9,
            "Main repomap service module that needs caching".to_string(),
        );
        state.add_file(
            "src/server/services/repomap/cache.rs".to_string(),
            0.9,
            "New file for cache implementation".to_string(),
        );
        state.add_file(
            "src/server/services/repomap/types.rs".to_string(),
            0.8,
            "Shared types for repomap caching".to_string(),
        );
        state.add_file(
            "migrations/YYYYMMDDHHMMSS_add_repomap_cache.sql".to_string(),
            0.7,
            "Database migration for cache table".to_string(),
        );
        state.status = SolverStatus::GeneratingChanges;
        ws_tx
            .send(Message::Text(format!(
                "Status Update: generating_changes, progress: 25.0, timestamp: {}",
                Utc::now().to_rfc3339()
            )))
            .map_err(|e| anyhow::anyhow!("Failed to send status message: {}", e))?;
        let (string_tx, mut string_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let ws_tx_clone = ws_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = string_rx.recv().await {
                let _ = ws_tx_clone.send(Message::Text(msg));
            }
        });
        self.start_generating_changes(&mut state, "/tmp/openagents", Some(string_tx))
            .await?;
        state.status = SolverStatus::Complete;
        ws_tx
            .send(Message::Text(format!(
                "Status Update: complete, progress: 100.0, timestamp: {}",
                Utc::now().to_rfc3339()
            )))
            .map_err(|e| anyhow::anyhow!("Failed to send status message: {}", e))?;
        Ok(())
    }

    pub async fn solve_repo(
        &mut self,
        ws_tx: UnboundedSender<Message>,
        repository: String,
        issue_number: i32,
    ) -> Result<()> {
        info!(
            "Starting repo solver process for {}, issue #{}",
            repository, issue_number
        );

        // Check OpenRouter API key
        if std::env::var("OPENROUTER_API_KEY").is_err() {
            let error_msg = "OpenRouter API key not found. Please set the OPENROUTER_API_KEY environment variable.";
            error!("{}", error_msg);
            ws_tx
                .send(Message::Text(format!("❌ Error: {}", error_msg)))
                .map_err(|e| anyhow::anyhow!("Failed to send error message: {}", e))?;
            return Err(anyhow::anyhow!(error_msg));
        }

        // Parse repository owner and name
        let parts: Vec<&str> = repository.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!(
                "Invalid repository format. Expected owner/name"
            ));
        }
        let (owner, name) = (parts[0], parts[1]);

        // Get GitHub token from environment
        let github_token =
            std::env::var("GITHUB_TOKEN").map_err(|_| anyhow::anyhow!("GITHUB_TOKEN not set"))?;

        // Initialize GitHub service
        let github = GitHubService::new(Some(github_token.clone()))
            .map_err(|e| anyhow::anyhow!("Failed to initialize GitHub service: {}", e))?;

        // Fetch issue details
        info!("Fetching issue #{} from {}/{}", issue_number, owner, name);
        let issue = github.get_issue(owner, name, issue_number).await?;

        // Create solver state
        let mut state = self
            .create_solver(
                issue_number,
                issue.title.clone(),
                issue.body.unwrap_or_default(),
            )
            .await?;

        // Send initial status
        ws_tx
            .send(Message::Text(format!(
                "Status Update: analyzing_files, progress: 0.0, timestamp: {}",
                Utc::now().to_rfc3339()
            )))
            .map_err(|e| anyhow::anyhow!("Failed to send status message: {}", e))?;

        // Create temporary directory for repository
        let temp_dir = tempfile::tempdir()?;
        let repo_path = temp_dir.path().to_path_buf();
        info!("Created temporary directory at {:?}", repo_path);

        // Clone repository
        let repo_url = format!("https://github.com/{}/{}.git", owner, name);
        info!("Cloning repository from {}", repo_url);
        if let Err(e) = Repository::clone(&repo_url, &repo_path) {
            error!("Failed to clone repository: {}", e);
            return Err(anyhow::anyhow!("Failed to clone repository: {}", e));
        }
        info!("Repository cloned successfully");

        // Create a channel for string messages
        let (string_tx, mut string_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let ws_tx_clone = ws_tx.clone();

        // Spawn a task to forward string messages to WebSocket
        tokio::spawn(async move {
            while let Some(msg) = string_rx.recv().await {
                let _ = ws_tx_clone.send(Message::Text(msg));
            }
        });

        // Start generating changes
        self.start_generating_changes(&mut state, &repo_path.to_string_lossy(), Some(string_tx))
            .await?;

        // Clean up temporary directory
        if let Err(e) = std::fs::remove_dir_all(&repo_path) {
            error!("Failed to clean up temporary directory: {}", e);
        }

        // Send completion status
        ws_tx
            .send(Message::Text(format!(
                "Status Update: complete, progress: 100.0, timestamp: {}",
                Utc::now().to_rfc3339()
            )))
            .map_err(|e| anyhow::anyhow!("Failed to send status message: {}", e))?;

        Ok(())
    }
}
