use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub search: String,
    pub replace: String,
    pub reason: Option<String>,
    pub analysis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub path: String,
    pub analysis: String,
    pub changes: Vec<Change>,
}

impl Change {
    pub fn new(search: String, replace: String, analysis: String) -> Self {
        Self {
            path: String::new(),
            search,
            replace,
            reason: None,
            analysis,
        }
    }

    pub fn with_reason(path: String, search: String, replace: String, reason: String) -> Self {
        Self {
            path,
            search,
            replace,
            reason: Some(reason),
            analysis: String::new(),
        }
    }
}

impl FileState {
    pub fn new(path: String, analysis: String) -> Self {
        Self {
            path,
            analysis,
            changes: Vec::new(),
        }
    }

    pub fn add_change(&mut self, search: String, replace: String, analysis: String) {
        self.changes.push(Change::new(search, replace, analysis));
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

impl PartialEq for ChangeError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::EmptyPath, Self::EmptyPath) => true,
            (Self::EmptyContent, Self::EmptyContent) => true,
            (Self::FileNotFound(a), Self::FileNotFound(b)) => a == b,
            (Self::NoMatch, Self::NoMatch) => true,
            (Self::MultipleMatches, Self::MultipleMatches) => true,
            (Self::InvalidFormat, Self::InvalidFormat) => true,
            (Self::InvalidJsonString, Self::InvalidJsonString) => true,
            (Self::IrrelevantChanges, Self::IrrelevantChanges) => true,
            (Self::IoError(_), Self::IoError(_)) => true,
            _ => false,
        }
    }
}

/// Result type for change operations
pub type ChangeResult<T> = Result<T, ChangeError>;