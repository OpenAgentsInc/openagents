use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::server::services::github::GitHubService;
use super::{Tool, ToolExecutor, ToolError};

pub struct GitHubTools {
    github_service: Arc<GitHubService>,
}

impl GitHubTools {
    pub fn new(github_service: Arc<GitHubService>) -> Self {
        Self { github_service }
    }

    fn fetch_issue_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository"
                },
                "issueNumber": {
                    "type": "number",
                    "description": "The number of the GitHub issue to fetch"
                }
            },
            "required": ["issueNumber", "owner", "repo"]
        })
    }

    fn create_pr_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository"
                },
                "title": {
                    "type": "string",
                    "description": "The title of the pull request"
                },
                "description": {
                    "type": "string",
                    "description": "The description of the pull request"
                },
                "head": {
                    "type": "string",
                    "description": "The name of the branch where your changes are implemented"
                },
                "base": {
                    "type": "string",
                    "description": "The name of the branch you want the changes pulled into"
                }
            },
            "required": ["title", "description", "head", "base", "owner", "repo"]
        })
    }
}

#[async_trait]
impl ToolExecutor for GitHubTools {
    async fn execute(&self, name: &str, args: Value) -> Result<String> {
        match name {
            "fetch_github_issue" => {
                let owner = args["owner"].as_str().ok_or(ToolError::InvalidArguments("owner is required".into()))?;
                let repo = args["repo"].as_str().ok_or(ToolError::InvalidArguments("repo is required".into()))?;
                let issue_number = args["issueNumber"].as_i64().ok_or(ToolError::InvalidArguments("issueNumber is required".into()))? as i32;

                let issue = self.github_service.get_issue(owner, repo, issue_number).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                Ok(serde_json::to_string_pretty(&issue)?)
            },
            "create_pull_request" => {
                let owner = args["owner"].as_str().ok_or(ToolError::InvalidArguments("owner is required".into()))?;
                let repo = args["repo"].as_str().ok_or(ToolError::InvalidArguments("repo is required".into()))?;
                let title = args["title"].as_str().ok_or(ToolError::InvalidArguments("title is required".into()))?;
                let description = args["description"].as_str().ok_or(ToolError::InvalidArguments("description is required".into()))?;
                let head = args["head"].as_str().ok_or(ToolError::InvalidArguments("head is required".into()))?;
                let base = args["base"].as_str().ok_or(ToolError::InvalidArguments("base is required".into()))?;

                let pr = self.github_service.create_pull_request(owner, repo, title, description, head, base).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                Ok(serde_json::to_string_pretty(&pr)?)
            },
            _ => Err(ToolError::InvalidArguments(format!("Unknown tool: {}", name)).into())
        }
    }

    fn get_available_tools(&self) -> Vec<Tool> {
        vec![
            Tool::new(
                "fetch_github_issue",
                "Fetch details of a GitHub issue",
                Self::fetch_issue_schema(),
            ),
            Tool::new(
                "create_pull_request",
                "Creates a new pull request with specified title, description, and branches",
                Self::create_pr_schema(),
            ),
        ]
    }

    fn validate_arguments(&self, name: &str, args: &Value) -> Result<()> {
        let schema = match name {
            "fetch_github_issue" => Self::fetch_issue_schema(),
            "create_pull_request" => Self::create_pr_schema(),
            _ => return Err(ToolError::InvalidArguments(format!("Unknown tool: {}", name)).into())
        };

        let schema = jsonschema::JSONSchema::compile(&schema)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        schema.validate(args)
            .map_err(|errors| {
                let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
                ToolError::InvalidArguments(error_messages.join(", "))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;

    mock! {
        GitHubService {
            fn get_issue(&self, owner: &str, repo: &str, issue_number: i32) -> Result<Value>;
            fn create_pull_request(&self, owner: &str, repo: &str, title: &str, description: &str, head: &str, base: &str) -> Result<Value>;
        }
    }

    #[tokio::test]
    async fn test_fetch_github_issue() {
        let mut mock_service = MockGitHubService::new();
        mock_service.expect_get_issue()
            .with(eq("owner"), eq("repo"), eq(123))
            .returning(|_, _, _| Ok(json!({
                "number": 123,
                "title": "Test Issue",
                "body": "Test Body"
            })));

        let tools = GitHubTools::new(Arc::new(mock_service));
        let args = json!({
            "owner": "owner",
            "repo": "repo",
            "issueNumber": 123
        });

        let result = tools.execute("fetch_github_issue", args).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        
        assert_eq!(parsed["number"], 123);
        assert_eq!(parsed["title"], "Test Issue");
        assert_eq!(parsed["body"], "Test Body");
    }

    #[tokio::test]
    async fn test_create_pull_request() {
        let mut mock_service = MockGitHubService::new();
        mock_service.expect_create_pull_request()
            .with(
                eq("owner"),
                eq("repo"),
                eq("Test PR"),
                eq("Test Description"),
                eq("feature"),
                eq("main")
            )
            .returning(|_, _, _, _, _, _| Ok(json!({
                "number": 1,
                "title": "Test PR",
                "body": "Test Description"
            })));

        let tools = GitHubTools::new(Arc::new(mock_service));
        let args = json!({
            "owner": "owner",
            "repo": "repo",
            "title": "Test PR",
            "description": "Test Description",
            "head": "feature",
            "base": "main"
        });

        let result = tools.execute("create_pull_request", args).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        
        assert_eq!(parsed["number"], 1);
        assert_eq!(parsed["title"], "Test PR");
        assert_eq!(parsed["body"], "Test Description");
    }

    #[test]
    fn test_validate_arguments() {
        let service = MockGitHubService::new();
        let tools = GitHubTools::new(Arc::new(service));

        // Valid fetch_github_issue args
        let valid_args = json!({
            "owner": "owner",
            "repo": "repo",
            "issueNumber": 123
        });
        assert!(tools.validate_arguments("fetch_github_issue", &valid_args).is_ok());

        // Invalid fetch_github_issue args
        let invalid_args = json!({
            "owner": "owner",
            // Missing repo
            "issueNumber": 123
        });
        assert!(tools.validate_arguments("fetch_github_issue", &invalid_args).is_err());
    }
}