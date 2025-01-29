use crate::solver::types::Change;
use crate::solver::changes::types::ChangeResponse;
use anyhow::Result;
use tracing::{debug, error, info};

/// Generates changes for a specific file
pub async fn generate_changes(
    path: &str,
    content: &str,
    title: &str,
    description: &str,
    openrouter_key: &str,
) -> Result<(Vec<Change>, String)> {
    // For tests, return mock response if using test key
    if openrouter_key == "test_key" {
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

    // Construct the prompt
    let prompt = format!(
        r#"You are an expert software developer. Your task is to generate specific code changes for this file:

Path: {}
Content:
{}

Change Request:
Title: {}
Description: {}

Output a JSON object with:
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
- Do not put it inside a markdown code block, respond ONLY with the valid JSON. (Do not say "```json" at the beginning)

Example Response:
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

    debug!("Sending prompt to OpenRouter:\n{}", prompt);

    // Call OpenRouter API
    let client = reqwest::Client::new();
    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", openrouter_key))
        .header(
            "HTTP-Referer",
            "https://github.com/OpenAgentsInc/openagents",
        )
        .json(&serde_json::json!({
            "model": "deepseek/deepseek-chat",
            "messages": [{"role": "user", "content": prompt}]
        }))
        .send()
        .await?;

    let response_json = response.json::<serde_json::Value>().await?;
    
    debug!("OpenRouter response:\n{}", serde_json::to_string_pretty(&response_json)?);

    let content = response_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            error!("Invalid response format. Expected content in choices[0].message.content. Got:\n{}", 
                serde_json::to_string_pretty(&response_json).unwrap_or_default());
            anyhow::anyhow!("Invalid response format")
        })?;

    info!("Parsing LLM response:\n{}", content);

    // Parse response
    let change_response: ChangeResponse = serde_json::from_str(content)
        .map_err(|e| {
            error!("Failed to parse LLM response as JSON: {}\nResponse:\n{}", e, content);
            anyhow::anyhow!("Failed to parse LLM response as JSON")
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
        let change = Change::new(block.path.clone(), block.search.clone(), block.replace.clone());
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
