use std::path::PathBuf;
use thiserror::Error;

/// Represents a code change to be applied to a file
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// Path to the file being modified
    pub path: String,
    /// Content to search for in the file
    pub search: String,
    /// Content to replace the search content with
    pub replace: String,
}

impl Change {
    /// Creates a new Change instance
    pub fn new(path: String, search: String, replace: String) -> Self {
        Self {
            path,
            search,
            replace,
        }
    }

    /// Validates that the change has non-empty path and content
    pub fn validate(&self) -> Result<(), ChangeError> {
        if self.path.is_empty() {
            return Err(ChangeError::EmptyPath);
        }
        if self.search.is_empty() && self.replace.is_empty() {
            return Err(ChangeError::EmptyContent);
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