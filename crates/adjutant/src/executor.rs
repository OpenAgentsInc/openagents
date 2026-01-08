//! Task execution - do the actual work using tools.

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
        }
    }
}

/// Execute a task using local tools.
///
/// This is where Adjutant does the actual work:
/// 1. Read relevant files
/// 2. Understand the context
/// 3. Make edits
/// 4. Run tests
/// 5. Commit if successful
pub async fn execute_with_tools(
    tools: &mut ToolRegistry,
    _workspace_root: &Path,
    task: &Task,
    plan: &TaskPlan,
) -> Result<TaskResult, AdjutantError> {
    tracing::info!("Executing task: {}", task.title);

    // For now, this is a simplified implementation.
    // In the full version, this would:
    // 1. Use Claude API to understand the task
    // 2. Generate edits based on the plan
    // 3. Apply edits using the Edit tool
    // 4. Run tests
    // 5. Commit changes

    let mut context = String::new();

    // 1. Read relevant files to build context
    tracing::info!("Reading {} relevant files", plan.files.len());
    for file in &plan.files {
        let result = tools.read(file).await?;
        if result.success {
            context.push_str(&format!("\n--- {} ---\n{}\n", file.display(), result.content));
        }
    }

    // 2. Log what we found
    tracing::info!(
        "Built context: {} bytes from {} files",
        context.len(),
        plan.files.len()
    );

    // 3. For now, return a placeholder result indicating human review needed
    // The full implementation would use Claude to generate and apply edits

    Ok(TaskResult {
        success: true,
        summary: format!(
            "Analyzed task '{}'. Found {} relevant files totaling {} tokens. Ready for implementation.",
            task.title,
            plan.files.len(),
            plan.estimated_tokens
        ),
        modified_files: Vec::new(),
        commit_hash: None,
        error: None,
    })
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
