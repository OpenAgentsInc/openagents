use anyhow::{anyhow, Result};
use openagents::solver::{planning::PlanningContext, streaming::handle_plan_stream};
use regex::Regex;
use tracing::{debug, info};

const ERR_NO_JSON: &str = "JSON block not found";
const ERR_NO_CHANGES: &str = "Changes array not found";
const ERR_WRONG_TARGET: &str = "Incorrect target file";
const GITHUB_RS_PATH: &str = "src/solver/github.rs";
const TARGET_FUNCTION: &str = "generate_pr_title";

/// Extracts JSON from markdown code block
fn extract_json_from_markdown(content: &str) -> Option<&str> {
    let re = Regex::new(r"```json\s*(\{[\s\S]*?\})\s*```").ok()?;
    re.captures(content)?.get(1).map(|m| m.as_str())
}

/// Gathers relevant file contents for context
async fn gather_context(issue_title: &str, issue_description: &str) -> Result<String> {
    // First, identify key files that need to be examined based on the issue
    let mut relevant_files = Vec::new();
    
    // For PR title generation, we know we need github.rs
    if issue_title.contains("PR title") || issue_description.contains("PR title") {
        relevant_files.push(GITHUB_RS_PATH);
    }

    // For JSON escaping issues, we need json.rs
    if issue_title.contains("JSON") || issue_description.contains("JSON") {
        relevant_files.push("src/solver/json.rs");
    }

    // Add any other relevant files based on keywords
    // TODO: Make this more sophisticated using the repo map

    // Build context string with file contents
    let mut context = String::new();
    for file in relevant_files {
        context.push_str(&format!("\nFile: {}\nContent:\n", file));
        // TODO: Use view_file to get actual content
        // For now, hardcoding the relevant function for testing
        if file == GITHUB_RS_PATH {
            context.push_str(r#"
async fn generate_pr_title(&self, issue_number: i32, context: &str) -> Result<String> {
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
}"#);
        }
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
    info!("Gathering relevant file contents...");
    let file_context = gather_context(title, description).await?;
    debug!("File context: {}", file_context);

    info!("Generating implementation plan...");
    let context = PlanningContext::new(ollama_url)?;
    let stream = context
        .generate_plan(issue_number, title, description, repo_map, &file_context)
        .await?;

    let full_response = handle_plan_stream(stream).await?;
    debug!("Full response: {}", full_response);

    // Extract JSON from markdown code block
    let json_str = extract_json_from_markdown(&full_response)
        .ok_or_else(|| anyhow!(ERR_NO_JSON))?;

    debug!("Extracted JSON: {}", json_str);

    // Parse JSON to verify it's valid
    let json: serde_json::Value = serde_json::from_str(json_str)?;

    // Verify response targets correct file
    let changes = json["changes"].as_array().ok_or_else(|| anyhow!(ERR_NO_CHANGES))?;
    let targets_github_rs = changes.iter().any(|c| {
        let path = c["path"].as_str().unwrap_or("");
        let search = c["search"].as_str().unwrap_or("");
        path == GITHUB_RS_PATH && search.contains(TARGET_FUNCTION)
    });

    if !targets_github_rs {
        return Err(anyhow!(ERR_WRONG_TARGET));
    }

    Ok(json_str.to_string())
}