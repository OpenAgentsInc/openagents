//! Telemetry bootstrap for server (tracing + optional Prometheus metrics).

use anyhow::Result;

pub struct TelemetryGuard;

pub fn init_telemetry(cfg: &data_runtime::configs::telemetry::TelemetryCfg) -> Result<TelemetryGuard> {
    use tracing_subscriber::{fmt, EnvFilter, prelude::*};
    let level = cfg.log_level.clone().unwrap_or_else(|| "info".to_string());
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));
    // Console JSON by default
    let fmt_layer = if cfg.json_logs.unwrap_or(true) {
        fmt::layer().json().boxed()
    } else {
        fmt::layer().boxed()
    };
    // Build registry
    let registry = tracing_subscriber::registry().with(filter).with(fmt_layer);
    registry.init();
    // Optional Prometheus metrics exporter (native only; not available on web)
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(addr) = &cfg.metrics_addr {
            let parsed = addr.parse();
            let addr = match parsed {
                Ok(a) => a,
                Err(_e) => {
                    // Fallback to a safe default and record an error counter
                    metrics::counter!("server.errors_total", "site" => "telemetry.parse_addr").increment(1);
                    std::net::SocketAddr::from(([127, 0, 0, 1], 9100))
                }
            };
            let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
            let _ = builder.with_http_listener(addr).install();
        }
    }
    // One-line effective config for operator visibility
    tracing::info!(
        target: "telemetry",
        log_level = ?cfg.log_level,
        json_logs = ?cfg.json_logs,
        metrics_addr = ?cfg.metrics_addr,
        otlp_endpoint = ?cfg.otlp_endpoint,
        trace_sample = ?cfg.trace_sample,
        "telemetry initialized"
    );
    Ok(TelemetryGuard)
}
