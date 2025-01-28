use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

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

impl GitHubService {
    pub fn new(token: Option<String>) -> Result<Self> {
        let token = token.ok_or_else(|| anyhow::anyhow!("GitHub token is required"))?;
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
            return Err(anyhow::anyhow!(
                "GitHub API request failed: {}",
                response.status()
            ));
        }

        let issue = response.json::<GitHubIssue>().await?;
        Ok(issue)
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
            return Err(anyhow::anyhow!(
                "Failed to post GitHub comment: {}",
                response.status()
            ));
        }

        Ok(())
    }

    pub async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
        base_branch: &str,
    ) -> Result<()> {
        // First get the SHA of the base branch
        let url = format!(
            "https://api.github.com/repos/{}/{}/git/ref/heads/{}",
            owner, repo, base_branch
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
            return Err(anyhow::anyhow!(
                "Failed to get base branch ref: {}",
                response.status()
            ));
        }

        let base_ref = response.json::<serde_json::Value>().await?;
        let sha = base_ref["object"]["sha"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid base branch ref response"))?;

        // Create the new branch
        let url = format!(
            "https://api.github.com/repos/{}/{}/git/refs",
            owner, repo
        );

        let payload = BranchPayload {
            ref_name: format!("refs/heads/{}", branch_name),
            sha: sha.to_string(),
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
            return Err(anyhow::anyhow!(
                "Failed to create branch: {}",
                response.status()
            ));
        }

        Ok(())
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
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls",
            owner, repo
        );

        let payload = PullRequestPayload {
            title: title.to_string(),
            body: description.to_string(),
            head: head.to_string(),
            base: base.to_string(),
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
            return Err(anyhow::anyhow!(
                "Failed to create pull request: {}",
                response.status()
            ));
        }

        Ok(())
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