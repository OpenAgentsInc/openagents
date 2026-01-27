use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

/// CI system types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CISystem {
    GitHubActions,
    CircleCI,
    Travis,
    Jenkins,
    Unknown,
}

/// CI status for a commit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CIStatus {
    Pending,
    Success,
    Failure,
    Error,
    Cancelled,
}

impl std::fmt::Display for CIStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CIStatus::Pending => write!(f, "pending"),
            CIStatus::Success => write!(f, "success"),
            CIStatus::Failure => write!(f, "failure"),
            CIStatus::Error => write!(f, "error"),
            CIStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// CI check result
#[derive(Debug, Clone)]
pub struct CICheckResult {
    pub status: CIStatus,
    pub checks_passed: usize,
    pub checks_total: usize,
    pub details: Vec<CheckDetail>,
}

#[derive(Debug, Clone)]
pub struct CheckDetail {
    pub name: String,
    pub status: CIStatus,
    pub conclusion: Option<String>,
    pub details_url: Option<String>,
}

/// CI integration client
pub struct CIClient {
    octocrab: Octocrab,
}

impl CIClient {
    /// Create a new CI client
    pub fn new(token: &str) -> Result<Self> {
        let octocrab = octocrab::OctocrabBuilder::new()
            .personal_token(token.to_string())
            .build()
            .context("Failed to build GitHub client for CI")?;

        Ok(Self { octocrab })
    }

    /// Detect CI system used by repository
    pub async fn detect_ci_system(&self, owner: &str, repo: &str) -> Result<CISystem> {
        debug!("Detecting CI system for {}/{}", owner, repo);

        let has_gh_actions = self
            .octocrab
            .repos(owner, repo)
            .get_content()
            .path(".github/workflows")
            .send()
            .await
            .is_ok();

        if has_gh_actions {
            info!("Detected GitHub Actions in {}/{}", owner, repo);
            return Ok(CISystem::GitHubActions);
        }

        let has_circle = self
            .octocrab
            .repos(owner, repo)
            .get_content()
            .path(".circleci/config.yml")
            .send()
            .await
            .is_ok();

        if has_circle {
            info!("Detected CircleCI in {}/{}", owner, repo);
            return Ok(CISystem::CircleCI);
        }

        let has_travis = self
            .octocrab
            .repos(owner, repo)
            .get_content()
            .path(".travis.yml")
            .send()
            .await
            .is_ok();

        if has_travis {
            info!("Detected Travis CI in {}/{}", owner, repo);
            return Ok(CISystem::Travis);
        }

        warn!("No CI system detected for {}/{}", owner, repo);
        Ok(CISystem::Unknown)
    }

    /// Check CI status for a commit
    pub async fn check_commit_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> Result<CICheckResult> {
        debug!(
            "Checking CI status for commit {} in {}/{}",
            sha, owner, repo
        );

        use octocrab::params::repos::Commitish;

        let commitish = Commitish::from(sha.to_string());
        let check_runs = self
            .octocrab
            .checks(owner, repo)
            .list_check_runs_for_git_ref(commitish)
            .send()
            .await
            .context("Failed to list check runs")?;

        let mut checks_passed = 0;
        let mut checks_total = 0;
        let mut details = Vec::new();
        let mut overall_status = CIStatus::Success;

        for run in check_runs.check_runs {
            checks_total += 1;

            let run_conclusion = run.conclusion.as_deref();
            let status = match run_conclusion {
                Some("success") | Some("neutral") => {
                    checks_passed += 1;
                    CIStatus::Success
                }
                Some("failure") => {
                    overall_status = CIStatus::Failure;
                    CIStatus::Failure
                }
                Some("cancelled") => CIStatus::Cancelled,
                Some("skipped") => {
                    checks_passed += 1;
                    CIStatus::Success
                }
                None => {
                    overall_status = CIStatus::Pending;
                    CIStatus::Pending
                }
                _ => {
                    overall_status = CIStatus::Error;
                    CIStatus::Error
                }
            };

            details.push(CheckDetail {
                name: run.name,
                status: status.clone(),
                conclusion: run.conclusion.as_ref().map(|c| c.as_str().to_string()),
                details_url: run.details_url,
            });
        }

        if checks_total == 0 {
            overall_status = CIStatus::Pending;
        }

        Ok(CICheckResult {
            status: overall_status,
            checks_passed,
            checks_total,
            details,
        })
    }

    /// Poll CI status until completion or timeout
    pub async fn poll_until_complete(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<CICheckResult> {
        info!("Polling CI status for {} in {}/{}", sha, owner, repo);

        let start = std::time::Instant::now();

        loop {
            let result = self.check_commit_status(owner, repo, sha).await?;

            match result.status {
                CIStatus::Success | CIStatus::Failure | CIStatus::Error | CIStatus::Cancelled => {
                    info!(
                        "CI completed with status: {} ({}/{} checks passed)",
                        result.status, result.checks_passed, result.checks_total
                    );
                    return Ok(result);
                }
                CIStatus::Pending => {
                    if start.elapsed() > timeout {
                        warn!("CI polling timed out after {:?}", timeout);
                        return Ok(result);
                    }

                    debug!(
                        "CI still pending ({}/{} checks), waiting {}s...",
                        result.checks_passed,
                        result.checks_total,
                        poll_interval.as_secs()
                    );

                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    /// Get CI status for a pull request
    pub async fn check_pr_status(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<CICheckResult> {
        debug!(
            "Checking CI status for PR #{} in {}/{}",
            pr_number, owner, repo
        );

        let pr = self
            .octocrab
            .pulls(owner, repo)
            .get(pr_number)
            .await
            .context("Failed to get pull request")?;

        let head_sha = pr.head.sha;

        self.check_commit_status(owner, repo, &head_sha).await
    }
}

/// Detect local test command for a repository
pub fn detect_test_command(repo_path: &std::path::Path) -> Option<String> {
    if repo_path.join("Cargo.toml").exists() {
        return Some("cargo test".to_string());
    }

    if repo_path.join("package.json").exists() {
        if repo_path.join("yarn.lock").exists() {
            return Some("yarn test".to_string());
        }
        return Some("npm test".to_string());
    }

    if repo_path.join("go.mod").exists() {
        return Some("go test ./...".to_string());
    }

    if repo_path.join("pytest.ini").exists() || repo_path.join("setup.py").exists() {
        return Some("pytest".to_string());
    }

    if repo_path.join("Makefile").exists() {
        let makefile = std::fs::read_to_string(repo_path.join("Makefile")).ok()?;
        if makefile.contains("test:") {
            return Some("make test".to_string());
        }
    }

    None
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_ci_status_display() {
        assert_eq!(CIStatus::Success.to_string(), "success");
        assert_eq!(CIStatus::Failure.to_string(), "failure");
        assert_eq!(CIStatus::Pending.to_string(), "pending");
    }

    #[test]
    fn test_detect_test_command_cargo() {
        let temp = std::env::temp_dir();
        let test_path = temp.join("test_cargo");
        std::fs::create_dir_all(&test_path).unwrap();
        std::fs::write(test_path.join("Cargo.toml"), "").unwrap();

        let cmd = detect_test_command(&test_path);
        assert_eq!(cmd, Some("cargo test".to_string()));

        std::fs::remove_dir_all(&test_path).ok();
    }
}
