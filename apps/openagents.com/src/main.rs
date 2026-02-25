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
    let log_format = std::env::var("OA_CONTROL_LOG_FORMAT")
        .ok()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "json".to_string());

    if log_format == "pretty" {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .pretty()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .json()
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(false)
            .init();
    }
}
