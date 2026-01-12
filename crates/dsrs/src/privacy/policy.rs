//! Privacy policy configuration for job dispatch.
//!
//! Controls which job types are allowed, trusted providers,
//! and how content is redacted/chunked before sending to swarm.

use super::{ChunkingPolicy, RedactionConfig, RedactionMode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Privacy policy for controlling swarm job dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    /// Redaction configuration.
    pub redaction: RedactionConfig,

    /// Chunking policy for content.
    pub chunking: ChunkingPolicy,

    /// Allowed job types (empty = all allowed).
    pub allowed_job_types: HashSet<String>,

    /// Trusted provider public keys (npub or hex).
    pub trusted_providers: Vec<String>,

    /// Whether to require verification for all jobs.
    pub require_verification: bool,

    /// Maximum content size to send (bytes).
    pub max_content_size: Option<usize>,

    /// Whether to allow sending file paths.
    pub allow_file_paths: bool,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            redaction: RedactionConfig::default(),
            chunking: ChunkingPolicy::default(),
            allowed_job_types: HashSet::new(), // Empty = all allowed
            trusted_providers: Vec::new(),
            require_verification: false,
            max_content_size: None,
            allow_file_paths: true,
        }
    }
}

impl PrivacyPolicy {
    /// Create a new privacy policy with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a policy for open source repos (minimal restrictions).
    pub fn open_source() -> Self {
        Self {
            redaction: RedactionConfig {
                mode: RedactionMode::None,
                preserve_structure: true,
                preserve_types: true,
                custom_patterns: Vec::new(),
                preserve_patterns: Vec::new(),
            },
            chunking: ChunkingPolicy::Full,
            allowed_job_types: HashSet::new(),
            trusted_providers: Vec::new(),
            require_verification: false,
            max_content_size: None,
            allow_file_paths: true,
        }
    }

    /// Create a policy for private repos (strict restrictions).
    pub fn private_repo() -> Self {
        let mut allowed = HashSet::new();
        // Only allow sandbox execution (verifiable) by default
        allowed.insert("oa.sandbox_run.v1".to_string());

        Self {
            redaction: RedactionConfig {
                mode: RedactionMode::PathsOnly,
                preserve_structure: true,
                preserve_types: true,
                custom_patterns: Vec::new(),
                preserve_patterns: Vec::new(),
            },
            chunking: ChunkingPolicy::MinimalSpans { context_lines: 5 },
            allowed_job_types: allowed,
            trusted_providers: Vec::new(),
            require_verification: true,
            max_content_size: Some(50_000), // 50KB max
            allow_file_paths: false,
        }
    }

    /// Create a paranoid policy (maximum privacy).
    pub fn paranoid() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert("oa.sandbox_run.v1".to_string());

        Self {
            redaction: RedactionConfig {
                mode: RedactionMode::Full,
                preserve_structure: true,
                preserve_types: true,
                custom_patterns: Vec::new(),
                preserve_patterns: Vec::new(),
            },
            chunking: ChunkingPolicy::MinimalSpans { context_lines: 2 },
            allowed_job_types: allowed,
            trusted_providers: Vec::new(),
            require_verification: true,
            max_content_size: Some(10_000), // 10KB max
            allow_file_paths: false,
        }
    }

    /// Set redaction mode.
    pub fn with_redaction_mode(mut self, mode: RedactionMode) -> Self {
        self.redaction.mode = mode;
        self
    }

    /// Set chunking policy.
    pub fn with_chunking(mut self, chunking: ChunkingPolicy) -> Self {
        self.chunking = chunking;
        self
    }

    /// Add an allowed job type.
    pub fn allow_job_type(mut self, job_type: impl Into<String>) -> Self {
        self.allowed_job_types.insert(job_type.into());
        self
    }

    /// Add multiple allowed job types.
    pub fn allow_job_types(mut self, job_types: impl IntoIterator<Item = String>) -> Self {
        self.allowed_job_types.extend(job_types);
        self
    }

    /// Add a trusted provider.
    pub fn trust_provider(mut self, provider_pubkey: impl Into<String>) -> Self {
        self.trusted_providers.push(provider_pubkey.into());
        self
    }

    /// Set max content size.
    pub fn with_max_content_size(mut self, size: usize) -> Self {
        self.max_content_size = Some(size);
        self
    }

    /// Enable verification requirement.
    pub fn require_verification(mut self) -> Self {
        self.require_verification = true;
        self
    }

    /// Check if a job type is allowed.
    pub fn is_job_allowed(&self, job_type: &str) -> bool {
        // Empty set means all jobs allowed
        if self.allowed_job_types.is_empty() {
            return true;
        }
        self.allowed_job_types.contains(job_type)
    }

    /// Check if a provider is trusted.
    pub fn is_provider_trusted(&self, provider_pubkey: &str) -> bool {
        // Empty list means any provider is acceptable
        if self.trusted_providers.is_empty() {
            return true;
        }
        self.trusted_providers
            .contains(&provider_pubkey.to_string())
    }

    /// Validate content against policy.
    pub fn validate_content(&self, content: &str) -> Result<(), PolicyViolation> {
        // Check content size
        if let Some(max_size) = self.max_content_size {
            if content.len() > max_size {
                return Err(PolicyViolation::ContentTooLarge {
                    size: content.len(),
                    max: max_size,
                });
            }
        }

        // Check for file paths if not allowed
        if !self.allow_file_paths && contains_file_path(content) {
            return Err(PolicyViolation::FilePathsNotAllowed);
        }

        Ok(())
    }
}

/// Policy violation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyViolation {
    /// Job type not in allowlist.
    JobTypeNotAllowed(String),
    /// Provider not trusted.
    UntrustedProvider(String),
    /// Content exceeds size limit.
    ContentTooLarge { size: usize, max: usize },
    /// File paths found when not allowed.
    FilePathsNotAllowed,
    /// Verification required but not available.
    VerificationRequired,
}

impl std::fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyViolation::JobTypeNotAllowed(t) => {
                write!(f, "Job type '{}' not allowed by privacy policy", t)
            }
            PolicyViolation::UntrustedProvider(p) => {
                write!(f, "Provider '{}' not in trusted list", p)
            }
            PolicyViolation::ContentTooLarge { size, max } => {
                write!(f, "Content size {} exceeds maximum allowed {}", size, max)
            }
            PolicyViolation::FilePathsNotAllowed => {
                write!(f, "File paths found in content but not allowed by policy")
            }
            PolicyViolation::VerificationRequired => {
                write!(f, "Job verification required but not available")
            }
        }
    }
}

impl std::error::Error for PolicyViolation {}

/// Check if content contains file paths.
fn contains_file_path(content: &str) -> bool {
    // Check for Unix paths
    if content.contains("/Users/")
        || content.contains("/home/")
        || content.contains("/var/")
        || content.contains("/tmp/")
    {
        return true;
    }

    // Check for Windows paths
    if content.contains("C:\\") || content.contains("D:\\") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = PrivacyPolicy::default();
        assert!(policy.is_job_allowed("any.job.type"));
        assert!(policy.is_provider_trusted("any_provider"));
        assert!(!policy.require_verification);
    }

    #[test]
    fn test_open_source_policy() {
        let policy = PrivacyPolicy::open_source();
        assert!(matches!(policy.redaction.mode, RedactionMode::None));
        assert!(matches!(policy.chunking, ChunkingPolicy::Full));
        assert!(policy.is_job_allowed("any.job.type"));
    }

    #[test]
    fn test_private_repo_policy() {
        let policy = PrivacyPolicy::private_repo();
        assert!(matches!(policy.redaction.mode, RedactionMode::PathsOnly));
        assert!(policy.is_job_allowed("oa.sandbox_run.v1"));
        assert!(!policy.is_job_allowed("oa.code_chunk_analysis.v1"));
        assert!(policy.require_verification);
        assert!(!policy.allow_file_paths);
    }

    #[test]
    fn test_paranoid_policy() {
        let policy = PrivacyPolicy::paranoid();
        assert!(matches!(policy.redaction.mode, RedactionMode::Full));
        assert_eq!(policy.max_content_size, Some(10_000));
    }

    #[test]
    fn test_job_allowlist() {
        let policy = PrivacyPolicy::new()
            .allow_job_type("job.type.one")
            .allow_job_type("job.type.two");

        assert!(policy.is_job_allowed("job.type.one"));
        assert!(policy.is_job_allowed("job.type.two"));
        assert!(!policy.is_job_allowed("job.type.three"));
    }

    #[test]
    fn test_trusted_providers() {
        let policy = PrivacyPolicy::new()
            .trust_provider("npub123abc")
            .trust_provider("npub456def");

        assert!(policy.is_provider_trusted("npub123abc"));
        assert!(policy.is_provider_trusted("npub456def"));
        assert!(!policy.is_provider_trusted("npub789ghi"));
    }

    #[test]
    fn test_content_validation() {
        let policy = PrivacyPolicy::private_repo();

        // Content too large
        let large_content = "x".repeat(100_000);
        assert!(matches!(
            policy.validate_content(&large_content),
            Err(PolicyViolation::ContentTooLarge { .. })
        ));

        // File paths not allowed
        let path_content = "Load file from /Users/alice/secret/data.txt";
        assert!(matches!(
            policy.validate_content(path_content),
            Err(PolicyViolation::FilePathsNotAllowed)
        ));

        // Valid content
        let valid_content = "fn main() { println!(\"hello\"); }";
        assert!(policy.validate_content(valid_content).is_ok());
    }

    #[test]
    fn test_builder_pattern() {
        let policy = PrivacyPolicy::new()
            .with_redaction_mode(RedactionMode::Identifiers)
            .with_chunking(ChunkingPolicy::minimal())
            .with_max_content_size(5000)
            .require_verification();

        assert!(matches!(policy.redaction.mode, RedactionMode::Identifiers));
        assert!(matches!(
            policy.chunking,
            ChunkingPolicy::MinimalSpans { .. }
        ));
        assert_eq!(policy.max_content_size, Some(5000));
        assert!(policy.require_verification);
    }
}
