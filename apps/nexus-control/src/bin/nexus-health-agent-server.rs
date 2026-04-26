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
    let event = report_log_event(report);

    match serde_json::to_string(&event) {
        Ok(serialized) => println!("{serialized}"),
        Err(error) => eprintln!("nexus_health_agent_server_log_error: {error}"),
    }
}

fn report_log_event(report: &Value) -> Value {
    let first_failed_endpoint = report
        .pointer("/external_reachability/endpoints")
        .and_then(Value::as_array)
        .and_then(|endpoints| {
            endpoints.iter().find(|endpoint| {
                endpoint
                    .get("ok")
                    .and_then(Value::as_bool)
                    .is_some_and(|ok| !ok)
            })
        })
        .cloned();

    json!({
        "event": "nexus_health_agent_server_run",
        "status": report.get("status"),
        "summary": report.pointer("/snapshot/classification/summary").or_else(|| report.pointer("/forge_event_request/summary")),
        "health_state": report.pointer("/forge_event_request/health_state"),
        "severity": report.pointer("/forge_event_request/severity"),
        "scheduler_status": report.pointer("/scheduler/status"),
        "scheduler_name": report.pointer("/scheduler/scheduler_name"),
        "scheduler_projection_freshness_status": report.pointer("/scheduler/projection_freshness_status"),
        "max_expected_detection_seconds": report.pointer("/scheduler/max_expected_detection_seconds"),
        "external_status": report.pointer("/external_reachability/status"),
        "external_source": report.pointer("/external_reachability/source"),
        "external_vantage_id": report.pointer("/external_reachability/vantage_id"),
        "external_route_count": report.pointer("/external_reachability/route_count"),
        "external_failed_route_count": report.pointer("/external_reachability/failed_route_count"),
        "external_cloudflare_edge_error_count": report.pointer("/external_reachability/cloudflare_edge_error_count"),
        "external_cloudflare_error_codes": report.pointer("/external_reachability/cloudflare_error_codes"),
        "external_first_failed_endpoint": first_failed_endpoint,
        "snapshot_status": report.get("snapshot_status"),
        "snapshot_observation_status": report.pointer("/snapshot/observation_status"),
        "snapshot_highest_severity": report.pointer("/snapshot/classification/highest_severity"),
        "treasury_degraded_reason": report.pointer("/snapshot/treasury/degraded_reason"),
        "treasury_wallet_runtime_status": report.pointer("/snapshot/treasury/wallet_runtime_status"),
        "payout_loop_health": report.pointer("/snapshot/treasury/payout_loop_health"),
    })
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

    use super::{DEFAULT_PORT, log_report, parse_port, report_log_event};

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
            "external_reachability": {"status": "reachable"}
        }));
    }

    #[test]
    fn report_log_event_extracts_current_health_agent_schema() {
        let event = report_log_event(&json!({
            "status": "completed",
            "snapshot_status": "captured",
            "snapshot": {
                "observation_status": "healthy",
                "classification": {
                    "summary": "Nexus public edge reachable",
                    "highest_severity": "info"
                },
                "treasury": {
                    "wallet_runtime_status": "connected",
                    "payout_loop_health": "healthy"
                }
            },
            "scheduler": {
                "status": "hosted",
                "scheduler_name": "nexus-health-runner-every-minute",
                "projection_freshness_status": "reported_by_forge_health_event_projection",
                "max_expected_detection_seconds": 60
            },
            "external_reachability": {
                "status": "reachable",
                "source": "external_public_probe",
                "vantage_id": "gcp-us-central1-cloud-run",
                "route_count": 3,
                "failed_route_count": 0,
                "cloudflare_edge_error_count": 0,
                "cloudflare_error_codes": []
            }
        }));

        assert_eq!(event["summary"], "Nexus public edge reachable");
        assert_eq!(event["snapshot_status"], "captured");
        assert_eq!(event["external_status"], "reachable");
        assert_eq!(event["external_vantage_id"], "gcp-us-central1-cloud-run");
        assert_eq!(event["scheduler_status"], "hosted");
        assert_eq!(event["max_expected_detection_seconds"], 60);
    }
}
