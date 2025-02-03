use crate::solver_impl::{changes_analysis::analyze_changes_with_deepseek, types::Changes};
use anyhow::Result;
use openagents::server::services::deepseek::DeepSeekService;
use openagents::server::services::ollama::OllamaService;
use openagents::solver::state::{SolverState, SolverStatus};
use std::path::Path;
use tracing::{debug, error, info};

pub async fn generate_changes(
    state: &mut SolverState,
    mistral: &OllamaService,
    deepseek: &DeepSeekService,
    repo_dir: &str,
) -> Result<()> {
    info!("Generating code changes...");
    state.update_status(SolverStatus::GeneratingCode);

    // First get DeepSeek's analysis of all files
    let (response, reasoning) =
        analyze_changes_with_deepseek(state, deepseek, Path::new(repo_dir)).await?;
    info!("DeepSeek analysis complete");
    debug!("DeepSeek reasoning: {}", reasoning);

    // Now process each file with Mistral using DeepSeek's analysis
    for file in &mut state.files {
        // Log paths BEFORE any operations
        let relative_path = &file.path;
        let absolute_path = Path::new(repo_dir).join(relative_path);
        info!("Processing file:");
        info!("  Relative path: {}", relative_path);
        info!("  Absolute path: {}", absolute_path.display());

        // Try to read the file content
        let file_content = match std::fs::read_to_string(&absolute_path) {
            Ok(content) => {
                debug!("Successfully read file content");
                content
            }
            Err(e) => {
                error!("Failed to read file:");
                error!("  Relative path: {}", relative_path);
                error!("  Absolute path: {}", absolute_path.display());
                error!("  Error: {}", e);
                return Err(e.into());
            }
        };

        let prompt = format!(
            "Based on DeepSeek's analysis and the current file content, generate specific code changes in JSON format.\n\n\
            DeepSeek Analysis:\n{}\n\n\
            DeepSeek Reasoning:\n{}\n\n\
            File: {}\n\
            Content:\n{}\n\n\
            IMPORTANT RULES:\n\
            1. The 'search' field MUST contain EXACT code that exists in the file content above\n\
            2. The 'replace' field must contain the complete new code to replace it\n\
            3. Do not use empty search strings - you must match existing code\n\
            4. Do not use code block markers like ```rust - just the raw code\n\
            5. For new additions, find a suitable insertion point in the existing code\n\
            6. Verify each search string exists in the file content before including it\n\
            7. Make sure search strings are unique - they should only match once in the file\n\
            8. Use #[test] for test attributes, not [test]\n\
            9. Include enough surrounding context in search strings to ensure unique matches\n\n\
            Return a JSON object with a 'changes' array containing objects with 'search', 'replace', and 'analysis' fields.", 
            response,
            reasoning,
            file.path,
            file_content
        );

        let format = serde_json::json!({
            "type": "object",
            "properties": {
                "changes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "search": {
                                "type": "string",
                                "minLength": 1
                            },
                            "replace": {
                                "type": "string",
                                "minLength": 1
                            },
                            "analysis": {
                                "type": "string",
                                "minLength": 1
                            }
                        },
                        "required": ["search", "replace", "analysis"]
                    }
                }
            },
            "required": ["changes"]
        });

        let changes: Changes = mistral.chat_structured(prompt, format).await?;

        // Validate and add changes to file state
        for change in changes.changes {
            // Verify the search string exists in the file content
            let matches: Vec<_> = file_content.match_indices(&change.search).collect();
            match matches.len() {
                0 => {
                    error!("Search string not found in file: {}", change.search);
                    continue;
                }
                1 => {
                    debug!("Found unique match for search string");
                    file.add_change(change.search, change.replace, change.analysis);
                }
                n => {
                    error!(
                        "Found {} matches for search string - must be unique: {}",
                        n, change.search
                    );
                    continue;
                }
            }
        }
    }

    state.update_status(SolverStatus::ReadyForCoding);
    Ok(())
}

pub async fn apply_file_changes(state: &mut SolverState, repo_dir: &str) -> Result<()> {
    info!("Applying code changes...");
    state.update_status(SolverStatus::Testing);

    // Log directory information
    let base_path = Path::new(repo_dir);
    debug!("Base path: {}", base_path.display());
    debug!(
        "Base path contents: {:?}",
        std::fs::read_dir(base_path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect::<Vec<_>>()
    );

    // Apply changes using the new apply_changes function
    openagents::solver::changes::apply_changes(state, repo_dir)?;

    state.update_status(SolverStatus::CreatingPr);
    Ok(())
}
