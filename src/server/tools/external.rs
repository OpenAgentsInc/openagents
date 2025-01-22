use std::sync::Arc;
use serde_json::Value;
use mockall::automock;
use crate::tools::{Tool, ToolError};

#[automock]
pub trait ExternalService {
    async fn scrape_webpage(&self, url: &str, only_main_content: bool) -> Result<String, ToolError>;
    async fn search_knowledge(&self, query: &str, limit: Option<i32>) -> Result<String, ToolError>;
}

pub struct ExternalTools {
    external_service: Arc<dyn ExternalService>,
}

impl ExternalTools {
    pub fn new(external_service: Arc<dyn ExternalService>) -> Self {
        Self { external_service }
    }
}

impl Tool for ExternalTools {
    fn name(&self) -> &'static str {
        "external_tools"
    }

    fn description(&self) -> &'static str {
        "External services like web scraping and knowledge base search"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["scrape", "search"],
                    "description": "Operation to perform"
                },
                "url": {
                    "type": "string",
                    "description": "URL to scrape"
                },
                "onlyMainContent": {
                    "type": "boolean",
                    "description": "Whether to extract only the main content"
                },
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results"
                }
            },
            "required": ["operation"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let operation = args["operation"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("operation is required".into()))?;

        match operation {
            "scrape" => {
                let url = args["url"].as_str()
                    .ok_or_else(|| ToolError::InvalidArguments("url is required for scrape operation".into()))?;
                let only_main_content = args["onlyMainContent"].as_bool().unwrap_or(false);

                self.external_service.scrape_webpage(url, only_main_content).await
            }
            "search" => {
                let query = args["query"].as_str()
                    .ok_or_else(|| ToolError::InvalidArguments("query is required for search operation".into()))?;
                let limit = args["limit"].as_i64().map(|l| l as i32);

                self.external_service.search_knowledge(query, limit).await
            }
            _ => Err(ToolError::InvalidArguments(format!("Unknown operation: {}", operation)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_scrape_webpage() {
        let mut mock_service = MockExternalService::new();
        mock_service
            .expect_scrape_webpage()
            .with(eq("https://example.com"), eq(false))
            .times(1)
            .returning(|_, _| Ok("scraped content".to_string()));

        let tools = ExternalTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "scrape",
            "url": "https://example.com",
            "onlyMainContent": false
        });

        let result = tools.execute(args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "scraped content");
    }

    #[tokio::test]
    async fn test_search_knowledge() {
        let mut mock_service = MockExternalService::new();
        mock_service
            .expect_search_knowledge()
            .with(eq("test query"), eq(Some(10)))
            .times(1)
            .returning(|_, _| Ok("search results".to_string()));

        let tools = ExternalTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "search",
            "query": "test query",
            "limit": 10
        });

        let result = tools.execute(args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "search results");
    }

    #[tokio::test]
    async fn test_invalid_operation() {
        let mock_service = MockExternalService::new();
        let tools = ExternalTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "invalid"
        });

        let result = tools.execute(args).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn test_missing_required_args() {
        let mock_service = MockExternalService::new();
        let tools = ExternalTools::new(Arc::new(mock_service));

        let args = serde_json::json!({
            "operation": "scrape"
            // Missing url
        });

        let result = tools.execute(args).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }
}