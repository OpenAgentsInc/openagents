//! Browser tool for web content fetching and search
//!
//! Inspired by GPT-OSS's SimpleBrowserTool, implemented in native Rust.

use async_trait::async_trait;
use serde::Deserialize;

use super::{Tool, ToolResult};

/// Browser tool for fetching web content
pub struct BrowserTool {
    client: reqwest::Client,
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("gpt-oss-agent/0.1")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct BrowserParams {
    action: BrowserAction,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BrowserAction {
    Open { url: String },
    Search { query: String },
    Find { url: String, text: String },
}

#[async_trait]
impl Tool for BrowserTool {
    async fn execute(&self, params: serde_json::Value) -> crate::Result<ToolResult> {
        let params: BrowserParams = serde_json::from_value(params)?;

        let result = match params.action {
            BrowserAction::Open { url } => self.open_url(&url).await,
            BrowserAction::Search { query } => self.search(&query).await,
            BrowserAction::Find { url, text } => self.find_text(&url, &text).await,
        };

        match result {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }

    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Fetch web content, search the web, or find text on a page"
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "object",
                    "oneOf": [
                        {
                            "type": "object",
                            "properties": {
                                "type": { "const": "open" },
                                "url": { "type": "string" }
                            },
                            "required": ["type", "url"]
                        },
                        {
                            "type": "object",
                            "properties": {
                                "type": { "const": "search" },
                                "query": { "type": "string" }
                            },
                            "required": ["type", "query"]
                        },
                        {
                            "type": "object",
                            "properties": {
                                "type": { "const": "find" },
                                "url": { "type": "string" },
                                "text": { "type": "string" }
                            },
                            "required": ["type", "url", "text"]
                        }
                    ]
                }
            },
            "required": ["action"]
        })
    }
}

impl BrowserTool {
    async fn open_url(&self, url: &str) -> crate::Result<String> {
        let response = self.client.get(url).send().await.map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Failed to fetch URL: {}", e))
        })?;

        let text = response.text().await.map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Failed to read response: {}", e))
        })?;

        // Simple HTML stripping (in production, use html2text or similar)
        let stripped = strip_html_basic(&text);
        Ok(stripped)
    }

    async fn search(&self, query: &str) -> crate::Result<String> {
        // For now, return a message that search is not implemented
        // In production, this would integrate with a search API
        Ok(format!(
            "Search functionality not yet implemented. Query: {}",
            query
        ))
    }

    async fn find_text(&self, url: &str, text: &str) -> crate::Result<String> {
        let content = self.open_url(url).await?;

        if content.contains(text) {
            Ok(format!("Found text '{}' in {}", text, url))
        } else {
            Ok(format!("Text '{}' not found in {}", text, url))
        }
    }
}

/// Basic HTML tag stripping
fn strip_html_basic(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    result
}
