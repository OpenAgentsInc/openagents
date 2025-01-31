use anyhow::{Context as _, Result};
use openagents::server::services::github_issue::GitHubService;
use std::path::Path;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get GitHub token from environment
    let github_token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN not set")?;

    // Get Ollama URL from environment or use default
    let ollama_url =
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    println!("Hello.");

    Ok(())
}
