//! Autopilot CLI - AI-powered coding assistant

use adjutant::cli::{execute, AutopilotCli};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("adjutant=info".parse()?)
                .add_directive("oanix=info".parse()?)
                .add_directive("issues=info".parse()?),
        )
        .init();

    // Parse CLI args
    let cli = AutopilotCli::parse();

    // Execute command
    execute(cli).await
}
