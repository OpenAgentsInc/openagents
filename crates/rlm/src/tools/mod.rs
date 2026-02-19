//! RLM Environment Tools for DSPy integration.
//!
//! These tools expose the RLM environment (repo traversal, file reading, search)
//! to DSPy predictors via the tool interface. Each tool returns SpanRefs for
//! provenance tracking.
//!
//! # Tool Categories
//!
//! - **Search**: `GrepTool` for pattern matching across files
//! - **Read**: `ReadLinesTool` for precise range extraction
//! - **Navigation**: `ListFilesTool` for directory traversal
//! - **Analysis**: `SymbolsTool` for AST-based symbol extraction
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm::tools::{GrepTool, ReadLinesTool};
//! use std::path::PathBuf;
//!
//! let grep = GrepTool::new(PathBuf::from("."));
//! let results = grep.search("fn main", &["**/*.rs"], 10).await?;
//!
//! for hit in results {
//!     println!("{}: {}", hit.span.path, hit.preview);
//! }
//! ```

pub mod grep;
pub mod list_files;
pub mod read_lines;
pub mod symbols;

pub use grep::{GrepHit, GrepTool};
pub use list_files::{FileInfo, ListFilesTool};
pub use read_lines::{ReadLinesTool, ReadResult};
pub use symbols::{SymbolInfo, SymbolKind, SymbolsTool};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// Common error type for tool operations.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),
}

/// Result type for tool operations.
pub type ToolResult<T> = Result<T, ToolError>;

/// Trait for RLM tools that can be attached to DSPy predictors.
///
/// Tools receive JSON arguments and return JSON results with embedded SpanRefs
/// for provenance tracking.
#[async_trait]
pub trait RlmTool: Send + Sync {
    /// Tool name for identification.
    fn name(&self) -> &str;

    /// Human-readable description for LLM understanding.
    fn description(&self) -> &str;

    /// JSON schema for the tool's arguments.
    fn args_schema(&self) -> Value;

    /// Execute the tool with JSON arguments.
    async fn execute(&self, args: Value) -> ToolResult<Value>;
}

/// Configuration for tool execution limits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Maximum number of results to return.
    pub max_results: usize,
    /// Maximum file size to process (bytes).
    pub max_file_size: u64,
    /// Timeout for tool execution (milliseconds).
    pub timeout_ms: u64,
    /// Git commit to pin results to (optional).
    pub commit: Option<String>,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            max_results: 100,
            max_file_size: 10 * 1024 * 1024, // 10MB
            timeout_ms: 30_000,              // 30 seconds
            commit: None,
        }
    }
}

/// Get the current git commit SHA for the repo.
pub fn get_current_commit(repo_root: &PathBuf) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}
