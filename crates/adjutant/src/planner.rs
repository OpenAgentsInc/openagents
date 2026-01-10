//! Task planning - understand what needs to be done.
//!
//! The planner analyzes a task and determines:
//! - Which files are relevant
//! - What the complexity is
//! - How many tokens it might require
//! - What strategy to use

use crate::{AdjutantError, Task, ToolRegistry};
use std::path::{Path, PathBuf};

/// Task complexity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Complexity {
    /// Simple single-file edit
    Low,
    /// Multi-file edit, moderate scope
    Medium,
    /// Complex refactoring, many files
    High,
    /// Massive scope, architectural changes
    VeryHigh,
}

/// A plan for executing a task.
#[derive(Debug, Clone)]
pub struct TaskPlan {
    /// Relevant files to read/edit
    pub files: Vec<PathBuf>,
    /// Estimated complexity
    pub complexity: Complexity,
    /// Estimated context tokens
    pub estimated_tokens: usize,
    /// Search queries to run
    pub search_queries: Vec<String>,
    /// Summary of what needs to be done
    pub summary: String,
}

/// Plan a task - analyze what needs to be done.
pub async fn plan_task(
    tools: &ToolRegistry,
    workspace_root: &Path,
    task: &Task,
) -> Result<TaskPlan, AdjutantError> {
    // Extract keywords from task
    let keywords = extract_keywords(&task.title, &task.description);

    // Search for relevant files
    let mut files = Vec::new();
    let mut search_queries = Vec::new();

    for keyword in &keywords {
        search_queries.push(keyword.clone());

        // Search for files containing the keyword
        let grep_result = tools.grep(keyword, None).await?;
        if grep_result.success {
            for line in grep_result.content.lines() {
                if let Some(file_path) = line.split(':').next() {
                    let path = PathBuf::from(file_path);
                    if !files.contains(&path) {
                        files.push(path);
                    }
                }
            }
        }
    }

    // Also check explicitly mentioned files from task
    for file in &task.files {
        if !files.contains(file) {
            files.push(file.clone());
        }
    }

    // Limit files and estimate tokens
    files.truncate(50); // Don't analyze more than 50 files
    let estimated_tokens = estimate_tokens(&files, workspace_root);

    // Determine complexity
    let complexity = determine_complexity(&files, estimated_tokens, &task.description);

    // Generate summary
    let summary = generate_summary(task, &files, complexity);

    Ok(TaskPlan {
        files,
        complexity,
        estimated_tokens,
        search_queries,
        summary,
    })
}

/// Extract relevant keywords from task title and description.
fn extract_keywords(title: &str, description: &str) -> Vec<String> {
    let mut keywords = Vec::new();

    // Common programming terms to look for
    let text = format!("{} {}", title, description).to_lowercase();

    // Extract quoted strings
    for part in text.split('"').enumerate() {
        if part.0 % 2 == 1 && !part.1.is_empty() {
            keywords.push(part.1.to_string());
        }
    }

    // Extract backtick code references
    for part in text.split('`').enumerate() {
        if part.0 % 2 == 1 && !part.1.is_empty() {
            keywords.push(part.1.to_string());
        }
    }

    // Extract CamelCase and snake_case identifiers
    let words: Vec<&str> = text.split_whitespace().collect();
    for word in words {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if clean.contains('_') || (clean.len() > 3 && clean.chars().any(|c| c.is_uppercase())) {
            if !keywords.contains(&clean.to_string()) {
                keywords.push(clean.to_string());
            }
        }
    }

    // Extract file paths
    for word in text.split_whitespace() {
        if word.contains('/') || word.ends_with(".rs") || word.ends_with(".ts") {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '_');
            if !keywords.contains(&clean.to_string()) {
                keywords.push(clean.to_string());
            }
        }
    }

    // Deduplicate and limit
    keywords.dedup();
    keywords.truncate(10);

    keywords
}

/// Estimate total tokens from file sizes.
fn estimate_tokens(files: &[PathBuf], workspace_root: &Path) -> usize {
    let mut total_bytes = 0;

    for file in files {
        let full_path = if file.is_absolute() {
            file.clone()
        } else {
            workspace_root.join(file)
        };

        if let Ok(metadata) = std::fs::metadata(&full_path) {
            total_bytes += metadata.len() as usize;
        }
    }

    // Rough estimate: 4 characters per token
    total_bytes / 4
}

/// Determine task complexity based on files and tokens.
pub fn determine_complexity(files: &[PathBuf], tokens: usize, description: &str) -> Complexity {
    let description_lower = description.to_lowercase();

    // Check for complexity indicators in description
    let has_refactor = description_lower.contains("refactor");
    let has_rewrite = description_lower.contains("rewrite");
    let has_migrate = description_lower.contains("migrat");
    let has_architecture = description_lower.contains("architect");

    if has_architecture || has_migrate || tokens > 200_000 || files.len() > 30 {
        Complexity::VeryHigh
    } else if has_refactor || has_rewrite || tokens > 100_000 || files.len() > 15 {
        Complexity::High
    } else if tokens > 50_000 || files.len() > 5 {
        Complexity::Medium
    } else {
        Complexity::Low
    }
}

/// Generate a summary of the planned work.
fn generate_summary(task: &Task, files: &[PathBuf], complexity: Complexity) -> String {
    let complexity_str = match complexity {
        Complexity::Low => "simple",
        Complexity::Medium => "moderate",
        Complexity::High => "complex",
        Complexity::VeryHigh => "very complex",
    };

    format!(
        "{} task affecting {} file(s): {}",
        complexity_str,
        files.len(),
        task.title
    )
}
