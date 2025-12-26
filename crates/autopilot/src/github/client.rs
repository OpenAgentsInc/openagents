//! GitHub API client
//!
//! Wraps the octocrab library for GitHub API operations.

use anyhow::{Context, Result};
use octocrab::Octocrab;
use std::collections::HashMap;

use super::models::*;

/// GitHub API client for repository operations
pub struct GitHubClient {
    octocrab: Octocrab,
    owner: String,
    repo: String,
}

impl GitHubClient {
    /// Create a new GitHub client for a specific repository
    ///
    /// # Arguments
    /// * `token` - GitHub access token
    /// * `owner` - Repository owner (user or organization)
    /// * `repo` - Repository name
    pub fn new(token: &str, owner: &str, repo: &str) -> Result<Self> {
        let octocrab = Octocrab::builder()
            .personal_token(token.to_string())
            .build()
            .context("Failed to build GitHub client")?;

        Ok(Self {
            octocrab,
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }

    /// Create client from a full repository name (owner/repo)
    pub fn from_full_name(token: &str, full_name: &str) -> Result<Self> {
        let parts: Vec<&str> = full_name.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid repository name: {}", full_name);
        }
        Self::new(token, parts[0], parts[1])
    }

    /// Parse repository URL into owner and repo
    ///
    /// Supports:
    /// - https://github.com/owner/repo
    /// - https://github.com/owner/repo.git
    /// - git@github.com:owner/repo.git
    /// - owner/repo
    pub fn parse_repo_url(url: &str) -> Result<(String, String)> {
        let url = url.trim();

        // Handle SSH URL
        if url.starts_with("git@github.com:") {
            let path = url.strip_prefix("git@github.com:").unwrap();
            let path = path.strip_suffix(".git").unwrap_or(path);
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() == 2 {
                return Ok((parts[0].to_string(), parts[1].to_string()));
            }
        }

        // Handle HTTPS URL
        if url.starts_with("https://github.com/") {
            let path = url.strip_prefix("https://github.com/").unwrap();
            let path = path.strip_suffix(".git").unwrap_or(path);
            let path = path.trim_end_matches('/');
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() >= 2 {
                return Ok((parts[0].to_string(), parts[1].to_string()));
            }
        }

        // Handle simple owner/repo format
        if !url.contains("://") && !url.contains('@') {
            let parts: Vec<&str> = url.split('/').collect();
            if parts.len() == 2 {
                return Ok((parts[0].to_string(), parts[1].to_string()));
            }
        }

        anyhow::bail!("Invalid repository URL: {}", url)
    }

    // ========== Repository Operations ==========

    /// Get repository information
    pub async fn get_repo_info(&self) -> Result<Repository> {
        let repo = self
            .octocrab
            .repos(&self.owner, &self.repo)
            .get()
            .await
            .context("Failed to get repository info")?;

        Ok(Repository {
            id: repo.id.0,
            name: repo.name,
            full_name: repo.full_name.unwrap_or_default(),
            owner: RepositoryOwner {
                login: repo.owner.as_ref().map(|o| o.login.clone()).unwrap_or_default(),
                id: repo.owner.as_ref().map(|o| o.id.0).unwrap_or(0),
                owner_type: repo
                    .owner
                    .as_ref()
                    .map(|o| o.r#type.clone())
                    .unwrap_or_default(),
            },
            description: repo.description,
            default_branch: repo.default_branch.unwrap_or_else(|| "main".to_string()),
            clone_url: repo.clone_url.map(|u| u.to_string()).unwrap_or_default(),
            ssh_url: repo.ssh_url.unwrap_or_default(),
            private: repo.private.unwrap_or(false),
            language: repo.language.and_then(|v| v.as_str().map(|s| s.to_string())),
        })
    }

    /// Detect programming languages in the repository
    pub async fn detect_languages(&self) -> Result<Vec<Language>> {
        let languages: HashMap<String, i64> = self
            .octocrab
            .repos(&self.owner, &self.repo)
            .list_languages()
            .await
            .context("Failed to get repository languages")?;

        let total: i64 = languages.values().sum();
        let mut result: Vec<Language> = languages
            .into_iter()
            .map(|(name, bytes)| {
                let percentage = if total > 0 {
                    (bytes as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                Language {
                    name,
                    bytes,
                    percentage,
                }
            })
            .collect();

        // Sort by bytes descending
        result.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        Ok(result)
    }

    /// Get the default branch name
    pub async fn get_default_branch(&self) -> Result<String> {
        let repo = self.get_repo_info().await?;
        Ok(repo.default_branch)
    }

    // ========== Issue Operations ==========

    /// List issues with optional label filter
    pub async fn list_issues(&self, labels: &[&str]) -> Result<Vec<Issue>> {
        // Convert labels to owned strings for the API call
        let label_strings: Vec<String> = labels.iter().map(|s| s.to_string()).collect();

        let issues = if label_strings.is_empty() {
            self.octocrab
                .issues(&self.owner, &self.repo)
                .list()
                .state(octocrab::params::State::Open)
                .send()
                .await
                .context("Failed to list issues")?
        } else {
            self.octocrab
                .issues(&self.owner, &self.repo)
                .list()
                .state(octocrab::params::State::Open)
                .labels(&label_strings)
                .send()
                .await
                .context("Failed to list issues")?
        };

        Ok(issues
            .items
            .into_iter()
            .filter(|i| i.pull_request.is_none()) // Filter out PRs
            .map(|i| Issue {
                number: i.number,
                title: i.title,
                body: i.body,
                state: match i.state {
                    octocrab::models::IssueState::Open => IssueState::Open,
                    octocrab::models::IssueState::Closed => IssueState::Closed,
                    _ => IssueState::Open,
                },
                labels: i
                    .labels
                    .into_iter()
                    .map(|l| Label {
                        name: l.name,
                        color: l.color,
                        description: l.description,
                    })
                    .collect(),
                assignees: i
                    .assignees
                    .into_iter()
                    .map(|a| User {
                        login: a.login,
                        id: a.id.0,
                    })
                    .collect(),
                created_at: i.created_at,
                updated_at: i.updated_at,
            })
            .collect())
    }

    /// Get a specific issue by number
    pub async fn get_issue(&self, number: u64) -> Result<Issue> {
        let issue = self
            .octocrab
            .issues(&self.owner, &self.repo)
            .get(number)
            .await
            .context(format!("Failed to get issue #{}", number))?;

        Ok(Issue {
            number: issue.number,
            title: issue.title,
            body: issue.body,
            state: match issue.state {
                octocrab::models::IssueState::Open => IssueState::Open,
                octocrab::models::IssueState::Closed => IssueState::Closed,
                _ => IssueState::Open,
            },
            labels: issue
                .labels
                .into_iter()
                .map(|l| Label {
                    name: l.name,
                    color: l.color,
                    description: l.description,
                })
                .collect(),
            assignees: issue
                .assignees
                .into_iter()
                .map(|a| User {
                    login: a.login,
                    id: a.id.0,
                })
                .collect(),
            created_at: issue.created_at,
            updated_at: issue.updated_at,
        })
    }

    /// Post a comment on an issue
    pub async fn post_comment(&self, issue_number: u64, body: &str) -> Result<()> {
        self.octocrab
            .issues(&self.owner, &self.repo)
            .create_comment(issue_number, body)
            .await
            .context(format!("Failed to comment on issue #{}", issue_number))?;
        Ok(())
    }

    /// Add a label to an issue
    pub async fn add_label(&self, issue_number: u64, label: &str) -> Result<()> {
        self.octocrab
            .issues(&self.owner, &self.repo)
            .add_labels(issue_number, &[label.to_string()])
            .await
            .context(format!("Failed to add label to issue #{}", issue_number))?;
        Ok(())
    }

    /// Remove a label from an issue
    pub async fn remove_label(&self, issue_number: u64, label: &str) -> Result<()> {
        self.octocrab
            .issues(&self.owner, &self.repo)
            .remove_label(issue_number, label)
            .await
            .context(format!("Failed to remove label from issue #{}", issue_number))?;
        Ok(())
    }

    /// Claim an issue for Autopilot
    ///
    /// This posts a claiming comment and adds the in-progress label.
    pub async fn claim_issue(&self, issue_number: u64) -> Result<()> {
        // Post claiming comment
        self.post_comment(
            issue_number,
            "🤖 **Autopilot** is claiming this issue and will start working on it.\n\n\
            I'll create a branch and PR when the implementation is ready.",
        )
        .await?;

        // Add in-progress label
        self.add_label(issue_number, "autopilot-in-progress").await?;

        Ok(())
    }

    // ========== Pull Request Operations ==========

    /// Create a pull request
    pub async fn create_pr(&self, pr: CreatePullRequest) -> Result<PullRequest> {
        let created = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .create(&pr.title, &pr.head, &pr.base)
            .body(&pr.body)
            .draft(pr.draft)
            .send()
            .await
            .context("Failed to create pull request")?;

        // Determine state from the state field
        let state = created.state.map_or(PullRequestState::Open, |s| {
            match s {
                octocrab::models::IssueState::Open => PullRequestState::Open,
                octocrab::models::IssueState::Closed => PullRequestState::Closed,
                _ => PullRequestState::Open,
            }
        });

        Ok(PullRequest {
            number: created.number,
            title: created.title.unwrap_or_default(),
            body: created.body,
            state,
            head: PullRequestRef {
                ref_name: created.head.ref_field,
                sha: created.head.sha,
            },
            base: PullRequestRef {
                ref_name: created.base.ref_field,
                sha: created.base.sha,
            },
            html_url: created.html_url.map(|u| u.to_string()).unwrap_or_default(),
            mergeable: created.mergeable,
            merged: created.merged_at.is_some(), // Use merged_at to determine if merged
            created_at: created.created_at.unwrap_or_else(chrono::Utc::now),
            updated_at: created.updated_at.unwrap_or_else(chrono::Utc::now),
        })
    }

    /// Get PR status including CI checks
    pub async fn get_pr_status(&self, number: u64) -> Result<PRStatus> {
        let pr = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .get(number)
            .await
            .context(format!("Failed to get PR #{}", number))?;

        // Get combined status for the PR head commit
        // Note: We use the combined status API since check runs API requires different ref format
        let combined_status = self
            .octocrab
            .repos(&self.owner, &self.repo)
            .combined_status_for_ref(&octocrab::params::repos::Reference::Branch(
                pr.head.ref_field.clone(),
            ))
            .await
            .ok();

        // Build check runs from combined status if available
        let checks = combined_status.as_ref().map(|s| &s.statuses);

        let check_runs: Vec<CheckRun> = checks
            .map(|statuses| {
                statuses
                    .iter()
                    .map(|s| {
                        let state_str = format!("{:?}", s.state).to_lowercase();
                        CheckRun {
                            name: s.context.clone().unwrap_or_default(),
                            status: state_str.clone(),
                            conclusion: Some(state_str),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Determine overall check status from combined status
        let check_status = combined_status
            .as_ref()
            .map(|s| {
                let state_str = format!("{:?}", s.state).to_lowercase();
                match state_str.as_str() {
                    "success" => CheckStatus::Success,
                    "failure" => CheckStatus::Failure,
                    "error" => CheckStatus::Error,
                    _ => CheckStatus::Pending,
                }
            })
            .unwrap_or(CheckStatus::Pending);

        // Determine state from the state field
        let state = pr.state.map_or(PullRequestState::Open, |s| {
            match s {
                octocrab::models::IssueState::Open => PullRequestState::Open,
                octocrab::models::IssueState::Closed => PullRequestState::Closed,
                _ => PullRequestState::Open,
            }
        });

        Ok(PRStatus {
            pr: PullRequest {
                number: pr.number,
                title: pr.title.unwrap_or_default(),
                body: pr.body,
                state,
                head: PullRequestRef {
                    ref_name: pr.head.ref_field,
                    sha: pr.head.sha,
                },
                base: PullRequestRef {
                    ref_name: pr.base.ref_field,
                    sha: pr.base.sha,
                },
                html_url: pr.html_url.map(|u| u.to_string()).unwrap_or_default(),
                mergeable: pr.mergeable,
                merged: pr.merged_at.is_some(), // Use merged_at to determine if merged
                created_at: pr.created_at.unwrap_or_else(chrono::Utc::now),
                updated_at: pr.updated_at.unwrap_or_else(chrono::Utc::now),
            },
            check_status,
            checks: check_runs,
        })
    }

    /// Post a comment on a PR
    pub async fn post_pr_comment(&self, pr_number: u64, body: &str) -> Result<()> {
        // PRs use the issues API for comments
        self.post_comment(pr_number, body).await
    }

    // ========== Branch Operations ==========

    /// Create a new branch from a reference
    pub async fn create_branch(&self, branch_name: &str, from_ref: &str) -> Result<()> {
        // Get the SHA of the source reference
        let reference = self
            .octocrab
            .repos(&self.owner, &self.repo)
            .get_ref(&octocrab::params::repos::Reference::Branch(
                from_ref.to_string(),
            ))
            .await
            .context(format!("Failed to get ref for {}", from_ref))?;

        let sha = match reference.object {
            octocrab::models::repos::Object::Commit { sha, .. } => sha,
            octocrab::models::repos::Object::Tag { sha, .. } => sha,
            _ => anyhow::bail!("Unexpected reference type"),
        };

        // Create the new branch
        self.octocrab
            .repos(&self.owner, &self.repo)
            .create_ref(
                &octocrab::params::repos::Reference::Branch(branch_name.to_string()),
                sha,
            )
            .await
            .context(format!("Failed to create branch {}", branch_name))?;

        Ok(())
    }

    /// Check if a branch exists
    pub async fn branch_exists(&self, branch_name: &str) -> Result<bool> {
        match self
            .octocrab
            .repos(&self.owner, &self.repo)
            .get_ref(&octocrab::params::repos::Reference::Branch(
                branch_name.to_string(),
            ))
            .await
        {
            Ok(_) => Ok(true),
            Err(octocrab::Error::GitHub { source, .. }) if source.message.contains("Not Found") => {
                Ok(false)
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// Generate a branch name for an issue
pub fn issue_branch_name(issue_number: u64, title: &str) -> String {
    // Slugify the title
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .take(5) // Limit to first 5 words
        .collect::<Vec<_>>()
        .join("-");

    format!("autopilot/{}-{}", issue_number, slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo_url_https() {
        let (owner, repo) = GitHubClient::parse_repo_url("https://github.com/openagents/openagents").unwrap();
        assert_eq!(owner, "openagents");
        assert_eq!(repo, "openagents");
    }

    #[test]
    fn test_parse_repo_url_https_with_git() {
        let (owner, repo) =
            GitHubClient::parse_repo_url("https://github.com/openagents/openagents.git").unwrap();
        assert_eq!(owner, "openagents");
        assert_eq!(repo, "openagents");
    }

    #[test]
    fn test_parse_repo_url_ssh() {
        let (owner, repo) =
            GitHubClient::parse_repo_url("git@github.com:openagents/openagents.git").unwrap();
        assert_eq!(owner, "openagents");
        assert_eq!(repo, "openagents");
    }

    #[test]
    fn test_parse_repo_url_simple() {
        let (owner, repo) = GitHubClient::parse_repo_url("openagents/openagents").unwrap();
        assert_eq!(owner, "openagents");
        assert_eq!(repo, "openagents");
    }

    #[test]
    fn test_issue_branch_name() {
        let name = issue_branch_name(123, "Fix the login bug in auth.rs");
        assert_eq!(name, "autopilot/123-fix-the-login-bug-in");
    }
}
