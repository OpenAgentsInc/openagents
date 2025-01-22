use std::sync::Arc;
use anyhow::Result;
use serde_json::Value;

use crate::server::services::github::GitHubService;
use super::{
    Tool, ToolExecutor, ToolError,
    github::GitHubTools,
    files::FileTools,
    external::ExternalTools,
};

pub struct ToolExecutorFactory {
    github_service: Arc<GitHubService>,
    firecrawl_api_key: String,
    team_knowledge_api_key: String,
}

impl ToolExecutorFactory {
    pub fn new(
        github_service: Arc<GitHubService>,
        firecrawl_api_key: String,
        team_knowledge_api_key: String,
    ) -> Self {
        Self {
            github_service,
            firecrawl_api_key,
            team_knowledge_api_key,
        }
    }

    pub fn create_executor(&self, tool_type: &str) -> Result<Box<dyn ToolExecutor>> {
        match tool_type {
            "github" => Ok(Box::new(GitHubTools::new(self.github_service.clone()))),
            "files" => Ok(Box::new(FileTools::new(self.github_service.clone()))),
            "external" => Ok(Box::new(ExternalTools::new(
                self.firecrawl_api_key.clone(),
                self.team_knowledge_api_key.clone(),
            ))),
            _ => Err(ToolError::InvalidArguments(format!("Unknown tool type: {}", tool_type)).into())
        }
    }

    pub fn get_all_tools(&self) -> Vec<Tool> {
        let mut tools = Vec::new();

        // Add GitHub tools
        if let Ok(executor) = self.create_executor("github") {
            tools.extend(executor.get_available_tools());
        }

        // Add file tools
        if let Ok(executor) = self.create_executor("files") {
            tools.extend(executor.get_available_tools());
        }

        // Add external tools
        if let Ok(executor) = self.create_executor("external") {
            tools.extend(executor.get_available_tools());
        }

        tools
    }

    pub async fn execute_tool(&self, name: &str, args: Value) -> Result<String> {
        // Determine tool type from name
        let tool_type = if name.starts_with("github_") {
            "github"
        } else if name.starts_with("file_") {
            "files"
        } else {
            "external"
        };

        let executor = self.create_executor(tool_type)?;
        executor.execute(name, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use serde_json::json;

    mock! {
        GitHubService {}
    }

    #[test]
    fn test_factory_creation() {
        let github_service = Arc::new(MockGitHubService::new());
        let factory = ToolExecutorFactory::new(
            github_service,
            "firecrawl-key".to_string(),
            "knowledge-key".to_string(),
        );

        // Test GitHub tools
        let github_executor = factory.create_executor("github");
        assert!(github_executor.is_ok());

        // Test file tools
        let file_executor = factory.create_executor("files");
        assert!(file_executor.is_ok());

        // Test external tools
        let external_executor = factory.create_executor("external");
        assert!(external_executor.is_ok());

        // Test invalid type
        let invalid_executor = factory.create_executor("invalid");
        assert!(invalid_executor.is_err());
    }

    #[test]
    fn test_get_all_tools() {
        let github_service = Arc::new(MockGitHubService::new());
        let factory = ToolExecutorFactory::new(
            github_service,
            "firecrawl-key".to_string(),
            "knowledge-key".to_string(),
        );

        let tools = factory.get_all_tools();

        // Verify we have tools from all categories
        assert!(tools.iter().any(|t| t.function.name.starts_with("github_")));
        assert!(tools.iter().any(|t| t.function.name.starts_with("file_")));
        assert!(tools.iter().any(|t| t.function.name == "scrape_webpage"));
        assert!(tools.iter().any(|t| t.function.name == "search_team_knowledge"));
    }

    #[tokio::test]
    async fn test_execute_tool() {
        let github_service = Arc::new(MockGitHubService::new());
        let factory = ToolExecutorFactory::new(
            github_service,
            "firecrawl-key".to_string(),
            "knowledge-key".to_string(),
        );

        // Test executing a GitHub tool
        let github_result = factory
            .execute_tool(
                "github_fetch_issue",
                json!({
                    "owner": "test",
                    "repo": "test",
                    "issueNumber": 1
                }),
            )
            .await;
        assert!(github_result.is_ok());

        // Test executing a file tool
        let file_result = factory
            .execute_tool(
                "file_view_file",
                json!({
                    "owner": "test",
                    "repo": "test",
                    "path": "test.rs",
                    "branch": "main"
                }),
            )
            .await;
        assert!(file_result.is_ok());

        // Test executing an external tool
        let external_result = factory
            .execute_tool(
                "scrape_webpage",
                json!({
                    "url": "https://example.com"
                }),
            )
            .await;
        assert!(external_result.is_ok());
    }
}