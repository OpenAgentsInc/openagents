use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_NEXUS_BASE_URL: &str = "https://nexus.openagents.com";
const DEFAULT_TIMEOUT_MS: u64 = 15_000;
const MAX_PUBLIC_STATS_AGE_MS: u64 = 120_000;
const MIN_TREASURY_RUNWAY_WINDOWS: u64 = 20;
const MAX_PAYOUT_DISPATCH_LAG_MULTIPLIER: u64 = 3;
const MIN_PAYOUT_DISPATCH_LAG_MS: u64 = 120_000;
const MAX_PAYOUT_CONFIRM_LAG_MS: u64 = 30 * 60 * 1_000;

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
    pub classification: NexusHealthClassification,
    #[serde(default)]
    pub verification_gates: BTreeMap<String, NexusHealthVerificationGate>,
    pub endpoints: BTreeMap<String, NexusHealthEndpointSnapshot>,
    pub treasury: NexusHealthTreasurySnapshot,
    pub training: NexusHealthTrainingSnapshot,
    pub fleet: NexusHealthFleetSnapshot,
    pub website: NexusHealthWebsiteSnapshot,
    pub infra: NexusHealthInfraSnapshot,
    #[serde(default)]
    pub issues: Vec<NexusHealthIssue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthClassification {
    pub state: String,
    pub summary: String,
    pub highest_severity: String,
    #[serde(default)]
    pub failed_predicates: Vec<NexusHealthPredicate>,
    #[serde(default)]
    pub passed_predicates: Vec<NexusHealthPredicate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthPredicate {
    pub predicate_id: String,
    pub severity: String,
    pub status: String,
    pub detail: String,
    pub remediation_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthVerificationGate {
    pub gate_id: String,
    pub status: String,
    pub passed: bool,
    #[serde(default)]
    pub checked_predicates: Vec<String>,
    #[serde(default)]
    pub failed_predicates: Vec<NexusHealthPredicate>,
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
    pub nexus_vm_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_relay_service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_relay_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflared_service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflared_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_restart_count_15m: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oom_kill_count_24h: Option<u64>,
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
    let (healthz, stats, treasury) = tokio::join!(
        fetch_endpoint(&client, &base_url, "healthz", "/healthz"),
        fetch_endpoint(&client, &base_url, "stats", "/api/stats"),
        fetch_endpoint(&client, &base_url, "treasury", "/v1/treasury/status")
    );
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
    let mut snapshot = NexusHealthSnapshot {
        schema_version: 1,
        generated_at_unix_ms,
        base_url: base_url.to_string(),
        observation_status: "unclassified".to_string(),
        classification: NexusHealthClassification::default(),
        verification_gates: BTreeMap::new(),
        endpoints,
        treasury: treasury_snapshot,
        training: training_snapshot,
        fleet: fleet_snapshot,
        website: website_snapshot,
        infra: infra_snapshot,
        issues,
    };
    let classification = classify_nexus_health(&snapshot);
    let verification_gates = verification_gates_for_classification(&classification);
    snapshot.observation_status = classification.state.clone();
    snapshot.classification = classification;
    snapshot.verification_gates = verification_gates;
    snapshot
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
        stats_fresh: stats_age_ms.is_some_and(|age| age <= MAX_PUBLIC_STATS_AGE_MS),
    }
}

fn infra_snapshot() -> NexusHealthInfraSnapshot {
    let google_project_id = std::env::var("GOOGLE_CLOUD_PROJECT")
        .ok()
        .or_else(|| std::env::var("CLOUDSDK_CORE_PROJECT").ok());
    let nexus_vm_name = std::env::var("NEXUS_HEALTH_GCP_VM_NAME")
        .ok()
        .or_else(|| Some("nexus-mainnet-1".to_string()));
    let nexus_vm_status = std::env::var("NEXUS_HEALTH_GCP_VM_STATUS").ok();
    let nexus_relay_status = std::env::var("NEXUS_HEALTH_NEXUS_RELAY_STATUS").ok();
    let cloudflared_status = std::env::var("NEXUS_HEALTH_CLOUDFLARED_STATUS").ok();
    let relay_restart_count_15m = std::env::var("NEXUS_HEALTH_RELAY_RESTART_COUNT_15M")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    let oom_kill_count_24h = std::env::var("NEXUS_HEALTH_OOM_KILL_COUNT_24H")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    NexusHealthInfraSnapshot {
        source: if google_project_id.is_some() {
            "env".to_string()
        } else {
            "not_available".to_string()
        },
        google_project_id,
        nexus_vm_name,
        nexus_vm_status,
        nexus_relay_service: Some("nexus-relay".to_string()),
        nexus_relay_status,
        cloudflared_service: Some("nexus-cloudflared".to_string()),
        cloudflared_status,
        relay_restart_count_15m,
        oom_kill_count_24h,
        note: Some(
            "GCP runtime state is not queried by this observation-only snapshot command"
                .to_string(),
        ),
    }
}

pub fn classify_nexus_health(snapshot: &NexusHealthSnapshot) -> NexusHealthClassification {
    let (failed_predicates, passed_predicates) = health_predicates(snapshot);
    let highest_severity = highest_severity(&failed_predicates);
    let state = health_state(snapshot, &failed_predicates);
    let summary = if failed_predicates.is_empty() {
        match state.as_str() {
            "verified_closed" => "all health predicates passed and the incident is verified closed",
            "recovering" => "all blocking predicates passed while recovery is active",
            _ => "all health predicates passed",
        }
        .to_string()
    } else {
        let first = failed_predicates
            .first()
            .map(|predicate| predicate.detail.as_str())
            .unwrap_or("health predicate failed");
        format!(
            "{} failed health predicate(s); highest severity: {}; first: {}",
            failed_predicates.len(),
            highest_severity,
            first
        )
    };
    NexusHealthClassification {
        state,
        summary,
        highest_severity,
        failed_predicates,
        passed_predicates,
    }
}

fn health_predicates(
    snapshot: &NexusHealthSnapshot,
) -> (Vec<NexusHealthPredicate>, Vec<NexusHealthPredicate>) {
    let mut failed = Vec::new();
    let mut passed = Vec::new();
    let endpoint_failures: Vec<_> = snapshot
        .endpoints
        .values()
        .filter(|endpoint| !endpoint.ok)
        .collect();
    if endpoint_failures.is_empty() {
        passed.push(passed_predicate(
            "public_endpoint_reachable",
            "all required public Nexus endpoints responded successfully",
        ));
    } else {
        for endpoint in endpoint_failures {
            failed.push(failed_predicate(
                "public_endpoint_reachable",
                "critical",
                format!(
                    "{} failed at {} with status {:?}: {}",
                    endpoint.route_id,
                    endpoint.path,
                    endpoint.status_code,
                    endpoint.error.as_deref().unwrap_or("endpoint probe failed")
                ),
                "Restore public Nexus reachability before doing secondary issue work.",
            ));
            if let Some(code) = endpoint.cloudflare_error_code {
                failed.push(failed_predicate(
                    "cloudflare_edge_failure",
                    "critical",
                    format!(
                        "{} returned Cloudflare error {} through the public edge",
                        endpoint.route_id, code
                    ),
                    "Check VM-local /healthz, nexus-relay, nexus-cloudflared, and the Cloudflare tunnel path in that order.",
                ));
            }
        }
    }

    if snapshot.treasury.treasury_enabled {
        passed.push(passed_predicate(
            "treasury_enabled",
            "treasury payout execution is enabled",
        ));
    } else {
        failed.push(failed_predicate(
            "treasury_enabled",
            "critical",
            "treasury payout execution is disabled",
            "Re-enable treasury only after confirming the wallet and payout policy state are safe.",
        ));
    }

    if let Some(reason) = snapshot.treasury.degraded_reason.as_ref() {
        failed.push(failed_predicate(
            "treasury_not_degraded",
            "critical",
            format!("treasury is degraded: {reason}"),
            "Inspect /v1/treasury/status and resolve the degraded_reason before claiming payout health.",
        ));
    } else {
        passed.push(passed_predicate(
            "treasury_not_degraded",
            "treasury status does not report a degraded reason",
        ));
    }

    if snapshot.treasury.wallet_connected {
        passed.push(passed_predicate(
            "wallet_connected",
            "treasury wallet runtime is connected",
        ));
    } else {
        failed.push(failed_predicate(
            "wallet_connected",
            "critical",
            "treasury wallet runtime is not connected",
            "Restore wallet runtime connectivity before dispatching or verifying payouts.",
        ));
    }

    if let Some(runway) = snapshot.treasury.balance_runway_windows {
        if runway < MIN_TREASURY_RUNWAY_WINDOWS {
            failed.push(failed_predicate(
                "treasury_balance_runway",
                "warning",
                format!(
                    "treasury balance runway is {runway} payout windows; floor is {MIN_TREASURY_RUNWAY_WINDOWS}"
                ),
                "Fund the treasury before the payout loop runs out of safe runway.",
            ));
        } else {
            passed.push(passed_predicate(
                "treasury_balance_runway",
                format!("treasury balance runway is {runway} payout windows"),
            ));
        }
    }

    if unhealthy_status(snapshot.treasury.payout_loop_health.as_str()) {
        failed.push(failed_predicate(
            "payout_loop_healthy",
            "error",
            format!(
                "payout loop health is {}",
                snapshot.treasury.payout_loop_health
            ),
            "Inspect payout loop runtime status and treasury continuity alerts.",
        ));
    } else {
        passed.push(passed_predicate(
            "payout_loop_healthy",
            format!(
                "payout loop health is {}",
                snapshot.treasury.payout_loop_health
            ),
        ));
    }

    let dispatch_floor_ms = snapshot
        .treasury
        .payout_interval_seconds
        .saturating_mul(1_000)
        .saturating_mul(MAX_PAYOUT_DISPATCH_LAG_MULTIPLIER)
        .max(MIN_PAYOUT_DISPATCH_LAG_MS);
    if snapshot.fleet.eligible_online_payout_targets > 0
        && snapshot.treasury.treasury_enabled
        && snapshot.treasury.wallet_connected
    {
        match snapshot.treasury.dispatch_lag_ms {
            Some(lag) if lag > dispatch_floor_ms => failed.push(failed_predicate(
                "payout_dispatch_fresh",
                "error",
                format!(
                    "eligible payout targets are online but last dispatch is stale: {lag}ms > {dispatch_floor_ms}ms"
                ),
                "Inspect the payout dispatcher, queue locks, and treasury reservation state.",
            )),
            Some(lag) => passed.push(passed_predicate(
                "payout_dispatch_fresh",
                format!("last payout dispatch lag is {lag}ms"),
            )),
            None if snapshot.treasury.payouts_dispatched_24h == 0 => failed.push(
                failed_predicate(
                    "payout_dispatch_fresh",
                    "error",
                    format!(
                        "{} eligible payout target(s) are online but no dispatch timestamp is available",
                        snapshot.fleet.eligible_online_payout_targets
                    ),
                    "Confirm the payout loop is running and can create a dispatch receipt.",
                ),
            ),
            None => passed.push(passed_predicate(
                "payout_dispatch_fresh",
                "dispatch timestamp is unavailable but payouts have dispatched in the last 24h",
            )),
        }
    } else {
        passed.push(passed_predicate(
            "payout_dispatch_fresh",
            "no eligible online payout targets require an immediate dispatch freshness check",
        ));
    }

    let payout_in_flight = snapshot
        .treasury
        .payout_sats_in_flight_total
        .saturating_add(snapshot.treasury.pending_confirmation_count);
    if payout_in_flight > 0 {
        match snapshot.treasury.confirm_lag_ms {
            Some(lag) if lag > MAX_PAYOUT_CONFIRM_LAG_MS => failed.push(failed_predicate(
                "payout_confirmation_fresh",
                "error",
                format!(
                    "payout confirmations are stale: {lag}ms > {MAX_PAYOUT_CONFIRM_LAG_MS}ms"
                ),
                "Inspect recent payout records, wallet sends, and confirmation reconciliation.",
            )),
            Some(lag) => passed.push(passed_predicate(
                "payout_confirmation_fresh",
                format!("last payout confirmation lag is {lag}ms"),
            )),
            None => failed.push(failed_predicate(
                "payout_confirmation_fresh",
                "error",
                "payouts are in flight but no confirmation timestamp is available",
                "Inspect confirmation reconciliation and the wallet history before claiming settlement health.",
            )),
        }
    } else {
        passed.push(passed_predicate(
            "payout_confirmation_fresh",
            "no in-flight payout requires a confirmation freshness check",
        ));
    }

    if snapshot.training.accepted_work_attention_payout_count > 0 {
        failed.push(failed_predicate(
            "accepted_work_payout_queue_healthy",
            "error",
            format!(
                "{} accepted-work payout(s) require attention",
                snapshot.training.accepted_work_attention_payout_count
            ),
            "Inspect the training payout ledger and reconcile or retry attention records.",
        ));
    } else {
        passed.push(passed_predicate(
            "accepted_work_payout_queue_healthy",
            "accepted-work payout queue has no attention records",
        ));
    }

    if snapshot
        .training
        .launch_health_overall_status
        .as_deref()
        .is_some_and(unhealthy_status)
    {
        failed.push(failed_predicate(
            "training_launch_healthy",
            "error",
            format!(
                "training launch health is {}",
                snapshot
                    .training
                    .launch_health_overall_status
                    .as_deref()
                    .unwrap_or("unknown")
            ),
            "Inspect public training launch health alerts before dispatching more work.",
        ));
    } else {
        passed.push(passed_predicate(
            "training_launch_healthy",
            format!(
                "training launch health is {}",
                snapshot
                    .training
                    .launch_health_overall_status
                    .as_deref()
                    .unwrap_or("not_reported")
            ),
        ));
    }

    if snapshot.training.nodes_online > 0
        && snapshot.training.runs_active == 0
        && snapshot.training.windows_active == 0
        && snapshot.training.active_run_id.is_none()
    {
        failed.push(failed_predicate(
            "training_dispatch_active",
            "warning",
            format!(
                "{} training node(s) are online but no training run or window is active",
                snapshot.training.nodes_online
            ),
            "If work is expected, run or inspect the hosted training dispatcher.",
        ));
    } else {
        passed.push(passed_predicate(
            "training_dispatch_active",
            "training dispatch is active or no online training nodes currently require work",
        ));
    }

    if snapshot.website.stats_fresh {
        passed.push(passed_predicate(
            "website_stats_fresh",
            "public stats age is within the freshness window",
        ));
    } else {
        failed.push(failed_predicate(
            "website_stats_fresh",
            "warning",
            "public stats are stale or missing as_of_unix_ms",
            "Refresh Nexus stats publication and openagents.com projections before trusting website telemetry.",
        ));
    }

    if let Some(status) = snapshot.infra.nexus_vm_status.as_deref() {
        if stopped_status(status) {
            failed.push(failed_predicate(
                "gcp_vm_available",
                "critical",
                format!("Nexus VM status is {status}"),
                "Start or repair the Nexus VM before attempting service-level recovery.",
            ));
        } else {
            passed.push(passed_predicate(
                "gcp_vm_available",
                format!("Nexus VM status is {status}"),
            ));
        }
    } else {
        passed.push(passed_predicate(
            "gcp_vm_available",
            "GCP VM status was not checked by this observation-only command",
        ));
    }

    if let Some(status) = snapshot.infra.nexus_relay_status.as_deref() {
        if stopped_status(status) {
            failed.push(failed_predicate(
                "nexus_relay_service_healthy",
                "critical",
                format!("nexus-relay service status is {status}"),
                "Restart or repair nexus-relay under the controlled recovery path.",
            ));
        } else {
            passed.push(passed_predicate(
                "nexus_relay_service_healthy",
                format!("nexus-relay service status is {status}"),
            ));
        }
    } else {
        passed.push(passed_predicate(
            "nexus_relay_service_healthy",
            "nexus-relay service status was not checked by this observation-only command",
        ));
    }

    if let Some(status) = snapshot.infra.cloudflared_status.as_deref() {
        if stopped_status(status) {
            failed.push(failed_predicate(
                "cloudflared_service_healthy",
                "critical",
                format!("nexus-cloudflared service status is {status}"),
                "Repair the Cloudflare tunnel service before treating public edge health as recovered.",
            ));
        } else {
            passed.push(passed_predicate(
                "cloudflared_service_healthy",
                format!("nexus-cloudflared service status is {status}"),
            ));
        }
    } else {
        passed.push(passed_predicate(
            "cloudflared_service_healthy",
            "nexus-cloudflared service status was not checked by this observation-only command",
        ));
    }

    if snapshot.infra.relay_restart_count_15m.unwrap_or(0) > 3 {
        failed.push(failed_predicate(
            "nexus_relay_restart_loop_absent",
            "critical",
            format!(
                "nexus-relay restarted {} time(s) in 15m",
                snapshot.infra.relay_restart_count_15m.unwrap_or(0)
            ),
            "Inspect relay logs and stop the restart loop before closing the incident.",
        ));
    } else {
        passed.push(passed_predicate(
            "nexus_relay_restart_loop_absent",
            "nexus-relay restart-loop signal is absent or below the configured threshold",
        ));
    }

    if snapshot.infra.oom_kill_count_24h.unwrap_or(0) > 0 {
        failed.push(failed_predicate(
            "nexus_oom_absent",
            "critical",
            format!(
                "{} OOM kill(s) were observed in 24h",
                snapshot.infra.oom_kill_count_24h.unwrap_or(0)
            ),
            "Increase memory headroom or fix the leaking workload before treating Nexus as stable.",
        ));
    } else {
        passed.push(passed_predicate(
            "nexus_oom_absent",
            "Nexus OOM signal is absent",
        ));
    }

    (failed, passed)
}

fn verification_gates_for_classification(
    classification: &NexusHealthClassification,
) -> BTreeMap<String, NexusHealthVerificationGate> {
    let gates = [
        (
            "payout_capability",
            vec![
                "treasury_enabled",
                "treasury_not_degraded",
                "wallet_connected",
                "treasury_balance_runway",
                "payout_loop_healthy",
                "payout_dispatch_fresh",
                "payout_confirmation_fresh",
                "accepted_work_payout_queue_healthy",
            ],
        ),
        (
            "training_dispatch",
            vec!["training_launch_healthy", "training_dispatch_active"],
        ),
        ("website_stats_freshness", vec!["website_stats_fresh"]),
        (
            "infra_availability",
            vec![
                "public_endpoint_reachable",
                "cloudflare_edge_failure",
                "gcp_vm_available",
                "nexus_relay_service_healthy",
                "cloudflared_service_healthy",
                "nexus_relay_restart_loop_absent",
                "nexus_oom_absent",
            ],
        ),
    ];
    gates
        .into_iter()
        .map(|(gate_id, predicate_ids)| {
            let failed_predicates: Vec<NexusHealthPredicate> = classification
                .failed_predicates
                .iter()
                .filter(|predicate| predicate_ids.contains(&predicate.predicate_id.as_str()))
                .cloned()
                .collect();
            let passed = failed_predicates.is_empty();
            (
                gate_id.to_string(),
                NexusHealthVerificationGate {
                    gate_id: gate_id.to_string(),
                    status: if passed { "passed" } else { "failed" }.to_string(),
                    passed,
                    checked_predicates: predicate_ids.into_iter().map(str::to_string).collect(),
                    failed_predicates,
                },
            )
        })
        .collect()
}

fn health_state(snapshot: &NexusHealthSnapshot, failed: &[NexusHealthPredicate]) -> String {
    if failed.is_empty()
        && snapshot
            .issues
            .iter()
            .any(|issue| issue.code == "incident_verified_closed")
    {
        return "verified_closed".to_string();
    }
    if snapshot
        .issues
        .iter()
        .any(|issue| issue.code == "recovery_active")
    {
        return "recovering".to_string();
    }
    if failed.is_empty() {
        return "healthy".to_string();
    }
    if failed.iter().any(|predicate| {
        matches!(
            predicate.predicate_id.as_str(),
            "public_endpoint_reachable" | "cloudflare_edge_failure"
        )
    }) {
        return "incident".to_string();
    }
    if failed.iter().any(|predicate| {
        matches!(
            predicate.predicate_id.as_str(),
            "treasury_enabled"
                | "treasury_not_degraded"
                | "wallet_connected"
                | "gcp_vm_available"
                | "nexus_relay_service_healthy"
                | "cloudflared_service_healthy"
                | "nexus_relay_restart_loop_absent"
                | "nexus_oom_absent"
        ) && predicate.severity == "critical"
    }) {
        return "needs_operator".to_string();
    }
    if failed
        .iter()
        .any(|predicate| matches!(predicate.severity.as_str(), "critical" | "error"))
    {
        "degraded".to_string()
    } else {
        "watch".to_string()
    }
}

fn highest_severity(predicates: &[NexusHealthPredicate]) -> String {
    predicates
        .iter()
        .map(|predicate| predicate.severity.as_str())
        .max_by_key(|severity| severity_rank(severity))
        .unwrap_or("none")
        .to_string()
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 4,
        "error" => 3,
        "warning" => 2,
        "info" => 1,
        _ => 0,
    }
}

fn failed_predicate(
    predicate_id: &str,
    severity: &str,
    detail: impl Into<String>,
    remediation_hint: &str,
) -> NexusHealthPredicate {
    NexusHealthPredicate {
        predicate_id: predicate_id.to_string(),
        severity: severity.to_string(),
        status: "failed".to_string(),
        detail: detail.into(),
        remediation_hint: remediation_hint.to_string(),
    }
}

fn passed_predicate(predicate_id: &str, detail: impl Into<String>) -> NexusHealthPredicate {
    NexusHealthPredicate {
        predicate_id: predicate_id.to_string(),
        severity: "info".to_string(),
        status: "passed".to_string(),
        detail: detail.into(),
        remediation_hint: "none".to_string(),
    }
}

fn unhealthy_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "bad"
            | "critical"
            | "degraded"
            | "failed"
            | "failure"
            | "unhealthy"
            | "error"
            | "errored"
            | "stalled"
            | "stale"
            | "panic"
            | "crashed"
            | "restart_loop"
    )
}

fn stopped_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "down"
            | "stopped"
            | "stopping"
            | "terminated"
            | "failed"
            | "failure"
            | "inactive"
            | "dead"
            | "crashed"
            | "restart_loop"
            | "oom"
    )
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
    let normalized_body = body.to_ascii_lowercase();
    if normalized_body.contains("error code: 1033") || normalized_body.contains("error 1033") {
        Some(1033)
    } else if status_code == 530 || normalized_body.contains("error 530") {
        Some(530)
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
        assert_eq!(snapshot.observation_status, "healthy");
        assert_eq!(snapshot.classification.state, "healthy");
        assert!(snapshot.classification.failed_predicates.is_empty());
        assert!(snapshot.verification_gates.values().all(|gate| gate.passed));
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
        assert_eq!(snapshot.observation_status, "incident");
        assert!(
            snapshot
                .issues
                .iter()
                .any(|issue| issue.code == "endpoint_stats_failed")
        );
        assert_failed(&snapshot, "public_endpoint_reachable");
        assert_failed(&snapshot, "cloudflare_edge_failure");
    }

    #[test]
    fn classifier_covers_known_incident_classes() {
        let cases: Vec<(
            &'static str,
            Box<dyn Fn(&mut NexusHealthSnapshot)>,
            &'static str,
            &'static str,
        )> = vec![
            (
                "cloudflare_1033",
                Box::new(|snapshot| {
                    let endpoint = snapshot.endpoints.get_mut("stats").expect("stats endpoint");
                    endpoint.ok = false;
                    endpoint.status_code = Some(530);
                    endpoint.cloudflare_error_code = Some(1033);
                    endpoint.error = Some("http_status_530_cloudflare_1033".to_string());
                }),
                "incident",
                "cloudflare_edge_failure",
            ),
            (
                "treasury_degraded",
                Box::new(|snapshot| {
                    snapshot.treasury.degraded_reason = Some("wallet sync stalled".to_string());
                }),
                "needs_operator",
                "treasury_not_degraded",
            ),
            (
                "treasury_disabled",
                Box::new(|snapshot| {
                    snapshot.treasury.treasury_enabled = false;
                }),
                "needs_operator",
                "treasury_enabled",
            ),
            (
                "wallet_disconnected",
                Box::new(|snapshot| {
                    snapshot.treasury.wallet_connected = false;
                }),
                "needs_operator",
                "wallet_connected",
            ),
            (
                "low_runway",
                Box::new(|snapshot| {
                    snapshot.treasury.balance_runway_windows = Some(3);
                }),
                "watch",
                "treasury_balance_runway",
            ),
            (
                "payout_dispatch_stall",
                Box::new(|snapshot| {
                    snapshot.fleet.eligible_online_payout_targets = 3;
                    snapshot.treasury.dispatch_lag_ms = Some(10 * 60 * 1_000);
                }),
                "degraded",
                "payout_dispatch_fresh",
            ),
            (
                "payout_confirmation_stall",
                Box::new(|snapshot| {
                    snapshot.treasury.payout_sats_in_flight_total = 25;
                    snapshot.treasury.pending_confirmation_count = 1;
                    snapshot.treasury.confirm_lag_ms = Some(45 * 60 * 1_000);
                }),
                "degraded",
                "payout_confirmation_fresh",
            ),
            (
                "training_launch_failure",
                Box::new(|snapshot| {
                    snapshot.training.launch_health_overall_status = Some("degraded".to_string());
                }),
                "degraded",
                "training_launch_healthy",
            ),
            (
                "website_stats_stale",
                Box::new(|snapshot| {
                    snapshot.website.stats_fresh = false;
                    snapshot.website.stats_age_ms = Some(MAX_PUBLIC_STATS_AGE_MS + 1);
                }),
                "watch",
                "website_stats_fresh",
            ),
            (
                "vm_down",
                Box::new(|snapshot| {
                    snapshot.infra.nexus_vm_status = Some("TERMINATED".to_string());
                }),
                "needs_operator",
                "gcp_vm_available",
            ),
            (
                "relay_restart_loop",
                Box::new(|snapshot| {
                    snapshot.infra.relay_restart_count_15m = Some(4);
                }),
                "needs_operator",
                "nexus_relay_restart_loop_absent",
            ),
            (
                "oom_seen",
                Box::new(|snapshot| {
                    snapshot.infra.oom_kill_count_24h = Some(1);
                }),
                "needs_operator",
                "nexus_oom_absent",
            ),
        ];

        for (name, mutate, expected_state, expected_predicate) in cases {
            let mut snapshot =
                fake_nexus_health_snapshot(DEFAULT_NEXUS_BASE_URL).expect("fake snapshot");
            mutate(&mut snapshot);
            let snapshot = refresh_classification(snapshot);
            assert_eq!(snapshot.classification.state, expected_state, "case {name}");
            assert_failed(&snapshot, expected_predicate);
        }
    }

    #[test]
    fn payout_idle_state_does_not_count_as_stalled() {
        let mut snapshot =
            fake_nexus_health_snapshot(DEFAULT_NEXUS_BASE_URL).expect("fake snapshot");
        snapshot.fleet.eligible_online_payout_targets = 0;
        snapshot.treasury.payout_sats_in_flight_total = 0;
        snapshot.treasury.pending_confirmation_count = 0;
        snapshot.treasury.dispatch_lag_ms = None;
        snapshot.treasury.confirm_lag_ms = None;
        snapshot.treasury.payouts_dispatched_24h = 0;
        let snapshot = refresh_classification(snapshot);
        assert_eq!(snapshot.classification.state, "healthy");
        assert_not_failed(&snapshot, "payout_dispatch_fresh");
        assert_not_failed(&snapshot, "payout_confirmation_fresh");
    }

    #[test]
    fn green_healthz_alone_is_not_enough_for_healthy() {
        let generated_at_unix_ms = 1_777_200_000_000;
        let snapshot = snapshot_from_fetches(
            DEFAULT_NEXUS_BASE_URL,
            generated_at_unix_ms,
            fake_fetch(
                "healthz",
                DEFAULT_NEXUS_BASE_URL,
                "/healthz",
                json!({"ok": true}),
            ),
            failed_fetch("stats", "/api/stats"),
            failed_fetch("treasury", "/v1/treasury/status"),
        );
        assert_eq!(snapshot.classification.state, "incident");
        assert_failed(&snapshot, "public_endpoint_reachable");
        assert!(
            !snapshot
                .verification_gates
                .get("infra_availability")
                .expect("infra gate")
                .passed
        );
    }

    #[test]
    fn failed_predicate_json_shape_is_stable() {
        let mut snapshot =
            fake_nexus_health_snapshot(DEFAULT_NEXUS_BASE_URL).expect("fake snapshot");
        snapshot.fleet.eligible_online_payout_targets = 3;
        snapshot.treasury.dispatch_lag_ms = Some(10 * 60 * 1_000);
        let snapshot = refresh_classification(snapshot);
        let encoded = serde_json::to_value(snapshot).expect("serialize snapshot");
        let predicate = &encoded["classification"]["failed_predicates"][0];
        for key in [
            "predicate_id",
            "severity",
            "status",
            "detail",
            "remediation_hint",
        ] {
            assert!(predicate.get(key).is_some(), "missing predicate key {key}");
        }
        let gate_predicate =
            &encoded["verification_gates"]["payout_capability"]["failed_predicates"][0];
        assert_eq!(gate_predicate["predicate_id"], "payout_dispatch_fresh");
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
            "classification",
            "verification_gates",
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

    fn failed_fetch(route_id: &str, path: &str) -> EndpointFetch {
        EndpointFetch {
            endpoint: NexusHealthEndpointSnapshot {
                route_id: route_id.to_string(),
                path: path.to_string(),
                url: format!("{}{}", DEFAULT_NEXUS_BASE_URL, path),
                ok: false,
                status_code: Some(503),
                latency_ms: Some(10),
                cloudflare_error_code: None,
                error: Some("http_status_503".to_string()),
            },
            body: None,
        }
    }

    fn refresh_classification(mut snapshot: NexusHealthSnapshot) -> NexusHealthSnapshot {
        let classification = classify_nexus_health(&snapshot);
        let verification_gates = verification_gates_for_classification(&classification);
        snapshot.observation_status = classification.state.clone();
        snapshot.classification = classification;
        snapshot.verification_gates = verification_gates;
        snapshot
    }

    fn assert_failed(snapshot: &NexusHealthSnapshot, predicate_id: &str) {
        assert!(
            snapshot
                .classification
                .failed_predicates
                .iter()
                .any(|predicate| predicate.predicate_id == predicate_id),
            "expected failed predicate {predicate_id}; got {:?}",
            snapshot
                .classification
                .failed_predicates
                .iter()
                .map(|predicate| predicate.predicate_id.as_str())
                .collect::<Vec<_>>()
        );
    }

    fn assert_not_failed(snapshot: &NexusHealthSnapshot, predicate_id: &str) {
        assert!(
            !snapshot
                .classification
                .failed_predicates
                .iter()
                .any(|predicate| predicate.predicate_id == predicate_id),
            "did not expect failed predicate {predicate_id}"
        );
    }
}
