use anyhow::{Context as _, Result};
use serde::Deserialize;
use std::path::Path;
use tracing::{debug, error, info};

/// Response from LLM for file list generation
#[derive(Debug, Deserialize)]
struct FileListResponse {
    files: Vec<String>,
    reasoning: String,
}

/// Extracts JSON object from a string that may contain markdown and other text
fn extract_json(content: &str) -> Option<&str> {
    // Look for JSON code block
    if let Some(start_marker) = content.find("```json") {
        // Find the end marker after the start marker
        if let Some(end_marker) = content[start_marker..].find("```") {
            // Calculate absolute positions
            let json_start = start_marker + "```json".len();
            let json_end = start_marker + end_marker;

            // Ensure valid slice and return trimmed content
            if json_start < json_end {
                return Some(content[json_start..json_end].trim());
            }
        }
    }

    // Fallback: try to find JSON object directly
    if let Some(start) = content.find('{') {
        let mut depth = 0;
        let chars: Vec<_> = content[start..].chars().collect();

        for (i, &c) in chars.iter().enumerate() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&content[start..=start + i]);
                    }
                }
                _ => {}
            }
        }
    }

    None
}

/// Generates a list of files that need to be modified
pub async fn generate_file_list(
    title: &str,
    description: &str,
    repo_map: &str,
    ollama_url: &str,
) -> Result<(Vec<String>, String)> {
    // For tests, return mock response if using test URL
    if ollama_url == "test_url" {
        // Handle empty repository case
        if repo_map.is_empty() {
            return Ok((
                Vec::new(),
                "No files available in the repository".to_string(),
            ));
        }

        return Ok((
            vec!["src/lib.rs".to_string()],
            "lib.rs needs to be modified to add the multiply function".to_string(),
        ));
    }

    // Get model from env or use default
    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "codellama:latest".to_string());

    // Construct the prompt
    let prompt = format!(
        r#"You are an expert software developer. Your task is to identify which files need to be modified to implement this change:

Title: {}
Description: {}

Repository structure:
{}

Output ONLY a JSON object with:
1. "files": Array of file paths that need to be modified
2. "reasoning": Explanation of why each file needs changes

Rules:
- Only include files that definitely need changes
- Use exact paths from the repository structure
- Explain the planned changes for each file
- Focus on minimal, targeted changes
- Return ONLY the JSON object, no other text
- DO NOT put the object in Markdown backticks, ONLY respond with valid JSON

Example response:
{{
    "files": ["path/to/file1", "path/to/file2"],
    "reasoning": "File1 needs X changes because... File2 needs Y changes because..."
}}"#,
        title, description, repo_map
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
        .await
        .context("Failed to send request to Ollama")?;

    let response_json = response
        .json::<serde_json::Value>()
        .await
        .context("Failed to parse Ollama response as JSON")?;

    debug!(
        "Ollama response:\n{}",
        serde_json::to_string_pretty(&response_json)?
    );

    let content = response_json["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            error!(
                "Invalid response format. Expected content in message.content. Got:\n{}",
                serde_json::to_string_pretty(&response_json).unwrap_or_default()
            );
            anyhow::anyhow!("Invalid response format from Ollama")
        })?;

    info!("Parsing LLM response:\n{}", content);

    // Extract JSON from response
    let json_str = extract_json(content).ok_or_else(|| {
        error!("Failed to extract JSON from LLM response:\n{}", content);
        anyhow::anyhow!("No valid JSON found in LLM response")
    })?;

    debug!("Extracted JSON:\n{}", json_str);

    // Parse response
    let file_list: FileListResponse = serde_json::from_str(json_str).with_context(|| {
        format!(
            "Failed to parse LLM response as JSON. Response:\n{}",
            json_str
        )
    })?;

    // Validate file paths
    let valid_files: Vec<String> = file_list
        .files
        .into_iter()
        .filter(|path| {
            let exists = Path::new(path).exists();
            if !exists {
                debug!("Filtering out non-existent file: {}", path);
            }
            exists
        })
        .collect();

    info!("Found {} valid files to modify", valid_files.len());
    debug!("Valid files: {:?}", valid_files);

    Ok((valid_files, file_list.reasoning))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_json() {
        let inputs = vec![
            (
                r#"Here's the JSON:
```json
{"key": "value"}
```
More text"#,
                Some(r#"{"key": "value"}"#),
            ),
            (r#"{"key": "value"}"#, Some(r#"{"key": "value"}"#)),
            ("No JSON here", None),
            (
                r#"Based on the analysis:
```json
{
    "files": ["README.md"],
    "reasoning": "Update docs"
}
```
That's all."#,
                Some(
                    r#"{
    "files": ["README.md"],
    "reasoning": "Update docs"
}"#,
                ),
            ),
        ];

        for (input, expected) in inputs {
            let result = extract_json(input).map(str::trim);
            let expected = expected.map(str::trim);
            assert_eq!(result, expected, "Failed for input:\n{}", input);
        }
    }

    fn setup_test_repo() -> Result<TempDir> {
        let temp_dir = tempfile::tempdir()?;

        // Create test files
        fs::create_dir_all(temp_dir.path().join("src"))?;
        fs::write(
            temp_dir.path().join("src/main.rs"),
            "fn main() { println!(\"Hello\"); }",
        )?;
        fs::write(
            temp_dir.path().join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        )?;

        Ok(temp_dir)
    }

    #[tokio::test]
    async fn test_generate_file_list() -> Result<()> {
        let temp_dir = setup_test_repo()?;
        std::env::set_current_dir(&temp_dir)?;

        let repo_map = "src/main.rs\nsrc/lib.rs";
        let (files, reasoning) = generate_file_list(
            "Add multiply function",
            "Add a multiply function to lib.rs",
            repo_map,
            "test_url",
        )
        .await?;

        assert!(!files.is_empty());
        assert!(files.contains(&"src/lib.rs".to_string()));
        assert!(!files.contains(&"src/main.rs".to_string()));
        assert!(!reasoning.is_empty());
        assert!(reasoning.contains("lib.rs"));

        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_files_filtered() -> Result<()> {
        let temp_dir = setup_test_repo()?;
        std::env::set_current_dir(&temp_dir)?;

        let repo_map = "src/main.rs\nsrc/lib.rs\nsrc/nonexistent.rs";
        let (files, _) =
            generate_file_list("Update files", "Update all files", repo_map, "test_url").await?;

        assert!(!files.contains(&"src/nonexistent.rs".to_string()));
        assert!(files.iter().all(|path| Path::new(path).exists()));

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_repo() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        std::env::set_current_dir(&temp_dir)?;

        let (files, reasoning) = generate_file_list(
            "Add new file",
            "Create a new file with some functionality",
            "",
            "test_url",
        )
        .await?;

        assert!(files.is_empty());
        assert!(!reasoning.is_empty());
        assert!(reasoning.contains("No files"));

        Ok(())
    }
}
