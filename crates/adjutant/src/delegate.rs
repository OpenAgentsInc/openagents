//! Delegation - when Adjutant needs help from Codex CLI or RLM.
//!
//! Adjutant can:
//! - Delegate complex multi-file refactors to Codex CLI
//! - Use RLM for massive context analysis

use crate::{AdjutantError, Task, TaskPlan, TaskResult};
use std::path::Path;
use std::process::Command;

/// Delegate complex work to Codex CLI.
///
/// This spawns Codex CLI as a subprocess with the task prompt.
/// Codex has access to all tools and can handle complex multi-file work.
pub async fn delegate_to_codex(
    workspace_root: &Path,
    task: &Task,
) -> Result<TaskResult, AdjutantError> {
    tracing::info!("Delegating to Codex: {}", task.title);

    // Check if codex is available
    let codex_path = which::which("codex").map_err(|_| {
        AdjutantError::DelegationFailed(
            "Codex CLI not found. Ensure `codex` is in your PATH.".to_string(),
        )
    })?;

    // Build the prompt
    let prompt = task.to_prompt();

    // Run Codex with the prompt
    let output = Command::new(codex_path)
        .arg("--print")
        .arg(&prompt)
        .arg("--allowedTools")
        .arg("Read,Edit,Bash,Glob,Grep")
        .current_dir(workspace_root)
        .output()
        .map_err(|e| AdjutantError::DelegationFailed(format!("Failed to run Codex: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(TaskResult {
            success: true,
            summary: format!("Codex completed task: {}", task.title),
            modified_files: extract_modified_files(&stdout),
            commit_hash: extract_commit_hash(&stdout),
            error: None,
            session_id: None,
        })
    } else {
        Ok(TaskResult {
            success: false,
            summary: String::new(),
            modified_files: Vec::new(),
            commit_hash: None,
            error: Some(format!("Codex failed: {}", stderr)),
            session_id: None,
        })
    }
}

/// Execute task using RLM for large context analysis.
///
/// RLM breaks large contexts into chunks and processes them in parallel,
/// then synthesizes the results.
pub async fn execute_with_rlm(
    workspace_root: &Path,
    task: &Task,
    plan: &TaskPlan,
) -> Result<TaskResult, AdjutantError> {
    tracing::info!(
        "Using RLM for large context task: {} ({} tokens)",
        task.title,
        plan.estimated_tokens
    );

    // Check if pylon is available (provides RLM)
    let pylon_path = which::which("pylon").map_err(|_| {
        AdjutantError::RlmError("Pylon CLI not found. Build with: cargo build -p pylon".to_string())
    })?;

    // Build the RLM query
    let query = format!(
        "Task: {}\n\nDescription: {}\n\nAnalyze the relevant code and suggest the implementation approach.",
        task.title, task.description
    );

    // Run pylon rlm
    let output = Command::new(pylon_path)
        .arg("rlm")
        .arg(&query)
        .current_dir(workspace_root)
        .output()
        .map_err(|e| AdjutantError::RlmError(format!("Failed to run RLM: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(TaskResult {
            success: true,
            summary: format!(
                "RLM analysis complete for '{}'. Analysis:\n{}",
                task.title,
                truncate(&stdout, 500)
            ),
            modified_files: Vec::new(),
            commit_hash: None,
            error: None,
            session_id: None,
        })
    } else {
        Ok(TaskResult {
            success: false,
            summary: String::new(),
            modified_files: Vec::new(),
            commit_hash: None,
            error: Some(format!("RLM failed: {}", stderr)),
            session_id: None,
        })
    }
}

/// Extract modified file paths from Codex output.
fn extract_modified_files(output: &str) -> Vec<String> {
    let mut files = Vec::new();

    // Look for patterns like "Edited file.rs" or "Wrote file.rs"
    for line in output.lines() {
        if line.contains("Edited") || line.contains("Wrote") || line.contains("Created") {
            // Extract the file path
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let file = parts.last().unwrap().trim();
                if !files.contains(&file.to_string()) {
                    files.push(file.to_string());
                }
            }
        }
    }

    files
}

/// Extract commit hash from Codex output.
fn extract_commit_hash(output: &str) -> Option<String> {
    // Look for patterns like "commit abc123" or "[abc123]"
    for line in output.lines() {
        if line.contains("commit") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in parts {
                // Check if it looks like a commit hash (7+ hex chars)
                let clean = part.trim_matches(|c: char| !c.is_alphanumeric());
                if clean.len() >= 7 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(clean.to_string());
                }
            }
        }
    }
    None
}

/// Truncate string to max length with ellipsis.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
