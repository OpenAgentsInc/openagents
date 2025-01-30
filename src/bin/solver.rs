use anyhow::{Context as _, Result};
use clap::Parser;
use openagents::solver::{Cli, Config};
use tracing::info;

mod solver_impl {
    pub mod issue;
    pub mod planning;
    pub mod solution;
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("openagents=debug".parse()?),
        )
        .init();

    let cli = Cli::parse();
    let config = Config::load().context("Failed to load configuration")?;

    // Clone tokens before moving
    let github_token = config.github_token.clone();
    let openrouter_api_key = config.openrouter_api_key.clone();

    // Handle issue details
    let (issue, comments) = solver_impl::issue::handle_issue(&cli, &github_token).await?;

    // Initialize solution context for repo map
    let mut solution = openagents::solver::SolutionContext::new(
        cli.issue,
        openrouter_api_key.clone(),
        Some(github_token.clone()),
    )
    .context("Failed to initialize solution context")?;

    // Clone repository and generate map
    let repo_url = format!("https://github.com/{}", cli.repo);
    info!("Cloning repository: {}", repo_url);
    solution
        .clone_repository(&repo_url)
        .context("Failed to clone repository")?;

    let repo_map = solution.generate_repo_map();

    // Generate implementation plan
    let plan = solver_impl::planning::handle_planning(&cli, &issue, &comments, &repo_map).await?;

    // Generate and apply solution
    solver_impl::solution::handle_solution(&cli, &issue, &comments, &plan, github_token, openrouter_api_key).await?;

    info!("Solver completed successfully");
    Ok(())
}