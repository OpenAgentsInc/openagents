use anyhow::Result;
use futures_util::StreamExt;
use openagents::server::services::deepseek::DeepSeekService;
use openagents::solver::state::SolverState;
use std::path::Path;
use tracing::{debug, info};

pub async fn analyze_changes_with_deepseek(
    state: &SolverState,
    deepseek: &DeepSeekService,
    repo_dir: &Path,
) -> Result<(String, String)> {
    info!("Starting DeepSeek changes analysis...");

    // Collect all file contents
    let mut file_contents = String::new();
    for file in &state.files {
        let path = repo_dir.join(&file.path);
        if let Ok(content) = std::fs::read_to_string(&path) {
            file_contents.push_str(&format!("\nFile: {}\nContent:\n{}\n", file.path, content));
        }
    }

    // Create the prompt for DeepSeek
    let prompt = format!(
        "Analyze these files and suggest specific code changes to implement the following issue:\n\n\
        Issue Analysis:\n{}\n\n\
        Files to modify:\n{}\n\n\
        Think through each change carefully and explain your reasoning. \
        For each file, explain what needs to be changed and why. \
        Show your step-by-step thinking process.\n\n\
        After your analysis, provide a summary of all proposed changes.",
        state.analysis,
        file_contents
    );

    // Get streaming response from DeepSeek
    let mut response = String::new();
    let mut reasoning = String::new();
    
    let mut stream = deepseek.chat_stream(prompt).await?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        debug!("DeepSeek chunk: {}", chunk);
        response.push_str(&chunk);
        reasoning.push_str(&chunk);
    }

    Ok((response, reasoning))
}