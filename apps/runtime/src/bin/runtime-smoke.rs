use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::StatusCode;
use serde_json::{Value, json};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,runtime_smoke=debug")),
        )
        .with_current_span(true)
        .init();

    let base_url =
        std::env::var("SMOKE_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:4100".to_string());
    run_smoke(&base_url).await
}

async fn run_smoke(base_url: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("build reqwest client")?;

    expect_status(
        &client,
        &format!("{base_url}/healthz"),
        StatusCode::OK,
        "healthz",
    )
    .await?;
    expect_status(
        &client,
        &format!("{base_url}/readyz"),
        StatusCode::OK,
        "readyz",
    )
    .await?;

    let create_response = client
        .post(format!("{base_url}/internal/v1/runs"))
        .json(&json!({
            "worker_id": "smoke:runtime",
            "metadata": { "source": "runtime-smoke" }
        }))
        .send()
        .await
        .context("POST /internal/v1/runs")?;
    ensure_status(
        create_response.status(),
        StatusCode::CREATED,
        "create runtime run",
    )?;
    let create_json: Value = create_response
        .json()
        .await
        .context("decode create run response")?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing /run/id in create response"))?;

    let append_response = client
        .post(format!("{base_url}/internal/v1/runs/{run_id}/events"))
        .json(&json!({
            "event_type": "run.started",
            "idempotency_key": format!("smoke-{}", Uuid::now_v7()),
            "payload": { "smoke": true }
        }))
        .send()
        .await
        .context("POST /internal/v1/runs/:run_id/events")?;
    ensure_status(
        append_response.status(),
        StatusCode::OK,
        "append runtime run event",
    )?;

    expect_status(
        &client,
        &format!("{base_url}/internal/v1/runs/{run_id}"),
        StatusCode::OK,
        "fetch runtime run",
    )
    .await?;

    tracing::info!(base_url, run_id, "runtime smoke passed");
    Ok(())
}

async fn expect_status(
    client: &reqwest::Client,
    url: &str,
    expected: StatusCode,
    operation: &str,
) -> Result<()> {
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    ensure_status(response.status(), expected, operation)
}

fn ensure_status(actual: StatusCode, expected: StatusCode, operation: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(anyhow!(
            "{operation} returned unexpected status {actual} (expected {expected})"
        ))
    }
}
