use anyhow::{anyhow, Result};
use openagents::solver::{planning::PlanningContext, streaming::handle_plan_stream};
use regex::Regex;
use serde_json::json;
use tracing::{debug, error, info};

// Error messages
const ERR_NO_CHANGES: &str = "Changes array not found";
const ERR_WRONG_TARGET: &str = "Incorrect target file";
const ERR_INVALID_JSON: &str = "Invalid JSON format";
const ERR_INVALID_CHANGES: &str = "Changes do not match issue intent";

// File paths
const GITHUB_RS_PATH: &str = "src/solver/github.rs";

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
const LOG_RETRY_ATTEMPT: &str = "Retry attempt {current} of {max}";
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
const EMPTY_STR: &str = "";
const MAX_RETRIES: u32 = 3;

/// Escapes special characters in JSON strings
fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('\u{0000}', "\\u0000")
        .replace('\u{001F}', "\\u001F")
}

/// Extracts JSON from markdown code block
fn extract_json_from_markdown(content: &str) -> Option<&str> {
    let re = Regex::new(r"```json\s*(\{[\s\S]*?\})\s*```").ok()?;
    re.captures(content)?.get(1).map(|m| m.as_str())
}

/// Validates that changes match the issue intent
fn validate_changes_relevance(changes: &[serde_json::Value], issue_title: &str) -> bool {
    // Extract keywords from issue title
    let keywords: Vec<&str> = issue_title
        .split_whitespace()
        .filter(|&word| !is_common_word(word))
        .collect();

    // Check if changes reasoning contains keywords
    changes.iter().any(|change| {
        let reason = change[KEY_REASON].as_str().unwrap_or(EMPTY_STR);
        keywords.iter().any(|&keyword| reason.contains(keyword))
    })
}

/// Checks if a word is too common to be meaningful
fn is_common_word(word: &str) -> bool {
    let common_words = ["the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for"];
    common_words.contains(&word.to_lowercase().as_str())
}

/// Gathers relevant file contents for context
async fn gather_context(issue_title: &str, issue_description: &str) -> Result<String> {
    let mut context = String::new();
    
    // Add explicit file content
    context.push_str("Current generate_pr_title implementation:\n");
    context.push_str(GITHUB_RS_CONTENT);
    
    // Add clear instructions
    context.push_str("\nRequirements:\n");
    context.push_str("1. Modify only the existing generate_pr_title function\n");
    context.push_str("2. Use the existing llm_service.chat call\n");
    context.push_str("3. Keep the current error handling\n");
    
    // Add file-specific context based on issue content
    if issue_title.contains("JSON") || issue_description.contains("JSON") {
        context.push_str("\nJSON handling requirements:\n");
        context.push_str("1. All strings must be properly escaped\n");
        context.push_str("2. Use proper JSON formatting\n");
        context.push_str("3. Handle control characters correctly\n");
    }

    Ok(context)
}

/// Fixes common JSON formatting issues
fn fix_common_json_issues(json_str: &str) -> String {
    json_str
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
        .replace("format!=", "format!")
}

/// Creates a properly escaped JSON change object
fn create_json_change(path: &str, search: &str, replace: &str, reason: &str) -> serde_json::Value {
    json!({
        KEY_PATH: path,
        KEY_SEARCH: escape_json_string(search),
        KEY_REPLACE: escape_json_string(replace),
        KEY_REASON: reason,
    })
}

/// Validates the LLM response format and content
fn validate_llm_response(json_str: &str, issue_title: &str) -> Result<bool> {
    // Parse JSON
    let json: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| anyhow!("{}: {}", ERR_INVALID_JSON, e))?;

    // Check for changes array
    let changes = json[KEY_CHANGES].as_array()
        .ok_or_else(|| anyhow!(ERR_NO_CHANGES))?;

    // Validate each change
    for change in changes {
        let path = change[KEY_PATH].as_str().unwrap_or(EMPTY_STR);
        let search = change[KEY_SEARCH].as_str().unwrap_or(EMPTY_STR);
        let replace = change[KEY_REPLACE].as_str().unwrap_or(EMPTY_STR);

        // Verify target file and function
        if path != GITHUB_RS_PATH || !search.contains(TARGET_FUNCTION) {
            error!(LOG_VALIDATION_ERROR, ERR_WRONG_TARGET);
            return Ok(false);
        }

        // Verify replace content is valid
        if replace.is_empty() || !replace.contains("llm_service.chat") {
            error!(LOG_VALIDATION_ERROR, "Replace content must use llm_service.chat");
            return Ok(false);
        }
    }

    // Validate changes match issue intent
    if !validate_changes_relevance(changes, issue_title) {
        error!(LOG_VALIDATION_ERROR, ERR_INVALID_CHANGES);
        return Ok(false);
    }

    Ok(true)
}

/// Retries LLM response generation with feedback
async fn retry_with_feedback(
    context: &PlanningContext,
    issue_number: i32,
    title: &str,
    description: &str,
    repo_map: &str,
    file_context: &str,
) -> Result<String> {
    for attempt in 0..MAX_RETRIES {
        info!(LOG_RETRY_ATTEMPT, current = attempt + 1, max = MAX_RETRIES);

        let stream = context
            .generate_plan(issue_number, title, description, repo_map, file_context)
            .await?;

        let response = handle_plan_stream(stream).await?;
        debug!(LOG_FULL_RESPONSE, response);

        if let Some(json_str) = extract_json_from_markdown(&response) {
            let fixed_json = fix_common_json_issues(json_str);
            if validate_llm_response(&fixed_json, title)? {
                // Create properly escaped JSON
                let json: serde_json::Value = serde_json::from_str(&fixed_json)?;
                let changes = json[KEY_CHANGES].as_array().unwrap();
                
                let escaped_changes: Vec<serde_json::Value> = changes
                    .iter()
                    .map(|c| create_json_change(
                        c[KEY_PATH].as_str().unwrap_or(EMPTY_STR),
                        c[KEY_SEARCH].as_str().unwrap_or(EMPTY_STR),
                        c[KEY_REPLACE].as_str().unwrap_or(EMPTY_STR),
                        c[KEY_REASON].as_str().unwrap_or(EMPTY_STR),
                    ))
                    .collect();

                let result = json!({
                    KEY_CHANGES: escaped_changes,
                    "reasoning": json["reasoning"],
                });

                return Ok(result.to_string());
            }
        }
    }

    Err(anyhow!("Failed to generate valid response after {} attempts", MAX_RETRIES))
}

pub async fn handle_planning(
    issue_number: i32,
    title: &str,
    description: &str,
    repo_map: &str,
    ollama_url: &str,
) -> Result<String> {
    info!(LOG_GATHERING);
    let file_context = gather_context(title, description).await?;
    debug!(LOG_FILE_CONTEXT, file_context);

    info!(LOG_GENERATING);
    let context = PlanningContext::new(ollama_url)?;
    
    // Use retry mechanism with feedback
    let json_str = retry_with_feedback(
        &context,
        issue_number,
        title,
        description,
        repo_map,
        &file_context,
    ).await?;

    debug!(LOG_EXTRACTED_JSON, json_str);
    Ok(json_str)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_validate_changes_relevance() {
        let changes = json!([{
            "path": GITHUB_RS_PATH,
            "search": "generate_pr_title",
            "replace": "new code",
            "reason": "Improve PR title generation"
        }]);
        
        assert!(validate_changes_relevance(
            changes.as_array().unwrap(),
            "Improve PR title generation"
        ));
    }

    #[test]
    fn test_fix_common_json_issues() {
        let input = r#"{"key": "value\\n with format!= macro"}"#;
        let fixed = fix_common_json_issues(input);
        assert!(fixed.contains("format!"));
        assert!(!fixed.contains("format!="));
    }

    #[test]
    fn test_escape_json_string() {
        let input = "line1\nline2\t\"quoted\"";
        let escaped = escape_json_string(input);
        assert!(escaped.contains("\\n"));
        assert!(escaped.contains("\\t"));
        assert!(escaped.contains("\\\""));
    }

    #[test]
    fn test_create_json_change() {
        let change = create_json_change(
            "test.rs",
            "old\ncode",
            "new\ncode",
            "test reason"
        );
        assert!(change[KEY_SEARCH].as_str().unwrap().contains("\\n"));
        assert!(change[KEY_REPLACE].as_str().unwrap().contains("\\n"));
    }
}