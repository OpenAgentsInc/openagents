use anyhow::{anyhow, Result};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub path: String,
    pub search: String,
    pub replace: String,
}

impl Change {
    pub fn new(path: String, search: String, replace: String) -> Self {
        Self {
            path,
            search,
            replace,
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

        // Replace must not be empty
        if self.replace.is_empty() {
            return Err(anyhow!("Replace content cannot be empty"));
        }

        // Path must not be empty
        if self.path.is_empty() {
            return Err(anyhow!("Path cannot be empty"));
        }

        // Path must be relative (not start with /)
        if self.path.starts_with('/') {
            return Err(anyhow!("Path must be relative (not start with /)"));
        }

        // Empty search is only allowed for new files
        if self.search.is_empty() {
            if self.replace.is_empty() {
                return Err(anyhow!("Replace content cannot be empty"));
            }
            return Ok(());
        }

        // For modifications (non-empty search), replace must not be empty
        if self.replace.is_empty() {
            return Err(anyhow!("Replace content cannot be empty"));
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
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for change operations
pub type ChangeResult<T> = Result<T, ChangeError>;

pub fn validate_pr_title(title: &str) -> Result<()> {
    // Title must contain one of these words
    if !title.contains("solver") && !title.contains("solution") && !title.contains("PR") {
        return Err(anyhow!(
            "PR title must contain 'solver', 'solution', or 'PR'"
        ));
    }

    // Title must not be too short
    if title.len() < 10 {
        return Err(anyhow!("PR title must be at least 10 characters"));
    }

    Ok(())
}
