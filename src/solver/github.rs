use crate::server::services::github_issue::{GitHubComment, GitHubIssue, GitHubService};
use crate::server::services::deepseek::DeepSeekService;
use anyhow::{anyhow, Result};
use tracing::{debug, info, warn};

pub struct GitHubContext {
    pub owner: String,
    pub repo: String,
    pub service: GitHubService,
    pub llm_service: DeepSeekService,
}

impl GitHubContext {
    pub fn new(repo_string: &str, token: String) -> Result<Self> {
        let repo_parts: Vec<&str> = repo_string.split('/').collect();
        if repo_parts.len() != 2 {
            return Err(anyhow!("Invalid repository format. Expected 'owner/name'"));
        }

        let (owner, repo) = (repo_parts[0].to_string(), repo_parts[1].to_string());
        let service = GitHubService::new(Some(token.clone()))?;
        let llm_service = DeepSeekService::new(token);

        Ok(Self {
            owner,
            repo,
            service,
            llm_service,
        })
    }

    pub async fn create_branch(&self, branch_name: &str, base_branch: &str) -> Result<()> {
        info!(
            "Creating branch '{}' from base '{}'",
            branch_name, base_branch
        );
        self.service
            .create_branch(&self.owner, &self.repo, branch_name, base_branch)
            .await?;
        info!("Successfully created branch '{}'", branch_name);
        Ok(())
    }

    /// Generates a descriptive PR title using an LLM
    async fn generate_pr_title(&self, issue_number: i32, context: &str) -> Result<String> {
        let prompt = format!(
            r#"Generate a concise, descriptive pull request title for issue #{} based on this context:

{}

Requirements:
1. Must start with "feat:", "fix:", "refactor:", etc.
2. Must be descriptive but succinct
3. Must not exceed 72 characters
4. Must not use "Implement solution for"
5. Must clearly state what the PR does

Example good titles:
- "feat: add multiply function to calculator"
- "fix: handle JSON escaping in PR titles"
- "refactor: improve PR title generation"

Example bad titles:
- "Implement solution for #123"
- "Add function"
- "Fix issue"

Generate title:"#,
            issue_number, context
        );

        let (response, _) = self.llm_service.chat(prompt, true).await?;

        let title = response.trim();
        
        // Validate title
        if title.len() < 10 || title.len() > 72 {
            warn!("Generated title has invalid length: {}", title.len());
            return Err(anyhow!("Generated title has invalid length"));
        }

        if !title.contains(':') {
            warn!("Generated title missing prefix: {}", title);
            return Err(anyhow!("Generated title must start with feat:, fix:, etc."));
        }

        let prefix = title.split(':').next().unwrap();
        if !["feat", "fix", "refactor", "docs", "test", "chore"].contains(&prefix) {
            warn!("Generated title has invalid prefix: {}", prefix);
            return Err(anyhow!("Generated title has invalid prefix"));
        }

        debug!("Generated PR title: {}", title);
        Ok(title.to_string())
    }

    pub async fn create_pull_request(
        &self,
        branch_name: &str,
        base_branch: &str,
        context: &str,
        description: &str,
        issue_number: i32,
    ) -> Result<()> {
        // Generate descriptive title
        let title = self.generate_pr_title(issue_number, context).await?;

        info!("Creating PR with title: {}", title);
        
        self.service
            .create_pull_request(
                &self.owner,
                &self.repo,
                branch_name,
                base_branch,
                &title,
                description,
            )
            .await
    }

    pub async fn post_comment(&self, issue_number: i32, comment: &str) -> Result<()> {
        self.service
            .post_comment(&self.owner, &self.repo, issue_number, comment)
            .await
    }

    pub async fn get_issue(&self, issue_number: i32) -> Result<GitHubIssue> {
        self.service
            .get_issue(&self.owner, &self.repo, issue_number)
            .await
    }

    pub async fn get_issue_comments(&self, issue_number: i32) -> Result<Vec<GitHubComment>> {
        self.service
            .get_issue_comments(&self.owner, &self.repo, issue_number)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;
    use tokio::test;

    #[test]
    async fn test_generate_pr_title() {
        let mut server = Server::new();
        let mock_response = json!({
            "choices": [{
                "message": {
                    "content": "feat: add multiply function"
                }
            }]
        });

        let mock = server.mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create();

        std::env::set_var("DEEPSEEK_API_URL", &server.url());
        
        let context = GitHubContext::new(
            "test/repo",
            "test_token".to_string(),
        ).unwrap();

        let test_context = "Add a multiply function that multiplies two integers";
        let title = context.generate_pr_title(123, test_context).await.unwrap();

        mock.assert();
        assert!(title.starts_with("feat:"));
        assert!(title.contains("multiply"));
        assert!(title.len() <= 72);
        assert!(title.len() >= 10);
    }

    #[test]
    async fn test_new_with_invalid_repo() {
        let result = GitHubContext::new("invalid", "token".to_string());
        assert!(result.is_err());
    }
}