//! Automated PR review checks for GitAfter
//!
//! This module provides automated validation for:
//! - Code compilation
//! - Tests passing
//! - Trajectory hash verification
//! - Diff-to-trajectory comparison
//! - Review approval status
//! - Dependency order for stacked PRs

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Status of an automated check
///
/// Represents the current state of a check in the review process.
///
/// # Examples
///
/// ```
/// use gitafter::review::CheckStatus;
///
/// let status = CheckStatus::Pass;
/// assert_eq!(status.to_string(), "✓ PASS");
///
/// let failed = CheckStatus::Fail;
/// assert_eq!(failed.to_string(), "✗ FAIL");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    /// Check passed
    Pass,
    /// Check failed
    Fail,
    /// Check skipped or not applicable
    Skip,
    /// Check is running
    Running,
    /// Check not yet run
    Pending,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckStatus::Pass => write!(f, "✓ PASS"),
            CheckStatus::Fail => write!(f, "✗ FAIL"),
            CheckStatus::Skip => write!(f, "○ SKIP"),
            CheckStatus::Running => write!(f, "⋯ RUNNING"),
            CheckStatus::Pending => write!(f, "- PENDING"),
        }
    }
}

/// Result of an automated check
///
/// Captures the outcome of running an automated check, including
/// status, optional message, and execution time.
///
/// # Examples
///
/// ```
/// use gitafter::review::{CheckResult, CheckStatus};
///
/// // Successful check with timing info
/// let result = CheckResult::new("compilation", "Code compiles", CheckStatus::Pass)
///     .with_message("All targets compiled successfully")
///     .with_duration(1234);
///
/// assert_eq!(result.id, "compilation");
/// assert_eq!(result.status, CheckStatus::Pass);
/// assert_eq!(result.message, Some("All targets compiled successfully".to_string()));
/// assert_eq!(result.duration_ms, Some(1234));
///
/// // Failed check with error details
/// let failed = CheckResult::new("tests", "Tests pass", CheckStatus::Fail)
///     .with_message("3 tests failed");
///
/// assert_eq!(failed.status, CheckStatus::Fail);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Check identifier
    pub id: String,
    /// Check name
    pub name: String,
    /// Status
    pub status: CheckStatus,
    /// Details or error message
    pub message: Option<String>,
    /// Execution time in milliseconds
    pub duration_ms: Option<u64>,
}

impl CheckResult {
    /// Create a new check result
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this check (e.g., "compilation", "tests")
    /// * `name` - Human-readable name for display
    /// * `status` - The check status (Pass, Fail, Skip, Running, Pending)
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::{CheckResult, CheckStatus};
    ///
    /// let result = CheckResult::new("clippy", "Clippy lints", CheckStatus::Pass);
    /// assert_eq!(result.id, "clippy");
    /// assert_eq!(result.name, "Clippy lints");
    /// assert_eq!(result.status, CheckStatus::Pass);
    /// ```
    pub fn new(id: impl Into<String>, name: impl Into<String>, status: CheckStatus) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            status,
            message: None,
            duration_ms: None,
        }
    }

    /// Set message
    ///
    /// Adds details or an error message to the check result.
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::{CheckResult, CheckStatus};
    ///
    /// let result = CheckResult::new("tests", "Tests", CheckStatus::Fail)
    ///     .with_message("2 tests failed: test_auth, test_parser");
    ///
    /// assert_eq!(result.message.unwrap(), "2 tests failed: test_auth, test_parser");
    /// ```
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set duration
    ///
    /// Records how long the check took to run, in milliseconds.
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::{CheckResult, CheckStatus};
    ///
    /// let result = CheckResult::new("build", "Build", CheckStatus::Pass)
    ///     .with_duration(5432);
    ///
    /// assert_eq!(result.duration_ms, Some(5432));
    /// ```
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

/// Automated check runner for PR reviews
///
/// Runs a comprehensive suite of automated checks on a pull request,
/// including compilation, tests, trajectory verification, and dependency
/// validation for stacked PRs.
///
/// # Examples
///
/// ```no_run
/// use gitafter::review::AutoCheckRunner;
///
/// # async fn example() {
/// // Basic checks for a PR
/// let runner = AutoCheckRunner::new("/path/to/repo", "pr-123");
/// let results = runner.run_all().await;
///
/// for result in results {
///     println!("{}: {}", result.name, result.status);
/// }
///
/// // With trajectory verification
/// let runner_with_trajectory = AutoCheckRunner::new("/path/to/repo", "pr-456")
///     .with_trajectory("session-abc123");
/// let results = runner_with_trajectory.run_all().await;
///
/// // For stacked PR with dependencies
/// let stacked_runner = AutoCheckRunner::new("/path/to/repo", "pr-789")
///     .with_dependencies(vec!["pr-123".to_string(), "pr-456".to_string()]);
/// let results = stacked_runner.run_all().await;
/// # }
/// ```
pub struct AutoCheckRunner {
    /// Path to the repository
    repo_path: PathBuf,
    /// Trajectory session ID (if available)
    trajectory_session_id: Option<String>,
    /// Stack dependencies (PR IDs this PR depends on)
    depends_on: Vec<String>,
    /// Nostr client for querying PR status (optional)
    nostr_client: Option<std::sync::Arc<crate::nostr::NostrClient>>,
}

impl AutoCheckRunner {
    /// Create a new auto-check runner
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the cloned repository
    /// * `pr_id` - Pull request identifier (for logging/tracking)
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::AutoCheckRunner;
    ///
    /// let runner = AutoCheckRunner::new("/tmp/my-repo", "pr-123");
    /// ```
    pub fn new(repo_path: impl AsRef<Path>, _pr_id: impl Into<String>) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
            trajectory_session_id: None,
            depends_on: Vec::new(),
            nostr_client: None,
        }
    }

    /// Set trajectory session ID
    ///
    /// Enables trajectory hash verification and diff-to-trajectory comparison
    /// checks when a trajectory session is provided.
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::AutoCheckRunner;
    ///
    /// let runner = AutoCheckRunner::new("/tmp/repo", "pr-123")
    ///     .with_trajectory("session-abc123");
    /// ```
    pub fn with_trajectory(mut self, session_id: impl Into<String>) -> Self {
        self.trajectory_session_id = Some(session_id.into());
        self
    }

    /// Set stack dependencies
    ///
    /// For stacked PRs, specify which PR IDs this PR depends on.
    /// The dependency order check will ensure all dependencies are
    /// merged before this PR can be merged.
    ///
    /// # Arguments
    ///
    /// * `depends_on` - Vector of PR IDs that must be merged first
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::AutoCheckRunner;
    ///
    /// // This PR depends on pr-100 and pr-101 being merged first
    /// let runner = AutoCheckRunner::new("/tmp/repo", "pr-102")
    ///     .with_dependencies(vec!["pr-100".to_string(), "pr-101".to_string()]);
    /// ```
    pub fn with_dependencies(mut self, depends_on: Vec<String>) -> Self {
        self.depends_on = depends_on;
        self
    }

    /// Set Nostr client for PR status queries
    ///
    /// Enables querying dependency PR status from Nostr relays.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use gitafter::review::AutoCheckRunner;
    /// use gitafter::nostr::NostrClient;
    /// use std::sync::Arc;
    ///
    /// let client = Arc::new(NostrClient::new(vec![], broadcaster)?);
    /// let runner = AutoCheckRunner::new("/tmp/repo", "pr-123")
    ///     .with_nostr_client(client);
    /// ```
    pub fn with_nostr_client(mut self, client: std::sync::Arc<crate::nostr::NostrClient>) -> Self {
        self.nostr_client = Some(client);
        self
    }

    /// Run all automated checks
    ///
    /// Executes the full suite of automated checks:
    /// - Code compilation (`cargo check`)
    /// - Tests passing (`cargo test`)
    /// - Trajectory hash verification (if trajectory provided)
    /// - Diff-to-trajectory comparison (if trajectory provided)
    /// - Stack dependency order (if dependencies provided)
    /// - Review approval status
    ///
    /// # Returns
    ///
    /// A vector of check results, one for each check that was run.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gitafter::review::{AutoCheckRunner, CheckStatus};
    ///
    /// # async fn example() {
    /// let runner = AutoCheckRunner::new("/tmp/my-repo", "pr-123");
    /// let results = runner.run_all().await;
    ///
    /// // Check if all passed
    /// let all_passed = results.iter().all(|r| r.status == CheckStatus::Pass);
    ///
    /// // Find failed checks
    /// let failures: Vec<_> = results.iter()
    ///     .filter(|r| r.status == CheckStatus::Fail)
    ///     .collect();
    ///
    /// if !failures.is_empty() {
    ///     for failure in failures {
    ///         eprintln!("FAILED: {} - {:?}", failure.name, failure.message);
    ///     }
    /// }
    /// # }
    /// ```
    pub async fn run_all(&self) -> Vec<CheckResult> {
        let mut results = Vec::new();

        // Code compilation check
        results.push(self.check_compilation().await);

        // Tests passing check
        results.push(self.check_tests().await);

        // Trajectory hash verification (if trajectory available)
        if self.trajectory_session_id.is_some() {
            results.push(self.check_trajectory_hash().await);
            results.push(self.check_diff_matches_trajectory().await);
        }

        // Stack dependency order check
        if !self.depends_on.is_empty() {
            results.push(self.check_dependency_order().await);
        }

        // Review approval status check
        results.push(self.check_review_approvals().await);

        results
    }

    /// Check if code compiles without errors
    async fn check_compilation(&self) -> CheckResult {
        let start = std::time::Instant::now();

        let output = Command::new("cargo")
            .arg("check")
            .arg("--all-targets")
            .current_dir(&self.repo_path)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match output {
            Ok(output) if output.status.success() => CheckResult::new(
                "compilation",
                "Code compiles without errors",
                CheckStatus::Pass,
            )
            .with_duration(duration_ms)
            .with_message("All targets compile successfully"),

            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let error_count = stderr.matches("error:").count();
                CheckResult::new(
                    "compilation",
                    "Code compiles without errors",
                    CheckStatus::Fail,
                )
                .with_duration(duration_ms)
                .with_message(format!("{} compilation error(s) found", error_count))
            }

            Err(e) => CheckResult::new(
                "compilation",
                "Code compiles without errors",
                CheckStatus::Fail,
            )
            .with_message(format!("Failed to run cargo check: {}", e)),
        }
    }

    /// Check if tests pass
    async fn check_tests(&self) -> CheckResult {
        let start = std::time::Instant::now();

        let output = Command::new("cargo")
            .arg("test")
            .arg("--all-targets")
            .current_dir(&self.repo_path)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let test_count = stdout.matches("test result: ok").count();
                CheckResult::new("tests", "Tests pass", CheckStatus::Pass)
                    .with_duration(duration_ms)
                    .with_message(format!("{} test suite(s) passed", test_count))
            }

            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let failed = stdout
                    .lines()
                    .find(|line| line.contains("test result:"))
                    .unwrap_or("Tests failed");
                CheckResult::new("tests", "Tests pass", CheckStatus::Fail)
                    .with_duration(duration_ms)
                    .with_message(failed.to_string())
            }

            Err(e) => CheckResult::new("tests", "Tests pass", CheckStatus::Fail)
                .with_message(format!("Failed to run cargo test: {}", e)),
        }
    }

    /// Check trajectory hash verification
    async fn check_trajectory_hash(&self) -> CheckResult {
        // This would integrate with the trajectory verifier module
        // For now, return a placeholder
        CheckResult::new(
            "trajectory-hash",
            "Trajectory hash matches claimed events",
            CheckStatus::Skip,
        )
        .with_message("Trajectory verification requires nostr client integration")
    }

    /// Check if diff matches trajectory tool calls
    async fn check_diff_matches_trajectory(&self) -> CheckResult {
        // This would use the trajectory/diff comparison module
        // For now, return a placeholder
        CheckResult::new(
            "diff-trajectory",
            "Diff matches trajectory tool calls",
            CheckStatus::Skip,
        )
        .with_message("Diff-to-trajectory comparison requires full trajectory data")
    }

    /// Check dependency order for stacked PRs
    async fn check_dependency_order(&self) -> CheckResult {
        if self.depends_on.is_empty() {
            return CheckResult::new(
                "dependency-order",
                "Required dependency PRs are merged",
                CheckStatus::Skip,
            )
            .with_message("No dependencies to check");
        }

        // If no Nostr client available, return pending
        let Some(client) = &self.nostr_client else {
            return CheckResult::new(
                "dependency-order",
                "Required dependency PRs are merged",
                CheckStatus::Pending,
            )
            .with_message("Nostr client not configured for dependency checks");
        };

        // Query each dependency PR's status
        let mut all_merged = true;
        let mut unmerged_prs = Vec::new();

        for pr_id in &self.depends_on {
            match client.get_pr_status(pr_id).await {
                Ok(kind) => {
                    // Convert kind to status string
                    let status = match kind {
                        1630 => "open",
                        1631 => "merged",
                        1632 => "closed",
                        1633 => "draft",
                        _ => "unknown",
                    };

                    if kind == 1631 {
                        // PR is merged (STATUS_APPLIED), continue
                    } else {
                        all_merged = false;
                        unmerged_prs.push(format!("{} ({})", pr_id, status));
                    }
                }
                Err(e) => {
                    return CheckResult::new(
                        "dependency-order",
                        "Required dependency PRs are merged",
                        CheckStatus::Fail,
                    )
                    .with_message(format!("Failed to query PR {}: {}", pr_id, e));
                }
            }
        }

        if all_merged {
            CheckResult::new(
                "dependency-order",
                "Required dependency PRs are merged",
                CheckStatus::Pass,
            )
            .with_message(format!(
                "All {} dependencies are merged",
                self.depends_on.len()
            ))
        } else {
            CheckResult::new(
                "dependency-order",
                "Required dependency PRs are merged",
                CheckStatus::Fail,
            )
            .with_message(format!(
                "Unmerged dependencies: {}",
                unmerged_prs.join(", ")
            ))
        }
    }

    /// Check review approval status
    async fn check_review_approvals(&self) -> CheckResult {
        // This would query nostr for review events
        // For now, return a placeholder
        CheckResult::new(
            "review-approvals",
            "Required review approvals present",
            CheckStatus::Pending,
        )
        .with_message("Review approval check requires nostr integration")
    }
}

/// CI integration indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiStatus {
    /// CI provider name (e.g., "GitHub Actions", "GitLab CI")
    pub provider: String,
    /// Status URL
    pub url: Option<String>,
    /// Overall status
    pub status: CheckStatus,
    /// Individual check results
    pub checks: Vec<CheckResult>,
}

impl CiStatus {
    /// Create a new CI status
    pub fn new(provider: impl Into<String>, status: CheckStatus) -> Self {
        Self {
            provider: provider.into(),
            url: None,
            status,
            checks: Vec::new(),
        }
    }

    /// Set status URL
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Add a check result
    pub fn add_check(mut self, check: CheckResult) -> Self {
        self.checks.push(check);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_status_display() {
        assert_eq!(CheckStatus::Pass.to_string(), "✓ PASS");
        assert_eq!(CheckStatus::Fail.to_string(), "✗ FAIL");
        assert_eq!(CheckStatus::Skip.to_string(), "○ SKIP");
        assert_eq!(CheckStatus::Running.to_string(), "⋯ RUNNING");
        assert_eq!(CheckStatus::Pending.to_string(), "- PENDING");
    }

    #[test]
    fn test_check_result_builder() {
        let result = CheckResult::new("test", "Test check", CheckStatus::Pass)
            .with_message("Everything looks good")
            .with_duration(1234);

        assert_eq!(result.id, "test");
        assert_eq!(result.name, "Test check");
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(result.message, Some("Everything looks good".to_string()));
        assert_eq!(result.duration_ms, Some(1234));
    }

    #[test]
    fn test_auto_check_runner_builder() {
        let runner = AutoCheckRunner::new("/tmp/repo", "pr-123")
            .with_trajectory("session-456")
            .with_dependencies(vec!["pr-100".to_string(), "pr-101".to_string()]);

        assert_eq!(runner.repo_path, PathBuf::from("/tmp/repo"));
        assert_eq!(
            runner.trajectory_session_id,
            Some("session-456".to_string())
        );
        assert_eq!(runner.depends_on.len(), 2);
    }

    #[test]
    fn test_ci_status_builder() {
        let ci = CiStatus::new("GitHub Actions", CheckStatus::Pass)
            .with_url("https://github.com/actions/123")
            .add_check(CheckResult::new("lint", "Linting", CheckStatus::Pass))
            .add_check(CheckResult::new("build", "Build", CheckStatus::Pass));

        assert_eq!(ci.provider, "GitHub Actions");
        assert_eq!(ci.url, Some("https://github.com/actions/123".to_string()));
        assert_eq!(ci.status, CheckStatus::Pass);
        assert_eq!(ci.checks.len(), 2);
    }
}
