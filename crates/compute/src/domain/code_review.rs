//! CodeReview job types for NIP-90 compute marketplace (Bazaar)
//!
//! CodeReview jobs analyze code changes and provide structured feedback.
//! The provider runs Claude Code (or similar) to review patches, PRs, or diffs.

use serde::{Deserialize, Serialize};

use nostr::nip90::{JobInput, JobRequest, JobResult, Nip90Error, KIND_JOB_CODE_REVIEW};

/// Severity level for review issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    /// Critical issue that must be fixed (security, data loss, etc.)
    Critical,
    /// Major issue that should be fixed before merge
    Major,
    /// Minor issue or improvement suggestion
    Minor,
    /// Informational or nitpick
    Nit,
}

impl IssueSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueSeverity::Critical => "critical",
            IssueSeverity::Major => "major",
            IssueSeverity::Minor => "minor",
            IssueSeverity::Nit => "nit",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "critical" => Some(IssueSeverity::Critical),
            "major" => Some(IssueSeverity::Major),
            "minor" => Some(IssueSeverity::Minor),
            "nit" | "nitpick" => Some(IssueSeverity::Nit),
            _ => None,
        }
    }
}

/// Category of review issue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueCategory {
    /// Security vulnerability
    Security,
    /// Bug or logic error
    Bug,
    /// Performance issue
    Performance,
    /// Code style or formatting
    Style,
    /// Documentation issue
    Documentation,
    /// Test coverage
    Testing,
    /// Architecture or design concern
    Architecture,
    /// Maintainability concern
    Maintainability,
    /// Other
    Other,
}

impl IssueCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueCategory::Security => "security",
            IssueCategory::Bug => "bug",
            IssueCategory::Performance => "performance",
            IssueCategory::Style => "style",
            IssueCategory::Documentation => "documentation",
            IssueCategory::Testing => "testing",
            IssueCategory::Architecture => "architecture",
            IssueCategory::Maintainability => "maintainability",
            IssueCategory::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "security" => Some(IssueCategory::Security),
            "bug" => Some(IssueCategory::Bug),
            "performance" => Some(IssueCategory::Performance),
            "style" => Some(IssueCategory::Style),
            "documentation" | "docs" => Some(IssueCategory::Documentation),
            "testing" | "test" => Some(IssueCategory::Testing),
            "architecture" | "arch" => Some(IssueCategory::Architecture),
            "maintainability" => Some(IssueCategory::Maintainability),
            "other" => Some(IssueCategory::Other),
            _ => None,
        }
    }
}

/// Approval status for the review
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    /// Changes look good, approve for merge
    Approve,
    /// Changes need modifications before merge
    RequestChanges,
    /// Comment only, no approval decision
    Comment,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApprovalStatus::Approve => "approve",
            ApprovalStatus::RequestChanges => "request_changes",
            ApprovalStatus::Comment => "comment",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "approve" | "approved" | "lgtm" => Some(ApprovalStatus::Approve),
            "request_changes" | "changes_requested" => Some(ApprovalStatus::RequestChanges),
            "comment" | "comments" => Some(ApprovalStatus::Comment),
            _ => None,
        }
    }
}

/// A single issue found during code review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    /// File path relative to repo root
    pub file_path: String,
    /// Line number (1-indexed, None for file-level issues)
    pub line: Option<u32>,
    /// End line for multi-line issues
    pub end_line: Option<u32>,
    /// Issue severity
    pub severity: IssueSeverity,
    /// Issue category
    pub category: IssueCategory,
    /// Short title for the issue
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Suggested fix (optional)
    pub suggestion: Option<String>,
    /// Code snippet showing the issue
    pub code_snippet: Option<String>,
}

impl ReviewIssue {
    /// Create a new review issue
    pub fn new(
        file_path: impl Into<String>,
        severity: IssueSeverity,
        category: IssueCategory,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            line: None,
            end_line: None,
            severity,
            category,
            title: title.into(),
            description: description.into(),
            suggestion: None,
            code_snippet: None,
        }
    }

    /// Set the line number
    pub fn at_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Set line range for multi-line issues
    pub fn at_lines(mut self, start: u32, end: u32) -> Self {
        self.line = Some(start);
        self.end_line = Some(end);
        self
    }

    /// Add a suggested fix
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add a code snippet
    pub fn with_snippet(mut self, snippet: impl Into<String>) -> Self {
        self.code_snippet = Some(snippet.into());
        self
    }
}

/// Type of input for the review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewInput {
    /// Unified diff content
    Diff(String),
    /// Pull request URL
    PullRequest { url: String },
    /// Commit SHA(s) to review
    Commits { repo: String, shas: Vec<String> },
    /// Specific files to review at a ref
    Files { repo: String, git_ref: String, paths: Vec<String> },
}

/// A code review request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewRequest {
    /// What to review
    pub input: ReviewInput,
    /// Repository URL (for context)
    pub repo: Option<String>,
    /// Base ref (for diff context)
    pub base_ref: Option<String>,
    /// Head ref (what we're reviewing)
    pub head_ref: Option<String>,
    /// Focus areas for the review
    pub focus_areas: Vec<IssueCategory>,
    /// Maximum time limit in seconds
    pub time_limit_secs: u32,
    /// Model preference
    pub model: Option<String>,
    /// Additional context or review guidelines
    pub guidelines: Option<String>,
    /// Whether to include style/nit suggestions
    pub include_nits: bool,
}

impl CodeReviewRequest {
    /// Create a review request for a diff
    pub fn from_diff(diff: impl Into<String>) -> Self {
        Self {
            input: ReviewInput::Diff(diff.into()),
            repo: None,
            base_ref: None,
            head_ref: None,
            focus_areas: Vec::new(),
            time_limit_secs: 300, // 5 minutes default
            model: None,
            guidelines: None,
            include_nits: false,
        }
    }

    /// Create a review request for a pull request
    pub fn from_pr(url: impl Into<String>) -> Self {
        Self {
            input: ReviewInput::PullRequest { url: url.into() },
            repo: None,
            base_ref: None,
            head_ref: None,
            focus_areas: Vec::new(),
            time_limit_secs: 300,
            model: None,
            guidelines: None,
            include_nits: false,
        }
    }

    /// Create a review request for commits
    pub fn from_commits(repo: impl Into<String>, shas: Vec<String>) -> Self {
        Self {
            input: ReviewInput::Commits {
                repo: repo.into(),
                shas,
            },
            repo: None,
            base_ref: None,
            head_ref: None,
            focus_areas: Vec::new(),
            time_limit_secs: 300,
            model: None,
            guidelines: None,
            include_nits: false,
        }
    }

    /// Set the repository URL
    pub fn with_repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = Some(repo.into());
        self
    }

    /// Set the base reference
    pub fn with_base_ref(mut self, base: impl Into<String>) -> Self {
        self.base_ref = Some(base.into());
        self
    }

    /// Set the head reference
    pub fn with_head_ref(mut self, head: impl Into<String>) -> Self {
        self.head_ref = Some(head.into());
        self
    }

    /// Add a focus area
    pub fn focus_on(mut self, category: IssueCategory) -> Self {
        if !self.focus_areas.contains(&category) {
            self.focus_areas.push(category);
        }
        self
    }

    /// Set time limit
    pub fn with_time_limit(mut self, secs: u32) -> Self {
        self.time_limit_secs = secs;
        self
    }

    /// Set model preference
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Add review guidelines
    pub fn with_guidelines(mut self, guidelines: impl Into<String>) -> Self {
        self.guidelines = Some(guidelines.into());
        self
    }

    /// Include nit/style suggestions
    pub fn include_nits(mut self, include: bool) -> Self {
        self.include_nits = include;
        self
    }

    /// Convert to NIP-90 JobRequest
    pub fn to_job_request(&self) -> Result<JobRequest, Nip90Error> {
        let mut request = JobRequest::new(KIND_JOB_CODE_REVIEW)?
            .add_param("time_limit_secs", self.time_limit_secs.to_string())
            .add_param("include_nits", self.include_nits.to_string());

        // Add input based on type
        match &self.input {
            ReviewInput::Diff(diff) => {
                request = request.add_input(JobInput::text(diff).with_marker("diff"));
            }
            ReviewInput::PullRequest { url } => {
                request = request.add_input(JobInput::url(url).with_marker("pr"));
            }
            ReviewInput::Commits { repo, shas } => {
                request = request.add_input(JobInput::url(repo).with_marker("repo"));
                for (i, sha) in shas.iter().enumerate() {
                    request = request.add_param(format!("commit_{}", i), sha);
                }
            }
            ReviewInput::Files { repo, git_ref, paths } => {
                request = request
                    .add_input(JobInput::url(repo).with_marker("repo"))
                    .add_param("git_ref", git_ref);
                for (i, path) in paths.iter().enumerate() {
                    request = request.add_param(format!("file_{}", i), path);
                }
            }
        }

        // Add optional fields
        if let Some(ref repo) = self.repo {
            request = request.add_param("repo_url", repo);
        }
        if let Some(ref base) = self.base_ref {
            request = request.add_param("base_ref", base);
        }
        if let Some(ref head) = self.head_ref {
            request = request.add_param("head_ref", head);
        }
        if let Some(ref model) = self.model {
            request = request.add_param("model", model);
        }
        if let Some(ref guidelines) = self.guidelines {
            request = request.add_param("guidelines", guidelines);
        }

        // Add focus areas
        for (i, area) in self.focus_areas.iter().enumerate() {
            request = request.add_param(format!("focus_{}", i), area.as_str());
        }

        Ok(request)
    }

    /// Parse from NIP-90 JobRequest
    pub fn from_job_request(request: &JobRequest) -> Result<Self, Nip90Error> {
        if request.kind != KIND_JOB_CODE_REVIEW {
            return Err(Nip90Error::InvalidKind(request.kind, "5933".to_string()));
        }

        let mut diff = None;
        let mut pr_url = None;
        let mut repo_url = None;
        let mut commits = Vec::new();
        let mut files = Vec::new();
        let mut git_ref = None;
        let mut base_ref = None;
        let mut head_ref = None;
        let mut focus_areas = Vec::new();
        let mut time_limit_secs = 300;
        let mut model = None;
        let mut guidelines = None;
        let mut include_nits = false;

        // Extract from inputs
        for input in &request.inputs {
            match input.marker.as_deref() {
                Some("diff") => diff = Some(input.data.clone()),
                Some("pr") => pr_url = Some(input.data.clone()),
                Some("repo") => repo_url = Some(input.data.clone()),
                _ => {}
            }
        }

        // Extract params
        for param in &request.params {
            match param.key.as_str() {
                "repo_url" => repo_url = Some(param.value.clone()),
                "base_ref" => base_ref = Some(param.value.clone()),
                "head_ref" => head_ref = Some(param.value.clone()),
                "git_ref" => git_ref = Some(param.value.clone()),
                "time_limit_secs" => {
                    if let Ok(v) = param.value.parse() {
                        time_limit_secs = v;
                    }
                }
                "model" => model = Some(param.value.clone()),
                "guidelines" => guidelines = Some(param.value.clone()),
                "include_nits" => include_nits = param.value == "true",
                key if key.starts_with("commit_") => {
                    commits.push(param.value.clone());
                }
                key if key.starts_with("file_") => {
                    files.push(param.value.clone());
                }
                key if key.starts_with("focus_") => {
                    if let Some(cat) = IssueCategory::from_str(&param.value) {
                        focus_areas.push(cat);
                    }
                }
                _ => {}
            }
        }

        // Determine input type
        let input = if let Some(diff) = diff {
            ReviewInput::Diff(diff)
        } else if let Some(url) = pr_url {
            ReviewInput::PullRequest { url }
        } else if !commits.is_empty() {
            ReviewInput::Commits {
                repo: repo_url.clone().unwrap_or_default(),
                shas: commits,
            }
        } else if !files.is_empty() {
            ReviewInput::Files {
                repo: repo_url.clone().unwrap_or_default(),
                git_ref: git_ref.unwrap_or_else(|| "HEAD".to_string()),
                paths: files,
            }
        } else {
            return Err(Nip90Error::MissingTag("review input (diff, pr, commits, or files)".to_string()));
        };

        Ok(Self {
            input,
            repo: repo_url,
            base_ref,
            head_ref,
            focus_areas,
            time_limit_secs,
            model,
            guidelines,
            include_nits,
        })
    }
}

/// Summary statistics for a code review
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewStats {
    /// Number of files reviewed
    pub files_reviewed: u32,
    /// Lines of code reviewed
    pub lines_reviewed: u32,
    /// Total issues found
    pub total_issues: u32,
    /// Critical issues
    pub critical_issues: u32,
    /// Major issues
    pub major_issues: u32,
    /// Minor issues
    pub minor_issues: u32,
    /// Nits/suggestions
    pub nits: u32,
}

impl ReviewStats {
    /// Check if there are blocking issues
    pub fn has_blocking_issues(&self) -> bool {
        self.critical_issues > 0 || self.major_issues > 0
    }
}

/// Result of a code review job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewResult {
    /// Overall approval status
    pub status: ApprovalStatus,
    /// Review summary/comment
    pub summary: String,
    /// Individual issues found
    pub issues: Vec<ReviewIssue>,
    /// Statistics
    pub stats: ReviewStats,
    /// SHA256 hash of the review content
    pub review_sha256: String,
    /// Token usage
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Processing duration in milliseconds
    pub duration_ms: u64,
    /// Model that was used
    pub model_used: String,
}

impl CodeReviewResult {
    /// Create a new approval result
    pub fn approve(summary: impl Into<String>) -> Self {
        Self {
            status: ApprovalStatus::Approve,
            summary: summary.into(),
            issues: Vec::new(),
            stats: ReviewStats::default(),
            review_sha256: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            duration_ms: 0,
            model_used: String::new(),
        }
    }

    /// Create a result requesting changes
    pub fn request_changes(summary: impl Into<String>) -> Self {
        Self {
            status: ApprovalStatus::RequestChanges,
            summary: summary.into(),
            issues: Vec::new(),
            stats: ReviewStats::default(),
            review_sha256: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            duration_ms: 0,
            model_used: String::new(),
        }
    }

    /// Create a comment-only result
    pub fn comment(summary: impl Into<String>) -> Self {
        Self {
            status: ApprovalStatus::Comment,
            summary: summary.into(),
            issues: Vec::new(),
            stats: ReviewStats::default(),
            review_sha256: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            duration_ms: 0,
            model_used: String::new(),
        }
    }

    /// Add an issue
    pub fn add_issue(mut self, issue: ReviewIssue) -> Self {
        // Update stats
        self.stats.total_issues += 1;
        match issue.severity {
            IssueSeverity::Critical => self.stats.critical_issues += 1,
            IssueSeverity::Major => self.stats.major_issues += 1,
            IssueSeverity::Minor => self.stats.minor_issues += 1,
            IssueSeverity::Nit => self.stats.nits += 1,
        }
        self.issues.push(issue);
        self
    }

    /// Set review hash
    pub fn with_hash(mut self, hash: impl Into<String>) -> Self {
        self.review_sha256 = hash.into();
        self
    }

    /// Set token usage
    pub fn with_tokens(mut self, input: u64, output: u64) -> Self {
        self.input_tokens = input;
        self.output_tokens = output;
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

    /// Set stats
    pub fn with_stats(mut self, stats: ReviewStats) -> Self {
        self.stats = stats;
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
        let content = serde_json::to_string(self)
            .map_err(|e| Nip90Error::Serialization(e.to_string()))?;

        let mut result = JobResult::new(KIND_JOB_CODE_REVIEW, request_id, customer_pubkey, content)?;

        if let Some(amt) = amount {
            result = result.with_amount(amt, bolt11);
        }

        Ok(result)
    }

    /// Parse from NIP-90 JobResult content
    pub fn from_job_result(result: &JobResult) -> Result<Self, Nip90Error> {
        serde_json::from_str(&result.content)
            .map_err(|e| Nip90Error::Serialization(e.to_string()))
    }

    /// Get the content for NIP-90 result event content field
    pub fn to_nip90_content(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_severity_roundtrip() {
        let severities = [
            IssueSeverity::Critical,
            IssueSeverity::Major,
            IssueSeverity::Minor,
            IssueSeverity::Nit,
        ];

        for s in severities {
            let str = s.as_str();
            let parsed = IssueSeverity::from_str(str).unwrap();
            assert_eq!(parsed, s);
        }
    }

    #[test]
    fn test_issue_category_roundtrip() {
        let categories = [
            IssueCategory::Security,
            IssueCategory::Bug,
            IssueCategory::Performance,
            IssueCategory::Style,
            IssueCategory::Documentation,
            IssueCategory::Testing,
            IssueCategory::Architecture,
            IssueCategory::Maintainability,
            IssueCategory::Other,
        ];

        for c in categories {
            let str = c.as_str();
            let parsed = IssueCategory::from_str(str).unwrap();
            assert_eq!(parsed, c);
        }
    }

    #[test]
    fn test_approval_status_roundtrip() {
        let statuses = [
            ApprovalStatus::Approve,
            ApprovalStatus::RequestChanges,
            ApprovalStatus::Comment,
        ];

        for s in statuses {
            let str = s.as_str();
            let parsed = ApprovalStatus::from_str(str).unwrap();
            assert_eq!(parsed, s);
        }
    }

    #[test]
    fn test_review_issue() {
        let issue = ReviewIssue::new(
            "src/main.rs",
            IssueSeverity::Major,
            IssueCategory::Security,
            "SQL injection vulnerability",
            "User input is not sanitized before being used in query",
        )
        .at_lines(42, 45)
        .with_suggestion("Use parameterized queries")
        .with_snippet("let query = format!(\"SELECT * FROM users WHERE id = {}\", user_input);");

        assert_eq!(issue.file_path, "src/main.rs");
        assert_eq!(issue.severity, IssueSeverity::Major);
        assert_eq!(issue.category, IssueCategory::Security);
        assert_eq!(issue.line, Some(42));
        assert_eq!(issue.end_line, Some(45));
        assert!(issue.suggestion.is_some());
    }

    #[test]
    fn test_code_review_request_from_diff() {
        let request = CodeReviewRequest::from_diff("--- a/file.rs\n+++ b/file.rs\n...")
            .with_repo("https://github.com/owner/repo")
            .focus_on(IssueCategory::Security)
            .focus_on(IssueCategory::Bug)
            .include_nits(true);

        assert!(matches!(request.input, ReviewInput::Diff(_)));
        assert!(request.focus_areas.contains(&IssueCategory::Security));
        assert!(request.include_nits);
    }

    #[test]
    fn test_code_review_request_from_pr() {
        let request = CodeReviewRequest::from_pr("https://github.com/owner/repo/pull/42")
            .with_time_limit(600)
            .with_model("claude-sonnet-4");

        assert!(matches!(request.input, ReviewInput::PullRequest { .. }));
        assert_eq!(request.time_limit_secs, 600);
    }

    #[test]
    fn test_code_review_request_to_job() {
        let request = CodeReviewRequest::from_diff("diff content")
            .focus_on(IssueCategory::Security);

        let job = request.to_job_request().unwrap();
        assert_eq!(job.kind, KIND_JOB_CODE_REVIEW);
    }

    #[test]
    fn test_review_stats() {
        let stats = ReviewStats {
            files_reviewed: 10,
            lines_reviewed: 500,
            total_issues: 5,
            critical_issues: 1,
            major_issues: 2,
            minor_issues: 1,
            nits: 1,
        };

        assert!(stats.has_blocking_issues());

        let clean_stats = ReviewStats {
            total_issues: 2,
            nits: 2,
            ..Default::default()
        };
        assert!(!clean_stats.has_blocking_issues());
    }

    #[test]
    fn test_code_review_result_approve() {
        let result = CodeReviewResult::approve("LGTM! Clean implementation.")
            .with_model("claude-sonnet-4")
            .with_duration(30000);

        assert_eq!(result.status, ApprovalStatus::Approve);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_code_review_result_with_issues() {
        let result = CodeReviewResult::request_changes("Several issues need addressing")
            .add_issue(ReviewIssue::new(
                "src/lib.rs",
                IssueSeverity::Critical,
                IssueCategory::Security,
                "Hardcoded credential",
                "API key is hardcoded in source",
            ))
            .add_issue(ReviewIssue::new(
                "src/main.rs",
                IssueSeverity::Minor,
                IssueCategory::Style,
                "Unused import",
                "Remove unused import",
            ));

        assert_eq!(result.status, ApprovalStatus::RequestChanges);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.stats.critical_issues, 1);
        assert_eq!(result.stats.minor_issues, 1);
        assert!(result.stats.has_blocking_issues());
    }

    #[test]
    fn test_code_review_result_serialization() {
        let result = CodeReviewResult::approve("Looks good")
            .add_issue(ReviewIssue::new(
                "src/lib.rs",
                IssueSeverity::Nit,
                IssueCategory::Style,
                "Consider renaming",
                "Variable name could be clearer",
            ))
            .with_model("claude-sonnet-4");

        let json = serde_json::to_string(&result).unwrap();
        let parsed: CodeReviewResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.status, result.status);
        assert_eq!(parsed.issues.len(), 1);
        assert_eq!(parsed.model_used, "claude-sonnet-4");
    }
}
