use anyhow::Result;
use openagents_control_service::build_router;
use openagents_control_service::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    init_tracing(&config.log_filter);

    let bind_addr = config.bind_addr;
    let static_dir = config.static_dir.clone();

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    tracing::info!(
        bind = %bind_addr,
        static_dir = %static_dir.display(),
        "openagents control service listening"
    );

    axum::serve(listener, build_router(config)).await?;
    Ok(())
}

fn init_tracing(default_filter: &str) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter.to_string()));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();
}
