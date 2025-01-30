use anyhow::{Context as _, Result};
use clap::Parser;
use openagents::server::services::github_issue::GitHubService;
use std::path::Path;
use tracing::{debug, info};

mod solver_impl;

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    issue: i32,

    #[clap(long)]
    repo: Option<String>,

    #[clap(long)]
    live: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Parse command line arguments
    let cli = Cli::parse();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get GitHub token from environment
    let github_token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN not set")?;

    // Get Ollama URL from environment or use default
    let ollama_url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    // Get repository owner/name
    let repo = cli
        .repo
        .unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
    let (owner, name) = repo
        .split_once('/')
        .context("Invalid repository format. Expected owner/name")?;

    // Initialize GitHub service
    let github = GitHubService::new(Some(github_token.clone()))
        .context("Failed to initialize GitHub service")?;

    // Fetch issue details
    info!("Fetching issue #{}", cli.issue);
    let issue = github.get_issue(owner, name, cli.issue).await?;

    // Generate repository map
    info!("Generating repository map...");
    let repo_map = openagents::repomap::generate_repo_map(Path::new("."));
    debug!("Repository map:\n{}", repo_map);

    // Generate implementation plan
    let plan = solver_impl::planning::handle_planning(
        cli.issue,
        &issue.title,
        issue.body.as_deref().unwrap_or("No description provided"),
        &repo_map,
        &ollama_url,
    )
    .await?;

    // Generate and apply solution
    solver_impl::solution::handle_solution(
        cli.issue,
        &issue.title,
        issue.body.as_deref().unwrap_or("No description provided"),
        &plan,
        &repo_map,
        &ollama_url,
    )
    .await?;

    // If in live mode, create branch and post comment
    if cli.live {
        // Create branch
        let branch_name = format!("solver/issue-{}", cli.issue);
        github
            .create_branch(owner, name, &branch_name, "main")
            .await?;

        // Post implementation plan as comment
        github
            .post_comment(
                owner,
                name,
                cli.issue,
                &format!("## Implementation Plan\n\n{}", plan),
            )
            .await?;

        // Create pull request
        github
            .create_pull_request(
                owner,
                name,
                &format!("Implement solution for #{}", cli.issue),
                &format!(
                    "This PR implements the solution for issue #{}\n\n## Implementation Plan\n\n{}",
                    cli.issue, plan
                ),
                &branch_name,
                "main",
            )
            .await?;
    } else {
        // Just print the plan
        println!("\nImplementation Plan:\n{}", plan);
    }

    Ok(())
}