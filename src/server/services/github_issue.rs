use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct GitHubService {
    client: Client,
    token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub number: i32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
}

impl GitHubService {
    pub fn new(token: Option<String>) -> Self {
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

        let mut request = self.client.get(&url);

        if let Some(token) = &self.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        request = request.header("User-Agent", "OpenAgents");

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "GitHub API request failed: {}",
                response.status()
            ));
        }

        let issue = response.json::<GitHubIssue>().await?;
        Ok(issue)
    }
}
