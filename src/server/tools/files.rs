use std::sync::Arc;
use serde_json::Value;
use mockall::predicate::*;
use mockall::automock;
use crate::tools::{Tool, ToolError};

#[automock]
pub trait FileService {
    async fn get_file_contents(&self, owner: &str, repo: &str, path: &str, branch: &str) -> Result<String, ToolError>;
    async fn get_directory_contents(&self, owner: &str, repo: &str, path: &str, branch: &str) -> Result<String, ToolError>;
    async fn create_file(&self, owner: &str, repo: &str, path: &str, content: &str, branch: &str) -> Result<String, ToolError>;
    async fn update_file(&self, owner: &str, repo: &str, path: &str, content: &str, branch: &str) -> Result<String, ToolError>;
}

pub struct FileTools {
    file_service: Arc<dyn FileService>,
}

impl FileTools {
    pub fn new(file_service: Arc<dyn FileService>) -> Self {
        Self { file_service }
    }
}

impl Tool for FileTools {
    fn name(&self) -> &'static str {
        "file_tools"
    }

    fn description(&self) -> &'static str {
        "File operations like viewing and modifying files"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["view", "list", "create", "update"],
                    "description": "Operation to perform"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory path"
                },
                "owner": {
                    "type": "string",
                    "description": "Repository owner"
                },
                "repo": {
                    "type": "string",
                    "description": "Repository name"
                },
                "branch": {
                    "type": "string",
                    "description": "Branch name"
                },
                "content": {
                    "type": "string",
                    "description": "File content for create/update operations"
                }
            },
            "required": ["operation", "path", "owner", "repo", "branch"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let operation = args["operation"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("operation is required".into()))?;
        let path = args["path"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("path is required".into()))?;
        let owner = args["owner"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("owner is required".into()))?;
        let repo = args["repo"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("repo is required".into()))?;
        let branch = args["branch"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("branch is required".into()))?;

        match operation {
            "view" => {
                self.file_service.get_file_contents(owner, repo, path, branch).await
            }
            "list" => {
                self.file_service.get_directory_contents(owner, repo, path, branch).await
            }
            "create" => {
                let content = args["content"].as_str()
                    .ok_or_else(|| ToolError::InvalidArguments("content is required for create operation".into()))?;
                self.file_service.create_file(owner, repo, path, content, branch).await
            }
            "update" => {
                let content = args["content"].as_str()
                    .ok_or_else(|| ToolError::InvalidArguments("content is required for update operation".into()))?;
                self.file_service.update_file(owner, repo, path, content, branch).await
            }
            _ => Err(ToolError::InvalidArguments(format!("Unknown operation: {}", operation)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_view_file() {
        let mut mock_service = MockFileService::new();
        mock_service
            .expect_get_file_contents()
            .with(eq("owner"), eq("repo"), eq("path/to/file"), eq("main"))
            .times(1)
            .returning(|_, _, _, _| Ok("file content".to_string()));

        let tools = FileTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "view",
            "path": "path/to/file",
            "owner": "owner",
            "repo": "repo",
            "branch": "main"
        });

        let result = tools.execute(args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "file content");
    }

    #[tokio::test]
    async fn test_list_directory() {
        let mut mock_service = MockFileService::new();
        mock_service
            .expect_get_directory_contents()
            .with(eq("owner"), eq("repo"), eq("path/to/dir"), eq("main"))
            .times(1)
            .returning(|_, _, _, _| Ok("directory content".to_string()));

        let tools = FileTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "list",
            "path": "path/to/dir",
            "owner": "owner",
            "repo": "repo",
            "branch": "main"
        });

        let result = tools.execute(args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "directory content");
    }

    #[tokio::test]
    async fn test_create_file() {
        let mut mock_service = MockFileService::new();
        mock_service
            .expect_create_file()
            .with(
                eq("owner"),
                eq("repo"),
                eq("path/to/file"),
                eq("content"),
                eq("main")
            )
            .times(1)
            .returning(|_, _, _, _, _| Ok("file created".to_string()));

        let tools = FileTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "create",
            "path": "path/to/file",
            "owner": "owner",
            "repo": "repo",
            "content": "content",
            "branch": "main"
        });

        let result = tools.execute(args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "file created");
    }

    #[tokio::test]
    async fn test_update_file() {
        let mut mock_service = MockFileService::new();
        mock_service
            .expect_update_file()
            .with(
                eq("owner"),
                eq("repo"),
                eq("path/to/file"),
                eq("new content"),
                eq("main")
            )
            .times(1)
            .returning(|_, _, _, _, _| Ok("file updated".to_string()));

        let tools = FileTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "update",
            "path": "path/to/file",
            "owner": "owner",
            "repo": "repo",
            "content": "new content",
            "branch": "main"
        });

        let result = tools.execute(args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "file updated");
    }

    #[tokio::test]
    async fn test_invalid_operation() {
        let mock_service = MockFileService::new();
        let tools = FileTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "invalid",
            "path": "path/to/file",
            "owner": "owner",
            "repo": "repo",
            "branch": "main"
        });

        let result = tools.execute(args).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }
}