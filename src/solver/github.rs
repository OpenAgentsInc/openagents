use anyhow::{anyhow, Result};
use crate::server::services::github_issue::GitHubService;

pub struct GitHubContext {
    pub owner: String,
    pub repo: String,
    pub service: GitHubService,
}

impl GitHubContext {
    pub fn new(repo_string: &str, token: String) -> Result<Self> {
        let repo_parts: Vec<&str> = repo_string.split('/').collect();
        if repo_parts.len() != 2 {
            return Err(anyhow!("Invalid repository format. Expected 'owner/name'"));
        }

        let (owner, repo) = (repo_parts[0].to_string(), repo_parts[1].to_string());
        let service = GitHubService::new(Some(token))?;

        Ok(Self {
            owner,
            repo,
            service,
        })
    }

    pub async fn create_branch(&self, branch_name: &str, base_branch: &str) -> Result<()> {
        self.service
            .create_branch(&self.owner, &self.repo, branch_name, base_branch)
            .await
    }

    pub async fn create_pull_request(
        &self,
        branch_name: &str,
        base_branch: &str,
        title: &str,
        description: &str,
    ) -> Result<()> {
        self.service
            .create_pull_request(
                &self.owner,
                &self.repo,
                branch_name,
                base_branch,
                title,
                description,
            )
            .await
    }

    pub async fn post_comment(&self, issue_number: i32, comment: &str) -> Result<()> {
        self.service
            .post_comment(&self.owner, &self.repo, issue_number, comment)
            .await
    }

    pub async fn get_issue(&self, issue_number: i32) -> Result<crate::server::services::github_issue::GitHubIssue> {
        self.service
            .get_issue(&self.owner, &self.repo, issue_number)
            .await
    }
}