use anyhow::{anyhow, Result};
use openagents::solver::{planning::PlanningContext, streaming::handle_plan_stream};
use regex::Regex;
use tracing::{debug, info};

/// Extracts JSON from markdown code block
fn extract_json_from_markdown(content: &str) -> Option<&str> {
    let re = Regex::new(r"```json\s*(\{[\s\S]*?\})\s*```").ok()?;
    re.captures(content)?.get(1).map(|m| m.as_str())
}

pub async fn handle_planning(
    issue_number: i32,
    title: &str,
    description: &str,
    repo_map: &str,
    ollama_url: &str,
) -> Result<String> {
    info!("Generating implementation plan...");

    let context = PlanningContext::new(ollama_url)?;
    let stream = context
        .generate_plan(issue_number, title, description, repo_map)
        .await?;

    let full_response = handle_plan_stream(stream).await?;
    debug!("Full response: {}", full_response);

    // Extract JSON from markdown code block
    let json_str = extract_json_from_markdown(&full_response)
        .ok_or_else(|| anyhow!("No JSON code block found in response"))?;

    debug!("Extracted JSON: {}", json_str);

    // Parse JSON to verify it's valid
    let json: serde_json::Value = serde_json::from_str(json_str)?;

    // Verify response targets correct file
    let changes = json["changes"].as_array().ok_or_else(|| anyhow!("No changes array in response"))?;
    let targets_github_rs = changes.iter().any(|c| {
        c["path"].as_str().unwrap_or("").contains("src/solver/github.rs") &&
        c["search"].as_str().unwrap_or("").contains("generate_pr_title")
    });

    if !targets_github_rs {
        return Err(anyhow!("Changes must target src/solver/github.rs generate_pr_title function"));
    }

    Ok(json_str.to_string())
}