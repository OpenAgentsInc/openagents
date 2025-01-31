use anyhow::{Context as _, Result};
use openagents::server::services::github_issue::GitHubService;
use openagents::server::services::ollama::OllamaService;
use std::path::Path;
use tracing::info;
use serde::{Deserialize, Serialize};

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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Hardcode our 'descriptive PR titles'
    let owner = "OpenAgentsInc";
    let name = "openagents";
    let issue_num = 637;

    // Get GitHub token from environment
    let github_token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN not set")?;

    // Get Ollama URL from environment or use default
    let _ollama_url =
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    // Initialize GitHub service
    let github = GitHubService::new(Some(github_token.clone()))
        .context("Failed to initialize GitHub service")?;

    // Fetch issue details
    info!("Fetching issue #{}", issue_num);
    let issue = github.get_issue(owner, name, issue_num).await?;
    let comments = github.get_issue_comments(owner, name, issue_num).await?;

    println!("Title: {}", issue.title);
    println!("Body: {}", issue.body.clone().unwrap_or_default());
    println!("State: {}", issue.state);

    // Print comments
    if !comments.is_empty() {
        println!("Comments:");
        for comment in comments {
            let body = comment.body.clone();
            println!("- {}", body);
        }
    } else {
        println!("No comments found.");
    }

    // Generate repository map
    info!("Generating repository map...");
    let repo_map = openagents::repomap::generate_repo_map(Path::new("."));
    
    let mistral = OllamaService::with_config(
        "http://192.168.1.189:11434",
        "mistral-small",
    );

    let prompt = format!(
        "Based on this issue and repository map, suggest 5 most relevant files that need to be modified. Return a JSON object with a 'files' array containing objects with 'path', 'relevance_score' (0-1), and 'reason' fields. Issue: {} - {}", 
        issue.title,
        issue.body.clone().unwrap_or_default()
    );

    info!("Prompt: {}", prompt);

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
    
    println!("\nRelevant files to modify:");
    for file in relevant_files.files {
        println!("- {} (score: {:.2})", file.path, file.relevance_score);
        println!("  Reason: {}", file.reason);
    }

    println!("Success.");

    Ok(())
}