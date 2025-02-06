use anyhow::{Result, anyhow};
use openagents::server::services::gemini::service::GeminiService;
use openagents::solver::state::{SolverState, SolverStatus};
use std::collections::HashSet;
use tracing::{debug, error, info};
use serde_json::Value;

pub async fn identify_files(
    state: &mut SolverState,
    valid_paths: &HashSet<String>,
) -> Result<()> {
    info!("Starting file identification process...");
    state.update_status(SolverStatus::Thinking);

    // Initialize Gemini service
    let gemini = GeminiService::new()?;

    // Get repository context from state
    let repo_context = state.analysis.clone();

    // Stream the analysis for real-time feedback
    let mut stream = gemini.analyze_files_stream(
        &state.issue_description,
        &valid_paths.iter().collect::<Vec<_>>(),
        &repo_context,
    ).await;

    println!("\nAnalyzing files...\n");
    let mut full_response = String::new();
    
    // Process the streaming response
    while let Some(update) = stream.recv().await {
        match update {
            openagents::server::services::gemini::types::StreamUpdate::Content(content) => {
                print!("{}", content);
                std::io::stdout().flush().ok();
                full_response.push_str(&content);
            }
            openagents::server::services::gemini::types::StreamUpdate::Done => break,
        }
    }
    println!("\nAnalysis complete.\n");

    // Extract JSON from the response
    let json_str = if let Some(start) = full_response.find('{') {
        if let Some(end) = full_response.rfind('}') {
            &full_response[start..=end]
        } else {
            return Err(anyhow!("Invalid JSON in response - no closing brace"));
        }
    } else {
        return Err(anyhow!("Invalid JSON in response - no opening brace"));
    };

    // Parse the JSON response
    let response: Value = serde_json::from_str(json_str)
        .map_err(|e| anyhow!("Failed to parse JSON response: {}", e))?;

    // Extract and validate files
    let files = response["files"].as_array()
        .ok_or_else(|| anyhow!("No files array in response"))?;

    // Add files to state
    for file in files {
        let path = file["path"].as_str()
            .ok_or_else(|| anyhow!("Missing path in file"))?
            .to_string();
        let relevance_score = file["relevance_score"].as_f64()
            .ok_or_else(|| anyhow!("Missing or invalid relevance_score"))?;
        let reason = file["reason"].as_str()
            .ok_or_else(|| anyhow!("Missing reason in file"))?
            .to_string();

        // Remove leading slash if present
        let path = if path.starts_with('/') {
            path[1..].to_string()
        } else {
            path
        };

        // Only add if path is in valid_paths
        if valid_paths.contains(&path) {
            debug!("Adding valid file: {}", path);
            // Convert relevance score from 1-10 to 0-1 for state storage
            let normalized_score = relevance_score / 10.0;
            state.add_file(path, reason, normalized_score);
        } else {
            error!("Skipping invalid file path: {}", path);
        }
    }

    Ok(())
}