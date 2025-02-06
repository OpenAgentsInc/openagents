use anyhow::Result;
use clap::Parser;
use openagents::{
    server::services::gemini::GeminiService,
    solver::state::{SolverState, SolverStatus},
};
use std::{collections::HashSet, env};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use solver_impl::{files::identify_files, issue::handle_issue};

mod solver_impl;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    issue: i32,

    #[arg(short, long)]
    repo: Option<String>,

    #[arg(short, long)]
    live: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Parse command line arguments
    let args = Args::parse();

    // Initialize services
    let gemini = GeminiService::new()?;

    // Get repository info
    let (owner, repo) = match args.repo {
        Some(repo_str) => {
            let parts: Vec<&str> = repo_str.split('/').collect();
            if parts.len() != 2 {
                error!("Invalid repository format. Use owner/name");
                std::process::exit(1);
            }
            (parts[0].to_string(), parts[1].to_string())
        }
        None => ("OpenAgentsInc".to_string(), "openagents".to_string()),
    };

    // Initialize state
    let mut state = SolverState::new(format!("{}-{}", owner, repo));

    // Handle issue
    info!("Fetching issue #{} from {}/{}", args.issue, owner, repo);
    state.update_status(SolverStatus::Starting);

    let issue = handle_issue(args.issue, &owner, &repo).await?;
    state.analysis = issue.body;

    // Get valid paths
    let valid_paths: HashSet<String> = HashSet::from_iter(vec![
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
        // Add more valid paths as needed
    ]);

    // Set repository context
    state.repo_context = format!("Repository: {}/{}", owner, repo);

    // Identify files to modify
    identify_files(&mut state, &gemini, &valid_paths).await?;

    // Print results
    info!("Analysis complete. Found {} files to modify:", state.files.len());
    for file in state.files.values() {
        info!(
            "- {} (relevance: {:.2}): {}",
            file.path, file.relevance, file.reason
        );
    }

    Ok(())
}