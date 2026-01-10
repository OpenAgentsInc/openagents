//! Task execution - do the actual work using tools.
//!
//! Prioritizes Claude (via claude-agent-sdk) if the CLI is available,
//! falls back to Cerebras TieredExecutor, then to analysis-only mode.

use crate::auth::has_claude_cli;
use crate::claude_executor::ClaudeExecutor;
use crate::tiered::TieredExecutor;
use crate::{AdjutantError, Task, TaskPlan, ToolRegistry};
use std::path::Path;

/// Result of task execution.
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// Whether the task succeeded
    pub success: bool,
    /// Summary of what was done
    pub summary: String,
    /// Files that were modified
    pub modified_files: Vec<String>,
    /// Git commit hash (if committed)
    pub commit_hash: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Session ID from the LLM provider (for conversation continuity)
    pub session_id: Option<String>,
}

impl TaskResult {
    /// Create a successful result.
    pub fn success(summary: impl Into<String>) -> Self {
        Self {
            success: true,
            summary: summary.into(),
            modified_files: Vec::new(),
            commit_hash: None,
            error: None,
            session_id: None,
        }
    }

    /// Create a failed result.
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            summary: String::new(),
            modified_files: Vec::new(),
            commit_hash: None,
            error: Some(error.into()),
            session_id: None,
        }
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

/// Execute a task using local tools.
///
/// This is where Adjutant does the actual work:
/// 1. Read relevant files to build context
/// 2. Use ClaudeExecutor (Claude Pro/Max) if available - PRIORITY
/// 3. Fall back to TieredExecutor (Cerebras) if Claude unavailable
/// 4. Fall back to analysis-only if neither is available
pub async fn execute_with_tools(
    tools: &mut ToolRegistry,
    workspace_root: &Path,
    task: &Task,
    plan: &TaskPlan,
) -> Result<TaskResult, AdjutantError> {
    tracing::info!("Executing task: {}", task.title);

    // 1. Read relevant files to build context
    let mut context = String::new();
    tracing::info!("Reading {} relevant files", plan.files.len());
    for file in &plan.files {
        let result = tools.read(file).await?;
        if result.success {
            context.push_str(&format!("\n--- {} ---\n{}\n", file.display(), result.content));
        }
    }

    tracing::info!(
        "Built context: {} bytes from {} files",
        context.len(),
        plan.files.len()
    );

    // PRIORITY 1: Use Claude if CLI is available (Pro/Max subscription)
    if has_claude_cli() {
        tracing::info!("Claude CLI detected - using ClaudeExecutor (Pro/Max)");
        let executor = ClaudeExecutor::new(workspace_root);
        match executor.execute(task, &context, tools).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Claude failed (maybe not authenticated), fall through to Cerebras
                tracing::warn!("ClaudeExecutor failed: {}. Falling back to Cerebras.", e);
            }
        }
    } else {
        tracing::info!("Claude CLI not found, checking Cerebras...");
    }

    // PRIORITY 2: Use TieredExecutor (Cerebras GLM 4.7 + Qwen-3-32B)
    match TieredExecutor::new() {
        Ok(mut executor) => {
            tracing::info!("Using TieredExecutor (Cerebras)");
            executor.execute(task, &context, tools).await
        }
        Err(e) => {
            // PRIORITY 3: Fall back to analysis-only mode
            tracing::warn!("No inference backend available: {}. Using analysis-only mode.", e);
            Ok(TaskResult {
                success: true,
                summary: format!(
                    "Analyzed task '{}'. Found {} relevant files totaling {} tokens.\n\
                     Install Claude CLI for Pro/Max execution, or set CEREBRAS_API_KEY for tiered inference.",
                    task.title,
                    plan.files.len(),
                    plan.estimated_tokens
                ),
                modified_files: Vec::new(),
                commit_hash: None,
                error: None,
                session_id: None,
            })
        }
    }
}

/// Run tests in the workspace.
pub async fn run_tests(tools: &ToolRegistry) -> Result<bool, AdjutantError> {
    // Try cargo test for Rust projects
    let cargo_result = tools.bash("cargo test --no-fail-fast 2>&1").await?;
    if cargo_result.success {
        return Ok(true);
    }

    // Try npm test for JS/TS projects
    let npm_result = tools.bash("npm test 2>&1").await?;
    if npm_result.success {
        return Ok(true);
    }

    // Try bun test
    let bun_result = tools.bash("bun test 2>&1").await?;
    if bun_result.success {
        return Ok(true);
    }

    // No tests found or tests failed
    Ok(false)
}

/// Create a git commit with the changes.
pub async fn create_commit(
    tools: &ToolRegistry,
    message: &str,
) -> Result<Option<String>, AdjutantError> {
    // Stage all changes
    let add_result = tools.bash("git add -A").await?;
    if !add_result.success {
        tracing::warn!("git add failed: {:?}", add_result.error);
        return Ok(None);
    }

    // Check if there are changes to commit
    let status_result = tools.bash("git status --porcelain").await?;
    if status_result.content.trim().is_empty() {
        tracing::info!("No changes to commit");
        return Ok(None);
    }

    // Create commit
    let commit_cmd = format!(
        "git commit -m \"{}\"",
        message.replace('"', "'")
    );
    let commit_result = tools.bash(&commit_cmd).await?;

    if !commit_result.success {
        tracing::warn!("git commit failed: {:?}", commit_result.error);
        return Ok(None);
    }

    // Get commit hash
    let hash_result = tools.bash("git rev-parse HEAD").await?;
    if hash_result.success {
        Ok(Some(hash_result.content.trim().to_string()))
    } else {
        Ok(None)
    }
}
