#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    let config = nexus_relay::durable::DurableRelayConfig::from_env()
        .map_err(|error| anyhow::anyhow!("failed to load nexus-relay config: {error}"))?;
    nexus_relay::durable::run_server(config).await
}
