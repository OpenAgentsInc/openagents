//! Browser tool for web content fetching and search
//!
//! Inspired by GPT-OSS's SimpleBrowserTool, implemented in native Rust.

use async_trait::async_trait;
use scraper::{Html, Selector};
use serde::Deserialize;

const SEARCH_ENDPOINT: &str = "https://duckduckgo.com/html/";
const MAX_SEARCH_RESULTS: usize = 5;

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
        let mut url = reqwest::Url::parse(SEARCH_ENDPOINT).map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Invalid search URL: {}", e))
        })?;
        url.query_pairs_mut().append_pair("q", query);

        let response = self.client.get(url).send().await.map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Search request failed: {}", e))
        })?;

        let body = response.text().await.map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Failed to read search response: {}", e))
        })?;

        let results = parse_duckduckgo_html(&body);
        if results.is_empty() {
            return Ok(format!("No results found for query: {}", query));
        }

        Ok(format!(
            "Search results for: {}\n{}",
            query,
            format_search_results(&results)
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: Option<String>,
}

fn parse_duckduckgo_html(html: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let result_selector = Selector::parse(".result").ok();
    let link_selector = Selector::parse(".result__a").ok();
    let snippet_selector = Selector::parse(".result__snippet").ok();

    let mut results = Vec::new();

    if let (Some(result_selector), Some(link_selector)) =
        (result_selector.as_ref(), link_selector.as_ref())
    {
        for result in document.select(result_selector) {
            if let Some(link) = result.select(link_selector).next() {
                let title = link.text().collect::<Vec<_>>().join("").trim().to_string();
                let url = link.value().attr("href").unwrap_or("").to_string();
                if title.is_empty() || url.is_empty() {
                    continue;
                }

                let snippet = snippet_selector
                    .as_ref()
                    .and_then(|selector| result.select(selector).next())
                    .map(|node| node.text().collect::<Vec<_>>().join("").trim().to_string())
                    .filter(|text| !text.is_empty());

                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                });
                if results.len() >= MAX_SEARCH_RESULTS {
                    break;
                }
            }
        }
    }

    if results.is_empty()
        && let Some(link_selector) = link_selector.as_ref()
    {
        for link in document.select(link_selector).take(MAX_SEARCH_RESULTS) {
            let title = link.text().collect::<Vec<_>>().join("").trim().to_string();
            let url = link.value().attr("href").unwrap_or("").to_string();
            if title.is_empty() || url.is_empty() {
                continue;
            }
            results.push(SearchResult {
                title,
                url,
                snippet: None,
            });
        }
    }

    results
}

fn format_search_results(results: &[SearchResult]) -> String {
    let mut lines = Vec::new();
    for (idx, result) in results.iter().enumerate() {
        lines.push(format!("{}. {}", idx + 1, result.title));
        lines.push(format!("   {}", result.url));
        if let Some(snippet) = &result.snippet {
            lines.push(format!("   {}", snippet));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duckduckgo_html() {
        let html = r#"
            <div class="results">
                <div class="result">
                    <a class="result__a" href="https://example.com/one">Example One</a>
                    <a class="result__snippet">First result snippet.</a>
                </div>
                <div class="result">
                    <a class="result__a" href="https://example.com/two">Example Two</a>
                    <a class="result__snippet">Second result snippet.</a>
                </div>
            </div>
        "#;

        let results = parse_duckduckgo_html(html);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Example One");
        assert_eq!(results[0].url, "https://example.com/one");
        assert_eq!(results[0].snippet.as_deref(), Some("First result snippet."));
    }

    #[test]
    fn test_format_search_results() {
        let results = vec![
            SearchResult {
                title: "Result One".to_string(),
                url: "https://example.com/one".to_string(),
                snippet: Some("Snippet one.".to_string()),
            },
            SearchResult {
                title: "Result Two".to_string(),
                url: "https://example.com/two".to_string(),
                snippet: None,
            },
        ];

        let formatted = format_search_results(&results);
        assert!(formatted.contains("1. Result One"));
        assert!(formatted.contains("https://example.com/one"));
        assert!(formatted.contains("Snippet one."));
        assert!(formatted.contains("2. Result Two"));
    }
}
