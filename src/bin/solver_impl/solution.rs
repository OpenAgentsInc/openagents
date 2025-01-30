use anyhow::{Context as _, Result};
use openagents::solver::{changes::generation::generate_changes, file_list::generate_file_list};
use tracing::{debug, info};

pub async fn handle_solution(
    _issue_number: i32,
    title: &str,
    description: &str,
    _plan: &str,
    repo_map: &str,
    ollama_url: &str,
) -> Result<()> {
    // Generate file list
    info!("Generating file list...");
    let (files, reasoning) = generate_file_list(title, description, repo_map, ollama_url).await?;
    debug!("Files to modify: {:?}", files);
    debug!("Reasoning: {}", reasoning);

    // Generate changes for each file
    info!("Generating changes...");
    for file in files {
        // Read current file content
        let content = std::fs::read_to_string(&file).context("Failed to read file")?;

        // Generate changes
        let (changes, reasoning) =
            generate_changes(&file, &content, title, description, ollama_url).await?;

        debug!("Changes for {}: {:?}", file, changes);
        debug!("Reasoning: {}", reasoning);

        // Apply changes
        for change in changes {
            info!("Applying changes to {}", change.path);
            let new_content = if change.search.is_empty() {
                // New file
                change.replace
            } else {
                // Replace existing content
                content.replace(&change.search, &change.replace)
            };

            std::fs::write(&change.path, new_content).context("Failed to write file")?;
        }
    }

    info!("Changes applied successfully");
    Ok(())
}
