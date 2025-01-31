use anyhow::{Context as _, Result};
use openagents::server::services::github_issue::GitHubService;
use openagents::server::services::ollama::OllamaService;
use openagents::server::services::Gateway;
use openagents::solver::handle_plan_stream;
use std::path::Path;
use tracing::info;

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
    let ollama_url =
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    // Initialize GitHub service
    let github = GitHubService::new(Some(github_token.clone()))
        .context("Failed to initialize GitHub service")?;

    // Fetch issue details
    info!("Fetching issue #{}", issue_num);
    let issue = github.get_issue(owner, name, issue_num).await?;
    let comments = github.get_issue_comments(owner, name, issue_num).await?;

    println!("Title: {}", issue.title);
    println!("Body: {}", issue.body.unwrap_or_default());
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
    info!("Repository map:\n{}", repo_map);
    
    let ollama = OllamaService::with_config(
        "http://192.168.1.189:11434",
        "deepseek-r1:14b",
    );

    let prompt = "Speculate about this: ${issue.title}, ${issue.body}".to_string();
    let stream = ollama.chat_stream(prompt.clone(), true).await?;
    let plan = handle_plan_stream(stream).await?;
    info!("Plan: {}", plan);

    println!("Success.");

    Ok(())
}
