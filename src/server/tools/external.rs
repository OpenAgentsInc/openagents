use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Tool, ToolExecutor, ToolError};

pub struct ExternalTools {
    firecrawl_api_key: String,
    team_knowledge_api_key: String,
}

impl ExternalTools {
    pub fn new(firecrawl_api_key: String, team_knowledge_api_key: String) -> Self {
        Self {
            firecrawl_api_key,
            team_knowledge_api_key,
        }
    }

    fn scrape_webpage_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL of the webpage to scrape"
                },
                "pageOptions": {
                    "type": "object",
                    "description": "Options for page processing",
                    "properties": {
                        "onlyMainContent": {
                            "type": "boolean",
                            "description": "Whether to extract only the main content of the page"
                        }
                    },
                    "additionalProperties": false
                }
            },
            "required": ["url"]
        })
    }

    fn search_team_knowledge_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to find relevant knowledge"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return",
                    "default": 10
                }
            },
            "required": ["query"]
        })
    }

    async fn scrape_webpage(&self, url: &str, only_main_content: bool) -> Result<String> {
        let client = reqwest::Client::new();
        let mut params = vec![("url", url)];
        if only_main_content {
            params.push(("onlyMainContent", "true"));
        }

        let response = client
            .post("https://api.firecrawl.com/scrape")
            .header("X-API-Key", &self.firecrawl_api_key)
            .form(&params)
            .send()
            .await
            .map_err(|e| ToolError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "Firecrawl API error: {}",
                response.status()
            )).into());
        }

        let content = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(content)
    }

    async fn search_knowledge(&self, query: &str, limit: Option<u32>) -> Result<String> {
        let client = reqwest::Client::new();
        let mut params = vec![("query", query)];
        if let Some(limit) = limit {
            params.push(("limit", &limit.to_string()));
        }

        let response = client
            .get("https://api.teamknowledge.com/search")
            .header("Authorization", format!("Bearer {}", self.team_knowledge_api_key))
            .query(&params)
            .send()
            .await
            .map_err(|e| ToolError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "Team Knowledge API error: {}",
                response.status()
            )).into());
        }

        let content = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(content)
    }
}

#[async_trait]
impl ToolExecutor for ExternalTools {
    async fn execute(&self, name: &str, args: Value) -> Result<String> {
        match name {
            "scrape_webpage" => {
                let url = args["url"].as_str().ok_or(ToolError::InvalidArguments("url is required".into()))?;
                let only_main_content = args
                    .get("pageOptions")
                    .and_then(|opts| opts["onlyMainContent"].as_bool())
                    .unwrap_or(false);

                self.scrape_webpage(url, only_main_content).await
            },
            "search_team_knowledge" => {
                let query = args["query"].as_str().ok_or(ToolError::InvalidArguments("query is required".into()))?;
                let limit = args["limit"].as_u64().map(|l| l as u32);

                self.search_knowledge(query, limit).await
            },
            _ => Err(ToolError::InvalidArguments(format!("Unknown tool: {}", name)).into())
        }
    }

    fn get_available_tools(&self) -> Vec<Tool> {
        vec![
            Tool::new(
                "scrape_webpage",
                "Scrapes a webpage using Firecrawl and returns the content in markdown format",
                Self::scrape_webpage_schema(),
            ),
            Tool::new(
                "search_team_knowledge",
                "Search team knowledge base using the provided query",
                Self::search_team_knowledge_schema(),
            ),
        ]
    }

    fn validate_arguments(&self, name: &str, args: &Value) -> Result<()> {
        let schema = match name {
            "scrape_webpage" => Self::scrape_webpage_schema(),
            "search_team_knowledge" => Self::search_team_knowledge_schema(),
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
    use mockito::{mock, server_url};
    use serde_json::json;

    #[tokio::test]
    async fn test_scrape_webpage() {
        let _m = mock("POST", "/scrape")
            .with_header("X-API-Key", "test-key")
            .with_body(r#"{"content": "Scraped content"}"#)
            .create();

        let tools = ExternalTools::new("test-key".to_string(), "test-key".to_string());
        let args = json!({
            "url": "https://example.com",
            "pageOptions": {
                "onlyMainContent": true
            }
        });

        let result = tools.execute("scrape_webpage", args).await.unwrap();
        assert_eq!(result, r#"{"content": "Scraped content"}"#);
    }

    #[tokio::test]
    async fn test_search_team_knowledge() {
        let _m = mock("GET", "/search")
            .with_header("Authorization", "Bearer test-key")
            .with_query_param("query", "test")
            .with_query_param("limit", "5")
            .with_body(r#"{"results": ["result1", "result2"]}"#)
            .create();

        let tools = ExternalTools::new("test-key".to_string(), "test-key".to_string());
        let args = json!({
            "query": "test",
            "limit": 5
        });

        let result = tools.execute("search_team_knowledge", args).await.unwrap();
        assert_eq!(result, r#"{"results": ["result1", "result2"]}"#);
    }

    #[test]
    fn test_validate_arguments() {
        let tools = ExternalTools::new("test-key".to_string(), "test-key".to_string());

        // Valid scrape_webpage args
        let valid_args = json!({
            "url": "https://example.com",
            "pageOptions": {
                "onlyMainContent": true
            }
        });
        assert!(tools.validate_arguments("scrape_webpage", &valid_args).is_ok());

        // Invalid scrape_webpage args
        let invalid_args = json!({
            "pageOptions": {
                "onlyMainContent": true
            }
            // Missing url
        });
        assert!(tools.validate_arguments("scrape_webpage", &invalid_args).is_err());

        // Valid search_team_knowledge args
        let valid_args = json!({
            "query": "test query",
            "limit": 5
        });
        assert!(tools.validate_arguments("search_team_knowledge", &valid_args).is_ok());

        // Invalid search_team_knowledge args
        let invalid_args = json!({
            "limit": 5
            // Missing query
        });
        assert!(tools.validate_arguments("search_team_knowledge", &invalid_args).is_err());
    }
}