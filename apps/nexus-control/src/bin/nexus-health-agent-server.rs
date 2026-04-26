use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::get};
use serde_json::{Value, json};

const ENV_PORT: &str = "PORT";
const ENV_SERVER_ARGS: &str = "NEXUS_HEALTH_AGENT_SERVER_ARGS";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_SERVER_ARGS: &str = "--dry-run,--json";

#[tokio::main]
async fn main() -> Result<()> {
    ensure_rustls_crypto_provider()?;
    let addr = SocketAddr::from(([0, 0, 0, 0], configured_port()?));
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/run", get(run_health_agent).post(run_health_agent));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind nexus-health-agent-server on {addr}"))?;
    axum::serve(listener, app)
        .await
        .context("serve nexus-health-agent-server")?;
    Ok(())
}

async fn healthz() -> Json<Value> {
    Json(json!({
        "ok": true,
        "service": "nexus-health-agent-server"
    }))
}

async fn run_health_agent() -> impl IntoResponse {
    match run_once().await {
        Ok(report) => {
            log_report(&report);
            (StatusCode::OK, Json(report)).into_response()
        }
        Err(error) => {
            log_error(&error);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "ok": false,
                    "error": error.to_string()
                })),
            )
                .into_response()
        }
    }
}

async fn run_once() -> Result<Value> {
    let args = server_args()?;
    let mut command = nexus_control::parse_nexus_health_agent_command(&args).map_err(|error| {
        anyhow::anyhow!(
            "failed to parse {ENV_SERVER_ARGS}: {error}; usage: {}",
            nexus_control::nexus_health_agent_usage()
        )
    })?;
    command.cycle_count = 1;
    command.cycle_index = 1;
    command.cycle_interval_seconds = 0;
    let report = nexus_control::run_nexus_health_agent(&command).await?;
    serde_json::to_value(report).context("serialize nexus health-agent server report")
}

fn server_args() -> Result<Vec<String>> {
    let raw = std::env::var(ENV_SERVER_ARGS).unwrap_or_else(|_| DEFAULT_SERVER_ARGS.to_string());
    let mut args = vec!["nexus-health-agent".to_string()];
    for part in raw.split(',') {
        let arg = part.trim();
        if !arg.is_empty() {
            args.push(arg.to_string());
        }
    }
    Ok(args)
}

fn configured_port() -> Result<u16> {
    match std::env::var(ENV_PORT) {
        Ok(value) => parse_port(Some(value.as_str())),
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_PORT),
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("{ENV_PORT} must contain valid unicode")
        }
    }
}

fn parse_port(raw: Option<&str>) -> Result<u16> {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(DEFAULT_PORT);
    };
    raw.parse::<u16>()
        .with_context(|| format!("invalid {ENV_PORT}: {raw}"))
}

fn log_report(report: &Value) {
    let event = json!({
        "event": "nexus_health_agent_server_run",
        "status": report.get("status"),
        "summary": report.get("summary"),
        "scheduler_status": report.pointer("/scheduler/status"),
        "max_expected_detection_seconds": report.pointer("/scheduler/max_expected_detection_seconds"),
        "external_reachable": report.pointer("/external_reachability/reachable"),
        "external_failure_kind": report.pointer("/external_reachability/failure_kind"),
        "external_status_code": report.pointer("/external_reachability/status_code"),
        "snapshot_status": report.pointer("/snapshot/status"),
        "snapshot_degraded": report.pointer("/snapshot/degraded"),
    });

    match serde_json::to_string(&event) {
        Ok(serialized) => println!("{serialized}"),
        Err(error) => eprintln!("nexus_health_agent_server_log_error: {error}"),
    }
}

fn log_error(error: &anyhow::Error) {
    let event = json!({
        "event": "nexus_health_agent_server_error",
        "ok": false,
        "error": error.to_string(),
    });

    match serde_json::to_string(&event) {
        Ok(serialized) => eprintln!("{serialized}"),
        Err(serialize_error) => eprintln!("nexus_health_agent_server_log_error: {serialize_error}"),
    }
}

fn ensure_rustls_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|error| anyhow::anyhow!("failed to install rustls crypto provider: {error:?}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{DEFAULT_PORT, log_report, parse_port};

    #[test]
    fn default_port_is_8080_when_blank() {
        assert_eq!(parse_port(None).expect("port"), DEFAULT_PORT);
        assert_eq!(parse_port(Some(" ")).expect("port"), DEFAULT_PORT);
    }

    #[test]
    fn parses_explicit_port() {
        assert_eq!(parse_port(Some("9090")).expect("port"), 9090);
    }

    #[test]
    fn report_logging_accepts_sparse_reports() {
        log_report(&json!({
            "status": "completed",
            "scheduler": {"status": "hosted"},
            "external_reachability": {"reachable": true}
        }));
    }
}
