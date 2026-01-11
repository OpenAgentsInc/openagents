//! Autopilot CLI - AI-powered coding assistant

use adjutant::cli::{execute, AutopilotCli};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging (quiet by default; use RUST_LOG for verbose output)
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        EnvFilter::new("warn")
            .add_directive("adjutant=warn".parse()?)
            .add_directive("oanix=warn".parse()?)
            .add_directive("issues=warn".parse()?)
    };
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();

    // Parse CLI args
    let cli = AutopilotCli::parse();

    // Execute command
    execute(cli).await
}
