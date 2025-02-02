use crate::solver_impl::types::RelevantFiles;
use anyhow::Result;
use openagents::server::services::deepseek::DeepSeekService;
use openagents::server::services::ollama::OllamaService;
use openagents::solver::state::{SolverState, SolverStatus};
use std::collections::HashSet;
use tracing::{debug, error, info};

use super::pre_analysis::analyze_with_deepseek;

pub async fn identify_files(
    state: &mut SolverState,
    mistral: &OllamaService,
    deepseek: &DeepSeekService,
    valid_paths: &HashSet<String>,
) -> Result<()> {
    info!("Starting file identification process...");
    state.update_status(SolverStatus::Thinking);

    // First get DeepSeek's analysis
    let (response, reasoning) = analyze_with_deepseek(state, deepseek, valid_paths).await?;

    // Now use Mistral to structure the output
    let prompt = format!(
        "Based on this analysis from another AI, suggest up to 10 most relevant files that need to be modified. \
        Return a JSON object with a 'files' array containing objects with 'path' (relative path, no leading slash), \
        'relevance_score' (1-10, where 10 is most relevant), and 'reason' fields.\n\n\
        IMPORTANT: You MUST ONLY use paths from this list:\n{}\n\n\
        Analysis:\n{}\n\n\
        Previous AI's Analysis:\n{}\n\n\
        Previous AI's Reasoning Process:\n{}", 
        valid_paths.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"),
        state.analysis,
        response,
        reasoning
    );

    let format = serde_json::json!({
        "type": "object",
        "properties": {
            "files": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string"
                        },
                        "relevance_score": {
                            "type": "number",
                            "minimum": 1,
                            "maximum": 10
                        },
                        "reason": {
                            "type": "string"
                        }
                    },
                    "required": ["path", "relevance_score", "reason"]
                },
                "maxItems": 10
            }
        },
        "required": ["files"]
    });

    let relevant_files: RelevantFiles = mistral.chat_structured(prompt, format).await?;

    // Add files to state, ensuring paths are relative and valid
    for mut file in relevant_files.files {
        // Remove leading slash if present
        if file.path.starts_with('/') {
            file.path = file.path[1..].to_string();
        }

        // Only add if path is in valid_paths
        if valid_paths.contains(&file.path) {
            debug!("Adding valid file: {}", file.path);
            // Convert relevance score from 1-10 to 0-1 for state storage
            let normalized_score = file.relevance_score / 10.0;
            state.add_file(file.path, file.reason, normalized_score);
        } else {
            error!("Skipping invalid file path: {}", file.path);
        }
    }

    Ok(())
}
