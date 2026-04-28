use anyhow::Result;

const DEFAULT_LOG_FILTER: &str = "warn,nip05_registrar=info";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)),
        )
        .with_target(false)
        .compact()
        .init();

    let config = nip05_registrar::Config::from_env()
        .map_err(|err| anyhow::anyhow!("failed to load nip05-registrar config: {err}"))?;
    nip05_registrar::run(config).await
}
