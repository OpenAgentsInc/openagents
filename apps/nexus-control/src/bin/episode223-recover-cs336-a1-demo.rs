use anyhow::{Context, Result, anyhow, bail};
use nexus_control::{DesktopSessionCreateRequest, DesktopSessionResponse};
use openagents_kernel_core::compute::{ComputeAdapterTrainingWindow, ComputeAdapterWindowStatus};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_DESKTOP_CLIENT_ID: &str = "episode223-cs336-a1-recover";
const DEFAULT_DEVICE_NAME: &str = "Episode 223 Recovery";
const DEFAULT_CLIENT_VERSION: &str = "episode223-cs336-a1-recover/v1";
const TRAINING_RUN_ID: &str = "run.cs336.a1.demo";

fn main() -> Result<()> {
    ensure_rustls_crypto_provider()?;
    let args = RecoverArgs::parse(std::env::args().skip(1))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;
    runtime.block_on(async_main(args))
}

async fn async_main(args: RecoverArgs) -> Result<()> {
    let session = mint_desktop_session(&args).await?;
    let client = Client::builder()
        .build()
        .context("failed to build reqwest client")?;
    let windows_endpoint = format!(
        "{}/api/training/windows?training_run_id={TRAINING_RUN_ID}",
        args.base_url.trim_end_matches('/')
    );
    let windows = client
        .get(windows_endpoint)
        .bearer_auth(session.access_token.as_str())
        .send()
        .await
        .context("failed to query training windows")?
        .error_for_status()
        .context("training windows request failed")?
        .json::<Vec<ComputeAdapterTrainingWindow>>()
        .await
        .context("failed to decode training windows response")?;
    let Some(active_window) = windows
        .into_iter()
        .filter(|window| window.status == ComputeAdapterWindowStatus::Active)
        .max_by(|lhs, rhs| {
            lhs.recorded_at_ms
                .cmp(&rhs.recorded_at_ms)
                .then_with(|| lhs.window_id.cmp(&rhs.window_id))
        })
    else {
        println!("recovered_empty_window=false");
        println!("reason=no_active_window");
        println!("training_run_id={TRAINING_RUN_ID}");
        return Ok(());
    };

    let scheduler_metadata = active_window
        .metadata
        .get("pylon_training_window")
        .cloned()
        .context("active window missing pylon_training_window metadata")?;
    let seal_deadline_ms = scheduler_metadata
        .get("seal_deadline_ms")
        .and_then(Value::as_i64)
        .context("active window missing seal_deadline_ms")?;
    let assignment_pubkeys = scheduler_metadata
        .get("assignment_plans")
        .and_then(Value::as_array)
        .map(|plans| {
            plans
                .iter()
                .filter_map(|plan| {
                    plan.get("node_pubkey_hex")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let now_ms = now_unix_ms();
    if active_window.total_contributions > 0
        || active_window.admitted_contributions > 0
        || active_window.accepted_contributions > 0
        || active_window.quarantined_contributions > 0
        || active_window.rejected_contributions > 0
        || active_window.replay_required_contributions > 0
    {
        bail!(
            "active window {} is not empty and cannot use empty-window recovery",
            active_window.window_id
        );
    }
    if seal_deadline_ms > now_ms {
        bail!(
            "active window {} is not overdue yet (seal_deadline_ms={seal_deadline_ms}, now_ms={now_ms})",
            active_window.window_id
        );
    }

    let recover_endpoint = format!(
        "{}/api/training/windows/{}/recover-empty",
        args.base_url.trim_end_matches('/'),
        active_window.window_id
    );
    let recover_request = RecoverEmptyTrainingWindowRequest {
        idempotency_key: format!(
            "episode223.cs336_a1_demo.recover_empty.{}",
            active_window.window_id
        ),
        recorded_at_ms: now_ms,
        window_id: active_window.window_id.clone(),
    };
    let recovered = client
        .post(recover_endpoint)
        .bearer_auth(session.access_token.as_str())
        .json(&recover_request)
        .send()
        .await
        .context("failed to post empty-window recovery")?
        .error_for_status()
        .context("empty-window recovery request failed")?
        .json::<TrainingWindowCoordinatorResponse>()
        .await
        .context("failed to decode empty-window recovery response")?;

    let summary_endpoint = format!(
        "{}/api/training/summary",
        args.base_url.trim_end_matches('/')
    );
    let summary = client
        .get(summary_endpoint)
        .bearer_auth(session.access_token.as_str())
        .send()
        .await
        .context("failed to fetch training summary")?
        .error_for_status()
        .context("training summary request failed")?
        .json::<Value>()
        .await
        .context("failed to decode training summary response")?;
    let (current_window_id, scheduler_window_state) = summary
        .get("runs")
        .and_then(Value::as_array)
        .and_then(|runs| {
            runs.iter().find_map(|run| {
                (run.get("training_run_id").and_then(Value::as_str) == Some(TRAINING_RUN_ID)).then(
                    || {
                        (
                            run.get("current_window_id")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            run.get("scheduler_window_state")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                        )
                    },
                )
            })
        })
        .unwrap_or_default();

    println!("recovered_empty_window=true");
    println!("base_url={}", args.base_url);
    println!("training_run_id={TRAINING_RUN_ID}");
    println!("recovered_window_id={}", recovered.window.window_id);
    println!(
        "recovered_window_status={}",
        recovered.window.status.label()
    );
    println!("assignment_pubkeys={}", assignment_pubkeys.join(","));
    println!("next_window_id={current_window_id}");
    println!("scheduler_window_state={scheduler_window_state}");
    Ok(())
}

#[derive(Clone, Debug)]
struct RecoverArgs {
    base_url: String,
    desktop_client_id: String,
    device_name: String,
    client_version: String,
}

impl RecoverArgs {
    fn parse<I>(mut args: I) -> Result<Self>
    where
        I: Iterator<Item = String>,
    {
        let mut parsed = Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            desktop_client_id: DEFAULT_DESKTOP_CLIENT_ID.to_string(),
            device_name: DEFAULT_DEVICE_NAME.to_string(),
            client_version: DEFAULT_CLIENT_VERSION.to_string(),
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--base-url" => parsed.base_url = next_arg(&mut args, "--base-url")?,
                "--desktop-client-id" => {
                    parsed.desktop_client_id = next_arg(&mut args, "--desktop-client-id")?
                }
                "--device-name" => parsed.device_name = next_arg(&mut args, "--device-name")?,
                "--client-version" => {
                    parsed.client_version = next_arg(&mut args, "--client-version")?
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => bail!("unknown argument `{other}`"),
            }
        }

        Ok(parsed)
    }
}

fn next_arg<I>(args: &mut I, flag: &str) -> Result<String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .with_context(|| format!("{flag} requires a value"))
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p nexus-control --bin episode223-recover-cs336-a1-demo -- [options]\n\
         \n\
         options:\n\
           --base-url <url>              Nexus base URL (default: {DEFAULT_BASE_URL})\n\
           --desktop-client-id <id>      Session client id\n\
           --device-name <name>          Session device name\n\
           --client-version <version>    Session client version"
    );
}

async fn mint_desktop_session(args: &RecoverArgs) -> Result<DesktopSessionResponse> {
    let client = Client::builder()
        .build()
        .context("failed to build reqwest client")?;
    let endpoint = format!(
        "{}/api/session/desktop",
        args.base_url.trim_end_matches('/')
    );
    let response = client
        .post(endpoint)
        .json(&DesktopSessionCreateRequest {
            desktop_client_id: args.desktop_client_id.clone(),
            device_name: Some(args.device_name.clone()),
            bound_nostr_pubkey: None,
            client_version: Some(args.client_version.clone()),
        })
        .send()
        .await
        .context("failed to mint desktop session")?;
    let response = response.error_for_status().map_err(|error| {
        anyhow!(
            "failed to mint desktop session against {}: {error}",
            args.base_url
        )
    })?;
    response
        .json::<DesktopSessionResponse>()
        .await
        .context("failed to decode desktop session response")
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn ensure_rustls_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|error| anyhow!("failed to install rustls crypto provider: {error:?}"))
}

#[derive(Debug, Serialize)]
struct RecoverEmptyTrainingWindowRequest {
    idempotency_key: String,
    recorded_at_ms: i64,
    window_id: String,
}

#[derive(Debug, Deserialize)]
struct TrainingWindowCoordinatorResponse {
    window: ComputeAdapterTrainingWindow,
}
