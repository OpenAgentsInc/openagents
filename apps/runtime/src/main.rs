use anyhow::Result;
use tracing_subscriber::EnvFilter;

use openagents_runtime_service::{config::Config, serve};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,openagents_runtime_service=debug")),
        )
        .with_current_span(true)
        .init();

    let config = Config::from_env()?;
    serve(config).await
}
