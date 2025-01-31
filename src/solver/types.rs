use anyhow::{anyhow, Result};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub path: String,
    pub search: String,
    pub replace: String,
    pub reason: Option<String>,
}

impl Change {
    pub fn new(path: String, search: String, replace: String) -> Self {
        Self {
            path,
            search,
            replace,
            reason: None,
        }
    }

    pub fn with_reason(path: String, search: String, replace: String, reason: String) -> Self {
        Self {
            path,
            search,
            replace,
            reason: Some(reason),
        }
    }

    pub fn validate(&self) -> Result<()> {
        // Path must not be empty
        if self.path.is_empty() {
            return Err(anyhow!("Path cannot be empty"));
        }

        // Path must be relative (not start with /)
        if self.path.starts_with('/') {
            return Err(anyhow!("Path must be relative (not start with /)"));
        }

        // Both search and replace cannot be empty
        if self.search.is_empty() && self.replace.is_empty() {
            return Err(anyhow!("Search content cannot be empty"));
        }

        // If reason is provided, it must not be empty
        if let Some(reason) = &self.reason {
            if reason.is_empty() {
                return Err(anyhow!("Change reason cannot be empty"));
            }
        }

        Ok(())
    }
}

/// Errors that can occur during change operations
#[derive(Error, Debug)]
pub enum ChangeError {
    #[error("File path cannot be empty")]
    EmptyPath,
    #[error("Both search and replace content cannot be empty")]
    EmptyContent,
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    #[error("No matching content found in file")]
    NoMatch,
    #[error("Multiple matches found for search content")]
    MultipleMatches,
    #[error("Invalid SEARCH/REPLACE block format")]
    InvalidFormat,
    #[error("Invalid JSON string")]
    InvalidJsonString,
    #[error("Changes not relevant to issue")]
    IrrelevantChanges,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for change operations
pub type ChangeResult<T> = Result<T, ChangeError>;

pub fn validate_pr_title(title: &str) -> Result<()> {
    // Title must be descriptive
    if title.len() < 20 {
        return Err(anyhow!("PR title must be at least 20 characters"));
    }

    // Title must contain issue reference
    if !title.contains('#') {
        return Err(anyhow!("PR title must reference the issue number"));
    }

    // Title must contain action verb
    let action_verbs = ["add", "fix", "update", "implement", "improve", "refactor"];
    if !action_verbs
        .iter()
        .any(|&verb| title.to_lowercase().contains(verb))
    {
        return Err(anyhow!("PR title must contain an action verb"));
    }

    // Title must not be too long
    if title.len() > 72 {
        return Err(anyhow!("PR title must not exceed 72 characters"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_with_reason() {
        let change = Change::with_reason(
            "test.rs".to_string(),
            "old".to_string(),
            "new".to_string(),
            "Update test".to_string(),
        );
        assert_eq!(change.reason, Some("Update test".to_string()));
    }

    #[test]
    fn test_validate_pr_title() {
        // Valid titles
        assert!(validate_pr_title("Add multiply function to implement #123").is_ok());
        assert!(validate_pr_title("Fix JSON parsing in solver for #456").is_ok());

        // Invalid titles
        assert!(validate_pr_title("Fix #123").is_err()); // Too short
        assert!(validate_pr_title("Add function").is_err()); // No issue reference
        assert!(validate_pr_title("The function needs to be added").is_err()); // No issue reference
        assert!(validate_pr_title("Something something something something something something very long title that exceeds the limit").is_err());
        // Too long
    }

    #[test]
    fn test_change_validation() {
        // Valid change
        let change = Change::with_reason(
            "test.rs".to_string(),
            "old".to_string(),
            "new".to_string(),
            "Update test".to_string(),
        );
        assert!(change.validate().is_ok());

        // Invalid path
        let change = Change::with_reason(
            "".to_string(),
            "old".to_string(),
            "new".to_string(),
            "Update test".to_string(),
        );
        assert!(change.validate().is_err());

        // Invalid reason
        let change = Change::with_reason(
            "test.rs".to_string(),
            "old".to_string(),
            "new".to_string(),
            "".to_string(),
        );
        assert!(change.validate().is_err());
    }
}
