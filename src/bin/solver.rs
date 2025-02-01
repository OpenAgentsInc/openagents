use anyhow::{Context as _, Result};
use openagents::server::services::github_issue::GitHubService;
use openagents::server::services::ollama::OllamaService;
use openagents::solver::state::SolverState;
use tracing::info;

mod solver_impl;
use solver_impl::{
    context::collect_context,
    files::identify_files,
    changes::{generate_changes, apply_file_changes},
};

const OLLAMA_URL: &str = "http://192.168.1.189:11434";

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

    info!("\nSolver completed successfully.");
    Ok(())
}