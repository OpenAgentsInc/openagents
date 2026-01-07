//! Pylon binary entrypoint

use clap::Parser;
use pylon::cli::{PylonCli, execute};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging with compute and nostr_client crates included
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("pylon=info".parse()?)
                .add_directive("compute=info".parse()?)
                .add_directive("nostr_client=info".parse()?)
                .add_directive("gpt_oss_metal=info".parse()?),
        )
        .init();

    // Bridge log crate to tracing (for compute::services::dvm_service which uses log::)
    tracing_log::LogTracer::init().ok();

    // Parse CLI args
    let cli = PylonCli::parse();

    // Execute command
    execute(cli).await
}
