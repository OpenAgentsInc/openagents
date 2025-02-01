use anyhow::{Context as _, Result};
use openagents::server::services::github_issue::GitHubService;
use openagents::server::services::ollama::OllamaService;
use openagents::solver::changes::apply_changes;
use openagents::solver::state::{SolverState, SolverStatus};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::collections::HashSet;
use tracing::{debug, info, error};

const OLLAMA_URL: &str = "http://192.168.1.189:11434";

#[derive(Debug, Serialize, Deserialize)]
struct RelevantFiles {
    files: Vec<FileInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileInfo {
    path: String,
    relevance_score: f32,
    reason: String,
}

fn extract_paths_from_repomap(repo_map: &str) -> HashSet<String> {
    let mut paths = HashSet::new();
    for line in repo_map.lines() {
        if line.contains(".rs:") {
            if let Some(path) = line.split(':').next() {
                paths.insert(path.trim().to_string());
            }
        }
    }
    debug!("Found {} valid paths in repo map", paths.len());
    debug!("Valid paths: {:?}", paths);
    paths
}

async fn collect_context(
    state: &mut SolverState,
    github: &GitHubService,
    owner: &str,
    name: &str,
    issue_num: i32,
) -> Result<(String, HashSet<String>)> {
    info!("Collecting context...");
    state.update_status(SolverStatus::CollectingContext);

    // Fetch issue details
    info!("Fetching issue #{}", issue_num);
    let issue = github.get_issue(owner, name, issue_num).await?;
    let comments = github.get_issue_comments(owner, name, issue_num).await?;

    // Generate repository map
    info!("Generating repository map...");
    let repo_dir = std::env::current_dir()?;
    debug!("Current directory: {}", repo_dir.display());
    debug!("Directory contents: {:?}", std::fs::read_dir(&repo_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>());
    let repo_map = openagents::repomap::generate_repo_map(&repo_dir);
    
    // Extract valid paths from repo map
    let valid_paths = extract_paths_from_repomap(&repo_map);

    // Update state with initial analysis
    state.analysis = format!(
        "Issue #{}: {}\n\nDescription: {}\n\nComments:\n{}\n\nRepository Map:\n{}\n\nValid file paths for modifications:\n{}", 
        issue_num,
        issue.title,
        issue.body.unwrap_or_default(),
        comments
            .iter()
            .map(|c| format!("- {}", c.body))
            .collect::<Vec<_>>()
            .join("\n"),
        repo_map,
        valid_paths.iter().collect::<Vec<_>>().join("\n")
    );

    // Return both the repo directory and valid paths
    Ok((repo_dir.to_string_lossy().to_string(), valid_paths))
}

async fn identify_files(
    state: &mut SolverState,
    mistral: &OllamaService,
    valid_paths: &HashSet<String>,
) -> Result<()> {
    info!("Identifying relevant files...");
    state.update_status(SolverStatus::Thinking);

    let prompt = format!(
        "Based on this analysis, suggest up to 5 most relevant files that need to be modified. Return a JSON object with a 'files' array containing objects with 'path' (relative path, no leading slash), 'relevance_score' (0-1), and 'reason' fields.\n\nIMPORTANT: You MUST ONLY use paths from this list:\n{}\n\nAnalysis:\n{}", 
        valid_paths.iter().collect::<Vec<_>>().join("\n"),
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

async fn generate_changes(
    state: &mut SolverState,
    mistral: &OllamaService,
    repo_dir: &str,
) -> Result<()> {
    info!("Generating code changes...");
    state.update_status(SolverStatus::GeneratingCode);

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
            },
            Err(e) => {
                error!("Failed to read file:");
                error!("  Relative path: {}", relative_path);
                error!("  Absolute path: {}", absolute_path.display());
                error!("  Error: {}", e);
                return Err(e.into());
            }
        };

        let prompt = format!(
            "Based on the analysis and EXACT current file content, suggest specific code changes needed. Return a JSON object with a 'changes' array containing objects with 'search' (exact code to replace), 'replace' (new code), and 'analysis' (reason) fields.\n\nAnalysis:\n{}\n\nFile: {}\nContent:\n{}\n\nIMPORTANT: The 'search' field must contain EXACT code that exists in the file. The 'replace' field must contain the complete new code to replace it. Do not use descriptions - only actual code.", 
            state.analysis,
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
                                "type": "string"
                            },
                            "replace": {
                                "type": "string"
                            },
                            "analysis": {
                                "type": "string"
                            }
                        },
                        "required": ["search", "replace", "analysis"]
                    }
                }
            },
            "required": ["changes"]
        });

        #[derive(Debug, Serialize, Deserialize)]
        struct Changes {
            changes: Vec<Change>,
        }

        #[derive(Debug, Serialize, Deserialize)]
        struct Change {
            search: String,
            replace: String,
            analysis: String,
        }

        let changes: Changes = mistral.chat_structured(prompt, format).await?;

        // Add changes to file state
        for change in changes.changes {
            file.add_change(change.search, change.replace, change.analysis);
        }
    }

    state.update_status(SolverStatus::ReadyForCoding);
    Ok(())
}

async fn apply_file_changes(state: &mut SolverState, repo_dir: &str) -> Result<()> {
    info!("Applying code changes...");
    state.update_status(SolverStatus::Testing);

    // Log directory information
    let base_path = Path::new(repo_dir);
    debug!("Base path: {}", base_path.display());
    debug!("Base path contents: {:?}", std::fs::read_dir(base_path)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>());

    // Apply changes using the new apply_changes function
    apply_changes(state, repo_dir)?;

    state.update_status(SolverStatus::CreatingPr);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize state
    let mut state = SolverState::new("Initial solver state".to_string());

    // Configuration
    let owner = "OpenAgentsInc";
    let name = "openagents";
    let issue_num = 651;

    // Get GitHub token from environment
    let github_token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN not set")?;

    // Initialize services
    let github = GitHubService::new(Some(github_token.clone()))
        .context("Failed to initialize GitHub service")?;

    // Use hardcoded Ollama URL but allow override from environment
    let ollama_url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| OLLAMA_URL.to_string());
    let mistral = OllamaService::with_config(&ollama_url, "mistral-small");

    // Execute solver loop
    let (repo_dir, valid_paths) = collect_context(&mut state, &github, owner, name, issue_num).await?;
    identify_files(&mut state, &mistral, &valid_paths).await?;
    generate_changes(&mut state, &mistral, &repo_dir).await?;
    apply_file_changes(&mut state, &repo_dir).await?;

    // Print final state
    println!("\nFinal solver state:");
    println!("Status: {:?}", state.status);
    println!("Files to modify:");
    for file in &state.files {
        println!("- {} (score: {:.2})", file.path, file.relevance_score);
        println!("  Analysis: {}", file.analysis);
        for change in &file.changes {
            println!("  Change:");
            println!("    Search:  {}", change.search);
            println!("    Replace: {}", change.replace);
            println!("    Analysis: {}", change.analysis);
        }
    }

    state.update_status(SolverStatus::Complete);
    println!("\nSolver completed successfully.");

    Ok(())
}