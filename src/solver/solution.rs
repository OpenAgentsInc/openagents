use crate::solver::context::SolverContext;
use anyhow::Result;
use std::fs;
use tracing::{debug, info};

pub async fn handle_solution(
    issue_number: i32,
    title: &str,
    description: &str,
    plan: &str,
    repo_map: &str,
    ollama_url: &str,
) -> Result<()> {
    let context = SolverContext::new()?;

    // Generate file list
    info!("Generating file list...");
    let (files, reasoning) = context
        .generate_file_list(title, description, repo_map, ollama_url)
        .await?;
    debug!("Files to modify: {:?}", files);
    debug!("Reasoning: {}", reasoning);

    // For each file, generate and apply changes
    for file in files {
        info!("Processing file: {}", file);

        // Read file content
        let content = fs::read_to_string(&file)?;

        // Generate changes
        let (changes, reasoning) = context
            .generate_changes(&file, &content, title, description, ollama_url)
            .await?;
        debug!("Changes for {}: {:?}", file, changes);
        debug!("Reasoning: {}", reasoning);

        // Apply changes
        context.apply_changes(&changes)?;
    }

    // Clean up
    context.cleanup()?;

    Ok(())
}
