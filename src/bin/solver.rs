use anyhow::{Context as _, Result};
use openagents::server::services::github_issue::GitHubService;
use openagents::server::services::ollama::OllamaService;
use openagents::solver::changes::apply_changes;
use openagents::solver::state::{SolverState, SolverStatus};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

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

async fn collect_context(
    state: &mut SolverState,
    github: &GitHubService,
    owner: &str,
    name: &str,
    issue_num: i32,
) -> Result<()> {
    info!("Collecting context...");
    state.update_status(SolverStatus::CollectingContext);

    // Fetch issue details
    info!("Fetching issue #{}", issue_num);
    let issue = github.get_issue(owner, name, issue_num).await?;
    let comments = github.get_issue_comments(owner, name, issue_num).await?;

    // Generate repository map
    info!("Generating repository map...");
    let repo_map = openagents::repomap::generate_repo_map(Path::new("."));

    // Update state with initial analysis
    state.analysis = format!(
        "Issue #{}: {}\n\nDescription: {}\n\nComments:\n{}\n\nRepository Map:\n{}",
        issue_num,
        issue.title,
        issue.body.unwrap_or_default(),
        comments
            .iter()
            .map(|c| format!("- {}", c.body))
            .collect::<Vec<_>>()
            .join("\n"),
        repo_map
    );

    Ok(())
}

async fn identify_files(
    state: &mut SolverState,
    mistral: &OllamaService,
) -> Result<()> {
    info!("Identifying relevant files...");
    state.update_status(SolverStatus::Thinking);

    let prompt = format!(
        "Based on this analysis, suggest 5 most relevant files that need to be modified. Return a JSON object with a 'files' array containing objects with 'path' (relative path, no leading slash), 'relevance_score' (0-1), and 'reason' fields.\n\nAnalysis:\n{}", 
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

    // Add files to state, ensuring paths are relative
    for mut file in relevant_files.files {
        // Remove leading slash if present
        if file.path.starts_with('/') {
            file.path = file.path[1..].to_string();
        }
        state.add_file(file.path, file.reason, file.relevance_score);
    }

    Ok(())
}

async fn generate_changes(
    state: &mut SolverState,
    mistral: &OllamaService,
) -> Result<()> {
    info!("Generating code changes...");
    state.update_status(SolverStatus::GeneratingCode);

    for file in &mut state.files {
        let prompt = format!(
            "Based on the analysis and file path, suggest specific code changes needed. Return a JSON object with a 'changes' array containing objects with 'search' (code to replace), 'replace' (new code), and 'analysis' (reason) fields.\n\nAnalysis:\n{}\n\nFile: {}", 
            state.analysis,
            file.path
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

async fn apply_file_changes(state: &mut SolverState) -> Result<()> {
    info!("Applying code changes...");
    state.update_status(SolverStatus::Testing);

    // Apply changes using the new apply_changes function
    apply_changes(state)?;

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
    collect_context(&mut state, &github, owner, name, issue_num).await?;
    identify_files(&mut state, &mistral).await?;
    generate_changes(&mut state, &mistral).await?;
    apply_file_changes(&mut state).await?;

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