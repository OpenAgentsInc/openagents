use anyhow::Result;
use openagents::server::services::deepseek::DeepSeekService;
use openagents::solver::state::{SolverState, SolverStatus};
use std::collections::HashSet;
use tracing::{debug, info};

pub async fn analyze_with_deepseek(
    state: &mut SolverState,
    deepseek: &DeepSeekService,
    valid_paths: &HashSet<String>,
) -> Result<String> {
    info!("Starting DeepSeek pre-analysis...");
    state.update_status(SolverStatus::Thinking);

    let prompt = format!(
        "You are a code analysis expert. Based on this analysis, think through which files would need to be modified. \
        Show your reasoning process step by step.\n\n\
        IMPORTANT: You MUST ONLY consider paths from this list:\n{}\n\n\
        Analysis:\n{}\n\n\
        Think through this step by step, explaining your reasoning. \
        At the end, provide a final conclusion listing the most relevant files (max 3) with their relevance scores (0-1) \
        and reasons for modification.",
        valid_paths.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"),
        state.analysis
    );

    debug!("Sending prompt to DeepSeek...");
    let mut stream = deepseek.chat_stream(prompt, true).await;
    let mut full_response = String::new();
    let mut reasoning = String::new();

    while let Some(update) = stream.recv().await {
        match update {
            StreamUpdate::Content(content) => {
                print!("{}", content);
                std::io::stdout().flush()?;
                full_response.push_str(&content);
            }
            StreamUpdate::Reasoning(r) => {
                reasoning.push_str(&r);
            }
            StreamUpdate::Done => break,
            _ => {}
        }
    }

    println!("\nDeepSeek analysis complete.\n");
    debug!("Full response: {}", full_response);
    
    if !reasoning.is_empty() {
        debug!("Reasoning trace: {}", reasoning);
    }

    Ok(full_response)
}