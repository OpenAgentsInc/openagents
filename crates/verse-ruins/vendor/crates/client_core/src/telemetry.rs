//! Client-side telemetry init (dev-friendly pretty logs by default).

#[allow(dead_code)]
pub fn init_client_telemetry(dev_pretty: bool) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = if dev_pretty { fmt::layer().pretty().boxed() } else { fmt::layer().boxed() };
    let registry = tracing_subscriber::registry().with(filter).with(fmt_layer);
    let _ = registry.try_init();
}

