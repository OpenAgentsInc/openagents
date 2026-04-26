use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::health::NexusHealthVerificationGate;
use crate::{HealthSnapshotCommand, NexusHealthSnapshot};

const DEFAULT_NEXUS_BASE_URL: &str = "https://nexus.openagents.com";
const DEFAULT_TIMEOUT_MS: u64 = 20_000;
const VERIFICATION_SCHEMA_VERSION: u32 = 1;

const SENSITIVE_KEY_MARKERS: &[&str] = &[
    "api_key",
    "authorization",
    "bearer",
    "credential",
    "env_file",
    "mnemonic",
    "password",
    "preimage",
    "private_key",
    "raw_env",
    "secret",
    "seed",
    "token",
];

const SENSITIVE_STRING_MARKERS: &[&str] = &[
    "bearer ",
    "authorization:",
    "private_key",
    "mnemonic=",
    "payment_preimage",
    "preimage=",
    "bolt11",
    "lnbc",
    "secret=",
    "api_key=",
    "token=",
    "access_token",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusHealthVerificationPackCommand {
    pub base_url: String,
    pub timeout_ms: u64,
    pub fake: bool,
    pub pretty: bool,
    pub deploy_dry_run: bool,
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthVerificationPackReport {
    pub schema_version: u32,
    pub generated_at_unix_ms: u64,
    pub report_id: String,
    pub base_url: String,
    pub mode: String,
    pub status: String,
    pub summary: String,
    pub snapshot_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<NexusHealthSnapshot>,
    pub required_checks: Vec<NexusHealthVerificationCheck>,
    pub advisory_checks: Vec<NexusHealthVerificationCheck>,
    pub forge_evidence: NexusHealthVerificationEvidence,
    pub redaction: NexusHealthVerificationRedactionReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthVerificationCheck {
    pub check_id: String,
    pub category: String,
    pub requirement: String,
    pub status: String,
    pub passed: bool,
    pub detail: String,
    pub remediation_hint: String,
    #[serde(default)]
    pub evidence: Value,
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthVerificationEvidence {
    pub artifact_kind: String,
    pub content_sha256: String,
    pub storage: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusHealthVerificationRedactionReport {
    pub payload_sensitive_keys_absent: bool,
    pub output_sensitive_strings_absent: bool,
}

pub fn nexus_health_verification_pack_usage() -> &'static str {
    "health verify [--base-url <url>] [--timeout-ms <ms>] [--fake] [--changed-path <path>] [--deploy-dry-run] [--pretty|--json]"
}

pub fn parse_nexus_health_verification_pack_command(
    args: &[String],
) -> Result<NexusHealthVerificationPackCommand> {
    let mut index = match args.get(2).map(String::as_str) {
        Some("verify" | "verification-pack") => 3,
        Some(other) => bail!("unknown health verification command `{other}`"),
        None => bail!("missing health verification command"),
    };
    let mut command = NexusHealthVerificationPackCommand {
        base_url: DEFAULT_NEXUS_BASE_URL.to_string(),
        timeout_ms: DEFAULT_TIMEOUT_MS,
        fake: false,
        pretty: false,
        deploy_dry_run: false,
        changed_paths: Vec::new(),
    };

    while let Some(arg) = args.get(index).map(String::as_str) {
        match arg {
            "--base-url" => {
                index += 1;
                command.base_url = required_arg(args, index, "--base-url")?.to_string();
            }
            "--timeout-ms" => {
                index += 1;
                command.timeout_ms =
                    parse_positive_u64(required_arg(args, index, "--timeout-ms")?, "--timeout-ms")?;
            }
            "--fake" => command.fake = true,
            "--pretty" => command.pretty = true,
            "--json" => command.pretty = false,
            "--deploy-dry-run" => command.deploy_dry_run = true,
            "--changed-path" => {
                index += 1;
                command
                    .changed_paths
                    .push(required_arg(args, index, "--changed-path")?.to_string());
            }
            "--help" | "-h" => bail!(
                "usage: nexus-control {}",
                nexus_health_verification_pack_usage()
            ),
            other => bail!("unknown health verification option `{other}`"),
        }
        index += 1;
    }

    validate_base_url(command.base_url.as_str())?;
    Ok(command)
}

pub async fn run_nexus_health_verification_pack_command(
    command: &NexusHealthVerificationPackCommand,
) -> Result<String> {
    let report = run_nexus_health_verification_pack(command).await?;
    if command.pretty {
        serde_json::to_string_pretty(&report).context("serialize Nexus health verification pack")
    } else {
        serde_json::to_string(&report).context("serialize Nexus health verification pack")
    }
}

pub async fn run_nexus_health_verification_pack(
    command: &NexusHealthVerificationPackCommand,
) -> Result<NexusHealthVerificationPackReport> {
    validate_base_url(command.base_url.as_str())?;
    if command.timeout_ms == 0 {
        bail!("timeout must be greater than zero");
    }

    let generated_at_unix_ms = now_unix_ms();
    let snapshot_outcome = capture_snapshot(command).await;
    let (required_checks, advisory_checks, snapshot_status, snapshot_error, snapshot) =
        match &snapshot_outcome {
            SnapshotOutcome::Captured(snapshot) => (
                required_checks_for_snapshot(snapshot),
                advisory_checks_for_snapshot(snapshot, command),
                "captured".to_string(),
                None,
                Some(snapshot.clone()),
            ),
            SnapshotOutcome::Failed(error) => (
                vec![failed_required_check(
                    "nexus.health.snapshot_captured",
                    "snapshot",
                    format!(
                        "Nexus health snapshot failed: {}",
                        redact_sensitive_text(error)
                    ),
                    "Fix Nexus reachability or health command configuration before trusting secondary checks.",
                    json!({"snapshot_error": redact_sensitive_text(error)}),
                )],
                failed_snapshot_advisory_checks(command),
                "failed".to_string(),
                Some(redact_sensitive_text(error)),
                None,
            ),
        };

    let required_failed = required_checks.iter().any(|check| check.status != "passed");
    let advisory_attention = advisory_checks
        .iter()
        .any(|check| check.status == "advisory");
    let status = if required_failed {
        "failed"
    } else if advisory_attention {
        "advisory"
    } else {
        "passed"
    }
    .to_string();
    let summary = summary_for_status(status.as_str(), &required_checks, &advisory_checks);
    let evidence_payload = json!({
        "schema_version": VERIFICATION_SCHEMA_VERSION,
        "base_url": command.base_url,
        "mode": if command.fake { "fake" } else { "live" },
        "status": status,
        "snapshot_status": snapshot_status,
        "required_checks": required_checks,
        "advisory_checks": advisory_checks,
    });
    let report_id = format!(
        "nexus-health-verification-{}",
        stable_value_digest(&evidence_payload)
    );
    let forge_evidence = NexusHealthVerificationEvidence {
        artifact_kind: "nexus.health.verification_pack".to_string(),
        content_sha256: stable_value_digest(&evidence_payload),
        storage: "inline.redacted".to_string(),
        summary: format!("Nexus health verification pack: {status}"),
    };

    let mut report = NexusHealthVerificationPackReport {
        schema_version: VERIFICATION_SCHEMA_VERSION,
        generated_at_unix_ms,
        report_id,
        base_url: command.base_url.clone(),
        mode: if command.fake { "fake" } else { "live" }.to_string(),
        status,
        summary,
        snapshot_status,
        snapshot_error,
        snapshot,
        required_checks,
        advisory_checks,
        forge_evidence,
        redaction: NexusHealthVerificationRedactionReport {
            payload_sensitive_keys_absent: true,
            output_sensitive_strings_absent: true,
        },
    };
    redact_report(&mut report)?;
    let report_value =
        serde_json::to_value(&report).context("serialize report for redaction proof")?;
    report.redaction = NexusHealthVerificationRedactionReport {
        payload_sensitive_keys_absent: !value_contains_sensitive_key(&report_value),
        output_sensitive_strings_absent: !value_contains_sensitive_string(&report_value),
    };
    Ok(report)
}

#[derive(Debug, Clone)]
enum SnapshotOutcome {
    Captured(NexusHealthSnapshot),
    Failed(String),
}

async fn capture_snapshot(command: &NexusHealthVerificationPackCommand) -> SnapshotOutcome {
    let snapshot_command = HealthSnapshotCommand {
        base_url: command.base_url.clone(),
        timeout_ms: command.timeout_ms,
        fake: command.fake,
        pretty: false,
    };
    match crate::run_health_snapshot_command(&snapshot_command).await {
        Ok(output) => serde_json::from_str::<NexusHealthSnapshot>(&output)
            .map(SnapshotOutcome::Captured)
            .unwrap_or_else(|error| SnapshotOutcome::Failed(error.to_string())),
        Err(error) => SnapshotOutcome::Failed(error.to_string()),
    }
}

fn required_checks_for_snapshot(
    snapshot: &NexusHealthSnapshot,
) -> Vec<NexusHealthVerificationCheck> {
    let mut checks = vec![passed_required_check(
        "nexus.health.snapshot_captured",
        "snapshot",
        "Nexus health snapshot was captured and decoded",
        json!({
            "schema_version": snapshot.schema_version,
            "observation_status": snapshot.observation_status,
            "classification_state": snapshot.classification.state,
        }),
    )];
    checks.extend([
        endpoint_required_check(snapshot, "healthz", "nexus.public.healthz_reachable"),
        endpoint_required_check(snapshot, "stats", "nexus.public.stats_reachable"),
        endpoint_required_check(snapshot, "treasury", "nexus.public.treasury_status_reachable"),
        bool_required_check(
            "nexus.treasury.wallet_connected",
            "treasury",
            snapshot.treasury.wallet_connected,
            "Treasury wallet runtime is connected",
            "Treasury wallet runtime is not connected",
            "Restore wallet runtime connectivity before claiming payout health.",
            json!({
                "wallet_runtime_status": snapshot.treasury.wallet_runtime_status,
                "wallet_balance_sats": snapshot.treasury.wallet_balance_sats,
            }),
        ),
        bool_required_check(
            "nexus.treasury.not_degraded",
            "treasury",
            snapshot.treasury.degraded_reason.is_none(),
            "Treasury status does not report a degraded reason",
            format!(
                "Treasury degraded reason is {}",
                snapshot
                    .treasury
                    .degraded_reason
                    .as_deref()
                    .unwrap_or("unknown")
            ),
            "Resolve treasury degraded_reason before dispatching or verifying payouts.",
            json!({"degraded_reason": snapshot.treasury.degraded_reason}),
        ),
        gate_required_check(
            snapshot,
            "payout_capability",
            "nexus.treasury.payout_capability_gate",
            "treasury",
            "Payout capability verification gate passed",
            "Payout capability verification gate failed",
            "Inspect treasury status, wallet connectivity, payout dispatch freshness, and confirmation reconciliation.",
        ),
        gate_required_check(
            snapshot,
            "website_stats_freshness",
            "nexus.website.stats_freshness_gate",
            "website",
            "Website stats freshness gate passed",
            "Website stats freshness gate failed",
            "Refresh Nexus stats publication and website projections before trusting public telemetry.",
        ),
        gate_required_check(
            snapshot,
            "infra_availability",
            "nexus.infra.availability_gate",
            "infra",
            "Infrastructure availability gate passed",
            "Infrastructure availability gate failed",
            "Restore public Nexus reachability, relay service health, and tunnel health before normal issue work.",
        ),
        passed_required_check(
            "nexus.pylon.version_floor_telemetry_projected",
            "pylon",
            "Pylon fleet compatibility counters are projected in the health snapshot",
            json!({
                "pylons_online_now": snapshot.fleet.pylons_online_now,
                "pylon_sessions_online_now": snapshot.fleet.pylon_sessions_online_now,
                "eligible_online_payout_targets": snapshot.fleet.eligible_online_payout_targets,
                "homework_worker_eligible_pylons_online_now": snapshot.fleet.homework_worker_eligible_pylons_online_now,
                "version_floor_blocked_beneficiaries_now": snapshot.fleet.version_floor_blocked_beneficiaries_now,
                "homework_worker_presence_only_blocker_counts": snapshot.fleet.homework_worker_presence_only_blocker_counts,
            }),
        ),
        training_dispatch_required_check(snapshot),
    ]);
    checks
}

fn advisory_checks_for_snapshot(
    snapshot: &NexusHealthSnapshot,
    command: &NexusHealthVerificationPackCommand,
) -> Vec<NexusHealthVerificationCheck> {
    vec![
        recent_payout_movement_check(snapshot),
        training_launch_health_check(snapshot),
        training_dispatch_smoke_check(snapshot),
        homework_worker_eligibility_check(snapshot),
        pylon_version_blocker_check(snapshot),
        changed_crate_test_selection_check(command),
        deploy_dry_run_check(command),
    ]
}

fn failed_snapshot_advisory_checks(
    command: &NexusHealthVerificationPackCommand,
) -> Vec<NexusHealthVerificationCheck> {
    vec![
        skipped_advisory_check(
            "nexus.payout.recent_movement",
            "treasury",
            "Skipped payout movement check because the health snapshot failed",
            "Capture a Nexus health snapshot first.",
            json!({}),
            Vec::new(),
        ),
        skipped_advisory_check(
            "nexus.training.dispatch_smoke",
            "training",
            "Skipped training dispatch smoke because the health snapshot failed",
            "Capture a Nexus health snapshot first.",
            json!({}),
            Vec::new(),
        ),
        changed_crate_test_selection_check(command),
        deploy_dry_run_check(command),
    ]
}

fn endpoint_required_check(
    snapshot: &NexusHealthSnapshot,
    route_id: &str,
    check_id: &str,
) -> NexusHealthVerificationCheck {
    match snapshot.endpoints.get(route_id) {
        Some(endpoint) if endpoint.ok => passed_required_check(
            check_id,
            "public_endpoint",
            format!("{} responded successfully", endpoint.path),
            json!({
                "route_id": endpoint.route_id,
                "path": endpoint.path,
                "status_code": endpoint.status_code,
                "latency_ms": endpoint.latency_ms,
            }),
        ),
        Some(endpoint) => failed_required_check(
            check_id,
            "public_endpoint",
            format!(
                "{} failed with status {:?}: {}",
                endpoint.path,
                endpoint.status_code,
                endpoint.error.as_deref().unwrap_or("endpoint probe failed")
            ),
            "Restore public Nexus reachability before closing health or payout work.",
            json!({
                "route_id": endpoint.route_id,
                "path": endpoint.path,
                "status_code": endpoint.status_code,
                "cloudflare_error_code": endpoint.cloudflare_error_code,
                "error": endpoint.error,
            }),
        ),
        None => failed_required_check(
            check_id,
            "public_endpoint",
            format!("required endpoint `{route_id}` was missing from the health snapshot"),
            "Fix health snapshot projection before trusting verification results.",
            json!({"route_id": route_id}),
        ),
    }
}

fn bool_required_check(
    check_id: &str,
    category: &str,
    passed: bool,
    passed_detail: impl Into<String>,
    failed_detail: impl Into<String>,
    remediation_hint: &str,
    evidence: Value,
) -> NexusHealthVerificationCheck {
    if passed {
        passed_required_check(check_id, category, passed_detail, evidence)
    } else {
        failed_required_check(
            check_id,
            category,
            failed_detail,
            remediation_hint,
            evidence,
        )
    }
}

fn gate_required_check(
    snapshot: &NexusHealthSnapshot,
    gate_id: &str,
    check_id: &str,
    category: &str,
    passed_detail: &str,
    failed_detail: &str,
    remediation_hint: &str,
) -> NexusHealthVerificationCheck {
    match snapshot.verification_gates.get(gate_id) {
        Some(gate) if gate.passed => {
            passed_required_check(check_id, category, passed_detail, gate_evidence(gate))
        }
        Some(gate) => failed_required_check(
            check_id,
            category,
            failed_detail,
            remediation_hint,
            gate_evidence(gate),
        ),
        None => failed_required_check(
            check_id,
            category,
            format!("verification gate `{gate_id}` was missing from the health snapshot"),
            "Fix health snapshot gate projection before trusting verification results.",
            json!({"gate_id": gate_id}),
        ),
    }
}

fn gate_evidence(gate: &NexusHealthVerificationGate) -> Value {
    json!({
        "gate_id": gate.gate_id,
        "status": gate.status,
        "passed": gate.passed,
        "checked_predicates": gate.checked_predicates,
        "failed_predicates": gate.failed_predicates,
    })
}

fn training_dispatch_required_check(
    snapshot: &NexusHealthSnapshot,
) -> NexusHealthVerificationCheck {
    let dispatch_visible = snapshot.training.runs_active > 0
        || snapshot.training.windows_active > 0
        || snapshot.training.active_run_id.is_some()
        || snapshot.training.accepted_closeouts > 0;
    let evidence = json!({
        "training_nodes_online": snapshot.training.nodes_online,
        "training_admitted_nodes_online": snapshot.training.admitted_nodes_online,
        "training_runs_active": snapshot.training.runs_active,
        "training_windows_active": snapshot.training.windows_active,
        "accepted_closeouts": snapshot.training.accepted_closeouts,
        "active_run_id": snapshot.training.active_run_id,
        "active_window_id": snapshot.training.active_window_id,
    });
    if snapshot.training.nodes_online == 0 {
        passed_required_check(
            "nexus.training.dispatch_smoke_required",
            "training",
            "No training nodes are online, so dispatch is not blocked by Nexus state",
            evidence,
        )
    } else if dispatch_visible {
        passed_required_check(
            "nexus.training.dispatch_smoke_required",
            "training",
            "Training nodes are online and run/window/closeout activity is visible",
            evidence,
        )
    } else {
        failed_required_check(
            "nexus.training.dispatch_smoke_required",
            "training",
            "Training nodes are online but no run, active window, or accepted closeout is visible",
            "Launch a bounded training dispatch or inspect the hosted dispatcher loop.",
            evidence,
        )
    }
}

fn training_launch_health_check(snapshot: &NexusHealthSnapshot) -> NexusHealthVerificationCheck {
    let status = snapshot
        .training
        .launch_health_overall_status
        .as_deref()
        .unwrap_or("not_reported");
    let evidence = json!({
        "launch_health_overall_status": snapshot.training.launch_health_overall_status,
        "windows_pending_validation": snapshot.training.windows_pending_validation,
        "validator_challenges_open": snapshot.training.validator_challenges_open,
        "validator_challenges_queued": snapshot.training.validator_challenges_queued,
        "accepted_work_pending_payout_count": snapshot.training.accepted_work_pending_payout_count,
        "accepted_work_attention_payout_count": snapshot.training.accepted_work_attention_payout_count,
    });
    if snapshot.training.launch_health_overall_status.is_none() {
        skipped_advisory_check(
            "nexus.training.launch_health",
            "training",
            "Training launch health is not reported by the current public stats snapshot",
            "Use dispatch smoke and run detail evidence when launch-health telemetry is unavailable.",
            evidence,
            Vec::new(),
        )
    } else if unhealthy_status(status) {
        advisory_attention_check(
            "nexus.training.launch_health",
            "training",
            format!("Training launch health is {status}"),
            "Inspect training launch alerts, but do not treat launch backlog alone as proof that dispatch is unavailable.",
            evidence,
            Vec::new(),
        )
    } else {
        passed_advisory_check(
            "nexus.training.launch_health",
            "training",
            format!("Training launch health is {status}"),
            evidence,
            Vec::new(),
        )
    }
}

fn recent_payout_movement_check(snapshot: &NexusHealthSnapshot) -> NexusHealthVerificationCheck {
    let evidence = json!({
        "eligible_online_payout_targets": snapshot.fleet.eligible_online_payout_targets,
        "payouts_dispatched_24h": snapshot.treasury.payouts_dispatched_24h,
        "payouts_confirmed_24h": snapshot.treasury.payouts_confirmed_24h,
        "last_dispatch_at_unix_ms": snapshot.treasury.last_dispatch_at_unix_ms,
        "last_confirmed_payout_at_unix_ms": snapshot.treasury.last_confirmed_payout_at_unix_ms,
    });
    if snapshot.fleet.eligible_online_payout_targets == 0 {
        return skipped_advisory_check(
            "nexus.payout.recent_movement",
            "treasury",
            "No eligible online payout targets were present, so payout movement is not expected",
            "Bring an eligible Pylon online before using this advisory as payout proof.",
            evidence,
            Vec::new(),
        );
    }
    if snapshot.treasury.payouts_dispatched_24h > 0 || snapshot.treasury.payouts_confirmed_24h > 0 {
        passed_advisory_check(
            "nexus.payout.recent_movement",
            "treasury",
            "Recent payout movement exists while eligible online payout targets are present",
            evidence,
            Vec::new(),
        )
    } else {
        advisory_attention_check(
            "nexus.payout.recent_movement",
            "treasury",
            "Eligible online payout targets exist but no payout dispatch or confirmation is visible in the last 24h",
            "Trigger a bounded payout-producing run or inspect the payout dispatcher before claiming live money movement.",
            evidence,
            vec![
                "cargo run -p nexus-control --bin nexus-control -- health verify --pretty"
                    .to_string(),
                "scripts/deploy/nexus/04-verify-gates.sh".to_string(),
            ],
        )
    }
}

fn training_dispatch_smoke_check(snapshot: &NexusHealthSnapshot) -> NexusHealthVerificationCheck {
    let evidence = json!({
        "training_nodes_online": snapshot.training.nodes_online,
        "training_admitted_nodes_online": snapshot.training.admitted_nodes_online,
        "training_runs_active": snapshot.training.runs_active,
        "training_windows_active": snapshot.training.windows_active,
        "accepted_closeouts": snapshot.training.accepted_closeouts,
        "payout_eligible_closeouts": snapshot.training.payout_eligible_closeouts,
        "active_run_id": snapshot.training.active_run_id,
        "active_window_id": snapshot.training.active_window_id,
    });
    if snapshot.training.nodes_online == 0 {
        return skipped_advisory_check(
            "nexus.training.dispatch_smoke",
            "training",
            "No training nodes are online, so dispatch smoke is not expected",
            "Bring at least one eligible Pylon online before using this advisory as work-distribution proof.",
            evidence,
            Vec::new(),
        );
    }
    if snapshot.training.runs_active > 0
        || snapshot.training.windows_active > 0
        || snapshot.training.accepted_closeouts > 0
    {
        passed_advisory_check(
            "nexus.training.dispatch_smoke",
            "training",
            "Training nodes are online and dispatch/run/closeout activity is visible",
            evidence,
            Vec::new(),
        )
    } else {
        advisory_attention_check(
            "nexus.training.dispatch_smoke",
            "training",
            "Training nodes are online but no run, active window, or accepted closeout is visible",
            "Launch a bounded training dispatch or inspect the hosted dispatcher loop.",
            evidence,
            vec![
                "cargo run -p nexus-control --bin nexus-control -- health verify --pretty"
                    .to_string(),
            ],
        )
    }
}

fn pylon_version_blocker_check(snapshot: &NexusHealthSnapshot) -> NexusHealthVerificationCheck {
    let blocked = snapshot
        .fleet
        .version_floor_blocked_beneficiaries_now
        .saturating_add(snapshot.fleet.readiness_blocked_beneficiaries_now);
    let evidence = json!({
        "version_floor_blocked_beneficiaries_now": snapshot.fleet.version_floor_blocked_beneficiaries_now,
        "readiness_blocked_beneficiaries_now": snapshot.fleet.readiness_blocked_beneficiaries_now,
        "missing_payout_target_blocked_beneficiaries_now": snapshot.fleet.missing_payout_target_blocked_beneficiaries_now,
        "duplicate_host_blocked_beneficiaries_now": snapshot.fleet.duplicate_host_blocked_beneficiaries_now,
        "duplicate_payout_target_blocked_beneficiaries_now": snapshot.fleet.duplicate_payout_target_blocked_beneficiaries_now,
    });
    if blocked == 0 {
        passed_advisory_check(
            "nexus.pylon.version_floor_blockers",
            "pylon",
            "No version-floor or readiness blockers are visible for current Pylons",
            evidence,
            Vec::new(),
        )
    } else {
        advisory_attention_check(
            "nexus.pylon.version_floor_blockers",
            "pylon",
            format!(
                "{blocked} Pylon beneficiary blocker(s) are visible for version floor or readiness"
            ),
            "Confirm the release floor is intentional and keep user install instructions aligned with the latest auto-updating Pylon release.",
            evidence,
            Vec::new(),
        )
    }
}

fn homework_worker_eligibility_check(
    snapshot: &NexusHealthSnapshot,
) -> NexusHealthVerificationCheck {
    let evidence = json!({
        "pylons_online_now": snapshot.fleet.pylons_online_now,
        "pylon_sessions_online_now": snapshot.fleet.pylon_sessions_online_now,
        "homework_worker_eligible_pylons_online_now": snapshot.fleet.homework_worker_eligible_pylons_online_now,
        "training_nodes_online": snapshot.training.nodes_online,
        "training_admitted_nodes_online": snapshot.training.admitted_nodes_online,
        "homework_worker_presence_only_blocker_counts": snapshot.fleet.homework_worker_presence_only_blocker_counts,
    });
    if snapshot.fleet.pylons_online_now == 0 {
        skipped_advisory_check(
            "nexus.pylon.homework_worker_eligibility",
            "pylon",
            "No online Pylons are present, so homework-worker eligibility is not expected",
            "Bring an updated Pylon online before using this advisory as dispatch proof.",
            evidence,
            Vec::new(),
        )
    } else if snapshot.fleet.homework_worker_eligible_pylons_online_now > 0 {
        passed_advisory_check(
            "nexus.pylon.homework_worker_eligibility",
            "pylon",
            "At least one online Pylon is eligible for homework-worker assignment",
            evidence,
            Vec::new(),
        )
    } else {
        advisory_attention_check(
            "nexus.pylon.homework_worker_eligibility",
            "pylon",
            "Online Pylons are present, but none are eligible homework workers",
            "Inspect the blocker counts to distinguish presence-only payout eligibility from homework worker admission.",
            evidence,
            Vec::new(),
        )
    }
}

fn changed_crate_test_selection_check(
    command: &NexusHealthVerificationPackCommand,
) -> NexusHealthVerificationCheck {
    let commands = changed_path_test_commands(&command.changed_paths);
    let evidence = json!({
        "changed_paths": command.changed_paths,
        "selected_commands": commands,
    });
    if command.changed_paths.is_empty() {
        skipped_advisory_check(
            "openagents.changed_crate_tests",
            "local_tests",
            "No changed paths were supplied, so changed-crate test selection is not applicable",
            "Pass one or more --changed-path values from the diff when attaching verification evidence to a code change.",
            evidence,
            Vec::new(),
        )
    } else if commands.is_empty() {
        passed_advisory_check(
            "openagents.changed_crate_tests",
            "local_tests",
            "Changed paths do not map to a Nexus/Pylon verification command",
            evidence,
            Vec::new(),
        )
    } else {
        advisory_attention_check(
            "openagents.changed_crate_tests",
            "local_tests",
            "Changed paths map to focused local test commands that should be attached as evidence",
            "Run the selected commands and attach their output or CI evidence before closing the work item.",
            evidence,
            commands,
        )
    }
}

fn deploy_dry_run_check(
    command: &NexusHealthVerificationPackCommand,
) -> NexusHealthVerificationCheck {
    let deploy_path_changed = command
        .changed_paths
        .iter()
        .any(|path| path.starts_with("scripts/deploy/nexus/") || path.starts_with("docs/deploy/"));
    let commands = vec![
        "scripts/deploy/nexus/test-health-runner-deploy-shell-guards.sh".to_string(),
        "NEXUS_HEALTH_RUNNER_DRY_RUN=true scripts/deploy/nexus/18-deploy-health-runner-job.sh"
            .to_string(),
    ];
    let evidence = json!({
        "deploy_dry_run_requested": command.deploy_dry_run,
        "deploy_path_changed": deploy_path_changed,
        "selected_commands": commands,
    });
    if command.deploy_dry_run || deploy_path_changed {
        advisory_attention_check(
            "nexus.deploy.dry_run_plan",
            "deploy",
            "Nexus deploy-adjacent paths changed or deploy dry-run was requested",
            "Run the shell guards and dry-run deploy command from a GCP-authenticated operator shell before live rollout.",
            evidence,
            commands,
        )
    } else {
        skipped_advisory_check(
            "nexus.deploy.dry_run_plan",
            "deploy",
            "No deploy-adjacent path change or explicit deploy dry-run request was supplied",
            "Use --deploy-dry-run when verifying deployment-lane changes.",
            evidence,
            commands,
        )
    }
}

fn changed_path_test_commands(changed_paths: &[String]) -> Vec<String> {
    let mut commands = BTreeSet::new();
    for path in changed_paths {
        if path.starts_with("apps/nexus-control/") {
            commands.insert("cargo test -p nexus-control health -- --nocapture".to_string());
            commands.insert(
                "cargo test -p nexus-control health_verification -- --nocapture".to_string(),
            );
            commands.insert("cargo check -p nexus-control --bins".to_string());
        }
        if path.starts_with("apps/pylon/") || path.starts_with("apps/pylon-tui/") {
            commands.insert("cargo test -p pylon --lib pylon_training -- --nocapture".to_string());
            commands.insert("cargo check -p pylon --bins".to_string());
        }
        if path.starts_with("scripts/deploy/nexus/") {
            commands.insert(
                "scripts/deploy/nexus/test-health-runner-deploy-shell-guards.sh".to_string(),
            );
        }
    }
    commands.into_iter().collect()
}

fn passed_required_check(
    check_id: &str,
    category: &str,
    detail: impl Into<String>,
    evidence: Value,
) -> NexusHealthVerificationCheck {
    NexusHealthVerificationCheck {
        check_id: check_id.to_string(),
        category: category.to_string(),
        requirement: "required".to_string(),
        status: "passed".to_string(),
        passed: true,
        detail: detail.into(),
        remediation_hint: "none".to_string(),
        evidence,
        commands: Vec::new(),
    }
}

fn failed_required_check(
    check_id: &str,
    category: &str,
    detail: impl Into<String>,
    remediation_hint: &str,
    evidence: Value,
) -> NexusHealthVerificationCheck {
    NexusHealthVerificationCheck {
        check_id: check_id.to_string(),
        category: category.to_string(),
        requirement: "required".to_string(),
        status: "failed".to_string(),
        passed: false,
        detail: detail.into(),
        remediation_hint: remediation_hint.to_string(),
        evidence,
        commands: Vec::new(),
    }
}

fn passed_advisory_check(
    check_id: &str,
    category: &str,
    detail: impl Into<String>,
    evidence: Value,
    commands: Vec<String>,
) -> NexusHealthVerificationCheck {
    NexusHealthVerificationCheck {
        check_id: check_id.to_string(),
        category: category.to_string(),
        requirement: "advisory".to_string(),
        status: "passed".to_string(),
        passed: true,
        detail: detail.into(),
        remediation_hint: "none".to_string(),
        evidence,
        commands,
    }
}

fn advisory_attention_check(
    check_id: &str,
    category: &str,
    detail: impl Into<String>,
    remediation_hint: &str,
    evidence: Value,
    commands: Vec<String>,
) -> NexusHealthVerificationCheck {
    NexusHealthVerificationCheck {
        check_id: check_id.to_string(),
        category: category.to_string(),
        requirement: "advisory".to_string(),
        status: "advisory".to_string(),
        passed: false,
        detail: detail.into(),
        remediation_hint: remediation_hint.to_string(),
        evidence,
        commands,
    }
}

fn skipped_advisory_check(
    check_id: &str,
    category: &str,
    detail: impl Into<String>,
    remediation_hint: &str,
    evidence: Value,
    commands: Vec<String>,
) -> NexusHealthVerificationCheck {
    NexusHealthVerificationCheck {
        check_id: check_id.to_string(),
        category: category.to_string(),
        requirement: "advisory".to_string(),
        status: "skipped".to_string(),
        passed: false,
        detail: detail.into(),
        remediation_hint: remediation_hint.to_string(),
        evidence,
        commands,
    }
}

fn summary_for_status(
    status: &str,
    required_checks: &[NexusHealthVerificationCheck],
    advisory_checks: &[NexusHealthVerificationCheck],
) -> String {
    let failed_required = required_checks
        .iter()
        .filter(|check| check.status == "failed")
        .count();
    let advisory_attention = advisory_checks
        .iter()
        .filter(|check| check.status == "advisory")
        .count();
    match status {
        "failed" => format!("{failed_required} required Nexus health verification check(s) failed"),
        "advisory" => format!(
            "required Nexus health checks passed; {advisory_attention} advisory check(s) need operator evidence"
        ),
        _ => "required Nexus health checks passed".to_string(),
    }
}

fn redact_report(report: &mut NexusHealthVerificationPackReport) -> Result<()> {
    let mut value = serde_json::to_value(&*report).context("serialize verification report")?;
    redact_value_in_place(&mut value);
    *report = serde_json::from_value(value).context("deserialize redacted verification report")?;
    Ok(())
}

fn redact_value_in_place(value: &mut Value) {
    match value {
        Value::Object(map) => redact_map_in_place(map),
        Value::Array(values) => values.iter_mut().for_each(redact_value_in_place),
        Value::String(text) => {
            *text = redact_sensitive_text(text.as_str());
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn redact_map_in_place(map: &mut Map<String, Value>) {
    let mut replacement = Map::new();
    let mut redacted_count = 0_u64;
    let original = std::mem::take(map);
    for (key, mut value) in original {
        if is_sensitive_key(&key) {
            redacted_count = redacted_count.saturating_add(1);
        } else {
            redact_value_in_place(&mut value);
            replacement.insert(key, value);
        }
    }
    if redacted_count > 0 {
        replacement.insert(
            "removed_sensitive_field_count".to_string(),
            json!(redacted_count),
        );
    }
    *map = replacement;
}

fn redact_sensitive_text(value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    if string_contains_sensitive_marker(value) {
        "[redacted]".to_string()
    } else {
        value.chars().take(320).collect()
    }
}

fn value_contains_sensitive_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, value)| is_sensitive_key(key) || value_contains_sensitive_key(value)),
        Value::Array(values) => values.iter().any(value_contains_sensitive_key),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

fn value_contains_sensitive_string(value: &Value) -> bool {
    match value {
        Value::String(text) => string_contains_sensitive_marker(text),
        Value::Object(map) => map.values().any(value_contains_sensitive_string),
        Value::Array(values) => values.iter().any(value_contains_sensitive_string),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SENSITIVE_KEY_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

fn string_contains_sensitive_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    SENSITIVE_STRING_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
        || lower.starts_with("sk-")
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

fn stable_value_digest(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

fn validate_base_url(base_url: &str) -> Result<()> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        bail!("base URL cannot be empty");
    }
    reqwest::Url::parse(
        if trimmed.ends_with('/') {
            trimmed.to_string()
        } else {
            format!("{trimmed}/")
        }
        .as_str(),
    )
    .with_context(|| format!("invalid base URL `{trimmed}`"))?;
    Ok(())
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))
}

fn parse_positive_u64(raw: &str, name: &str) -> Result<u64> {
    let value = raw
        .parse::<u64>()
        .with_context(|| format!("invalid {name} value `{raw}`"))?;
    if value == 0 {
        bail!("{name} must be greater than zero");
    }
    Ok(value)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_verification_pack_passes_required_checks() {
        let command = NexusHealthVerificationPackCommand {
            base_url: DEFAULT_NEXUS_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            fake: true,
            pretty: false,
            deploy_dry_run: false,
            changed_paths: Vec::new(),
        };
        let report = run_nexus_health_verification_pack(&command)
            .await
            .expect("verification report");
        assert_eq!(report.status, "passed");
        assert_eq!(report.snapshot_status, "captured");
        assert!(report.snapshot.is_some());
        assert!(report.required_checks.iter().all(|check| check.passed));
        assert!(
            report
                .required_checks
                .iter()
                .any(|check| check.check_id == "nexus.treasury.payout_capability_gate")
        );
        assert_eq!(
            report.forge_evidence.artifact_kind,
            "nexus.health.verification_pack"
        );
        assert!(report.redaction.payload_sensitive_keys_absent);
        assert!(report.redaction.output_sensitive_strings_absent);
    }

    #[test]
    fn parse_verification_pack_options() {
        let args = vec![
            "nexus-control".to_string(),
            "health".to_string(),
            "verify".to_string(),
            "--base-url".to_string(),
            "http://127.0.0.1:42020".to_string(),
            "--timeout-ms".to_string(),
            "1234".to_string(),
            "--fake".to_string(),
            "--changed-path".to_string(),
            "apps/nexus-control/src/health.rs".to_string(),
            "--deploy-dry-run".to_string(),
            "--pretty".to_string(),
        ];
        let command =
            parse_nexus_health_verification_pack_command(args.as_slice()).expect("parse command");
        assert_eq!(command.base_url, "http://127.0.0.1:42020");
        assert_eq!(command.timeout_ms, 1234);
        assert!(command.fake);
        assert!(command.deploy_dry_run);
        assert!(command.pretty);
        assert_eq!(
            command.changed_paths,
            vec!["apps/nexus-control/src/health.rs".to_string()]
        );
    }

    #[tokio::test]
    async fn endpoint_failure_is_machine_readable_failure() {
        let command = NexusHealthVerificationPackCommand {
            base_url: "http://127.0.0.1:9".to_string(),
            timeout_ms: 1,
            fake: false,
            pretty: false,
            deploy_dry_run: false,
            changed_paths: Vec::new(),
        };
        let report = run_nexus_health_verification_pack(&command)
            .await
            .expect("verification report");
        assert_eq!(report.status, "failed");
        assert_eq!(report.snapshot_status, "captured");
        assert!(report.snapshot_error.is_none());
        assert!(
            report
                .required_checks
                .iter()
                .any(|check| check.check_id == "nexus.public.healthz_reachable"
                    && check.status == "failed")
        );
    }

    #[tokio::test]
    async fn changed_paths_select_focused_test_commands() {
        let command = NexusHealthVerificationPackCommand {
            base_url: DEFAULT_NEXUS_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            fake: true,
            pretty: false,
            deploy_dry_run: false,
            changed_paths: vec![
                "apps/nexus-control/src/health.rs".to_string(),
                "scripts/deploy/nexus/18-deploy-health-runner-job.sh".to_string(),
            ],
        };
        let report = run_nexus_health_verification_pack(&command)
            .await
            .expect("verification report");
        let changed_check = report
            .advisory_checks
            .iter()
            .find(|check| check.check_id == "openagents.changed_crate_tests")
            .expect("changed path check");
        assert_eq!(changed_check.status, "advisory");
        assert!(
            changed_check.commands.iter().any(|command| command
                == "cargo test -p nexus-control health_verification -- --nocapture")
        );
        let deploy_check = report
            .advisory_checks
            .iter()
            .find(|check| check.check_id == "nexus.deploy.dry_run_plan")
            .expect("deploy dry-run check");
        assert_eq!(deploy_check.status, "advisory");
    }

    #[tokio::test]
    async fn training_launch_backlog_is_advisory_not_required_dispatch_failure() {
        let command = NexusHealthVerificationPackCommand {
            base_url: DEFAULT_NEXUS_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            fake: true,
            pretty: false,
            deploy_dry_run: false,
            changed_paths: Vec::new(),
        };
        let mut report = run_nexus_health_verification_pack(&command)
            .await
            .expect("verification report");
        let mut snapshot = report.snapshot.take().expect("snapshot");
        snapshot.training.launch_health_overall_status = Some("bad".to_string());
        snapshot.training.nodes_online = 3;
        snapshot.training.runs_active = 1;
        let required = required_checks_for_snapshot(&snapshot);
        assert!(
            required.iter().all(|check| check.status == "passed"),
            "launch backlog should not fail required dispatch smoke: {required:?}"
        );
        let advisory = training_launch_health_check(&snapshot);
        assert_eq!(advisory.status, "advisory");
        assert_eq!(advisory.check_id, "nexus.training.launch_health");
    }

    #[test]
    fn redaction_removes_sensitive_keys_and_strings() {
        let mut value = json!({
            "safe": "ok",
            "api_key": "sk-test",
            "nested": {
                "detail": "token=raw-value"
            }
        });
        redact_value_in_place(&mut value);
        assert!(value.get("api_key").is_none());
        assert_eq!(value["removed_sensitive_field_count"], 1);
        assert_eq!(value["nested"]["detail"], "[redacted]");
        assert!(!value_contains_sensitive_key(&value));
        assert!(!value_contains_sensitive_string(&value));
    }
}
