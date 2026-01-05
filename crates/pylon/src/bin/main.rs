//! Pylon binary entrypoint

use clap::Parser;
use pylon::cli::{PylonCli, execute};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("pylon=info".parse()?)
                .add_directive("gpt_oss_metal=info".parse()?),
        )
        .init();

    // Parse CLI args
    let cli = PylonCli::parse();

    // Execute command
    execute(cli).await
}
