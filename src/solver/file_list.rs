use crate::server::services::gateway::Gateway;
use anyhow::{anyhow, Result};
use std::path::Path;

/// Generate a list of files that need to be modified based on the issue
pub async fn generate_file_list(
    gateway: &(impl Gateway + ?Sized),
    issue_title: &str,
    issue_body: &str,
    repo_map: &str,
    temp_dir: &Path,
) -> Result<Vec<String>> {
    let prompt = format!(
        r#"You are an expert software developer tasked with implementing a solution for a GitHub issue.
Based on the issue details and repository structure, list the files that need to be modified.

Issue Title: {}
Issue Description: {}

Repository Structure:
{}

Output a valid JSON array of file paths that need to be modified to implement this solution.
Only include files that actually exist in the repository.
Format: ["path/to/file1", "path/to/file2", ...]"#,
        issue_title, issue_body, repo_map
    );

    let (response, _) = gateway.chat(prompt, false).await?;
    
    // Extract JSON array from response
    let json_str = response
        .lines()
        .find(|line| line.trim().starts_with('['))
        .ok_or_else(|| anyhow!("No JSON array found in response"))?;

    let files: Vec<String> = serde_json::from_str(json_str)
        .map_err(|e| anyhow!("Failed to parse file list: {}", e))?;

    // Validate all files exist
    for file in &files {
        let file_path = temp_dir.join(file);
        if !file_path.exists() {
            return Err(anyhow!("Listed file does not exist: {}", file));
        }
    }

    Ok(files)
}