use anyhow::{anyhow, Result};
use clap::Parser;
use openagents::{
    repo::{cleanup_temp_dir, clone_repository, run_cargo_tests, RepoContext},
    repomap::generate_repo_map,
};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// GitHub repository URL
    #[arg(short, long)]
    url: String,

    /// Output file path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Run tests after cloning
    #[arg(short, long)]
    test: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    // Extract owner/repo from URL
    let repo_parts: Vec<&str> = cli.url.split('/').collect();
    let (owner, repo) = match repo_parts.len() {
        n if n >= 2 => (
            repo_parts[n - 2].to_string(),
            repo_parts[n - 1].trim_end_matches(".git").to_string(),
        ),
        _ => return Err(anyhow!("Invalid repository URL format")),
    };

    info!("Analyzing repository: {}/{}", owner, repo);

    // Create temporary directory
    let temp_dir = std::env::temp_dir().join(format!("repo_{}_{}", owner, repo));
    cleanup_temp_dir(&temp_dir);

    // Initialize context
    let ctx = RepoContext::new(
        temp_dir,
        std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
        None,
    );

    // Clone repository
    info!("Cloning repository...");
    clone_repository(&cli.url, &ctx.temp_dir, ctx.github_token.as_deref())?;

    // Run tests if requested
    if cli.test {
        info!("Running cargo tests...");
        let test_output = run_cargo_tests(&ctx.temp_dir)?;
        if !test_output {
            println!("Note: Some tests failed, but continuing with analysis...");
        }
    }

    // Generate repository map
    info!("Generating repository map...");
    let map = generate_repo_map(&ctx.temp_dir);

    // Output results
    match cli.output {
        Some(path) => {
            info!("Writing output to {:?}", path);
            std::fs::write(path, map)?;
        }
        None => println!("{}", map),
    }

    // Clean up
    cleanup_temp_dir(&ctx.temp_dir);

    Ok(())
}
