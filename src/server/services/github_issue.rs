use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

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

impl GitHubService {
    pub fn new(token: String) -> Self {
        Self {
            client: Client::new(),
            token,
        }
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
}

pub async fn post_github_comment(
    issue_number: i32,
    comment: &str,
    owner: &str,
    repo: &str,
    token: &str,
) -> Result<()> {
    let service = GitHubService::new(token.to_string());
    service
        .post_comment(owner, repo, issue_number, comment)
        .await
}