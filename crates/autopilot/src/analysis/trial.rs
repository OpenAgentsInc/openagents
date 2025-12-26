//! Trial Run
//!
//! Limited autopilot run to demonstrate capabilities.
//! Shows what a PR would look like without actually creating it.

use std::time::SystemTime;

use super::SuggestedIssue;
use crate::github::models::ConnectedRepo;

/// Configuration for a trial run
#[derive(Debug, Clone)]
pub struct TrialRunConfig {
    /// Maximum duration in seconds
    pub max_duration_secs: u32,
    /// Maximum tool calls
    pub max_tool_calls: u32,
    /// Maximum files to modify
    pub max_files: u32,
    /// Dry run (don't actually modify files)
    pub dry_run: bool,
    /// Model to use
    pub model: String,
}

impl Default for TrialRunConfig {
    fn default() -> Self {
        Self {
            max_duration_secs: 300, // 5 minutes
            max_tool_calls: 10,
            max_files: 3,
            dry_run: true, // Always dry run by default
            model: "sonnet".to_string(),
        }
    }
}

impl TrialRunConfig {
    /// Create a minimal trial config (very limited)
    pub fn minimal() -> Self {
        Self {
            max_duration_secs: 60,
            max_tool_calls: 3,
            max_files: 1,
            dry_run: true,
            model: "haiku".to_string(),
        }
    }

    /// Create a standard trial config
    pub fn standard() -> Self {
        Self::default()
    }
}

/// A file change in the trial run
#[derive(Debug, Clone)]
pub struct TrialFileChange {
    /// File path
    pub path: String,
    /// Change type
    pub change_type: FileChangeType,
    /// Diff content (unified format)
    pub diff: String,
    /// Lines added
    pub lines_added: u32,
    /// Lines removed
    pub lines_removed: u32,
}

/// Type of file change
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
}

impl FileChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileChangeType::Added => "added",
            FileChangeType::Modified => "modified",
            FileChangeType::Deleted => "deleted",
        }
    }
}

/// Result of a trial run
#[derive(Debug, Clone)]
pub struct TrialRunResult {
    /// Repository analyzed
    pub repo_full_name: String,
    /// Issue that was addressed
    pub issue_title: String,
    /// Issue description
    pub issue_description: String,
    /// Proposed PR title
    pub pr_title: String,
    /// Proposed PR description
    pub pr_description: String,
    /// Files that would be changed
    pub file_changes: Vec<TrialFileChange>,
    /// Duration of the trial in milliseconds
    pub duration_ms: u64,
    /// Tool calls made
    pub tool_calls: u32,
    /// Tokens used (input)
    pub tokens_in: u64,
    /// Tokens used (output)
    pub tokens_out: u64,
    /// Estimated cost in USD
    pub cost_usd: f64,
    /// Whether the trial was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Upgrade prompt to show
    pub upgrade_prompt: String,
}

impl TrialRunResult {
    /// Create a new trial result
    pub fn new(repo_full_name: impl Into<String>, issue: &SuggestedIssue) -> Self {
        Self {
            repo_full_name: repo_full_name.into(),
            issue_title: issue.title.clone(),
            issue_description: issue.body.clone(),
            pr_title: format!("Fix: {}", issue.title),
            pr_description: String::new(),
            file_changes: Vec::new(),
            duration_ms: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            cost_usd: 0.0,
            success: false,
            error: None,
            upgrade_prompt: Self::default_upgrade_prompt(),
        }
    }

    /// Default upgrade prompt
    fn default_upgrade_prompt() -> String {
        "Upgrade to OpenAgents Pro to:\n\
         - Run unlimited autopilot sessions\n\
         - Process larger codebases\n\
         - Access advanced analysis features\n\
         - Get priority support"
            .to_string()
    }

    /// Mark as successful
    pub fn success(mut self) -> Self {
        self.success = true;
        self
    }

    /// Mark as failed with error
    pub fn failed(mut self, error: impl Into<String>) -> Self {
        self.success = false;
        self.error = Some(error.into());
        self
    }

    /// Set PR details
    pub fn with_pr(mut self, title: impl Into<String>, description: impl Into<String>) -> Self {
        self.pr_title = title.into();
        self.pr_description = description.into();
        self
    }

    /// Add a file change
    pub fn add_file_change(mut self, change: TrialFileChange) -> Self {
        self.file_changes.push(change);
        self
    }

    /// Set usage stats
    pub fn with_usage(
        mut self,
        duration_ms: u64,
        tool_calls: u32,
        tokens_in: u64,
        tokens_out: u64,
    ) -> Self {
        self.duration_ms = duration_ms;
        self.tool_calls = tool_calls;
        self.tokens_in = tokens_in;
        self.tokens_out = tokens_out;
        self.cost_usd = Self::estimate_cost(tokens_in, tokens_out);
        self
    }

    /// Estimate cost based on token usage
    fn estimate_cost(tokens_in: u64, tokens_out: u64) -> f64 {
        // Rough Sonnet pricing: $3/M input, $15/M output
        let input_cost = tokens_in as f64 * 3.0 / 1_000_000.0;
        let output_cost = tokens_out as f64 * 15.0 / 1_000_000.0;
        input_cost + output_cost
    }

    /// Get total lines changed
    pub fn total_lines_changed(&self) -> u32 {
        self.file_changes
            .iter()
            .map(|c| c.lines_added + c.lines_removed)
            .sum()
    }

    /// Generate a summary
    pub fn summary(&self) -> String {
        let mut lines = vec![
            format!("## Trial Run: {}", self.repo_full_name),
            String::new(),
            format!("**Issue:** {}", self.issue_title),
            format!("**Status:** {}", if self.success { "✅ Success" } else { "❌ Failed" }),
            String::new(),
        ];

        if self.success {
            lines.push("### Proposed PR".to_string());
            lines.push(format!("**Title:** {}", self.pr_title));
            lines.push(format!("\n{}\n", self.pr_description));

            if !self.file_changes.is_empty() {
                lines.push("### File Changes".to_string());
                for change in &self.file_changes {
                    lines.push(format!(
                        "- `{}` ({}) +{} -{}",
                        change.path,
                        change.change_type.as_str(),
                        change.lines_added,
                        change.lines_removed
                    ));
                }
                lines.push(String::new());
            }

            lines.push("### Usage".to_string());
            lines.push(format!("- Duration: {}ms", self.duration_ms));
            lines.push(format!("- Tool calls: {}", self.tool_calls));
            lines.push(format!("- Tokens: {} in, {} out", self.tokens_in, self.tokens_out));
            lines.push(format!("- Estimated cost: ${:.4}", self.cost_usd));
        } else if let Some(ref error) = self.error {
            lines.push(format!("**Error:** {}", error));
        }

        lines.push(String::new());
        lines.push("---".to_string());
        lines.push(self.upgrade_prompt.clone());

        lines.join("\n")
    }
}

/// Trial run executor
pub struct TrialRun {
    config: TrialRunConfig,
}

impl TrialRun {
    /// Create a new trial run executor
    pub fn new(config: TrialRunConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(TrialRunConfig::default())
    }

    /// Execute a trial run for an issue
    ///
    /// This creates a simulated result for now.
    /// In production, it would actually run autopilot with limits.
    pub fn execute(&self, repo: &ConnectedRepo, issue: &SuggestedIssue) -> TrialRunResult {
        let start = SystemTime::now();

        // Simulate some work
        let result = TrialRunResult::new(&repo.full_name, issue)
            .with_pr(
                format!("Add: {}", issue.title),
                format!(
                    "This PR addresses: {}\n\n## Changes\n- Added implementation\n- Added tests\n\nFixes #1",
                    issue.title
                ),
            )
            .add_file_change(TrialFileChange {
                path: "src/lib.rs".to_string(),
                change_type: FileChangeType::Modified,
                diff: "@@ -10,6 +10,15 @@\n fn existing() {}\n+\n+/// New function\n+pub fn new_feature() -> bool {\n+    true\n+}\n".to_string(),
                lines_added: 5,
                lines_removed: 0,
            })
            .add_file_change(TrialFileChange {
                path: "src/tests.rs".to_string(),
                change_type: FileChangeType::Added,
                diff: "+#[test]\n+fn test_new_feature() {\n+    assert!(new_feature());\n+}\n".to_string(),
                lines_added: 4,
                lines_removed: 0,
            })
            .with_usage(
                start.elapsed().unwrap_or_default().as_millis() as u64 + 2000,
                5,
                3000,
                1500,
            )
            .success();

        result
    }

    /// Check if we're within limits
    pub fn within_limits(&self, tool_calls: u32, files_modified: u32, elapsed_secs: u32) -> bool {
        tool_calls <= self.config.max_tool_calls
            && files_modified <= self.config.max_files
            && elapsed_secs <= self.config.max_duration_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::IssueComplexity;

    fn create_test_repo() -> ConnectedRepo {
        ConnectedRepo {
            id: 1,
            owner: "test".to_string(),
            repo: "repo".to_string(),
            full_name: "test/repo".to_string(),
            default_branch: "main".to_string(),
            languages: vec![],
            connected_at: chrono::Utc::now(),
        }
    }

    fn create_test_issue() -> SuggestedIssue {
        SuggestedIssue {
            title: "Add unit tests".to_string(),
            body: "Add tests for core module".to_string(),
            labels: vec!["testing".to_string()],
            complexity: IssueComplexity::GoodFirstIssue,
            affected_files: vec!["src/core.rs".to_string()],
        }
    }

    #[test]
    fn test_default_config() {
        let config = TrialRunConfig::default();
        assert_eq!(config.max_duration_secs, 300);
        assert!(config.dry_run);
    }

    #[test]
    fn test_minimal_config() {
        let config = TrialRunConfig::minimal();
        assert_eq!(config.max_duration_secs, 60);
        assert_eq!(config.max_tool_calls, 3);
    }

    #[test]
    fn test_trial_result_creation() {
        let issue = create_test_issue();
        let result = TrialRunResult::new("test/repo", &issue);

        assert_eq!(result.repo_full_name, "test/repo");
        assert_eq!(result.issue_title, "Add unit tests");
        assert!(!result.success);
    }

    #[test]
    fn test_trial_result_success() {
        let issue = create_test_issue();
        let result = TrialRunResult::new("test/repo", &issue).success();
        assert!(result.success);
    }

    #[test]
    fn test_trial_result_failed() {
        let issue = create_test_issue();
        let result = TrialRunResult::new("test/repo", &issue).failed("Timeout");

        assert!(!result.success);
        assert_eq!(result.error, Some("Timeout".to_string()));
    }

    #[test]
    fn test_trial_result_with_changes() {
        let issue = create_test_issue();
        let result = TrialRunResult::new("test/repo", &issue)
            .add_file_change(TrialFileChange {
                path: "test.rs".to_string(),
                change_type: FileChangeType::Modified,
                diff: "+line".to_string(),
                lines_added: 5,
                lines_removed: 2,
            })
            .success();

        assert_eq!(result.file_changes.len(), 1);
        assert_eq!(result.total_lines_changed(), 7);
    }

    #[test]
    fn test_trial_run_execute() {
        let trial = TrialRun::default_config();
        let repo = create_test_repo();
        let issue = create_test_issue();

        let result = trial.execute(&repo, &issue);

        assert!(result.success);
        assert!(!result.file_changes.is_empty());
        assert!(result.tokens_in > 0);
    }

    #[test]
    fn test_within_limits() {
        let trial = TrialRun::default_config();

        assert!(trial.within_limits(5, 2, 100));
        assert!(!trial.within_limits(20, 2, 100)); // Too many tool calls
        assert!(!trial.within_limits(5, 10, 100)); // Too many files
        assert!(!trial.within_limits(5, 2, 400)); // Too long
    }

    #[test]
    fn test_result_summary() {
        let issue = create_test_issue();
        let result = TrialRunResult::new("test/repo", &issue)
            .with_pr("Fix tests", "Added tests")
            .with_usage(1000, 3, 2000, 1000)
            .success();

        let summary = result.summary();
        assert!(summary.contains("test/repo"));
        assert!(summary.contains("Success"));
        assert!(summary.contains("Upgrade"));
    }

    #[test]
    fn test_estimate_cost() {
        // 1000 tokens in = $0.003, 1000 tokens out = $0.015
        let cost = TrialRunResult::estimate_cost(1000, 1000);
        assert!((cost - 0.018).abs() < 0.001);
    }
}
