use std::env;

use nexus_control::{HealthSnapshotCommand, NexusHealthPredicate, NexusHealthSnapshot};
use serde::{Deserialize, Serialize};

const DEFAULT_NEXUS_BASE_URL: &str = "https://nexus.openagents.com";
const DEFAULT_TIMEOUT_MS: u64 = 15_000;
const NEXUS_HEALTH_BASE_URL_ENV: &str = "OPENAGENTS_NEXUS_HEALTH_BASE_URL";
const NEXUS_HEALTH_TIMEOUT_MS_ENV: &str = "OPENAGENTS_NEXUS_HEALTH_TIMEOUT_MS";
const NEXUS_HEALTH_FAKE_ENV: &str = "OPENAGENTS_AUTOPILOT_NEXUS_HEALTH_FAKE";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NexusHealthRequest {
    pub base_url: Option<String>,
    pub timeout_ms: Option<u64>,
    pub fake: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotNexusHealthProjection {
    pub schema_version: u32,
    pub generated_at_unix_ms: u64,
    pub source: String,
    pub base_url: String,
    pub state: String,
    pub severity: String,
    pub summary: String,
    pub exact_cause: String,
    pub subsystems: Vec<AutopilotHealthSubsystem>,
    pub active_run: AutopilotHealthActiveRun,
    pub queued_followups: Vec<AutopilotHealthFollowup>,
    pub stop_state: AutopilotHealthStopState,
    pub latest_action: Option<AutopilotHealthAction>,
    pub event_timeline: Vec<AutopilotHealthEvent>,
    pub failed_predicates: Vec<AutopilotHealthPredicate>,
    pub verification_gates: Vec<AutopilotHealthGate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthSubsystem {
    pub id: String,
    pub label: String,
    pub state: String,
    pub summary: String,
    pub detail: String,
    pub metrics: Vec<AutopilotHealthMetric>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthMetric {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthActiveRun {
    pub run_id: Option<String>,
    pub window_id: Option<String>,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthFollowup {
    pub id: String,
    pub severity: String,
    pub owner: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthStopState {
    pub can_cancel: bool,
    pub state: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthAction {
    pub id: String,
    pub state: String,
    pub summary: String,
    pub actor: String,
    pub observed_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthEvent {
    pub id: String,
    pub at_unix_ms: u64,
    pub state: String,
    pub title: String,
    pub detail: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthPredicate {
    pub predicate_id: String,
    pub severity: String,
    pub status: String,
    pub detail: String,
    pub remediation_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotHealthGate {
    pub gate_id: String,
    pub status: String,
    pub passed: bool,
}

#[tauri::command]
pub async fn nexus_health_status() -> Result<AutopilotNexusHealthProjection, String> {
    nexus_health_status_with_options(NexusHealthRequest::default()).await
}

pub fn nexus_health_status_blocking(
    request: NexusHealthRequest,
) -> Result<AutopilotNexusHealthProjection, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| clean_text(&format!("failed to start Nexus health runtime: {error}")))?;
    runtime.block_on(nexus_health_status_with_options(request))
}

pub async fn nexus_health_status_with_options(
    request: NexusHealthRequest,
) -> Result<AutopilotNexusHealthProjection, String> {
    let command = HealthSnapshotCommand {
        base_url: request
            .base_url
            .or_else(env_base_url)
            .unwrap_or_else(|| DEFAULT_NEXUS_BASE_URL.to_string()),
        timeout_ms: request
            .timeout_ms
            .or_else(env_timeout_ms)
            .unwrap_or(DEFAULT_TIMEOUT_MS),
        fake: request.fake || env_fake(),
        pretty: false,
    };
    let payload = nexus_control::run_health_snapshot_command(&command)
        .await
        .map_err(|error| clean_text(&format!("failed to capture Nexus health: {error}")))?;
    let snapshot: NexusHealthSnapshot = serde_json::from_str(&payload)
        .map_err(|error| clean_text(&format!("failed to decode Nexus health snapshot: {error}")))?;
    Ok(project_snapshot(snapshot))
}

pub fn project_snapshot(snapshot: NexusHealthSnapshot) -> AutopilotNexusHealthProjection {
    let failed_predicates = snapshot
        .classification
        .failed_predicates
        .iter()
        .map(project_predicate)
        .collect::<Vec<_>>();
    let exact_cause = exact_cause(&snapshot);
    let event_timeline = event_timeline(&snapshot);
    let queued_followups = queued_followups(&snapshot.classification.failed_predicates);
    let state = normalize_state(&snapshot.classification.state);
    let generated_at_unix_ms = snapshot.generated_at_unix_ms;
    let summary = clean_text(&snapshot.classification.summary);

    AutopilotNexusHealthProjection {
        schema_version: 1,
        generated_at_unix_ms,
        source: "nexus-control health snapshot".to_string(),
        base_url: clean_text(&snapshot.base_url),
        state: state.clone(),
        severity: clean_text(&snapshot.classification.highest_severity),
        summary,
        exact_cause,
        subsystems: subsystems(&snapshot),
        active_run: AutopilotHealthActiveRun {
            run_id: snapshot
                .training
                .active_run_id
                .clone()
                .or_else(|| snapshot.training.latest_run_id.clone())
                .map(|value| clean_text(&value)),
            window_id: snapshot
                .training
                .active_window_id
                .clone()
                .or_else(|| snapshot.training.latest_window_id.clone())
                .map(|value| clean_text(&value)),
            status: snapshot
                .training
                .latest_run_status
                .clone()
                .unwrap_or_else(|| {
                    if snapshot.training.runs_active > 0 {
                        "active".to_string()
                    } else {
                        "no active run".to_string()
                    }
                }),
            detail: format!(
                "{} nodes online, {} active run(s), {} payout-eligible closeout(s)",
                snapshot.training.nodes_online,
                snapshot.training.runs_active,
                snapshot.training.payout_eligible_closeouts
            ),
        },
        queued_followups,
        stop_state: stop_state(&state),
        latest_action: Some(AutopilotHealthAction {
            id: "nexus.health.snapshot.refresh".to_string(),
            state: "observed".to_string(),
            summary: "Refreshed Nexus health projection from public Nexus endpoints.".to_string(),
            actor: "Autopilot local shell".to_string(),
            observed_at_unix_ms: generated_at_unix_ms,
        }),
        event_timeline,
        failed_predicates,
        verification_gates: snapshot
            .verification_gates
            .values()
            .map(|gate| AutopilotHealthGate {
                gate_id: clean_text(&gate.gate_id),
                status: clean_text(&gate.status),
                passed: gate.passed,
            })
            .collect(),
    }
}

fn subsystems(snapshot: &NexusHealthSnapshot) -> Vec<AutopilotHealthSubsystem> {
    let failed = &snapshot.classification.failed_predicates;
    vec![
        AutopilotHealthSubsystem {
            id: "service".to_string(),
            label: "Nexus Service".to_string(),
            state: endpoint_state(snapshot),
            summary: endpoint_summary(snapshot),
            detail: endpoint_detail(snapshot),
            metrics: snapshot
                .endpoints
                .values()
                .map(|endpoint| AutopilotHealthMetric {
                    label: endpoint.route_id.clone(),
                    value: match (endpoint.ok, endpoint.status_code, endpoint.latency_ms) {
                        (true, Some(status), Some(latency)) => {
                            format!("{status} in {latency}ms")
                        }
                        (true, Some(status), None) => status.to_string(),
                        (false, Some(status), _) => {
                            format!(
                                "{status} {}",
                                clean_text(endpoint.error.as_deref().unwrap_or("failed"))
                            )
                        }
                        (false, None, _) => {
                            clean_text(endpoint.error.as_deref().unwrap_or("failed"))
                        }
                        _ => "observed".to_string(),
                    },
                })
                .collect(),
        },
        AutopilotHealthSubsystem {
            id: "treasury".to_string(),
            label: "Treasury Wallet".to_string(),
            state: bool_state(
                snapshot.treasury.treasury_enabled && snapshot.treasury.wallet_connected,
            ),
            summary: format!(
                "wallet {} with {} sats available",
                snapshot
                    .treasury
                    .wallet_runtime_status
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                snapshot.treasury.wallet_balance_sats
            ),
            detail: snapshot
                .treasury
                .degraded_reason
                .clone()
                .map(|reason| clean_text(&reason))
                .unwrap_or_else(|| "No treasury degraded reason reported.".to_string()),
            metrics: vec![
                metric(
                    "balance",
                    format!("{} sats", snapshot.treasury.wallet_balance_sats),
                ),
                metric(
                    "registered identities",
                    snapshot.treasury.registered_payout_identities,
                ),
                metric(
                    "runway",
                    optional_u64(snapshot.treasury.balance_runway_windows, "windows"),
                ),
            ],
        },
        AutopilotHealthSubsystem {
            id: "payout".to_string(),
            label: "Payout Loop".to_string(),
            state: payout_state(&snapshot.treasury.payout_loop_health, failed),
            summary: format!(
                "loop {}, {} confirmed and {} failed in 24h",
                snapshot.treasury.payout_loop_health,
                snapshot.treasury.payouts_confirmed_24h,
                snapshot.treasury.payouts_failed_24h
            ),
            detail: format!(
                "{} pending confirmation, {} accepted-work sats in flight",
                snapshot.treasury.pending_confirmation_count,
                snapshot.treasury.accepted_work_payout_sats_in_flight_total
            ),
            metrics: vec![
                metric("dispatches 24h", snapshot.treasury.payouts_dispatched_24h),
                metric("confirmed 24h", snapshot.treasury.payouts_confirmed_24h),
                metric("pending", snapshot.treasury.pending_confirmation_count),
            ],
        },
        AutopilotHealthSubsystem {
            id: "training".to_string(),
            label: "Training Dispatch".to_string(),
            state: training_state(snapshot, failed),
            summary: format!(
                "{} nodes online, {} admitted, {} active run(s)",
                snapshot.training.nodes_online,
                snapshot.training.admitted_nodes_online,
                snapshot.training.runs_active
            ),
            detail: format!(
                "latest run {}, window {}, launch health {}",
                snapshot
                    .training
                    .latest_run_id
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
                snapshot
                    .training
                    .latest_window_id
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
                snapshot
                    .training
                    .launch_health_overall_status
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            ),
            metrics: vec![
                metric("accepted closeouts", snapshot.training.accepted_closeouts),
                metric(
                    "payout eligible",
                    snapshot.training.payout_eligible_closeouts,
                ),
                metric(
                    "pending payout",
                    snapshot.training.accepted_work_pending_payout_count,
                ),
            ],
        },
        AutopilotHealthSubsystem {
            id: "website".to_string(),
            label: "Website Stats".to_string(),
            state: bool_state(snapshot.website.stats_fresh),
            summary: if snapshot.website.stats_fresh {
                "openagents.com stats source is fresh".to_string()
            } else {
                "openagents.com stats source is stale".to_string()
            },
            detail: format!(
                "source {}, age {}",
                snapshot.website.source,
                snapshot
                    .website
                    .stats_age_ms
                    .map(|value| format!("{value}ms"))
                    .unwrap_or_else(|| "unknown".to_string())
            ),
            metrics: vec![
                metric("source", snapshot.website.source.clone()),
                metric("fresh", snapshot.website.stats_fresh),
            ],
        },
        AutopilotHealthSubsystem {
            id: "fleet".to_string(),
            label: "Pylon Fleet".to_string(),
            state: bool_state(
                snapshot.fleet.pylons_online_now > 0
                    || snapshot.fleet.pylon_sessions_online_now > 0,
            ),
            summary: format!(
                "{} pylons online, {} eligible payout targets",
                snapshot.fleet.pylons_online_now, snapshot.fleet.eligible_online_payout_targets
            ),
            detail: format!(
                "{} duplicate-host blocked, {} version-floor blocked, {} readiness blocked",
                snapshot.fleet.duplicate_host_blocked_beneficiaries_now,
                snapshot.fleet.version_floor_blocked_beneficiaries_now,
                snapshot.fleet.readiness_blocked_beneficiaries_now
            ),
            metrics: vec![
                metric("sessions online", snapshot.fleet.pylon_sessions_online_now),
                metric(
                    "reported hosts",
                    snapshot.fleet.pylon_reported_hosts_online_now,
                ),
                metric("sellable", snapshot.fleet.sellable_pylons_online_now),
            ],
        },
        AutopilotHealthSubsystem {
            id: "infra".to_string(),
            label: "Cloud Infra".to_string(),
            state: infra_state(snapshot, failed),
            summary: snapshot.infra.note.clone().unwrap_or_else(|| {
                "GCP runtime state is not included in this public snapshot.".to_string()
            }),
            detail: format!(
                "VM {}, relay {}, tunnel {}",
                snapshot
                    .infra
                    .nexus_vm_status
                    .clone()
                    .unwrap_or_else(|| "not checked".to_string()),
                snapshot
                    .infra
                    .nexus_relay_status
                    .clone()
                    .unwrap_or_else(|| "not checked".to_string()),
                snapshot
                    .infra
                    .cloudflared_status
                    .clone()
                    .unwrap_or_else(|| "not checked".to_string())
            ),
            metrics: vec![
                metric(
                    "project",
                    snapshot
                        .infra
                        .google_project_id
                        .clone()
                        .unwrap_or_else(|| "not checked".to_string()),
                ),
                metric(
                    "vm",
                    snapshot
                        .infra
                        .nexus_vm_name
                        .clone()
                        .unwrap_or_else(|| "nexus-mainnet-1".to_string()),
                ),
            ],
        },
    ]
}

fn event_timeline(snapshot: &NexusHealthSnapshot) -> Vec<AutopilotHealthEvent> {
    let at = snapshot.generated_at_unix_ms;
    let failed_count = snapshot.classification.failed_predicates.len();
    let endpoint_count = snapshot.endpoints.len();
    let endpoint_failed_count = snapshot
        .endpoints
        .values()
        .filter(|endpoint| !endpoint.ok)
        .count();
    let mut events = vec![
        event(
            "snapshot.captured",
            at,
            normalize_state(&snapshot.observation_status),
            "Health snapshot captured",
            format!(
                "Nexus reported {} with {} failed predicate(s).",
                snapshot.classification.state, failed_count
            ),
            "nexus-control health snapshot",
        ),
        event(
            "public.endpoints",
            at,
            if endpoint_failed_count == 0 {
                "healthy"
            } else {
                "degraded"
            },
            "Public routes checked",
            format!("{endpoint_count} route(s) checked, {endpoint_failed_count} failed."),
            "healthz, stats, treasury status",
        ),
        event(
            "treasury.wallet",
            at,
            bool_state(snapshot.treasury.wallet_connected),
            "Treasury wallet inspected",
            format!(
                "Wallet {}, balance {} sats.",
                snapshot
                    .treasury
                    .wallet_runtime_status
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                snapshot.treasury.wallet_balance_sats
            ),
            "treasury status",
        ),
        event(
            "payout.loop",
            at,
            payout_state(
                &snapshot.treasury.payout_loop_health,
                &snapshot.classification.failed_predicates,
            ),
            "Payout loop inspected",
            format!(
                "{} confirmed, {} failed, {} pending in 24h.",
                snapshot.treasury.payouts_confirmed_24h,
                snapshot.treasury.payouts_failed_24h,
                snapshot.treasury.pending_confirmation_count
            ),
            "treasury payout counters",
        ),
        event(
            "training.dispatch",
            at,
            training_state(snapshot, &snapshot.classification.failed_predicates),
            "Training dispatch inspected",
            format!(
                "{} online node(s), latest run {}.",
                snapshot.training.nodes_online,
                snapshot
                    .training
                    .latest_run_id
                    .clone()
                    .unwrap_or_else(|| "none".to_string())
            ),
            "training launch health",
        ),
    ];

    for predicate in snapshot.classification.failed_predicates.iter().take(5) {
        events.push(event(
            format!("predicate.{}", predicate.predicate_id),
            at,
            normalize_state(&predicate.status),
            format!("Predicate failed: {}", predicate.predicate_id),
            clean_text(&predicate.detail),
            clean_text(&predicate.remediation_hint),
        ));
    }

    events
}

fn queued_followups(predicates: &[NexusHealthPredicate]) -> Vec<AutopilotHealthFollowup> {
    predicates
        .iter()
        .map(|predicate| AutopilotHealthFollowup {
            id: clean_text(&predicate.predicate_id),
            severity: clean_text(&predicate.severity),
            owner: "Forge health agent".to_string(),
            action: clean_text(&predicate.remediation_hint),
            detail: clean_text(&predicate.detail),
        })
        .collect()
}

fn stop_state(state: &str) -> AutopilotHealthStopState {
    if state == "healthy" {
        return AutopilotHealthStopState {
            can_cancel: false,
            state: "idle".to_string(),
            reason: "No active health recovery action is running from this local Autopilot shell."
                .to_string(),
        };
    }

    AutopilotHealthStopState {
        can_cancel: false,
        state: "view_only".to_string(),
        reason:
            "Autopilot can inspect Nexus health, but hosted recovery cancellation stays with the health-agent control plane."
                .to_string(),
    }
}

fn endpoint_state(snapshot: &NexusHealthSnapshot) -> String {
    if snapshot.endpoints.values().all(|endpoint| endpoint.ok) {
        "healthy".to_string()
    } else {
        "degraded".to_string()
    }
}

fn endpoint_summary(snapshot: &NexusHealthSnapshot) -> String {
    let failed = snapshot
        .endpoints
        .values()
        .filter(|endpoint| !endpoint.ok)
        .count();
    if failed == 0 {
        format!(
            "all {} public routes are reachable",
            snapshot.endpoints.len()
        )
    } else {
        format!(
            "{failed} of {} public routes failed",
            snapshot.endpoints.len()
        )
    }
}

fn endpoint_detail(snapshot: &NexusHealthSnapshot) -> String {
    let failed = snapshot
        .endpoints
        .values()
        .filter(|endpoint| !endpoint.ok)
        .map(|endpoint| {
            format!(
                "{}: {}",
                endpoint.route_id,
                clean_text(endpoint.error.as_deref().unwrap_or("failed"))
            )
        })
        .collect::<Vec<_>>();
    if failed.is_empty() {
        return "No public endpoint failure detected.".to_string();
    }
    failed.join("; ")
}

fn exact_cause(snapshot: &NexusHealthSnapshot) -> String {
    if snapshot.classification.failed_predicates.is_empty() {
        return "No failed health predicates.".to_string();
    }
    snapshot
        .classification
        .failed_predicates
        .iter()
        .map(|predicate| {
            format!(
                "{}: {}",
                clean_text(&predicate.predicate_id),
                clean_text(&predicate.detail)
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn project_predicate(predicate: &NexusHealthPredicate) -> AutopilotHealthPredicate {
    AutopilotHealthPredicate {
        predicate_id: clean_text(&predicate.predicate_id),
        severity: clean_text(&predicate.severity),
        status: clean_text(&predicate.status),
        detail: clean_text(&predicate.detail),
        remediation_hint: clean_text(&predicate.remediation_hint),
    }
}

fn payout_state(health: &str, failed: &[NexusHealthPredicate]) -> String {
    if failed
        .iter()
        .any(|predicate| predicate.predicate_id.contains("payout"))
    {
        return "degraded".to_string();
    }
    match health {
        "running" | "idle" | "ok" | "healthy" => "healthy".to_string(),
        "warning" | "degraded" => "degraded".to_string(),
        other => clean_text(other),
    }
}

fn training_state(snapshot: &NexusHealthSnapshot, failed: &[NexusHealthPredicate]) -> String {
    if failed
        .iter()
        .any(|predicate| predicate.predicate_id.contains("training"))
    {
        return "degraded".to_string();
    }
    match snapshot.training.launch_health_overall_status.as_deref() {
        Some("ok" | "healthy" | "running") => "healthy".to_string(),
        Some(status) => clean_text(status),
        None if snapshot.training.nodes_online > 0 => "healthy".to_string(),
        None => "waiting".to_string(),
    }
}

fn infra_state(snapshot: &NexusHealthSnapshot, failed: &[NexusHealthPredicate]) -> String {
    if failed.iter().any(|predicate| {
        predicate.predicate_id.contains("cloudflare")
            || predicate.predicate_id.contains("gcp")
            || predicate.predicate_id.contains("relay")
    }) {
        return "degraded".to_string();
    }
    match snapshot.infra.nexus_vm_status.as_deref() {
        Some("RUNNING" | "running" | "active") => "healthy".to_string(),
        Some(status) => clean_text(status),
        None => "not checked".to_string(),
    }
}

fn bool_state(ok: bool) -> String {
    if ok {
        "healthy".to_string()
    } else {
        "degraded".to_string()
    }
}

fn normalize_state(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "ok" | "passed" | "healthy" | "running" | "reachable" => "healthy".to_string(),
        "warn" | "warning" | "degraded" | "failed" | "error" | "unreachable" => {
            "degraded".to_string()
        }
        "none" | "" => "unknown".to_string(),
        other => clean_text(other),
    }
}

fn event(
    id: impl Into<String>,
    at_unix_ms: u64,
    state: impl Into<String>,
    title: impl Into<String>,
    detail: impl Into<String>,
    evidence: impl Into<String>,
) -> AutopilotHealthEvent {
    AutopilotHealthEvent {
        id: clean_text(&id.into()),
        at_unix_ms,
        state: normalize_state(&state.into()),
        title: clean_text(&title.into()),
        detail: clean_text(&detail.into()),
        evidence: clean_text(&evidence.into()),
    }
}

fn metric(label: impl Into<String>, value: impl ToString) -> AutopilotHealthMetric {
    AutopilotHealthMetric {
        label: clean_text(&label.into()),
        value: clean_text(&value.to_string()),
    }
}

fn optional_u64(value: Option<u64>, unit: &str) -> String {
    value
        .map(|count| format!("{count} {unit}"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn env_base_url() -> Option<String> {
    env::var(NEXUS_HEALTH_BASE_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_timeout_ms() -> Option<u64> {
    env::var(NEXUS_HEALTH_TIMEOUT_MS_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
}

fn env_fake() -> bool {
    env::var(NEXUS_HEALTH_FAKE_ENV)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn clean_text(value: &str) -> String {
    let trimmed = value.trim();
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.contains("authorization: bearer")
        || lowered.contains("bearer ")
        || lowered.contains("nexus_admin_bearer_token")
        || lowered.contains("openai_api_key")
        || lowered.contains("spark_wallet_mnemonic")
        || lowered.contains("-----begin")
    {
        return "[redacted]".to_string();
    }

    trimmed
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use nexus_control::{HealthSnapshotCommand, NexusHealthSnapshot};

    use super::{
        NexusHealthRequest, clean_text, nexus_health_status_with_options, project_snapshot,
    };

    fn fake_snapshot() -> NexusHealthSnapshot {
        let command = HealthSnapshotCommand {
            base_url: "https://nexus.openagents.com".to_string(),
            timeout_ms: 1_000,
            fake: true,
            pretty: false,
        };
        let payload = pollster::block_on(nexus_control::run_health_snapshot_command(&command))
            .unwrap_or_else(|error| panic!("fake health snapshot should render: {error}"));
        serde_json::from_str(&payload)
            .unwrap_or_else(|error| panic!("fake health snapshot should decode: {error}"))
    }

    #[test]
    fn projection_has_operator_sections() {
        let projection = project_snapshot(fake_snapshot());

        assert_eq!(projection.state, "healthy");
        assert!(
            projection
                .subsystems
                .iter()
                .any(|item| item.id == "service")
        );
        assert!(
            projection
                .subsystems
                .iter()
                .any(|item| item.id == "treasury")
        );
        assert!(projection.subsystems.iter().any(|item| item.id == "payout"));
        assert!(
            projection
                .subsystems
                .iter()
                .any(|item| item.id == "training")
        );
        assert!(
            projection
                .subsystems
                .iter()
                .any(|item| item.id == "website")
        );
        assert!(projection.subsystems.iter().any(|item| item.id == "fleet"));
        assert!(projection.subsystems.iter().any(|item| item.id == "infra"));
        assert!(projection.event_timeline.len() >= 5);
    }

    #[test]
    fn projection_avoids_internal_noise_words() {
        let projection = project_snapshot(fake_snapshot());
        let payload = serde_json::to_string(&projection)
            .unwrap_or_else(|error| panic!("projection should serialize: {error}"))
            .to_ascii_lowercase();

        for forbidden in [
            "needs attention",
            "sync stale",
            "stack backtrace",
            "authorization: bearer",
            "nexus_admin_bearer_token",
            "openai_api_key",
        ] {
            assert!(
                !payload.contains(forbidden),
                "projection leaked forbidden text: {forbidden}"
            );
        }
    }

    #[test]
    fn projection_keeps_followup_and_stop_shapes() {
        let projection = project_snapshot(fake_snapshot());

        assert!(projection.queued_followups.is_empty());
        assert!(!projection.stop_state.can_cancel);
        assert_eq!(projection.stop_state.state, "idle");
        assert!(projection.latest_action.is_some());
    }

    #[test]
    fn health_command_supports_fake_projection() {
        let projection = pollster::block_on(nexus_health_status_with_options(NexusHealthRequest {
            base_url: None,
            timeout_ms: Some(1_000),
            fake: true,
        }))
        .unwrap_or_else(|error| panic!("fake Autopilot health projection should render: {error}"));

        assert_eq!(projection.source, "nexus-control health snapshot");
        assert_eq!(projection.state, "healthy");
    }

    #[test]
    fn redaction_removes_token_like_strings() {
        assert_eq!(
            clean_text("failed with Authorization: Bearer secret-token"),
            "[redacted]"
        );
        assert_eq!(
            clean_text("normal endpoint timeout"),
            "normal endpoint timeout"
        );
    }
}
