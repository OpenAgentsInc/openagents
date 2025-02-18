mod analyzer;
mod conversions;

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct GitHubService {
    client: Client,
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub number: i32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitHubUser {
    pub login: String,
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubComment {
    pub id: i64,
    pub body: String,
    pub user: GitHubUser,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
struct CommentPayload {
    body: String,
}

#[derive(Debug, Serialize)]
struct BranchPayload {
    #[serde(rename = "ref")]
    ref_name: String,
    sha: String,
}

#[derive(Debug, Serialize)]
struct PullRequestPayload {
    title: String,
    body: String,
    head: String,
    base: String,
}

pub use crate::server::services::openrouter::GitHubIssueAnalysis;
pub use analyzer::GitHubIssueAnalyzer;

impl GitHubService {
    pub fn new(token: Option<String>) -> Result<Self> {
        let token = token.ok_or_else(|| anyhow!("GitHub token is required"))?;
        Ok(Self {
            client: Client::new(),
            token,
        })
    }

    pub async fn get_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
    ) -> Result<GitHubIssue> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}",
            owner, repo, issue_number
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("GitHub API request failed: {}", response.status()));
        }

        let issue = response.json::<GitHubIssue>().await?;
        Ok(issue)
    }

    pub async fn get_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
    ) -> Result<Vec<GitHubComment>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}/comments",
            owner, repo, issue_number
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get issue comments: {}",
                response.status()
            ));
        }

        let comments = response.json::<Vec<GitHubComment>>().await?;
        Ok(comments)
    }

    pub async fn post_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
        comment: &str,
    ) -> Result<()> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}/comments",
            owner, repo, issue_number
        );

        let payload = CommentPayload {
            body: comment.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .header("Accept", "application/vnd.github.v3+json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await?;
            return Err(anyhow!(
                "Failed to post GitHub comment: {} - {}",
                status,
                error_body
            ));
        }

        Ok(())
    }

    pub async fn check_branch_exists(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
    ) -> Result<bool> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/git/ref/heads/{}",
            owner, repo, branch_name
        );

        debug!("Checking if branch exists: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    pub async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
        base_branch: &str,
    ) -> Result<()> {
        debug!("Creating branch '{}' from '{}'", branch_name, base_branch);

        // Check if branch already exists
        if self.check_branch_exists(owner, repo, branch_name).await? {
            warn!("Branch '{}' already exists", branch_name);
            return Ok(());
        }

        // First get the SHA of the base branch
        let url = format!(
            "https://api.github.com/repos/{}/{}/git/ref/heads/{}",
            owner, repo, base_branch
        );

        debug!("Getting base branch SHA from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await?;
            return Err(anyhow!(
                "Failed to get base branch ref: {} - {}",
                status,
                error_body
            ));
        }

        let base_ref = response.json::<serde_json::Value>().await?;
        let sha = base_ref["object"]["sha"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid base branch ref response"))?;

        debug!("Got base branch SHA: {}", sha);

        // Create the new branch
        let url = format!("https://api.github.com/repos/{}/{}/git/refs", owner, repo);

        let payload = BranchPayload {
            ref_name: format!("refs/heads/{}", branch_name),
            sha: sha.to_string(),
        };

        debug!("Creating branch with payload: {:?}", payload);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .header("Accept", "application/vnd.github.v3+json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await?;
            return Err(anyhow!(
                "Failed to create branch: {} - {}",
                status,
                error_body
            ));
        }

        debug!("Successfully created branch '{}'", branch_name);
        Ok(())
    }

    pub async fn check_branch_has_commits(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
    ) -> Result<bool> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/commits?sha={}",
            owner, repo, branch_name
        );

        debug!("Checking commits on branch: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await?;
            warn!(
                "Failed to check branch commits: {} - {}",
                status, error_body
            );
            return Ok(false);
        }

        let commits: Vec<serde_json::Value> = response.json().await?;
        debug!(
            "Found {} commits on branch '{}'",
            commits.len(),
            branch_name
        );

        Ok(!commits.is_empty())
    }

    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        head: &str,
        base: &str,
        title: &str,
        description: &str,
    ) -> Result<()> {
        debug!("Creating PR: {} -> {}", head, base);

        // First check if branch exists and has commits
        if !self.check_branch_exists(owner, repo, head).await? {
            return Err(anyhow!("Head branch '{}' does not exist", head));
        }

        if !self.check_branch_has_commits(owner, repo, head).await? {
            return Err(anyhow!("Head branch '{}' has no commits", head));
        }

        let url = format!("https://api.github.com/repos/{}/{}/pulls", owner, repo);

        let payload = PullRequestPayload {
            title: title.to_string(),
            body: description.to_string(),
            head: head.to_string(),
            base: base.to_string(),
        };

        debug!("Creating PR with payload: {:?}", payload);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .header("Accept", "application/vnd.github.v3+json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await?;
            return Err(anyhow!(
                "Failed to create pull request: {} - {}",
                status,
                error_body
            ));
        }

        debug!("Successfully created PR");
        Ok(())
    }

    pub async fn list_issues(&self, owner: &str, repo: &str) -> Result<Vec<GitHubIssue>> {
        let url = format!("https://api.github.com/repos/{}/{}/issues", owner, repo);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "OpenAgents")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("GitHub API request failed: {}", response.status()));
        }

        let issues = response.json::<Vec<GitHubIssue>>().await?;
        Ok(issues)
    }
}

pub async fn post_github_comment(
    issue_number: i32,
    comment: &str,
    owner: &str,
    repo: &str,
    token: &str,
) -> Result<()> {
    let service = GitHubService::new(Some(token.to_string()))?;
    service
        .post_comment(owner, repo, issue_number, comment)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::services::openrouter::{OpenRouterConfig, OpenRouterService};

    #[tokio::test]
    #[ignore = "Requires OPENROUTER_API_KEY in environment"]
    async fn test_analyze_issue() {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        let api_key = match std::env::var("OPENROUTER_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                println!("Skipping test: OPENROUTER_API_KEY not set in environment");
                return;
            }
        };

        let config = OpenRouterConfig {
            test_mode: true,
            ..Default::default()
        };
        let openrouter = OpenRouterService::with_config(api_key, config);
        let mut analyzer = GitHubIssueAnalyzer::new(openrouter);

        let test_issue = r#"
            Title: Add dark mode support

            We need to add dark mode support to improve user experience during nighttime usage.
            This should include:
            - A toggle switch in the settings
            - Dark color palette
            - Persistent preference storage
            - Automatic switching based on system preferences
        "#;

        let analysis = analyzer.analyze_issue(test_issue).await.unwrap();
        assert!(!analysis.files.is_empty());
        assert!(analysis.files.iter().all(|f| !f.filepath.is_empty()));
        assert!(analysis.files.iter().all(|f| !f.comment.is_empty()));
        assert!(analysis
            .files
            .iter()
            .all(|f| f.priority >= 1 && f.priority <= 10));
    }
}
