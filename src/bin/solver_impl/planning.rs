use anyhow::{Result};
use openagents::solver::{planning::PlanningContext, streaming::handle_plan_stream};
use regex::Regex;
use futures_util::StreamExt;
use tracing::{debug, info};
use serde_json::Value;

// Error messages
const ERR_NO_CHANGES: &str = "Changes array not found";
const ERR_WRONG_TARGET: &str = "Incorrect target file";
const ERR_INVALID_JSON: &str = "Invalid JSON format";
const ERR_INVALID_CHANGES: &str = "Changes do not match issue intent";

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
}

/// Creates a properly escaped JSON change object
fn create_json_change(path: &str, search: &str, replace: &str, reason: &str) -> Value {
    serde_json::json!({
        KEY_PATH: path,
        KEY_SEARCH: escape_json_string(search),
        KEY_REPLACE: escape_json_string(replace),
        KEY_REASON: reason,
    })
}

/// Extracts JSON from markdown code block
fn extract_json_from_markdown(content: &str) -> Option<&str> {
    let re = Regex::new(r"```json\s*(\{[\s\S]*?\})\s*```").ok()?;
    re.captures(content)?.get(1).map(|m| m.as_str())
}

/// Validates JSON content from LLM response
fn validate_llm_response(json_str: &str) -> Result<bool> {
    let json: Value = serde_json::from_str(json_str)?;
    
    // Check for required fields
    if !json.is_object() || !json.get(KEY_CHANGES).is_some() {
        debug!(LOG_VALIDATION_ERROR, "Missing required fields");
        return Ok(false);
    }

    // Validate each change
    if let Some(changes) = json[KEY_CHANGES].as_array() {
        for change in changes {
            if !change.get(KEY_PATH).is_some() || 
               !change.get(KEY_SEARCH).is_some() ||
               !change.get(KEY_REPLACE).is_some() {
                debug!(LOG_VALIDATION_ERROR, "Change missing required fields");
                return Ok(false);
            }
        }
    }

    Ok(true)
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
    
    let plan_stream = context
        .generate_plan(issue_number, title, description, repo_map, &file_context)
        .await?;

    let mut response = String::new();
    let mut chunks = handle_plan_stream(plan_stream).await?;
    while let Some(chunk) = chunks.next().await {
        response.push_str(&chunk?);
    }

    if let Some(json_str) = extract_json_from_markdown(&response) {
        debug!(LOG_EXTRACTED_JSON, json_str);
        if validate_llm_response(json_str)? {
            return Ok(response);
        }
    }

    debug!(LOG_FULL_RESPONSE, response);
    Ok(response)
}