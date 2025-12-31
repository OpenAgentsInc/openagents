//! PatchGen job types for NIP-90 compute marketplace (Bazaar)
//!
//! PatchGen jobs generate code patches from issue descriptions using agentic AI.
//! The provider runs Claude Code (or similar) in a sandbox to understand the codebase
//! and produce a verifiable patch.

use serde::{Deserialize, Serialize};

use nostr::nip90::{JobInput, JobRequest, JobResult, Nip90Error, KIND_JOB_PATCH_GEN};

/// Path filter for restricting which files the agent can see/modify
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PathFilter {
    /// Glob patterns to include (e.g., "src/**/*.rs")
    pub include: Vec<String>,
    /// Glob patterns to exclude (e.g., "**/tests/**")
    pub exclude: Vec<String>,
}

impl PathFilter {
    /// Create a filter that includes all files
    pub fn all() -> Self {
        Self {
            include: vec!["**/*".to_string()],
            exclude: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
                "**/vendor/**".to_string(),
            ],
        }
    }

    /// Create a filter for specific paths
    pub fn new(include: Vec<String>) -> Self {
        Self {
            include,
            exclude: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
            ],
        }
    }

    /// Add an include pattern
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include.push(pattern.into());
        self
    }

    /// Add an exclude pattern
    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude.push(pattern.into());
        self
    }
}

/// A patch generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchGenRequest {
    /// Repository URL (git clone URL)
    pub repo: String,
    /// Git reference (branch, tag, or commit SHA)
    pub git_ref: String,
    /// Issue description or task to complete
    pub issue: String,
    /// Optional issue URL for additional context
    pub issue_url: Option<String>,
    /// Path filter to restrict file access
    pub path_filter: PathFilter,
    /// Maximum time limit in seconds
    pub time_limit_secs: u32,
    /// Model preference (e.g., "claude-sonnet-4", "claude-opus-4")
    pub model: Option<String>,
    /// Additional context or constraints
    pub context: Option<String>,
    /// Whether to run tests after generating patch
    pub run_tests: bool,
    /// Test command to run (defaults to auto-detect)
    pub test_command: Option<String>,
}

impl PatchGenRequest {
    /// Create a new patch generation request
    pub fn new(
        repo: impl Into<String>,
        git_ref: impl Into<String>,
        issue: impl Into<String>,
    ) -> Self {
        Self {
            repo: repo.into(),
            git_ref: git_ref.into(),
            issue: issue.into(),
            issue_url: None,
            path_filter: PathFilter::all(),
            time_limit_secs: 900, // 15 minutes default
            model: None,
            context: None,
            run_tests: true,
            test_command: None,
        }
    }

    /// Set the issue URL
    pub fn with_issue_url(mut self, url: impl Into<String>) -> Self {
        self.issue_url = Some(url.into());
        self
    }

    /// Set the path filter
    pub fn with_path_filter(mut self, filter: PathFilter) -> Self {
        self.path_filter = filter;
        self
    }

    /// Set the time limit
    pub fn with_time_limit(mut self, secs: u32) -> Self {
        self.time_limit_secs = secs;
        self
    }

    /// Set the preferred model
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Add additional context
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Set whether to run tests
    pub fn with_tests(mut self, run_tests: bool) -> Self {
        self.run_tests = run_tests;
        self
    }

    /// Set the test command
    pub fn with_test_command(mut self, cmd: impl Into<String>) -> Self {
        self.test_command = Some(cmd.into());
        self
    }

    /// Convert to NIP-90 JobRequest
    pub fn to_job_request(&self) -> Result<JobRequest, Nip90Error> {
        let mut request = JobRequest::new(KIND_JOB_PATCH_GEN)?
            .add_input(JobInput::url(&self.repo).with_marker("repo"))
            .add_input(JobInput::text(&self.issue).with_marker("issue"))
            .add_param("git_ref", &self.git_ref)
            .add_param("time_limit_secs", self.time_limit_secs.to_string())
            .add_param("run_tests", self.run_tests.to_string());

        // Add optional fields
        if let Some(ref url) = self.issue_url {
            request = request.add_param("issue_url", url);
        }

        if let Some(ref model) = self.model {
            request = request.add_param("model", model);
        }

        if let Some(ref context) = self.context {
            request = request.add_param("context", context);
        }

        if let Some(ref cmd) = self.test_command {
            request = request.add_param("test_command", cmd);
        }

        // Add path filter patterns
        for (i, pattern) in self.path_filter.include.iter().enumerate() {
            request = request.add_param(format!("include_{}", i), pattern);
        }
        for (i, pattern) in self.path_filter.exclude.iter().enumerate() {
            request = request.add_param(format!("exclude_{}", i), pattern);
        }

        Ok(request)
    }

    /// Parse from NIP-90 JobRequest
    pub fn from_job_request(request: &JobRequest) -> Result<Self, Nip90Error> {
        if request.kind != KIND_JOB_PATCH_GEN {
            return Err(Nip90Error::InvalidKind(request.kind, "5932".to_string()));
        }

        let mut repo = String::new();
        let mut issue = String::new();
        let mut git_ref = String::new();
        let mut issue_url = None;
        let mut include_patterns = Vec::new();
        let mut exclude_patterns = Vec::new();
        let mut time_limit_secs = 900;
        let mut model = None;
        let mut context = None;
        let mut run_tests = true;
        let mut test_command = None;

        // Extract from inputs
        for input in &request.inputs {
            match input.marker.as_deref() {
                Some("repo") => repo = input.data.clone(),
                Some("issue") => issue = input.data.clone(),
                _ => {}
            }
        }

        // Extract params
        for param in &request.params {
            match param.key.as_str() {
                "git_ref" => git_ref = param.value.clone(),
                "issue_url" => issue_url = Some(param.value.clone()),
                "time_limit_secs" => {
                    if let Ok(v) = param.value.parse() {
                        time_limit_secs = v;
                    }
                }
                "model" => model = Some(param.value.clone()),
                "context" => context = Some(param.value.clone()),
                "run_tests" => run_tests = param.value == "true",
                "test_command" => test_command = Some(param.value.clone()),
                key if key.starts_with("include_") => {
                    include_patterns.push(param.value.clone());
                }
                key if key.starts_with("exclude_") => {
                    exclude_patterns.push(param.value.clone());
                }
                _ => {}
            }
        }

        let path_filter = if include_patterns.is_empty() {
            PathFilter::all()
        } else {
            PathFilter {
                include: include_patterns,
                exclude: exclude_patterns,
            }
        };

        Ok(Self {
            repo,
            git_ref,
            issue,
            issue_url,
            path_filter,
            time_limit_secs,
            model,
            context,
            run_tests,
            test_command,
        })
    }
}

/// Verification data for a generated patch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchVerification {
    /// Whether the patch applies cleanly to the target ref
    pub applies_cleanly: bool,
    /// Test exit code (0 = pass, None = not run)
    pub test_exit_code: Option<i32>,
    /// Test output (stdout + stderr)
    pub test_output: Option<String>,
    /// Number of files changed
    pub files_changed: u32,
    /// Lines added
    pub lines_added: u32,
    /// Lines removed
    pub lines_removed: u32,
}

impl PatchVerification {
    /// Create verification data for a successful patch
    pub fn success(files_changed: u32, lines_added: u32, lines_removed: u32) -> Self {
        Self {
            applies_cleanly: true,
            test_exit_code: None,
            test_output: None,
            files_changed,
            lines_added,
            lines_removed,
        }
    }

    /// Set test results
    pub fn with_test_results(mut self, exit_code: i32, output: impl Into<String>) -> Self {
        self.test_exit_code = Some(exit_code);
        self.test_output = Some(output.into());
        self
    }

    /// Check if tests passed
    pub fn tests_passed(&self) -> bool {
        self.test_exit_code == Some(0)
    }
}

/// Token usage during patch generation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    /// Input tokens consumed
    pub input_tokens: u64,
    /// Output tokens generated
    pub output_tokens: u64,
    /// Cache read tokens (if applicable)
    pub cache_read_tokens: u64,
    /// Cache write tokens (if applicable)
    pub cache_write_tokens: u64,
}

impl TokenUsage {
    /// Total tokens consumed
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Result of a patch generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchGenResult {
    /// The generated patch in unified diff format
    pub patch: String,
    /// SHA256 hash of the patch content
    pub patch_sha256: String,
    /// Trajectory ID for replay/debugging
    pub trajectory_id: Option<String>,
    /// Verification data
    pub verification: PatchVerification,
    /// Token usage
    pub usage: TokenUsage,
    /// Processing duration in milliseconds
    pub duration_ms: u64,
    /// Model that was used
    pub model_used: String,
    /// Summary of changes made
    pub summary: String,
    /// Files that were modified
    pub files_modified: Vec<String>,
}

impl PatchGenResult {
    /// Create a new patch result
    pub fn new(patch: impl Into<String>, patch_sha256: impl Into<String>) -> Self {
        Self {
            patch: patch.into(),
            patch_sha256: patch_sha256.into(),
            trajectory_id: None,
            verification: PatchVerification::success(0, 0, 0),
            usage: TokenUsage::default(),
            duration_ms: 0,
            model_used: String::new(),
            summary: String::new(),
            files_modified: Vec::new(),
        }
    }

    /// Set the trajectory ID
    pub fn with_trajectory(mut self, id: impl Into<String>) -> Self {
        self.trajectory_id = Some(id.into());
        self
    }

    /// Set verification data
    pub fn with_verification(mut self, verification: PatchVerification) -> Self {
        self.verification = verification;
        self
    }

    /// Set token usage
    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = usage;
        self
    }

    /// Set duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Set model used
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model_used = model.into();
        self
    }

    /// Set summary
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// Add a modified file
    pub fn add_file(mut self, path: impl Into<String>) -> Self {
        self.files_modified.push(path.into());
        self
    }

    /// Convert to NIP-90 JobResult
    pub fn to_job_result(
        &self,
        request_id: &str,
        customer_pubkey: &str,
        amount: Option<u64>,
        bolt11: Option<String>,
    ) -> Result<JobResult, Nip90Error> {
        // Content is the patch itself for easy access
        let mut result = JobResult::new(KIND_JOB_PATCH_GEN, request_id, customer_pubkey, &self.patch)?;

        if let Some(amt) = amount {
            result = result.with_amount(amt, bolt11);
        }

        Ok(result)
    }

    /// Get metadata as JSON for result tags
    pub fn to_metadata_json(&self) -> Result<String, Nip90Error> {
        serde_json::to_string(self).map_err(|e| Nip90Error::Serialization(e.to_string()))
    }

    /// Parse from JSON metadata
    pub fn from_json(json: &str) -> Result<Self, Nip90Error> {
        serde_json::from_str(json).map_err(|e| Nip90Error::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_filter_all() {
        let filter = PathFilter::all();
        assert!(filter.include.contains(&"**/*".to_string()));
        assert!(filter.exclude.contains(&"**/node_modules/**".to_string()));
    }

    #[test]
    fn test_path_filter_builder() {
        let filter = PathFilter::new(vec!["src/**/*.rs".to_string()])
            .include("crates/**/*.rs")
            .exclude("**/tests/**");

        assert!(filter.include.contains(&"src/**/*.rs".to_string()));
        assert!(filter.include.contains(&"crates/**/*.rs".to_string()));
        assert!(filter.exclude.contains(&"**/tests/**".to_string()));
    }

    #[test]
    fn test_patch_gen_request_new() {
        let request = PatchGenRequest::new(
            "https://github.com/owner/repo.git",
            "main",
            "Fix the bug in the login handler",
        );

        assert_eq!(request.repo, "https://github.com/owner/repo.git");
        assert_eq!(request.git_ref, "main");
        assert_eq!(request.issue, "Fix the bug in the login handler");
        assert_eq!(request.time_limit_secs, 900);
        assert!(request.run_tests);
    }

    #[test]
    fn test_patch_gen_request_builder() {
        let request = PatchGenRequest::new(
            "https://github.com/owner/repo.git",
            "main",
            "Add dark mode support",
        )
        .with_issue_url("https://github.com/owner/repo/issues/42")
        .with_time_limit(1800)
        .with_model("claude-sonnet-4")
        .with_context("Use the existing theme system")
        .with_tests(true)
        .with_test_command("cargo test");

        assert_eq!(request.issue_url, Some("https://github.com/owner/repo/issues/42".to_string()));
        assert_eq!(request.time_limit_secs, 1800);
        assert_eq!(request.model, Some("claude-sonnet-4".to_string()));
        assert_eq!(request.context, Some("Use the existing theme system".to_string()));
        assert_eq!(request.test_command, Some("cargo test".to_string()));
    }

    #[test]
    fn test_patch_gen_request_to_job_request() {
        let request = PatchGenRequest::new(
            "https://github.com/owner/repo.git",
            "main",
            "Fix the bug",
        );

        let job = request.to_job_request().unwrap();
        assert_eq!(job.kind, KIND_JOB_PATCH_GEN);
        assert_eq!(job.inputs.len(), 2); // repo + issue
    }

    #[test]
    fn test_patch_verification() {
        let verification = PatchVerification::success(3, 50, 10)
            .with_test_results(0, "All tests passed");

        assert!(verification.applies_cleanly);
        assert!(verification.tests_passed());
        assert_eq!(verification.files_changed, 3);
        assert_eq!(verification.lines_added, 50);
        assert_eq!(verification.lines_removed, 10);
    }

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_write_tokens: 100,
        };

        assert_eq!(usage.total(), 1500);
    }

    #[test]
    fn test_patch_gen_result() {
        let result = PatchGenResult::new(
            "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n+// Fixed bug",
            "abc123def456",
        )
        .with_trajectory("traj-001")
        .with_model("claude-sonnet-4")
        .with_summary("Fixed the login bug by adding null check")
        .add_file("src/main.rs")
        .with_duration(60000);

        assert_eq!(result.trajectory_id, Some("traj-001".to_string()));
        assert_eq!(result.model_used, "claude-sonnet-4");
        assert_eq!(result.duration_ms, 60000);
        assert!(result.files_modified.contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn test_patch_gen_result_serialization() {
        let result = PatchGenResult::new("diff content", "sha256hash")
            .with_model("claude-sonnet-4")
            .with_summary("Test summary");

        let json = result.to_metadata_json().unwrap();
        let parsed = PatchGenResult::from_json(&json).unwrap();

        assert_eq!(parsed.patch, result.patch);
        assert_eq!(parsed.model_used, result.model_used);
        assert_eq!(parsed.summary, result.summary);
    }
}
