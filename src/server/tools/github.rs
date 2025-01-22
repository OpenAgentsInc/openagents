use std::sync::Arc;
use serde_json::Value;
use mockall::automock;
use crate::tools::{Tool, ToolError};

#[automock]
pub trait GitHubService {
    async fn get_issue(&self, owner: &str, repo: &str, issue_number: i32) -> Result<String, ToolError>;
    async fn create_pull_request(&self, owner: &str, repo: &str, title: &str, description: &str, head: &str, base: &str) -> Result<String, ToolError>;
    async fn get_file_contents(&self, owner: &str, repo: &str, path: &str, branch: &str) -> Result<String, ToolError>;
    async fn get_directory_contents(&self, owner: &str, repo: &str, path: &str, branch: &str) -> Result<String, ToolError>;
    async fn create_file(&self, owner: &str, repo: &str, path: &str, content: &str, branch: &str) -> Result<String, ToolError>;
    async fn update_file(&self, owner: &str, repo: &str, path: &str, content: &str, branch: &str) -> Result<String, ToolError>;
}

pub struct GitHubTools {
    github_service: Arc<dyn GitHubService>,
}

impl GitHubTools {
    pub fn new(github_service: Arc<dyn GitHubService>) -> Self {
        Self { github_service }
    }
}

impl Tool for GitHubTools {
    fn name(&self) -> &'static str {
        "github_tools"
    }

    fn description(&self) -> &'static str {
        "GitHub operations like fetching issues and managing pull requests"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "Repository owner"
                },
                "repo": {
                    "type": "string",
                    "description": "Repository name"
                },
                "issue_number": {
                    "type": "number",
                    "description": "Issue number"
                }
            },
            "required": ["owner", "repo", "issue_number"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let owner = args["owner"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("owner is required".into()))?;
        let repo = args["repo"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("repo is required".into()))?;
        let issue_number = args["issue_number"].as_i64()
            .ok_or_else(|| ToolError::InvalidArguments("issue_number is required".into()))? as i32;

        self.github_service.get_issue(owner, repo, issue_number).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_github_tools_execute() {
        let mut mock_service = MockGitHubService::new();
        mock_service
            .expect_get_issue()
            .with(eq("owner"), eq("repo"), eq(123))
            .times(1)
            .returning(|_, _, _| Ok("Issue content".to_string()));

        let tools = GitHubTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "owner": "owner",
            "repo": "repo",
            "issue_number": 123
        });

        let result = tools.execute(args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Issue content");
    }

    #[tokio::test]
    async fn test_github_tools_invalid_args() {
        let mock_service = MockGitHubService::new();
        let tools = GitHubTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "owner": "owner",
            // Missing repo and issue_number
        });

        let result = tools.execute(args).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }
}