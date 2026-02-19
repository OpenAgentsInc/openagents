//! RepoIndex job types for NIP-90 compute marketplace
//!
//! RepoIndex jobs generate embeddings and symbol indexes for git repositories.

use serde::{Deserialize, Serialize};

use nostr::nip90::{JobInput, JobRequest, JobResult, KIND_JOB_REPO_INDEX, Nip90Error};

/// Type of index to generate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexType {
    /// Vector embeddings for semantic search
    Embeddings,
    /// Symbol table (functions, classes, etc.)
    Symbols,
    /// File content digests for deduplication
    Digests,
    /// Dependency graph
    Dependencies,
    /// All available index types
    All,
}

impl IndexType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IndexType::Embeddings => "embeddings",
            IndexType::Symbols => "symbols",
            IndexType::Digests => "digests",
            IndexType::Dependencies => "dependencies",
            IndexType::All => "all",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "embeddings" => Some(IndexType::Embeddings),
            "symbols" => Some(IndexType::Symbols),
            "digests" => Some(IndexType::Digests),
            "dependencies" => Some(IndexType::Dependencies),
            "all" => Some(IndexType::All),
            _ => None,
        }
    }
}

/// A repository index request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIndexRequest {
    /// Repository URL (git clone URL)
    pub repo: String,
    /// Git reference (branch, tag, or commit SHA)
    pub git_ref: String,
    /// Types of indexes to generate
    pub index_types: Vec<IndexType>,
    /// File patterns to include (glob syntax)
    pub include_patterns: Vec<String>,
    /// File patterns to exclude (glob syntax)
    pub exclude_patterns: Vec<String>,
    /// Maximum file size to process in bytes
    pub max_file_size: Option<u64>,
    /// Embedding model to use (if generating embeddings)
    pub embedding_model: Option<String>,
}

impl RepoIndexRequest {
    /// Create a new repo index request
    pub fn new(repo: impl Into<String>, git_ref: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            git_ref: git_ref.into(),
            index_types: vec![IndexType::Symbols],
            include_patterns: vec!["**/*".to_string()],
            exclude_patterns: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
                "**/vendor/**".to_string(),
            ],
            max_file_size: Some(1024 * 1024), // 1 MB default
            embedding_model: None,
        }
    }

    /// Request all index types
    pub fn all_indexes(mut self) -> Self {
        self.index_types = vec![IndexType::All];
        self
    }

    /// Add an index type to generate
    pub fn add_index_type(mut self, index_type: IndexType) -> Self {
        if !self.index_types.contains(&index_type) {
            self.index_types.push(index_type);
        }
        self
    }

    /// Add an include pattern
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }

    /// Add an exclude pattern
    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Set maximum file size
    pub fn with_max_file_size(mut self, bytes: u64) -> Self {
        self.max_file_size = Some(bytes);
        self
    }

    /// Set embedding model
    pub fn with_embedding_model(mut self, model: impl Into<String>) -> Self {
        self.embedding_model = Some(model.into());
        self
    }

    /// Convert to NIP-90 JobRequest
    pub fn to_job_request(&self) -> Result<JobRequest, Nip90Error> {
        let mut request = JobRequest::new(KIND_JOB_REPO_INDEX)?
            .add_input(JobInput::url(&self.repo).with_marker("repo"))
            .add_param("git_ref", &self.git_ref);

        // Add index types
        for (i, index_type) in self.index_types.iter().enumerate() {
            request = request.add_param(format!("index_type_{}", i), index_type.as_str());
        }

        // Add include patterns
        for (i, pattern) in self.include_patterns.iter().enumerate() {
            request = request.add_param(format!("include_{}", i), pattern);
        }

        // Add exclude patterns
        for (i, pattern) in self.exclude_patterns.iter().enumerate() {
            request = request.add_param(format!("exclude_{}", i), pattern);
        }

        // Add max file size
        if let Some(size) = self.max_file_size {
            request = request.add_param("max_file_size", size.to_string());
        }

        // Add embedding model
        if let Some(ref model) = self.embedding_model {
            request = request.add_param("embedding_model", model);
        }

        Ok(request)
    }

    /// Parse from NIP-90 JobRequest
    pub fn from_job_request(request: &JobRequest) -> Result<Self, Nip90Error> {
        if request.kind != KIND_JOB_REPO_INDEX {
            return Err(Nip90Error::InvalidKind(request.kind, "5931".to_string()));
        }

        let mut repo = String::new();
        let mut git_ref = String::new();
        let mut index_types = Vec::new();
        let mut include_patterns = Vec::new();
        let mut exclude_patterns = Vec::new();
        let mut max_file_size = None;
        let mut embedding_model = None;

        // Extract repo from inputs
        for input in &request.inputs {
            if input.marker.as_deref() == Some("repo") {
                repo = input.data.clone();
            }
        }

        // Extract params
        for param in &request.params {
            match param.key.as_str() {
                "git_ref" => git_ref = param.value.clone(),
                "max_file_size" => max_file_size = param.value.parse().ok(),
                "embedding_model" => embedding_model = Some(param.value.clone()),
                key if key.starts_with("index_type_") => {
                    if let Some(idx_type) = IndexType::from_str(&param.value) {
                        index_types.push(idx_type);
                    }
                }
                key if key.starts_with("include_") => {
                    include_patterns.push(param.value.clone());
                }
                key if key.starts_with("exclude_") => {
                    exclude_patterns.push(param.value.clone());
                }
                _ => {}
            }
        }

        Ok(Self {
            repo,
            git_ref,
            index_types,
            include_patterns,
            exclude_patterns,
            max_file_size,
            embedding_model,
        })
    }
}

/// A symbol extracted from code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, struct, etc.)
    pub kind: String,
    /// File path relative to repo root
    pub file_path: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
    /// Documentation/docstring if available
    pub documentation: Option<String>,
    /// Signature or definition
    pub signature: Option<String>,
}

/// A file digest entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDigest {
    /// File path relative to repo root
    pub path: String,
    /// SHA256 hash of content
    pub sha256: String,
    /// File size in bytes
    pub size: u64,
    /// Detected language
    pub language: Option<String>,
    /// Number of lines
    pub line_count: u32,
}

/// An embedding entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingEntry {
    /// File path relative to repo root
    pub path: String,
    /// Line range (start, end)
    pub line_range: (u32, u32),
    /// The text that was embedded
    pub text_preview: String,
    /// Vector embedding (serialized)
    pub embedding: Vec<f32>,
}

/// A dependency entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Package/module name
    pub name: String,
    /// Version constraint
    pub version: Option<String>,
    /// Whether it's a dev dependency
    pub dev: bool,
    /// Source file that declares this dependency
    pub source_file: String,
}

/// Index data for a specific type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexData {
    Embeddings {
        model: String,
        dimension: u32,
        entries: Vec<EmbeddingEntry>,
    },
    Symbols {
        entries: Vec<Symbol>,
    },
    Digests {
        entries: Vec<FileDigest>,
    },
    Dependencies {
        entries: Vec<Dependency>,
    },
}

impl IndexData {
    pub fn index_type(&self) -> IndexType {
        match self {
            IndexData::Embeddings { .. } => IndexType::Embeddings,
            IndexData::Symbols { .. } => IndexType::Symbols,
            IndexData::Digests { .. } => IndexType::Digests,
            IndexData::Dependencies { .. } => IndexType::Dependencies,
        }
    }

    pub fn entry_count(&self) -> usize {
        match self {
            IndexData::Embeddings { entries, .. } => entries.len(),
            IndexData::Symbols { entries } => entries.len(),
            IndexData::Digests { entries } => entries.len(),
            IndexData::Dependencies { entries } => entries.len(),
        }
    }
}

/// Result of a repo index job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIndexResult {
    /// Generated indexes
    pub indexes: Vec<IndexData>,
    /// Total tokens processed (for embeddings)
    pub tokens_processed: u64,
    /// Number of files indexed
    pub files_indexed: u32,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Processing duration in milliseconds
    pub duration_ms: u64,
    /// Errors encountered (non-fatal)
    pub errors: Vec<String>,
}

impl RepoIndexResult {
    /// Create a new empty result
    pub fn new() -> Self {
        Self {
            indexes: Vec::new(),
            tokens_processed: 0,
            files_indexed: 0,
            bytes_processed: 0,
            duration_ms: 0,
            errors: Vec::new(),
        }
    }

    /// Add an index
    pub fn add_index(mut self, index: IndexData) -> Self {
        self.indexes.push(index);
        self
    }

    /// Record an error
    pub fn add_error(mut self, error: impl Into<String>) -> Self {
        self.errors.push(error.into());
        self
    }

    /// Set processing stats
    pub fn with_stats(mut self, tokens: u64, files: u32, bytes: u64, duration_ms: u64) -> Self {
        self.tokens_processed = tokens;
        self.files_indexed = files;
        self.bytes_processed = bytes;
        self.duration_ms = duration_ms;
        self
    }

    /// Convert to NIP-90 JobResult
    pub fn to_job_result(
        &self,
        request_id: &str,
        customer_pubkey: &str,
        amount: Option<u64>,
    ) -> Result<JobResult, Nip90Error> {
        let content =
            serde_json::to_string(self).map_err(|e| Nip90Error::Serialization(e.to_string()))?;

        let mut result = JobResult::new(KIND_JOB_REPO_INDEX, request_id, customer_pubkey, content)?;

        if let Some(amt) = amount {
            result = result.with_amount(amt, None);
        }

        Ok(result)
    }

    /// Parse from NIP-90 JobResult content
    pub fn from_job_result(result: &JobResult) -> Result<Self, Nip90Error> {
        serde_json::from_str(&result.content).map_err(|e| Nip90Error::Serialization(e.to_string()))
    }
}

impl Default for RepoIndexResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_type_roundtrip() {
        let types = [
            IndexType::Embeddings,
            IndexType::Symbols,
            IndexType::Digests,
            IndexType::Dependencies,
            IndexType::All,
        ];

        for t in types {
            let s = t.as_str();
            let parsed = IndexType::from_str(s).unwrap();
            assert_eq!(parsed, t);
        }
    }

    #[test]
    fn test_repo_index_request_new() {
        let request = RepoIndexRequest::new("https://github.com/owner/repo.git", "main");

        assert_eq!(request.repo, "https://github.com/owner/repo.git");
        assert_eq!(request.git_ref, "main");
        assert_eq!(request.index_types, vec![IndexType::Symbols]);
    }

    #[test]
    fn test_repo_index_request_builder() {
        let request = RepoIndexRequest::new("https://github.com/owner/repo.git", "main")
            .add_index_type(IndexType::Embeddings)
            .include("**/*.rs")
            .exclude("**/tests/**")
            .with_max_file_size(512 * 1024)
            .with_embedding_model("text-embedding-3-small");

        assert!(request.index_types.contains(&IndexType::Symbols));
        assert!(request.index_types.contains(&IndexType::Embeddings));
        assert!(request.include_patterns.contains(&"**/*.rs".to_string()));
        assert!(
            request
                .exclude_patterns
                .contains(&"**/tests/**".to_string())
        );
        assert_eq!(request.max_file_size, Some(512 * 1024));
        assert_eq!(
            request.embedding_model,
            Some("text-embedding-3-small".to_string())
        );
    }

    #[test]
    fn test_repo_index_request_to_job() {
        let request = RepoIndexRequest::new("https://github.com/owner/repo.git", "main");
        let job = request.to_job_request().unwrap();

        assert_eq!(job.kind, KIND_JOB_REPO_INDEX);
        assert!(!job.inputs.is_empty());
    }

    #[test]
    fn test_symbol() {
        let symbol = Symbol {
            name: "parse_json".to_string(),
            kind: "function".to_string(),
            file_path: "src/parser.rs".to_string(),
            line: 42,
            column: 1,
            documentation: Some("Parses JSON string".to_string()),
            signature: Some("fn parse_json(s: &str) -> Result<Value>".to_string()),
        };

        assert_eq!(symbol.name, "parse_json");
        assert_eq!(symbol.line, 42);
    }

    #[test]
    fn test_index_data_entry_count() {
        let symbols = IndexData::Symbols {
            entries: vec![Symbol {
                name: "foo".to_string(),
                kind: "function".to_string(),
                file_path: "src/lib.rs".to_string(),
                line: 1,
                column: 1,
                documentation: None,
                signature: None,
            }],
        };

        assert_eq!(symbols.entry_count(), 1);
        assert_eq!(symbols.index_type(), IndexType::Symbols);
    }

    #[test]
    fn test_repo_index_result_serialization() {
        let result = RepoIndexResult::new()
            .with_stats(1000, 50, 100000, 5000)
            .add_index(IndexData::Symbols {
                entries: vec![Symbol {
                    name: "main".to_string(),
                    kind: "function".to_string(),
                    file_path: "src/main.rs".to_string(),
                    line: 1,
                    column: 1,
                    documentation: None,
                    signature: Some("fn main()".to_string()),
                }],
            });

        let json = serde_json::to_string(&result).unwrap();
        let parsed: RepoIndexResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.tokens_processed, 1000);
        assert_eq!(parsed.files_indexed, 50);
        assert_eq!(parsed.indexes.len(), 1);
    }
}
