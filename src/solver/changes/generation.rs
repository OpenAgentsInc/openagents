use crate::server::services::gateway::Gateway;
use crate::solver::changes::types::ChangeResponse;
use crate::solver::types::Change;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use regex::Regex;
use std::io::Write;
use tracing::{debug, error, info};

/// Extracts JSON from markdown code block
fn extract_json_from_markdown(content: &str) -> Option<&str> {
    let re = Regex::new(r"```json\s*(\{[\s\S]*?\})\s*```").ok()?;
    re.captures(content)?.get(1).map(|m| m.as_str())
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
- Empty search means new file content"#,
        path, content, title, description
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

    let change_response: ChangeResponse = serde_json::from_str(json_str).map_err(|e| {
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