use anyhow::Result;
use openagents::server::services::ollama::OllamaService;
use openagents::solver::state::{SolverState, SolverStatus};
use std::collections::HashSet;
use tracing::{debug, error, info};
use crate::solver_impl::types::RelevantFiles;

pub async fn identify_files(
    state: &mut SolverState,
    mistral: &OllamaService,
    valid_paths: &HashSet<String>,
) -> Result<()> {
    info!("Identifying relevant files...");
    state.update_status(SolverStatus::Thinking);

    let prompt = format!(
        "Based on this analysis, suggest up to 5 most relevant files that need to be modified. Return a JSON object with a 'files' array containing objects with 'path' (relative path, no leading slash), 'relevance_score' (0-1), and 'reason' fields.\n\nIMPORTANT: You MUST ONLY use paths from this list:\n{}\n\nAnalysis:\n{}", 
        valid_paths.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"),
        state.analysis
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
                            "minimum": 0,
                            "maximum": 1
                        },
                        "reason": {
                            "type": "string"
                        }
                    },
                    "required": ["path", "relevance_score", "reason"]
                }
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
            state.add_file(file.path, file.reason, file.relevance_score);
        } else {
            error!("Skipping invalid file path: {}", file.path);
        }
    }

    Ok(())
}