use crate::solver::changes::types::ChangeResponse;
use crate::solver::types::Change;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use std::io::Write;
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

First, think through the changes needed. Then output a JSON object with:
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

    // Initialize Ollama service
    let service = crate::server::services::ollama::OllamaService::new();
    let mut stream = service.chat_stream(prompt, true).await?;

    // Process the stream
    let mut json_response = None;
    while let Some(result) = stream.next().await {
        match result {
            Ok(content) => {
                // Check if this is a JSON response
                if content.starts_with("\n<json>") && content.ends_with("</json>") {
                    let json_str = &content[6..content.len()-7]; // Remove <json> tags
                    json_response = Some(json_str.to_string());
                } else {
                    // Print thinking process
                    print!("{}", content);
                    std::io::stdout().flush()?;
                }
            }
            Err(e) => {
                error!("Error in stream: {}", e);
                break;
            }
        }
    }

    // Parse the JSON response
    let json_str = json_response.ok_or_else(|| anyhow!("No JSON response found in stream"))?;
    info!("Parsing LLM response:\n{}", json_str);

    let change_response: ChangeResponse = serde_json::from_str(&json_str).map_err(|e| {
        error!(
            "Failed to parse LLM response as JSON: {}\nResponse:\n{}",
            e, json_str
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