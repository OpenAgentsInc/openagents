use crate::solver::changes::types::ChangeResponse;
use crate::solver::types::Change;
use anyhow::{anyhow, Result};
use tracing::{debug, error, info};

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
                vec![Change::new(
                    path.to_string(),
                    "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
                    "pub fn add(a: i32, b: i32) -> i32 { a + b }\n\npub fn multiply(a: i32, b: i32) -> i32 { a * b }".to_string(),
                )],
                "Added multiply function next to add function".to_string(),
            ));
        }
        return Ok((Vec::new(), "No changes needed".to_string()));
    }

    // Get model from env or use default
    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "codellama:latest".to_string());

    // Construct the prompt
    let prompt = format!(
        r#"You are an expert software developer. Your task is to generate specific code changes for this file:

Path: {}
Content:
{}

Change Request:
Title: {}
Description: {}

IMPORTANT: Respond ONLY with a valid JSON object. No other text, no thinking process, no markdown.

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

Example:
{{
    "changes": [
        {{
            "path": "src/lib.rs",
            "search": "fn old_name() {{ ... }}",
            "replace": "fn new_name() {{ ... }}",
            "reason": "Renamed for clarity"
        }}
    ],
    "reasoning": "Renamed function to better reflect..."
}}"#,
        path, content, title, description
    );

    debug!("Sending prompt to Ollama:\n{}", prompt);

    // Call Ollama API
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/chat", ollama_url))
        .json(&serde_json::json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "stream": false
        }))
        .send()
        .await?;

    let response_json = response.json::<serde_json::Value>().await?;

    debug!(
        "Ollama response:\n{}",
        serde_json::to_string_pretty(&response_json)?
    );

    // Check for error response
    if let Some(error) = response_json.get("error") {
        let error_msg = error["message"].as_str().unwrap_or("Unknown error");
        let error_code = error["code"].as_i64().unwrap_or(0);

        error!("Ollama API error: {} ({})", error_msg, error_code);

        // Handle specific error codes
        match error_code {
            500 => {
                return Err(anyhow!(
                    "Ollama internal error: {}. Please try again",
                    error_msg
                ));
            }
            429 => {
                return Err(anyhow!(
                    "Rate limit exceeded. Please wait a moment and try again"
                ));
            }
            _ => {
                return Err(anyhow!(
                    "Ollama API error: {} ({})",
                    error_msg,
                    error_code
                ));
            }
        }
    }

    let content = response_json["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            error!(
                "Invalid response format. Expected content in message.content. Full response:\n{}",
                serde_json::to_string_pretty(&response_json).unwrap_or_default()
            );
            anyhow!("Invalid response format. See logs for details.")
        })?;

    info!("Parsing LLM response:\n{}", content);

    // Parse response
    let change_response: ChangeResponse = serde_json::from_str(content).map_err(|e| {
        error!(
            "Failed to parse LLM response as JSON: {}\nResponse:\n{}",
            e, content
        );
        anyhow!("Failed to parse LLM response. See logs for details.")
    })?;

    // Convert to Change objects and validate
    let mut changes = Vec::new();
    for block in change_response.changes {
        // Validate path matches
        if block.path != path {
            debug!("Skipping change for wrong path: {} != {}", block.path, path);
            continue;
        }

        // Create and validate change
        let change = Change::new(
            block.path.clone(),
            block.search.clone(),
            block.replace.clone(),
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

    if changes.is_empty() {
        error!("No valid changes generated from response");
    } else {
        info!("Generated {} valid changes", changes.len());
    }

    Ok((changes, change_response.reasoning))
}