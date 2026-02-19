//! Multi-lane retrieval system for code exploration.
//!
//! Provides pluggable backends for different retrieval strategies:
//! - Ripgrep: Text/regex search
//! - LSP: Definitions, references, implementations
//! - Semantic: Vector embeddings
//! - Git: Blame, log, diff signals

pub mod git;
pub mod lsp;
pub mod ripgrep;
pub mod router;
pub mod semantic;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Result from a retrieval query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// File path relative to repo root.
    pub path: String,

    /// Starting line number (1-indexed).
    pub start_line: usize,

    /// Ending line number (1-indexed, inclusive).
    pub end_line: usize,

    /// The retrieved content.
    pub content: String,

    /// Relevance score (0.0 to 1.0).
    pub score: f32,

    /// Which lane produced this result.
    pub lane: String,

    /// Optional metadata (e.g., symbol name, git author).
    pub metadata: HashMap<String, String>,
}

impl RetrievalResult {
    /// Create a new retrieval result.
    pub fn new(
        path: impl Into<String>,
        start_line: usize,
        end_line: usize,
        content: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            start_line,
            end_line,
            content: content.into(),
            score: 1.0,
            lane: String::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set the score.
    pub fn with_score(mut self, score: f32) -> Self {
        self.score = score;
        self
    }

    /// Set the lane.
    pub fn with_lane(mut self, lane: impl Into<String>) -> Self {
        self.lane = lane.into();
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Configuration for retrieval queries.
#[derive(Debug, Clone, Default)]
pub struct RetrievalConfig {
    /// Maximum number of results to return.
    pub k: usize,

    /// Minimum score threshold.
    pub min_score: f32,

    /// Include surrounding context lines.
    pub context_lines: usize,

    /// File patterns to include (glob).
    pub include_patterns: Vec<String>,

    /// File patterns to exclude (glob).
    pub exclude_patterns: Vec<String>,
}

impl RetrievalConfig {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self {
            k: 10,
            min_score: 0.0,
            context_lines: 3,
            include_patterns: vec![],
            exclude_patterns: vec![],
        }
    }

    /// Set max results.
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set minimum score.
    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = min_score;
        self
    }

    /// Set context lines.
    pub fn with_context(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }
}

/// Trait for retrieval backends.
#[async_trait]
pub trait RepoIndex: Send + Sync {
    /// Query the index with a search string.
    async fn query(&self, query: &str, config: &RetrievalConfig) -> Result<Vec<RetrievalResult>>;

    /// Get the lane name for this backend.
    fn lane_name(&self) -> &str;

    /// Whether this backend supports semantic/fuzzy matching.
    fn supports_semantic(&self) -> bool;

    /// Index or re-index the repository.
    async fn build_index(&self, repo_path: &PathBuf) -> Result<()> {
        // Default: no-op for backends that don't need indexing
        let _ = repo_path;
        Ok(())
    }

    /// Check if the backend is available/configured.
    async fn is_available(&self) -> bool {
        true
    }
}

/// Statistics about a retrieval operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalStats {
    /// Number of results found.
    pub result_count: usize,

    /// Time taken in milliseconds.
    pub duration_ms: u64,

    /// Lane used.
    pub lane: String,

    /// Files searched (if known).
    pub files_searched: Option<usize>,
}

// Re-export backends
pub use git::GitIndex;
pub use lsp::LspIndex;
pub use ripgrep::RipgrepIndex;
pub use router::LaneRouter;
pub use semantic::SemanticIndex;
