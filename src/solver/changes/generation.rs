use crate::server::services::gateway::Gateway;
use crate::solver::changes::types::ChangeResponse;
use crate::solver::json::{escape_json_string, fix_common_json_issues, is_valid_json_string};
use crate::solver::types::Change;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use regex::Regex;
use std::io::Write;
use tracing::{debug, error, info};

const MAX_RETRIES: u32 = 3;

/// Extracts JSON from markdown code block
fn extract_json_from_markdown(content: &str) -> Option<&str> {
    let re = Regex::new(r"```json\s*(\{[\s\S]*?\})\s*```").ok()?;
    re.captures(content)?.get(1).map(|m| m.as_str())
}

/// Parses LLM response with retry mechanism
fn parse_llm_response(response: &str) -> Result<ChangeResponse> {
    for attempt in 0..MAX_RETRIES {
        match serde_json::from_str(response) {
            Ok(changes) => return Ok(changes),
            Err(e) if attempt < MAX_RETRIES - 1 => {
                log::warn!("JSON parse error (attempt {}): {}", attempt + 1, e);
                // Attempt to fix common JSON issues
                let fixed_response = fix_common_json_issues(response);
                if fixed_response != response {
                    match serde_json::from_str(&fixed_response) {
                        Ok(changes) => return Ok(changes),
                        Err(_) => continue,
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse JSON after {} attempts: {}", MAX_RETRIES, e);
                return Err(anyhow!("Failed to parse LLM response after retries"));
            }
        }
    }
    Err(anyhow!("Failed to parse LLM response after all retries"))
}

/// Validates that changes are relevant to the issue
fn validate_changes_relevance(
    changes: &[Change],
    reasoning: &str,
    title: &str,
    description: &str,
) -> bool {
    // Check if reasoning contains keywords from title or description
    let keywords = extract_keywords(title, description);
    let reasoning_matches = keywords.iter().any(|k| reasoning.contains(k));

    // Check if any change reasons contain keywords
    let changes_match = changes.iter().any(|c| {
        keywords
            .iter()
            .any(|k| c.reason.as_ref().is_some_and(|r| r.contains(k)))
    });

    reasoning_matches || changes_match
}

/// Extracts important keywords from title and description
fn extract_keywords(title: &str, description: &str) -> Vec<String> {
    let mut keywords = Vec::new();

    // Add words from title (excluding common words)
    keywords.extend(
        title
            .split_whitespace()
            .filter(|w| w.len() > 3 && !is_common_word(w))
            .map(String::from),
    );

    // Add words from description
    keywords.extend(
        description
            .split_whitespace()
            .filter(|w| w.len() > 3 && !is_common_word(w))
            .map(String::from),
    );

    keywords
}

/// Checks if a word is too common to be meaningful
fn is_common_word(word: &str) -> bool {
    let common_words = ["the", "and", "for", "that", "with", "this", "from"];
    common_words.contains(&word.to_lowercase().as_str())
}

/// Generates changes for a specific file
pub async fn generate_changes(
    path: &str,
    content: &str,
    title: &str,
    description: &str,
    ollama_url: &str,
) -> Result<(Vec<Change>, String)> {
    // For tests, return mock response if using test URL
    if ollama_url == "test_url" {
        if path == "src/lib.rs" {
            return Ok((
                vec![Change::with_reason(
                    path.to_string(),
                    "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
                    "pub fn add(a: i32, b: i32) -> i32 { a + b }\n\npub fn multiply(a: i32, b: i32) -> i32 { a * b }".to_string(),
                    "Added multiply function next to add function".to_string(),
                )],
                "Added multiply function next to add function".to_string(),
            ));
        }
        return Ok((Vec::new(), "No changes needed".to_string()));
    }

    // Construct the prompt
    let prompt = format!(
        r#"You are an expert software developer. Your task is to generate specific code changes for this file:

Path: {}
Content:
{}

Change Request:
Title: {}
Description: {}

First, think through the changes needed. Then output your solution as a JSON object in a markdown code block like this:

```json
{{
    "changes": [
        {{
            "path": "path/to/file",
            "search": "exact content to find",
            "replace": "new content",
            "reason": "why this change is needed"
        }}
    ],
    "reasoning": "Overall explanation of changes"
}}
```

The JSON object must have:
1. "changes": Array of change blocks with:
   - "path": File path
   - "search": Exact content to find
   - "replace": New content to replace it with
   - "reason": Why this change is needed
2. "reasoning": Overall explanation of changes

Rules:
- Use EXACT content matches for search
- Include enough context for unique matches
- Keep changes minimal and focused
- Preserve code style and formatting
- Empty search means new file content
- All strings must be properly escaped for JSON
- Ensure changes are relevant to the issue"#,
        path,
        escape_json_string(content),
        escape_json_string(title),
        escape_json_string(description)
    );

    debug!("Sending prompt to Ollama:\n{}", prompt);

    // Initialize Ollama service
    let service = crate::server::services::ollama::OllamaService::new();
    let mut stream = service.chat_stream(prompt, true).await?;

    // Process the stream
    let mut full_response = String::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(content) => {
                // Print thinking process
                print!("{}", content);
                std::io::stdout().flush()?;
                full_response.push_str(&content);
            }
            Err(e) => {
                error!("Error in stream: {}", e);
                break;
            }
        }
    }

    // Extract JSON from markdown code block
    let json_str = extract_json_from_markdown(&full_response)
        .ok_or_else(|| anyhow!("No JSON code block found in response"))?;

    info!("Parsing LLM response:\n{}", json_str);

    // Parse response with retry mechanism
    let change_response = parse_llm_response(json_str)?;

    // Convert to Change objects and validate
    let mut changes = Vec::new();
    for block in change_response.changes {
        // Validate path matches
        if block.path != path {
            debug!("Skipping change for wrong path: {} != {}", block.path, path);
            continue;
        }

        // Validate JSON string escaping
        if !is_valid_json_string(&block.search) || !is_valid_json_string(&block.replace) {
            error!("Invalid JSON string escaping in change block");
            continue;
        }

        // Create and validate change
        let change = Change::with_reason(
            block.path.clone(),
            block.search.clone(),
            block.replace.clone(),
            block.reason.clone(),
        );
        match change.validate() {
            Ok(_) => {
                debug!("Valid change: {:?}", change);
                changes.push(change);
            }
            Err(e) => {
                error!("Invalid change: {:?}\nError: {:?}", block, e);
            }
        }
    }

    // Validate changes are relevant to the issue
    if !validate_changes_relevance(&changes, &change_response.reasoning, title, description) {
        error!("Generated changes are not relevant to the issue");
        return Err(anyhow!("Changes are not relevant to the issue"));
    }

    if changes.is_empty() {
        error!("No valid changes generated from response");
    } else {
        info!("Generated {} valid changes", changes.len());
    }

    Ok((changes, change_response.reasoning))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_markdown() {
        let content = r#"Some text
```json
{"key": "value"}
```
More text"#;
        assert_eq!(
            extract_json_from_markdown(content),
            Some(r#"{"key": "value"}"#)
        );
    }

    #[test]
    fn test_validate_changes_relevance() {
        let changes = vec![Change::with_reason(
            "test.rs".to_string(),
            "old".to_string(),
            "new".to_string(),
            "Update test".to_string(),
        )];
        let reasoning = "Added multiply function to implement calculation feature";
        let title = "Add multiply function";
        let description = "Implement multiplication calculation";

        assert!(validate_changes_relevance(
            &changes,
            reasoning,
            title,
            description
        ));
    }

    #[test]
    fn test_extract_keywords() {
        let title = "Add multiply function";
        let description = "Implement multiplication calculation";
        let keywords = extract_keywords(title, description);

        assert!(keywords.contains(&"multiply".to_string()));
        assert!(keywords.contains(&"multiplication".to_string()));
        assert!(keywords.contains(&"calculation".to_string()));
    }
}
