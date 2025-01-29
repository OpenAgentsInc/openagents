use anyhow::{Context as _, Result};
use clap::Parser;
use openagents::solver::{Cli, Config};
use tracing::info;

mod issue;
mod planning;
mod solution;

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
    let (issue, comments) = issue::handle_issue(&cli, &github_token).await?;

    // Generate implementation plan
    let plan = planning::handle_planning(&cli, &issue, &comments).await?;

    // Generate and apply solution
    solution::handle_solution(&cli, &issue, &comments, &plan, github_token, openrouter_api_key).await?;

    info!("Solver completed successfully");
    Ok(())
}