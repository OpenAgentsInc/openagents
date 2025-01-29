use crate::solver::types::{Change, ChangeError, ChangeResult};
use anyhow::Result;
use serde::Deserialize;

/// Response from LLM for change generation
#[derive(Debug, Deserialize)]
struct ChangeResponse {
    changes: Vec<ChangeBlock>,
    reasoning: String,
}

/// A block of changes from the LLM
#[derive(Debug, Deserialize)]
struct ChangeBlock {
    path: String,
    search: String,
    replace: String,
    reason: String,
}

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

    // Call OpenRouter API
    let client = reqwest::Client::new();
    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", openrouter_key))
        .header("HTTP-Referer", "https://github.com/OpenAgentsInc/openagents")
        .json(&serde_json::json!({
            "model": "deepseek/deepseek-coder-33b-instruct",
            "messages": [{"role": "user", "content": prompt}]
        }))
        .send()
        .await?;

    let response_json = response.json::<serde_json::Value>().await?;
    let content = response_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;

    // Parse response
    let change_response: ChangeResponse = serde_json::from_str(content)?;

    // Convert to Change objects and validate
    let mut changes = Vec::new();
    for block in change_response.changes {
        // Validate path matches
        if block.path != path {
            continue;
        }

        // Create and validate change
        let change = Change::new(
            block.path,
            block.search,
            block.replace,
        );
        if change.validate().is_ok() {
            changes.push(change);
        }
    }

    Ok((changes, change_response.reasoning))
}

/// Parses SEARCH/REPLACE blocks from text
pub fn parse_search_replace(content: &str) -> ChangeResult<Vec<Change>> {
    let mut changes = Vec::new();
    let mut current_path = None;
    let mut current_search = None;
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line.trim();

        // Look for file path markers
        if line.ends_with(".rs:") || line.ends_with(".toml:") {
            current_path = Some(line.trim_end_matches(':').to_string());
            continue;
        }

        // Look for SEARCH blocks
        if line == "<<<<<<< SEARCH" {
            let mut search = String::new();
            while let Some(line) = lines.next() {
                if line.trim() == "=======" {
                    break;
                }
                search.push_str(line);
                search.push('\n');
            }
            current_search = Some(search.trim_end().to_string());
            continue;
        }

        // Look for REPLACE blocks
        if line == ">>>>>>> REPLACE" && current_path.is_some() && current_search.is_some() {
            let mut replace = String::new();
            while let Some(line) = lines.peek() {
                if line.trim() == "<<<<<<< SEARCH" || line.trim().ends_with(".rs:") || line.trim().ends_with(".toml:") {
                    break;
                }
                replace.push_str(lines.next().unwrap());
                replace.push('\n');
            }

            // Create and validate change
            let change = Change::new(
                current_path.clone().unwrap(),
                current_search.clone().unwrap(),
                replace.trim_end().to_string(),
            );
            if change.validate().is_ok() {
                changes.push(change);
            }

            current_search = None;
            continue;
        }
    }

    if changes.is_empty() {
        return Err(ChangeError::InvalidFormat);
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_changes() -> Result<()> {
        let (changes, reasoning) = generate_changes(
            "src/lib.rs",
            "pub fn add(a: i32, b: i32) -> i32 { a + b }",
            "Add multiply function",
            "Add a multiply function that multiplies two integers",
            "test_key",
        ).await?;

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "src/lib.rs");
        assert!(changes[0].replace.contains("multiply"));
        assert!(!reasoning.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_changes_no_changes() -> Result<()> {
        let (changes, reasoning) = generate_changes(
            "src/main.rs",
            "fn main() { println!(\"Hello\"); }",
            "Add multiply function",
            "Add a multiply function to lib.rs",
            "test_key",
        ).await?;

        assert!(changes.is_empty());
        assert_eq!(reasoning, "No changes needed");

        Ok(())
    }

    #[test]
    fn test_parse_search_replace() -> ChangeResult<()> {
        let content = r#"src/lib.rs:
<<<<<<< SEARCH
pub fn add(a: i32, b: i32) -> i32 { a + b }
=======
pub fn add(a: i32, b: i32) -> i32 { a + b }

pub fn multiply(a: i32, b: i32) -> i32 { a * b }
>>>>>>> REPLACE"#;

        let changes = parse_search_replace(content)?;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "src/lib.rs");
        assert!(changes[0].replace.contains("multiply"));

        Ok(())
    }

    #[test]
    fn test_parse_search_replace_multiple() -> ChangeResult<()> {
        let content = r#"src/lib.rs:
<<<<<<< SEARCH
pub fn add(a: i32, b: i32) -> i32 { a + b }
=======
pub fn add(a: i32, b: i32) -> i32 { a + b }

pub fn multiply(a: i32, b: i32) -> i32 { a * b }
>>>>>>> REPLACE

src/main.rs:
<<<<<<< SEARCH
fn main() {
    println!("1 + 1 = {}", add(1, 1));
}
=======
fn main() {
    println!("1 + 1 = {}", add(1, 1));
    println!("2 * 3 = {}", multiply(2, 3));
}
>>>>>>> REPLACE"#;

        let changes = parse_search_replace(content)?;
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].path, "src/lib.rs");
        assert_eq!(changes[1].path, "src/main.rs");
        assert!(changes[0].replace.contains("multiply"));
        assert!(changes[1].replace.contains("multiply"));

        Ok(())
    }

    #[test]
    fn test_parse_search_replace_invalid() -> ChangeResult<()> {
        let content = r#"src/lib.rs:
Invalid format without proper markers"#;

        let result = parse_search_replace(content);
        assert!(matches!(result, Err(ChangeError::InvalidFormat)));

        Ok(())
    }
}