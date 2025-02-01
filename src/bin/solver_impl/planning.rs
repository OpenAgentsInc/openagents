use anyhow::{anyhow, Result};
use openagents::solver::{planning::PlanningContext, streaming::handle_plan_stream};
use regex::Regex;
use tracing::{debug, error, info};

// File paths
const GITHUB_RS_PATH: &str = "src/solver/github.rs";
const JSON_RS_PATH: &str = "src/solver/json.rs";

// JSON keys
const KEY_CHANGES: &str = "changes";
const KEY_PATH: &str = "path";
const KEY_SEARCH: &str = "search";
const KEY_REPLACE: &str = "replace";
const KEY_REASON: &str = "reason";

// Log messages
const LOG_GATHERING: &str = "Gathering relevant file contents...";
const LOG_GENERATING: &str = "Generating implementation plan...";
const LOG_FILE_CONTEXT: &str = "File context: {}";
const LOG_FULL_RESPONSE: &str = "Full response: {}";
const LOG_EXTRACTED_JSON: &str = "Extracted JSON: {}";
const LOG_RETRY_ATTEMPT: &str = "Retry attempt {} of {}";
const LOG_VALIDATION_ERROR: &str = "Validation error: {}";

// Function content
const GITHUB_RS_CONTENT: &str = r###"async fn generate_pr_title(&self, issue_number: i32, context: &str) -> Result<String> {
    let prompt = format!(
        r#"Generate a concise, descriptive pull request title for issue #{} based on this context:

{}

Requirements:
1. Must start with "feat:", "fix:", "refactor:", etc.
2. Must be descriptive but succinct
3. Must not exceed 72 characters
4. Must not use "Implement solution for"
5. Must clearly state what the PR does

Example good titles:
- "feat: add multiply function to calculator"
- "fix: handle JSON escaping in PR titles"
- "refactor: improve PR title generation"

Example bad titles:
- "Implement solution for #123"
- "Add function"
- "Fix issue"

Generate title:"#,
        issue_number, context
    );

    let (response, _) = self.llm_service.chat(prompt, true).await?;

    let title = response.trim();
    
    // Validate title
    if title.len() < 10 || title.len() > 72 {
        error!("Generated title has invalid length: {}", title.len());
        return Err(anyhow!("Generated title has invalid length"));
    }

    if !title.contains(':') {
        error!("Generated title missing prefix: {}", title);
        return Err(anyhow!("Generated title must start with feat:, fix:, etc."));
    }

    let prefix = title.split(':').next().unwrap();
    if !["feat", "fix", "refactor", "docs", "test", "chore"].contains(&prefix) {
        error!("Generated title has invalid prefix: {}", prefix);
        return Err(anyhow!("Generated title has invalid prefix"));
    }

    debug!("Generated PR title: {}", title);
    Ok(title.to_string())
}"###;

// Other constants
const TARGET_FUNCTION: &str = "generate_pr_title";

/// Extracts JSON from markdown code block
fn extract_json_from_markdown(content: &str) -> Option<&str> {
    let re = Regex::new(r"```json\s*(\{[\s\S]*?\})\s*```").ok()?;
    re.captures(content)?.get(1).map(|m| m.as_str())
}

/// Fixes common JSON formatting issues
fn fix_common_json_issues(json_str: &str) -> String {
    json_str
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
        .replace("format!=", "format!")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_json_from_markdown() {
        let markdown = r#"Some text
```json
{"key": "value"}
```
More text"#;
        let json = extract_json_from_markdown(markdown).unwrap();
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_fix_common_json_issues() {
        let input = r#"{"key": "value\\n with format!= macro"}"#;
        let fixed = fix_common_json_issues(input);
        assert!(fixed.contains("format!"));
        assert!(!fixed.contains("format!="));
    }
}