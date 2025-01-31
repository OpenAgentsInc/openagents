use anyhow::{Context as _, Result};
use clap::Parser;
use openagents::server::services::github_issue::GitHubService;
use openagents::server::services::ollama::OllamaService;
use std::path::Path;
use tracing::info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Issue number to solve
    #[arg(short, long)]
    issue: u64,

    /// Repository owner/name (default: OpenAgentsInc/openagents)
    #[arg(short, long)]
    repo: Option<String>,

    /// Live mode - actually create branch and PR
    #[arg(short, long)]
    live: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Parse command line arguments
    let cli = Args::parse();

    // Parse repo owner/name
    let (owner, name) = match cli.repo {
        Some(ref repo) => {
            let parts: Vec<&str> = repo.split('/').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid repo format. Expected owner/name");
            }
            (parts[0].to_string(), parts[1].to_string())
        }
        None => ("OpenAgentsInc".to_string(), "openagents".to_string()),
    };

    // Get GitHub token from environment
    let github_token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN not set")?;

    // Get Ollama URL from environment or use default
    let ollama_url =
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    // Initialize GitHub service
    let github = GitHubService::new(Some(github_token.clone()))
        .context("Failed to initialize GitHub service")?;

    // Fetch issue details
    info!("Fetching issue #{}", cli.issue);
    let issue = github.get_issue(&owner, &name, cli.issue).await?;
    let _comments = github.get_issue_comments(&owner, &name, cli.issue).await?;

    println!("Title: {}", issue.title);
    println!("Body: {}", issue.body.clone().unwrap_or_default());
    println!("State: {}", issue.state);

    // Generate repository map
    info!("Generating repository map...");
    let repo_map = openagents::repomap::generate_repo_map(Path::new("."));

    let mistral = OllamaService::with_config(&ollama_url, "mistral-small");

    let prompt = format!(
        "Based on this issue and repository map, suggest 5 most relevant files that need to be modified. Return a JSON object with a 'files' array containing objects with 'path', 'relevance_score' (0-1), and 'reason' fields. Issue: {} - {}\\n\\nRepository map:\\n{}", 
        issue.title,
        issue.body.clone().unwrap_or_default(),
        repo_map
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

    let response = mistral.chat_structured(prompt, format).await?;
    println!("Response: {:?}", response);

    println!("Success.");

    Ok(())
}