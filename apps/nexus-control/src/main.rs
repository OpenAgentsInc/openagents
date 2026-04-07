#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args: Vec<String> = std::env::args().collect();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    if matches!(args.get(1).map(String::as_str), Some("treasury")) {
        let config = nexus_control::ServiceConfig::from_env()
            .map_err(|error| anyhow::anyhow!("failed to load nexus-control config: {error}"))?;
        let command = nexus_control::parse_treasury_command(&args).map_err(|error| {
            anyhow::anyhow!(
                "failed to parse treasury command: {error}\nusage: nexus-control {}",
                nexus_control::treasury_usage()
            )
        })?;
        let output = nexus_control::run_treasury_command(&config.treasury, &command).await?;
        println!("{output}");
        return Ok(());
    }

    let config = nexus_control::ServiceConfig::from_env()
        .map_err(|error| anyhow::anyhow!("failed to load nexus-control config: {error}"))?;
    nexus_control::run_server(config).await
}
