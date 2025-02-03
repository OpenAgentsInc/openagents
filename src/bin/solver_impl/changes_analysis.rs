use anyhow::Result;
use openagents::server::services::deepseek::DeepSeekService;
use openagents::services::StreamUpdate;
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

    let mut rx = deepseek.chat_stream(prompt, false).await;

    while let Some(update) = rx.recv().await {
        match update {
            StreamUpdate::Content(content) => {
                let content = content.to_string();
                debug!("DeepSeek chunk: {}", content);
                response.push_str(&content);
                reasoning.push_str(&content);
            }
            StreamUpdate::Done => {
                debug!("DeepSeek stream complete");
                break;
            }
            _ => {
                debug!("Unexpected stream update");
            }
        }
    }

    Ok((response, reasoning))
}
