use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::server::services::github::GitHubService;
use super::{Tool, ToolExecutor, ToolError};

pub struct FileTools {
    github_service: Arc<GitHubService>,
}

impl FileTools {
    pub fn new(github_service: Arc<GitHubService>) -> Self {
        Self { github_service }
    }

    fn view_file_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the file to view"
                },
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository"
                },
                "branch": {
                    "type": "string",
                    "description": "The branch to view the file from"
                }
            },
            "required": ["path", "owner", "repo", "branch"]
        })
    }

    fn view_hierarchy_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to view the hierarchy"
                },
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository"
                },
                "branch": {
                    "type": "string",
                    "description": "The branch to view the hierarchy from"
                }
            },
            "required": ["path", "owner", "repo", "branch"]
        })
    }

    fn create_file_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path where the new file should be created"
                },
                "content": {
                    "type": "string",
                    "description": "The content of the new file"
                },
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository"
                },
                "branch": {
                    "type": "string",
                    "description": "The branch to create the file on"
                }
            },
            "required": ["path", "content", "owner", "repo", "branch"]
        })
    }

    fn rewrite_file_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the file to rewrite"
                },
                "content": {
                    "type": "string",
                    "description": "The new content to write to the file"
                },
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository"
                },
                "branch": {
                    "type": "string",
                    "description": "The branch to update"
                }
            },
            "required": ["path", "content", "owner", "repo", "branch"]
        })
    }
}

#[async_trait]
impl ToolExecutor for FileTools {
    async fn execute(&self, name: &str, args: Value) -> Result<String> {
        match name {
            "view_file" => {
                let path = args["path"].as_str().ok_or(ToolError::InvalidArguments("path is required".into()))?;
                let owner = args["owner"].as_str().ok_or(ToolError::InvalidArguments("owner is required".into()))?;
                let repo = args["repo"].as_str().ok_or(ToolError::InvalidArguments("repo is required".into()))?;
                let branch = args["branch"].as_str().ok_or(ToolError::InvalidArguments("branch is required".into()))?;

                let content = self.github_service.get_file_contents(owner, repo, path, branch).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                Ok(content)
            },
            "view_hierarchy" => {
                let path = args["path"].as_str().ok_or(ToolError::InvalidArguments("path is required".into()))?;
                let owner = args["owner"].as_str().ok_or(ToolError::InvalidArguments("owner is required".into()))?;
                let repo = args["repo"].as_str().ok_or(ToolError::InvalidArguments("repo is required".into()))?;
                let branch = args["branch"].as_str().ok_or(ToolError::InvalidArguments("branch is required".into()))?;

                let contents = self.github_service.get_directory_contents(owner, repo, path, branch).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                Ok(serde_json::to_string_pretty(&contents)?)
            },
            "create_file" => {
                let path = args["path"].as_str().ok_or(ToolError::InvalidArguments("path is required".into()))?;
                let content = args["content"].as_str().ok_or(ToolError::InvalidArguments("content is required".into()))?;
                let owner = args["owner"].as_str().ok_or(ToolError::InvalidArguments("owner is required".into()))?;
                let repo = args["repo"].as_str().ok_or(ToolError::InvalidArguments("repo is required".into()))?;
                let branch = args["branch"].as_str().ok_or(ToolError::InvalidArguments("branch is required".into()))?;

                self.github_service.create_file(owner, repo, path, content, branch).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                Ok(format!("File {} created successfully", path))
            },
            "rewrite_file" => {
                let path = args["path"].as_str().ok_or(ToolError::InvalidArguments("path is required".into()))?;
                let content = args["content"].as_str().ok_or(ToolError::InvalidArguments("content is required".into()))?;
                let owner = args["owner"].as_str().ok_or(ToolError::InvalidArguments("owner is required".into()))?;
                let repo = args["repo"].as_str().ok_or(ToolError::InvalidArguments("repo is required".into()))?;
                let branch = args["branch"].as_str().ok_or(ToolError::InvalidArguments("branch is required".into()))?;

                self.github_service.update_file(owner, repo, path, content, branch).await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                Ok(format!("File {} updated successfully", path))
            },
            _ => Err(ToolError::InvalidArguments(format!("Unknown tool: {}", name)).into())
        }
    }

    fn get_available_tools(&self) -> Vec<Tool> {
        vec![
            Tool::new(
                "view_file",
                "View file contents at path",
                Self::view_file_schema(),
            ),
            Tool::new(
                "view_hierarchy",
                "View file/folder hierarchy at path (one level deep)",
                Self::view_hierarchy_schema(),
            ),
            Tool::new(
                "create_file",
                "Creates a new file at the given path with the provided content",
                Self::create_file_schema(),
            ),
            Tool::new(
                "rewrite_file",
                "Rewrites the contents of a file at the given path",
                Self::rewrite_file_schema(),
            ),
        ]
    }

    fn validate_arguments(&self, name: &str, args: &Value) -> Result<()> {
        let schema = match name {
            "view_file" => Self::view_file_schema(),
            "view_hierarchy" => Self::view_hierarchy_schema(),
            "create_file" => Self::create_file_schema(),
            "rewrite_file" => Self::rewrite_file_schema(),
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
            fn get_file_contents(&self, owner: &str, repo: &str, path: &str, branch: &str) -> Result<String>;
            fn get_directory_contents(&self, owner: &str, repo: &str, path: &str, branch: &str) -> Result<Value>;
            fn create_file(&self, owner: &str, repo: &str, path: &str, content: &str, branch: &str) -> Result<()>;
            fn update_file(&self, owner: &str, repo: &str, path: &str, content: &str, branch: &str) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn test_view_file() {
        let mut mock_service = MockGitHubService::new();
        mock_service.expect_get_file_contents()
            .with(eq("owner"), eq("repo"), eq("path/to/file"), eq("main"))
            .returning(|_, _, _, _| Ok("file contents".to_string()));

        let tools = FileTools::new(Arc::new(mock_service));
        let args = json!({
            "owner": "owner",
            "repo": "repo",
            "path": "path/to/file",
            "branch": "main"
        });

        let result = tools.execute("view_file", args).await.unwrap();
        assert_eq!(result, "file contents");
    }

    #[tokio::test]
    async fn test_view_hierarchy() {
        let mut mock_service = MockGitHubService::new();
        mock_service.expect_get_directory_contents()
            .with(eq("owner"), eq("repo"), eq("path/to/dir"), eq("main"))
            .returning(|_, _, _, _| Ok(json!([
                {"name": "file1.rs", "type": "file"},
                {"name": "file2.rs", "type": "file"}
            ])));

        let tools = FileTools::new(Arc::new(mock_service));
        let args = json!({
            "owner": "owner",
            "repo": "repo",
            "path": "path/to/dir",
            "branch": "main"
        });

        let result = tools.execute("view_hierarchy", args).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0]["name"], "file1.rs");
        assert_eq!(parsed[1]["name"], "file2.rs");
    }

    #[tokio::test]
    async fn test_create_file() {
        let mut mock_service = MockGitHubService::new();
        mock_service.expect_create_file()
            .with(
                eq("owner"),
                eq("repo"),
                eq("path/to/file"),
                eq("content"),
                eq("main")
            )
            .returning(|_, _, _, _, _| Ok(()));

        let tools = FileTools::new(Arc::new(mock_service));
        let args = json!({
            "owner": "owner",
            "repo": "repo",
            "path": "path/to/file",
            "content": "content",
            "branch": "main"
        });

        let result = tools.execute("create_file", args).await.unwrap();
        assert_eq!(result, "File path/to/file created successfully");
    }

    #[tokio::test]
    async fn test_rewrite_file() {
        let mut mock_service = MockGitHubService::new();
        mock_service.expect_update_file()
            .with(
                eq("owner"),
                eq("repo"),
                eq("path/to/file"),
                eq("new content"),
                eq("main")
            )
            .returning(|_, _, _, _, _| Ok(()));

        let tools = FileTools::new(Arc::new(mock_service));
        let args = json!({
            "owner": "owner",
            "repo": "repo",
            "path": "path/to/file",
            "content": "new content",
            "branch": "main"
        });

        let result = tools.execute("rewrite_file", args).await.unwrap();
        assert_eq!(result, "File path/to/file updated successfully");
    }

    #[test]
    fn test_validate_arguments() {
        let service = MockGitHubService::new();
        let tools = FileTools::new(Arc::new(service));

        // Valid view_file args
        let valid_args = json!({
            "owner": "owner",
            "repo": "repo",
            "path": "path/to/file",
            "branch": "main"
        });
        assert!(tools.validate_arguments("view_file", &valid_args).is_ok());

        // Invalid view_file args
        let invalid_args = json!({
            "owner": "owner",
            // Missing repo
            "path": "path/to/file",
            "branch": "main"
        });
        assert!(tools.validate_arguments("view_file", &invalid_args).is_err());
    }
}