use crate::server::services::github_issue::{GitHubComment, GitHubIssue, GitHubService};
use crate::server::services::deepseek::DeepSeekService;
use crate::solver::json::{escape_json_string, fix_common_json_issues};
use anyhow::{anyhow, Result};
use tracing::{debug, error, info, warn};

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

    /// Validates a PR title against required format and constraints
    fn validate_pr_title(&self, title: &str) -> Result<()> {
        // Check length constraints
        if title.len() < 10 || title.len() > 72 {
            return Err(anyhow!("PR title must be between 10 and 72 characters"));
        }

        // Must contain a colon for type prefix
        if !title.contains(':') {
            return Err(anyhow!("PR title must have format <type>: <description>"));
        }

        // Validate prefix
        let prefix = title.split(':').next().unwrap();
        let valid_prefixes = ["feat", "fix", "refactor", "docs", "test", "chore", "style", "perf"];
        if !valid_prefixes.contains(&prefix) {
            return Err(anyhow!("Invalid PR title prefix: {}", prefix));
        }

        // Check for banned phrases
        let banned_phrases = ["implement solution for", "fixes #", "resolves #"];
        for phrase in banned_phrases {
            if title.to_lowercase().contains(phrase) {
                return Err(anyhow!("PR title contains banned phrase: {}", phrase));
            }
        }

        // Description part should not be empty
        let desc = title.split(':').nth(1).unwrap_or("").trim();
        if desc.is_empty() {
            return Err(anyhow!("PR title must have a description after the prefix"));
        }

        Ok(())
    }

    /// Generates a descriptive PR title using an LLM with retry logic
    async fn generate_pr_title(&self, issue_number: i32, context: &str) -> Result<String> {
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 0..max_retries {
            if attempt > 0 {
                warn!("Retrying PR title generation (attempt {})", attempt + 1);
            }

            let prompt = format!(
                r#"Generate a concise, descriptive pull request title for issue #{} based on this context:

{}

Requirements:
1. Must start with one of: feat:, fix:, refactor:, docs:, test:, chore:, style:, perf:
2. Must be descriptive but succinct (10-72 chars)
3. Must clearly describe the change
4. Must not use generic phrases like "Implement solution for"
5. Must not include issue references like "fixes #123"

Example good titles:
- "feat: add multiply function to calculator"
- "fix: handle JSON escaping in PR titles"
- "refactor: improve PR title generation"
- "style: format code with rustfmt"
- "perf: optimize database queries"

Example bad titles:
- "Implement solution for #123" (wrong format)
- "Add function" (too vague)
- "feat:without space" (missing space after colon)
- "feat: implement the new feature that was requested in the issue" (too long)

Generate exactly one line containing only the title:"#,
                issue_number, escape_json_string(context)
            );

            match self.llm_service.chat(prompt, true).await {
                Ok((response, _)) => {
                    let title = response.trim();
                    
                    match self.validate_pr_title(title) {
                        Ok(()) => {
                            debug!("Generated valid PR title: {}", title);
                            return Ok(title.to_string());
                        }
                        Err(e) => {
                            warn!("Generated invalid PR title: {}", e);
                            last_error = Some(e);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    warn!("LLM request failed: {}", e);
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Failed to generate valid PR title")))
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
    use tokio;

    #[tokio::test]
    async fn test_generate_pr_title() {
        let context = GitHubContext::new(
            "test/repo",
            "test_token".to_string(),
        ).unwrap();

        let test_context = "Add a multiply function that multiplies two integers";
        let title = context.generate_pr_title(123, test_context).await.unwrap();

        assert!(title.starts_with("feat:") || title.starts_with("fix:"));
        assert!(title.contains("multiply"));
        assert!(title.len() <= 72);
        assert!(title.len() >= 10);
        assert!(title.split(':').nth(1).unwrap().starts_with(" "));
    }

    #[test]
    fn test_validate_pr_title() {
        let context = GitHubContext::new(
            "test/repo",
            "test_token".to_string(),
        ).unwrap();

        // Valid titles
        assert!(context.validate_pr_title("feat: add multiply function").is_ok());
        assert!(context.validate_pr_title("fix: handle JSON escaping").is_ok());
        assert!(context.validate_pr_title("refactor: improve title generation").is_ok());

        // Invalid titles
        assert!(context.validate_pr_title("add function").is_err()); // No prefix
        assert!(context.validate_pr_title("feat:no space").is_err()); // No space after colon
        assert!(context.validate_pr_title("feat: ").is_err()); // Empty description
        assert!(context.validate_pr_title("invalid: wrong prefix").is_err()); // Invalid prefix
        assert!(context.validate_pr_title("feat: implement solution for #123").is_err()); // Banned phrase
        assert!(context.validate_pr_title("feat: a").is_err()); // Too short
        assert!(context.validate_pr_title("feat: this title is way too long and exceeds the maximum length limit of 72 characters").is_err()); // Too long
    }

    #[test]
    fn test_new_with_invalid_repo() {
        let result = GitHubContext::new("invalid", "token".to_string());
        assert!(result.is_err());
    }
}