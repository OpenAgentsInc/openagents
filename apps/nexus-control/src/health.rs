use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_NEXUS_BASE_URL: &str = "https://nexus.openagents.com";
const DEFAULT_TIMEOUT_MS: u64 = 8_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSnapshotCommand {
    pub base_url: String,
    pub timeout_ms: u64,
    pub fake: bool,
    pub pretty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthSnapshot {
    pub schema_version: u32,
    pub generated_at_unix_ms: u64,
    pub base_url: String,
    pub observation_status: String,
    pub endpoints: BTreeMap<String, NexusHealthEndpointSnapshot>,
    pub treasury: NexusHealthTreasurySnapshot,
    pub training: NexusHealthTrainingSnapshot,
    pub fleet: NexusHealthFleetSnapshot,
    pub website: NexusHealthWebsiteSnapshot,
    pub infra: NexusHealthInfraSnapshot,
    #[serde(default)]
    pub issues: Vec<NexusHealthIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthEndpointSnapshot {
    pub route_id: String,
    pub path: String,
    pub url: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflare_error_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthTreasurySnapshot {
    pub treasury_enabled: bool,
    pub wallet_connected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_status: Option<String>,
    pub wallet_balance_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_sync_lag_ms: Option<u64>,
    pub registered_payout_identities: u64,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub payout_loop_health: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_runtime_status: Option<String>,
    pub payout_sats_in_flight_total: u64,
    pub payout_sats_in_flight_24h: u64,
    pub accepted_work_payout_sats_in_flight_total: u64,
    pub availability_stipend_payout_sats_in_flight_total: u64,
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub pending_confirmation_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dispatch_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_confirmed_payout_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_lag_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm_lag_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_runway_windows: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_runway_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthTrainingSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_run_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_window_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_health_overall_status: Option<String>,
    pub nodes_online: u64,
    pub admitted_nodes_online: u64,
    pub runs_active: u64,
    pub windows_active: u64,
    pub windows_pending_validation: u64,
    pub validator_challenges_open: u64,
    pub validator_challenges_queued: u64,
    pub accepted_closeouts: u64,
    pub payout_eligible_closeouts: u64,
    pub accepted_work_pending_payout_count: u64,
    pub accepted_work_attention_payout_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthFleetSnapshot {
    pub pylons_online_now: u64,
    pub pylon_sessions_online_now: u64,
    pub sellable_pylons_online_now: u64,
    pub inference_ready_pylons_online_now: u64,
    pub pylon_reported_hosts_online_now: u64,
    pub eligible_online_payout_targets: u64,
    pub duplicate_host_blocked_beneficiaries_now: u64,
    pub duplicate_payout_target_blocked_beneficiaries_now: u64,
    pub missing_payout_target_blocked_beneficiaries_now: u64,
    pub version_floor_blocked_beneficiaries_now: u64,
    pub readiness_blocked_beneficiaries_now: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthWebsiteSnapshot {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats_as_of_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats_age_ms: Option<u64>,
    pub stats_fresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthInfraSnapshot {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_vm_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_relay_service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflared_service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthIssue {
    pub code: String,
    pub severity: String,
    pub detail: String,
}

struct EndpointFetch {
    endpoint: NexusHealthEndpointSnapshot,
    body: Option<Value>,
}

pub fn health_snapshot_usage() -> &'static str {
    "health snapshot [--base-url <url>] [--timeout-ms <ms>] [--fake] [--pretty|--json]"
}

pub fn parse_health_snapshot_command(args: &[String]) -> Result<HealthSnapshotCommand> {
    let mut index = match args.get(2).map(String::as_str) {
        None => 2,
        Some("snapshot") => 3,
        Some(other) => bail!("unknown health command `{other}`"),
    };
    let mut command = HealthSnapshotCommand {
        base_url: DEFAULT_NEXUS_BASE_URL.to_string(),
        timeout_ms: DEFAULT_TIMEOUT_MS,
        fake: false,
        pretty: false,
    };
    while let Some(arg) = args.get(index).map(String::as_str) {
        match arg {
            "--base-url" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    bail!("--base-url requires a value");
                };
                command.base_url = value.clone();
            }
            "--timeout-ms" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    bail!("--timeout-ms requires a value");
                };
                command.timeout_ms = value
                    .parse::<u64>()
                    .with_context(|| format!("invalid --timeout-ms value `{value}`"))?;
                if command.timeout_ms == 0 {
                    bail!("--timeout-ms must be greater than zero");
                }
            }
            "--fake" => command.fake = true,
            "--pretty" => command.pretty = true,
            "--json" => command.pretty = false,
            "--help" | "-h" => bail!("usage: nexus-control {}", health_snapshot_usage()),
            other => bail!("unknown health snapshot option `{other}`"),
        }
        index += 1;
    }
    validate_base_url(command.base_url.as_str())?;
    Ok(command)
}

pub async fn run_health_snapshot_command(command: &HealthSnapshotCommand) -> Result<String> {
    let snapshot = if command.fake {
        fake_nexus_health_snapshot(command.base_url.as_str())?
    } else {
        fetch_nexus_health_snapshot(command.base_url.as_str(), command.timeout_ms).await?
    };
    if command.pretty {
        serde_json::to_string_pretty(&snapshot).context("serialize nexus health snapshot")
    } else {
        serde_json::to_string(&snapshot).context("serialize nexus health snapshot")
    }
}

async fn fetch_nexus_health_snapshot(
    base_url: &str,
    timeout_ms: u64,
) -> Result<NexusHealthSnapshot> {
    let base_url = normalize_base_url(base_url)?;
    let client = Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .context("build health snapshot HTTP client")?;
    let generated_at_unix_ms = now_unix_ms();
    let healthz = fetch_endpoint(&client, &base_url, "healthz", "/healthz").await;
    let stats = fetch_endpoint(&client, &base_url, "stats", "/api/stats").await;
    let treasury = fetch_endpoint(&client, &base_url, "treasury", "/v1/treasury/status").await;
    Ok(snapshot_from_fetches(
        base_url.as_str(),
        generated_at_unix_ms,
        healthz,
        stats,
        treasury,
    ))
}

async fn fetch_endpoint(
    client: &Client,
    base_url: &Url,
    route_id: &str,
    path: &str,
) -> EndpointFetch {
    let url = match endpoint_url(base_url, path) {
        Ok(url) => url,
        Err(error) => {
            return EndpointFetch {
                endpoint: NexusHealthEndpointSnapshot {
                    route_id: route_id.to_string(),
                    path: path.to_string(),
                    url: base_url.to_string(),
                    ok: false,
                    status_code: None,
                    latency_ms: None,
                    cloudflare_error_code: None,
                    error: Some(redact_sensitive_text(error.to_string().as_str())),
                },
                body: None,
            };
        }
    };
    let started = Instant::now();
    let response = client.get(url.clone()).send().await;
    let latency_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    match response {
        Ok(response) => {
            let status = response.status();
            let status_code = status.as_u16();
            let text = response
                .text()
                .await
                .unwrap_or_else(|error| error.to_string());
            let cloudflare_error_code = cloudflare_error_code(status_code, text.as_str());
            let body = serde_json::from_str::<Value>(text.as_str()).ok();
            let error = (!status.is_success()).then(|| {
                format!(
                    "http_status_{status_code}{}",
                    cloudflare_error_code
                        .map(|code| format!("_cloudflare_{code}"))
                        .unwrap_or_default()
                )
            });
            EndpointFetch {
                endpoint: NexusHealthEndpointSnapshot {
                    route_id: route_id.to_string(),
                    path: path.to_string(),
                    url: url.to_string(),
                    ok: status.is_success(),
                    status_code: Some(status_code),
                    latency_ms: Some(latency_ms),
                    cloudflare_error_code,
                    error,
                },
                body,
            }
        }
        Err(error) => EndpointFetch {
            endpoint: NexusHealthEndpointSnapshot {
                route_id: route_id.to_string(),
                path: path.to_string(),
                url: url.to_string(),
                ok: false,
                status_code: error.status().map(|status| status.as_u16()),
                latency_ms: Some(latency_ms),
                cloudflare_error_code: error
                    .status()
                    .and_then(|status| cloudflare_error_code(status.as_u16(), "")),
                error: Some(redact_sensitive_text(error.to_string().as_str())),
            },
            body: None,
        },
    }
}

fn snapshot_from_fetches(
    base_url: &str,
    generated_at_unix_ms: u64,
    healthz: EndpointFetch,
    stats: EndpointFetch,
    treasury: EndpointFetch,
) -> NexusHealthSnapshot {
    let stats_body = stats.body.as_ref();
    let treasury_body = treasury.body.as_ref();
    let mut endpoints = BTreeMap::new();
    endpoints.insert("healthz".to_string(), healthz.endpoint);
    endpoints.insert("stats".to_string(), stats.endpoint);
    endpoints.insert("treasury".to_string(), treasury.endpoint);
    let mut issues = endpoint_issues(&endpoints);
    let treasury_snapshot = treasury_snapshot(treasury_body, stats_body);
    let training_snapshot = training_snapshot(stats_body, treasury_body);
    let fleet_snapshot = fleet_snapshot(stats_body, treasury_body);
    let website_snapshot = website_snapshot(stats_body, generated_at_unix_ms);
    let infra_snapshot = infra_snapshot();
    if let Some(reason) = treasury_snapshot.degraded_reason.as_ref() {
        issues.push(NexusHealthIssue {
            code: "treasury_degraded".to_string(),
            severity: "critical".to_string(),
            detail: reason.clone(),
        });
    }
    if !treasury_snapshot.wallet_connected {
        issues.push(NexusHealthIssue {
            code: "treasury_wallet_not_connected".to_string(),
            severity: "critical".to_string(),
            detail: "treasury wallet runtime is not connected".to_string(),
        });
    }
    if !website_snapshot.stats_fresh {
        issues.push(NexusHealthIssue {
            code: "stats_stale".to_string(),
            severity: "warning".to_string(),
            detail: "Nexus public stats are stale or missing as-of time".to_string(),
        });
    }
    let observation_status = if issues
        .iter()
        .any(|issue| issue.severity == "critical" || issue.severity == "error")
    {
        "degraded"
    } else if issues.is_empty() {
        "observed"
    } else {
        "watch"
    }
    .to_string();
    NexusHealthSnapshot {
        schema_version: 1,
        generated_at_unix_ms,
        base_url: base_url.to_string(),
        observation_status,
        endpoints,
        treasury: treasury_snapshot,
        training: training_snapshot,
        fleet: fleet_snapshot,
        website: website_snapshot,
        infra: infra_snapshot,
        issues,
    }
}

fn fake_nexus_health_snapshot(base_url: &str) -> Result<NexusHealthSnapshot> {
    let base_url = normalize_base_url(base_url)?;
    let generated_at_unix_ms = 1_777_200_000_000;
    let healthz = fake_fetch(
        "healthz",
        base_url.as_str(),
        "/healthz",
        json!({"ok": true}),
    );
    let stats = fake_fetch(
        "stats",
        base_url.as_str(),
        "/api/stats",
        json!({
            "as_of_unix_ms": generated_at_unix_ms - 1_000,
            "pylons_online_now": 8,
            "pylon_sessions_online_now": 9,
            "sellable_pylons_online_now": 7,
            "inference_ready_pylons_online_now": 7,
            "pylon_reported_hosts_online_now": 5,
            "training_nodes_online": 4,
            "training_admitted_nodes_online": 4,
            "training_runs_active": 2,
            "training_windows_active": 1,
            "training_windows_pending_validation": 1,
            "training_validator_challenges_open": 3,
            "training_validator_challenges_queued": 2,
            "training_accepted_closeouts": 6,
            "training_payout_eligible_closeouts": 5,
            "training_public_state": {
                "active_run_id": "run.fake.active",
                "active_window_id": "window.fake.active.0001",
                "launch_health": {"overall_status": "ok"},
                "runs": [{
                    "training_run_id": "run.fake.active",
                    "current_window_id": "window.fake.active.0001",
                    "run_status": "running",
                    "scheduler_window_state": "active"
                }]
            }
        }),
    );
    let treasury = fake_fetch(
        "treasury",
        base_url.as_str(),
        "/v1/treasury/status",
        json!({
            "treasury_enabled": true,
            "wallet_runtime_status": "connected",
            "wallet_balance_sats": 25_000,
            "wallet_balance_updated_at_unix_ms": generated_at_unix_ms - 2_000,
            "wallet_sync_lag_ms": 2_000,
            "registered_payout_identities": 12,
            "payout_sats_per_window": 25,
            "payout_interval_seconds": 60,
            "payout_loop_health": "idle",
            "payout_loop_runtime_status": "idle",
            "payout_sats_in_flight_total": 50,
            "payout_sats_in_flight_24h": 50,
            "accepted_work_payout_sats_in_flight_total": 25,
            "availability_stipend_payout_sats_in_flight_total": 25,
            "payouts_dispatched_24h": 4,
            "payouts_confirmed_24h": 6,
            "payouts_failed_24h": 0,
            "pending_confirmation_count": 2,
            "last_dispatch_at_unix_ms": generated_at_unix_ms - 30_000,
            "last_confirmed_payout_at_unix_ms": generated_at_unix_ms - 45_000,
            "dispatch_lag_ms": 30_000,
            "confirm_lag_ms": 45_000,
            "eligible_online_payout_targets": 7,
            "duplicate_host_blocked_beneficiaries_now": 1,
            "duplicate_payout_target_blocked_beneficiaries_now": 0,
            "missing_payout_target_blocked_beneficiaries_now": 0,
            "version_floor_blocked_beneficiaries_now": 0,
            "readiness_blocked_beneficiaries_now": 0,
            "training_payout_ledger_summary": {
                "accepted_work_pending_payout_count": 1,
                "accepted_work_attention_payout_count": 0
            }
        }),
    );
    Ok(snapshot_from_fetches(
        base_url.as_str(),
        generated_at_unix_ms,
        healthz,
        stats,
        treasury,
    ))
}

fn fake_fetch(route_id: &str, base_url: &str, path: &str, body: Value) -> EndpointFetch {
    EndpointFetch {
        endpoint: NexusHealthEndpointSnapshot {
            route_id: route_id.to_string(),
            path: path.to_string(),
            url: format!("{}{}", base_url.trim_end_matches('/'), path),
            ok: true,
            status_code: Some(200),
            latency_ms: Some(1),
            cloudflare_error_code: None,
            error: None,
        },
        body: Some(body),
    }
}

fn endpoint_issues(
    endpoints: &BTreeMap<String, NexusHealthEndpointSnapshot>,
) -> Vec<NexusHealthIssue> {
    endpoints
        .values()
        .filter(|endpoint| !endpoint.ok)
        .map(|endpoint| NexusHealthIssue {
            code: format!("endpoint_{}_failed", endpoint.route_id),
            severity: if endpoint.cloudflare_error_code.is_some() {
                "critical".to_string()
            } else {
                "error".to_string()
            },
            detail: endpoint
                .error
                .clone()
                .unwrap_or_else(|| "endpoint probe failed".to_string()),
        })
        .collect()
}

fn treasury_snapshot(
    treasury: Option<&Value>,
    stats: Option<&Value>,
) -> NexusHealthTreasurySnapshot {
    let payout_sats_per_window = first_u64(
        &[
            treasury,
            stats.and_then(|value| value.get("nexus_treasury_payout_sats_per_window")),
        ],
        &["payout_sats_per_window"],
    )
    .unwrap_or(0);
    let payout_interval_seconds = first_u64(
        &[
            treasury,
            stats.and_then(|value| value.get("nexus_treasury_payout_interval_seconds")),
        ],
        &["payout_interval_seconds"],
    )
    .unwrap_or(0);
    let wallet_balance_sats = first_u64(
        &[
            treasury,
            stats.and_then(|value| value.get("nexus_wallet_balance_sats")),
        ],
        &["wallet_balance_sats"],
    )
    .unwrap_or(0);
    let wallet_runtime_status = first_string(
        &[
            treasury,
            stats.and_then(|value| value.get("nexus_wallet_runtime_status")),
        ],
        &["wallet_runtime_status"],
    );
    let balance_runway_windows =
        (payout_sats_per_window > 0).then(|| wallet_balance_sats / payout_sats_per_window);
    let balance_runway_seconds =
        balance_runway_windows.map(|windows| windows.saturating_mul(payout_interval_seconds));
    NexusHealthTreasurySnapshot {
        treasury_enabled: first_bool(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_treasury_enabled")),
            ],
            &["treasury_enabled"],
        )
        .unwrap_or(false),
        wallet_connected: wallet_runtime_status.as_deref() == Some("connected"),
        degraded_reason: first_string(&[treasury], &["degraded_reason"]),
        wallet_runtime_status,
        wallet_balance_sats,
        wallet_balance_updated_at_unix_ms: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_wallet_balance_updated_at_unix_ms")),
            ],
            &["wallet_balance_updated_at_unix_ms"],
        ),
        wallet_sync_lag_ms: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_wallet_sync_lag_ms")),
            ],
            &["wallet_sync_lag_ms"],
        ),
        registered_payout_identities: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_registered_payout_identities")),
            ],
            &["registered_payout_identities"],
        )
        .unwrap_or(0),
        payout_sats_per_window,
        payout_interval_seconds,
        payout_loop_health: first_string(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_payout_loop_health")),
            ],
            &["payout_loop_health"],
        )
        .unwrap_or_else(|| "unknown".to_string()),
        payout_loop_runtime_status: first_string(&[treasury], &["payout_loop_runtime_status"]),
        payout_sats_in_flight_total: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_payout_sats_in_flight_total")),
            ],
            &["payout_sats_in_flight_total"],
        )
        .unwrap_or(0),
        payout_sats_in_flight_24h: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_payout_sats_in_flight_24h")),
            ],
            &["payout_sats_in_flight_24h"],
        )
        .unwrap_or(0),
        accepted_work_payout_sats_in_flight_total: first_u64(
            &[
                treasury,
                stats
                    .and_then(|value| value.get("nexus_accepted_work_payout_sats_in_flight_total")),
            ],
            &["accepted_work_payout_sats_in_flight_total"],
        )
        .unwrap_or(0),
        availability_stipend_payout_sats_in_flight_total: first_u64(
            &[
                treasury,
                stats.and_then(|value| {
                    value.get("nexus_availability_stipend_payout_sats_in_flight_total")
                }),
            ],
            &["availability_stipend_payout_sats_in_flight_total"],
        )
        .unwrap_or(0),
        payouts_dispatched_24h: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_payouts_dispatched_24h")),
            ],
            &["payouts_dispatched_24h"],
        )
        .unwrap_or(0),
        payouts_confirmed_24h: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_payouts_confirmed_24h")),
            ],
            &["payouts_confirmed_24h"],
        )
        .unwrap_or(0),
        payouts_failed_24h: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_payouts_failed_24h")),
            ],
            &["payouts_failed_24h"],
        )
        .unwrap_or(0),
        pending_confirmation_count: first_u64(&[treasury], &["pending_confirmation_count"])
            .unwrap_or(0),
        last_dispatch_at_unix_ms: first_u64(&[treasury], &["last_dispatch_at_unix_ms"]),
        last_confirmed_payout_at_unix_ms: first_u64(
            &[treasury],
            &["last_confirmed_payout_at_unix_ms"],
        ),
        dispatch_lag_ms: first_u64(&[treasury], &["dispatch_lag_ms"]),
        confirm_lag_ms: first_u64(&[treasury], &["confirm_lag_ms"]),
        balance_runway_windows,
        balance_runway_seconds,
    }
}

fn training_snapshot(
    stats: Option<&Value>,
    treasury: Option<&Value>,
) -> NexusHealthTrainingSnapshot {
    let training_public = stats.and_then(|value| value.get("training_public_state"));
    let latest_run = training_public
        .and_then(|value| value.get("runs"))
        .and_then(Value::as_array)
        .and_then(|runs| runs.first());
    let training_ledger = treasury.and_then(|value| value.get("training_payout_ledger_summary"));
    NexusHealthTrainingSnapshot {
        active_run_id: first_string(&[training_public], &["active_run_id"]),
        active_window_id: first_string(&[training_public], &["active_window_id"]),
        latest_run_id: first_string(&[latest_run], &["training_run_id"]),
        latest_window_id: first_string(&[latest_run], &["current_window_id"]),
        latest_run_status: first_string(&[latest_run], &["run_status"]),
        latest_window_state: first_string(&[latest_run], &["scheduler_window_state"]),
        launch_health_overall_status: training_public
            .and_then(|value| value.get("launch_health"))
            .and_then(|value| string_field(value, "overall_status")),
        nodes_online: first_u64(&[stats], &["training_nodes_online"]).unwrap_or(0),
        admitted_nodes_online: first_u64(&[stats], &["training_admitted_nodes_online"])
            .unwrap_or(0),
        runs_active: first_u64(&[stats], &["training_runs_active"]).unwrap_or(0),
        windows_active: first_u64(&[stats], &["training_windows_active"]).unwrap_or(0),
        windows_pending_validation: first_u64(&[stats], &["training_windows_pending_validation"])
            .unwrap_or(0),
        validator_challenges_open: first_u64(&[stats], &["training_validator_challenges_open"])
            .unwrap_or(0),
        validator_challenges_queued: first_u64(&[stats], &["training_validator_challenges_queued"])
            .unwrap_or(0),
        accepted_closeouts: first_u64(&[stats], &["training_accepted_closeouts"]).unwrap_or(0),
        payout_eligible_closeouts: first_u64(&[stats], &["training_payout_eligible_closeouts"])
            .unwrap_or(0),
        accepted_work_pending_payout_count: first_u64(
            &[training_ledger],
            &["accepted_work_pending_payout_count"],
        )
        .unwrap_or(0),
        accepted_work_attention_payout_count: first_u64(
            &[training_ledger],
            &["accepted_work_attention_payout_count"],
        )
        .unwrap_or(0),
    }
}

fn fleet_snapshot(stats: Option<&Value>, treasury: Option<&Value>) -> NexusHealthFleetSnapshot {
    NexusHealthFleetSnapshot {
        pylons_online_now: first_u64(&[stats], &["pylons_online_now"]).unwrap_or(0),
        pylon_sessions_online_now: first_u64(&[stats], &["pylon_sessions_online_now"]).unwrap_or(0),
        sellable_pylons_online_now: first_u64(&[stats], &["sellable_pylons_online_now"])
            .unwrap_or(0),
        inference_ready_pylons_online_now: first_u64(
            &[stats],
            &["inference_ready_pylons_online_now"],
        )
        .unwrap_or(0),
        pylon_reported_hosts_online_now: first_u64(&[stats], &["pylon_reported_hosts_online_now"])
            .unwrap_or(0),
        eligible_online_payout_targets: first_u64(
            &[
                treasury,
                stats.and_then(|value| {
                    value.get("nexus_placeholder_payout_eligible_online_targets")
                }),
            ],
            &["eligible_online_payout_targets"],
        )
        .unwrap_or(0),
        duplicate_host_blocked_beneficiaries_now: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_duplicate_host_blocked_beneficiaries_now")),
            ],
            &["duplicate_host_blocked_beneficiaries_now"],
        )
        .unwrap_or(0),
        duplicate_payout_target_blocked_beneficiaries_now: first_u64(
            &[
                treasury,
                stats.and_then(|value| {
                    value.get("nexus_duplicate_payout_target_blocked_beneficiaries_now")
                }),
            ],
            &["duplicate_payout_target_blocked_beneficiaries_now"],
        )
        .unwrap_or(0),
        missing_payout_target_blocked_beneficiaries_now: first_u64(
            &[
                treasury,
                stats.and_then(|value| {
                    value.get("nexus_missing_payout_target_blocked_beneficiaries_now")
                }),
            ],
            &["missing_payout_target_blocked_beneficiaries_now"],
        )
        .unwrap_or(0),
        version_floor_blocked_beneficiaries_now: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_version_floor_blocked_beneficiaries_now")),
            ],
            &["version_floor_blocked_beneficiaries_now"],
        )
        .unwrap_or(0),
        readiness_blocked_beneficiaries_now: first_u64(
            &[
                treasury,
                stats.and_then(|value| value.get("nexus_readiness_blocked_beneficiaries_now")),
            ],
            &["readiness_blocked_beneficiaries_now"],
        )
        .unwrap_or(0),
    }
}

fn website_snapshot(
    stats: Option<&Value>,
    generated_at_unix_ms: u64,
) -> NexusHealthWebsiteSnapshot {
    let stats_as_of_unix_ms = first_u64(&[stats], &["as_of_unix_ms"]);
    let stats_age_ms = stats_as_of_unix_ms.map(|as_of| generated_at_unix_ms.saturating_sub(as_of));
    NexusHealthWebsiteSnapshot {
        source: "nexus_api_stats".to_string(),
        stats_as_of_unix_ms,
        stats_age_ms,
        stats_fresh: stats_age_ms.is_some_and(|age| age <= 120_000),
    }
}

fn infra_snapshot() -> NexusHealthInfraSnapshot {
    let google_project_id = std::env::var("GOOGLE_CLOUD_PROJECT")
        .ok()
        .or_else(|| std::env::var("CLOUDSDK_CORE_PROJECT").ok());
    let nexus_vm_name = std::env::var("NEXUS_HEALTH_GCP_VM_NAME")
        .ok()
        .or_else(|| Some("nexus-mainnet-1".to_string()));
    NexusHealthInfraSnapshot {
        source: if google_project_id.is_some() {
            "env".to_string()
        } else {
            "not_available".to_string()
        },
        google_project_id,
        nexus_vm_name,
        nexus_relay_service: Some("nexus-relay".to_string()),
        cloudflared_service: Some("nexus-cloudflared".to_string()),
        note: Some(
            "GCP runtime state is not queried by this observation-only snapshot command"
                .to_string(),
        ),
    }
}

fn first_u64(candidates: &[Option<&Value>], keys: &[&str]) -> Option<u64> {
    candidates.iter().find_map(|candidate| {
        let value = (*candidate)?;
        if keys.is_empty() {
            value_to_u64(value)
        } else {
            keys.iter()
                .find_map(|key| value.get(*key).and_then(value_to_u64))
        }
    })
}

fn first_bool(candidates: &[Option<&Value>], keys: &[&str]) -> Option<bool> {
    candidates.iter().find_map(|candidate| {
        let value = (*candidate)?;
        keys.iter()
            .find_map(|key| value.get(*key).and_then(Value::as_bool))
    })
}

fn first_string(candidates: &[Option<&Value>], keys: &[&str]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        let value = (*candidate)?;
        keys.iter().find_map(|key| string_field(value, key))
    })
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(redact_sensitive_text)
}

fn value_to_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
}

fn validate_base_url(base_url: &str) -> Result<()> {
    normalize_base_url(base_url).map(|_| ())
}

fn normalize_base_url(base_url: &str) -> Result<Url> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        bail!("base URL cannot be empty");
    }
    let normalized = if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    };
    Url::parse(normalized.as_str()).with_context(|| format!("invalid base URL `{trimmed}`"))
}

fn endpoint_url(base_url: &Url, path: &str) -> Result<Url> {
    base_url
        .join(path.trim_start_matches('/'))
        .with_context(|| format!("invalid endpoint path `{path}`"))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

fn cloudflare_error_code(status_code: u16, body: &str) -> Option<u16> {
    if status_code == 530 || body.contains("Error 530") {
        Some(530)
    } else if body.contains("error code: 1033") || body.contains("Error 1033") {
        Some(1033)
    } else {
        None
    }
}

fn redact_sensitive_text(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let sensitive_markers = [
        "bearer ",
        "authorization",
        "private_key",
        "mnemonic",
        "payment_preimage",
        "preimage",
        "bolt11",
        "lnbc",
        "secret",
        "api_key",
        "token=",
        "access_token",
    ];
    if sensitive_markers
        .iter()
        .any(|marker| lower.contains(marker))
    {
        "[redacted]".to_string()
    } else {
        value.chars().take(240).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_health_snapshot_command_defaults_to_public_nexus() {
        let args = vec!["nexus-control".to_string(), "health".to_string()];
        let command = parse_health_snapshot_command(args.as_slice()).expect("parse command");
        assert_eq!(command.base_url, DEFAULT_NEXUS_BASE_URL);
        assert_eq!(command.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert!(!command.fake);
    }

    #[test]
    fn parse_health_snapshot_command_accepts_options() {
        let args = vec![
            "nexus-control".to_string(),
            "health".to_string(),
            "snapshot".to_string(),
            "--base-url".to_string(),
            "http://127.0.0.1:8080".to_string(),
            "--timeout-ms".to_string(),
            "1234".to_string(),
            "--fake".to_string(),
            "--pretty".to_string(),
        ];
        let command = parse_health_snapshot_command(args.as_slice()).expect("parse command");
        assert_eq!(command.base_url, "http://127.0.0.1:8080");
        assert_eq!(command.timeout_ms, 1234);
        assert!(command.fake);
        assert!(command.pretty);
    }

    #[test]
    fn fake_snapshot_contains_required_sections() {
        let snapshot = fake_nexus_health_snapshot(DEFAULT_NEXUS_BASE_URL).expect("fake snapshot");
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.observation_status, "observed");
        assert!(snapshot.endpoints.contains_key("healthz"));
        assert!(snapshot.endpoints.contains_key("stats"));
        assert!(snapshot.endpoints.contains_key("treasury"));
        assert_eq!(snapshot.treasury.wallet_balance_sats, 25_000);
        assert_eq!(
            snapshot.training.latest_run_id.as_deref(),
            Some("run.fake.active")
        );
        assert_eq!(snapshot.fleet.pylons_online_now, 8);
        assert!(snapshot.website.stats_fresh);
        assert_eq!(
            snapshot.infra.nexus_relay_service.as_deref(),
            Some("nexus-relay")
        );
    }

    #[test]
    fn endpoint_failures_are_structured_without_panics() {
        let generated_at_unix_ms = 1_777_200_000_000;
        let failed = EndpointFetch {
            endpoint: NexusHealthEndpointSnapshot {
                route_id: "stats".to_string(),
                path: "/api/stats".to_string(),
                url: "https://nexus.openagents.com/api/stats".to_string(),
                ok: false,
                status_code: Some(530),
                latency_ms: Some(10),
                cloudflare_error_code: Some(530),
                error: Some("http_status_530_cloudflare_530".to_string()),
            },
            body: None,
        };
        let snapshot = snapshot_from_fetches(
            DEFAULT_NEXUS_BASE_URL,
            generated_at_unix_ms,
            fake_fetch(
                "healthz",
                DEFAULT_NEXUS_BASE_URL,
                "/healthz",
                json!({"ok": true}),
            ),
            failed,
            fake_fetch(
                "treasury",
                DEFAULT_NEXUS_BASE_URL,
                "/v1/treasury/status",
                json!({"wallet_runtime_status": "connected", "treasury_enabled": true}),
            ),
        );
        assert_eq!(snapshot.observation_status, "degraded");
        assert!(
            snapshot
                .issues
                .iter()
                .any(|issue| issue.code == "endpoint_stats_failed")
        );
    }

    #[test]
    fn redaction_masks_sensitive_text() {
        let redacted = redact_sensitive_text(
            "Authorization: Bearer abc payment_preimage=abc lnbc1example token=abc",
        );
        assert_eq!(redacted, "[redacted]");
    }

    #[test]
    fn snapshot_schema_names_are_stable() {
        let snapshot = fake_nexus_health_snapshot(DEFAULT_NEXUS_BASE_URL).expect("fake snapshot");
        let encoded = serde_json::to_value(snapshot).expect("serialize snapshot");
        for key in [
            "schema_version",
            "generated_at_unix_ms",
            "base_url",
            "observation_status",
            "endpoints",
            "treasury",
            "training",
            "fleet",
            "website",
            "infra",
            "issues",
        ] {
            assert!(encoded.get(key).is_some(), "missing key {key}");
        }
    }
}
