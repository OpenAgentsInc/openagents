//! Worktree Guardrails - ENFORCE file operation boundaries
//!
//! Technical enforcement that PREVENTS agents from editing files outside their
//! worktree during parallel execution. Uses path validation to check if file
//! operations stay within the isolation boundary.
//!
//! This is NOT guidance - it's a hard barrier that cannot be bypassed.

use std::path::{Path, PathBuf};

/// Tools that operate on files and need boundary checking
pub const FILE_TOOLS: &[&str] = &["Read", "Edit", "Write", "Glob", "NotebookEdit"];

/// Result of a worktree guard check
#[derive(Debug, Clone)]
pub enum GuardResult {
    /// Operation is allowed
    Allow,
    /// Operation is blocked with an error message
    Block { message: String },
}

impl GuardResult {
    /// Whether the operation should continue
    pub fn should_continue(&self) -> bool {
        matches!(self, GuardResult::Allow)
    }

    /// Get the error message if blocked
    pub fn error_message(&self) -> Option<&str> {
        match self {
            GuardResult::Allow => None,
            GuardResult::Block { message } => Some(message),
        }
    }
}

/// Check if a tool is a file operation tool
pub fn is_file_tool(tool_name: &str) -> bool {
    FILE_TOOLS.contains(&tool_name)
}

/// Check if a path is within the worktree boundary
pub fn is_path_within_worktree(file_path: &str, worktree_path: &str, base_cwd: &str) -> bool {
    let base = Path::new(base_cwd);
    let absolute_path = if Path::new(file_path).is_absolute() {
        PathBuf::from(file_path)
    } else {
        base.join(file_path)
    };

    let worktree_abs = if Path::new(worktree_path).is_absolute() {
        PathBuf::from(worktree_path)
    } else {
        base.join(worktree_path)
    };

    // Canonicalize worktree (should exist)
    let worktree_abs = worktree_abs.canonicalize().unwrap_or(worktree_abs);

    // For the file path, normalize it without requiring it to exist
    // by resolving the parent and appending the filename
    let normalized_path = if let Some(parent) = absolute_path.parent() {
        if let Ok(canonical_parent) = parent.canonicalize() {
            if let Some(filename) = absolute_path.file_name() {
                canonical_parent.join(filename)
            } else {
                canonical_parent
            }
        } else {
            // Parent doesn't exist either, just use the path as-is
            absolute_path.clone()
        }
    } else {
        absolute_path.clone()
    };

    // Check if the path starts with the worktree path
    normalized_path.starts_with(&worktree_abs)
}

/// Check a file operation against the worktree boundary
pub fn check_worktree_guard(
    tool_name: &str,
    file_path: &str,
    worktree_path: &str,
) -> GuardResult {
    // Only guard file operations
    if !is_file_tool(tool_name) {
        return GuardResult::Allow;
    }

    // Resolve to absolute path
    let worktree_abs = std::fs::canonicalize(worktree_path)
        .unwrap_or_else(|_| PathBuf::from(worktree_path));

    let absolute_path = if Path::new(file_path).is_absolute() {
        PathBuf::from(file_path)
    } else {
        worktree_abs.join(file_path)
    };

    let absolute_path = std::fs::canonicalize(&absolute_path).unwrap_or(absolute_path);

    // Check if path is within worktree
    if absolute_path.starts_with(&worktree_abs) {
        return GuardResult::Allow;
    }

    // Block: File operation outside worktree
    let message = format!(
        r#"WORKTREE ISOLATION VIOLATION - OPERATION BLOCKED

Tool: {}
Requested path: {}
Resolved to: {}
Your worktree: {}

This file is OUTSIDE your worktree boundary. The operation has been BLOCKED.

You MUST only access files within your worktree:
  {}/

The worktree contains ALL project files. You do not need to access anything outside it.

Use paths relative to your current directory or absolute paths within your worktree."#,
        tool_name,
        file_path,
        absolute_path.display(),
        worktree_abs.display(),
        worktree_abs.display()
    );

    GuardResult::Block { message }
}

/// Extract file path from common tool input patterns
pub fn extract_file_path(tool_input: &serde_json::Value) -> Option<&str> {
    // Try common field names
    tool_input
        .get("file_path")
        .or_else(|| tool_input.get("path"))
        .or_else(|| tool_input.get("pattern"))
        .or_else(|| tool_input.get("notebook_path"))
        .and_then(|v| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_file_tool() {
        assert!(is_file_tool("Read"));
        assert!(is_file_tool("Edit"));
        assert!(is_file_tool("Write"));
        assert!(is_file_tool("Glob"));
        assert!(is_file_tool("NotebookEdit"));
        assert!(!is_file_tool("Bash"));
        assert!(!is_file_tool("WebFetch"));
    }

    #[test]
    fn test_is_path_within_worktree() {
        let temp = TempDir::new().unwrap();
        let worktree = temp.path().to_str().unwrap();

        // Create a subdirectory
        std::fs::create_dir_all(temp.path().join("src")).unwrap();

        // Relative path within worktree
        assert!(is_path_within_worktree("src/main.rs", worktree, worktree));

        // Absolute path within worktree
        let abs_path = temp.path().join("src/main.rs");
        assert!(is_path_within_worktree(
            abs_path.to_str().unwrap(),
            worktree,
            worktree
        ));

        // Path outside worktree
        assert!(!is_path_within_worktree("/etc/passwd", worktree, worktree));
    }

    #[test]
    fn test_check_worktree_guard_allows_non_file_tools() {
        let result = check_worktree_guard("Bash", "/etc/passwd", "/tmp/worktree");
        assert!(result.should_continue());
    }

    #[test]
    fn test_check_worktree_guard_blocks_outside() {
        let temp = TempDir::new().unwrap();
        let worktree = temp.path().to_str().unwrap();

        let result = check_worktree_guard("Read", "/etc/passwd", worktree);
        assert!(!result.should_continue());
        assert!(result.error_message().unwrap().contains("VIOLATION"));
    }

    #[test]
    fn test_check_worktree_guard_allows_inside() {
        let temp = TempDir::new().unwrap();
        let worktree = temp.path().to_str().unwrap();

        // Create a file inside
        std::fs::write(temp.path().join("test.txt"), "hello").unwrap();

        let result = check_worktree_guard("Read", "test.txt", worktree);
        assert!(result.should_continue());
    }

    #[test]
    fn test_extract_file_path() {
        let input = serde_json::json!({ "file_path": "/path/to/file.rs" });
        assert_eq!(extract_file_path(&input), Some("/path/to/file.rs"));

        let input = serde_json::json!({ "path": "/another/path" });
        assert_eq!(extract_file_path(&input), Some("/another/path"));

        let input = serde_json::json!({ "pattern": "**/*.rs" });
        assert_eq!(extract_file_path(&input), Some("**/*.rs"));

        let input = serde_json::json!({ "command": "ls" });
        assert_eq!(extract_file_path(&input), None);
    }
}
