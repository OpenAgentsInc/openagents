use crate::solver_impl::types::RelevantFiles;
use anyhow::Result;
use openagents::server::services::gemini::GeminiService;
use openagents::solver::state::{SolverState, SolverStatus};
use std::collections::HashSet;
use tracing::{debug, error, info};

pub async fn identify_files(
    state: &mut SolverState,
    gemini: &GeminiService,
    valid_paths: &HashSet<String>,
) -> Result<()> {
    info!("Starting file identification process...");
    state.update_status(SolverStatus::Thinking);

    // Convert HashSet to Vec for Gemini API
    let valid_paths_vec: Vec<String> = valid_paths.iter().cloned().collect();

    // Get Gemini's analysis
    let response = gemini
        .analyze_files(
            &state.analysis,
            &valid_paths_vec,
            &state.repo_context,
        )
        .await?;

    // Parse response into RelevantFiles
    let relevant_files: RelevantFiles = serde_json::from_value(response)?;

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