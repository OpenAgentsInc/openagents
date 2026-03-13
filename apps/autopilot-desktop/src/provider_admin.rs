use std::path::PathBuf;
use std::time::Instant;

use openagents_provider_substrate::{
    ProviderAdminConfig, ProviderAdminRuntime, ProviderAdminUpdate, ProviderControlEvent,
    ProviderDesiredMode, ProviderEarningsSummary, ProviderHealthEvent, ProviderIdentityMetadata,
    ProviderJsonEntry, ProviderPayoutSummary, ProviderPersistedSnapshot, ProviderReceiptSummary,
    ProviderRecentJob, ProviderRuntimeStatusSnapshot, ProviderSnapshotParts,
    assemble_provider_persisted_snapshot, describe_provider_product_id,
};
use serde_json::json;

use crate::app_state::{JobHistoryReceiptRow, RenderState};
use crate::economy_kernel_receipts::{Asset, Money, MoneyAmount, Receipt};

const PROVIDER_ADMIN_SYNC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const PROVIDER_ADMIN_ROW_LIMIT: usize = 32;

pub type DesktopProviderAdminRuntime = ProviderAdminRuntime;

pub fn spawn_runtime() -> Result<DesktopProviderAdminRuntime, String> {
    ProviderAdminRuntime::spawn(ProviderAdminConfig::new(
        provider_admin_db_path(),
        provider_admin_listen_addr()?,
    ))
}

pub fn set_desired_mode(state: &mut RenderState, desired_mode: ProviderDesiredMode) {
    let Some(runtime) = state.provider_admin_runtime.as_ref() else {
        return;
    };
    if let Err(error) = runtime.set_desired_mode(desired_mode) {
        state.provider_admin_last_error = Some(error);
    }
}

pub fn pump_runtime(state: &mut RenderState) -> bool {
    let mut changed = false;
    if state.provider_runtime.refresh_sandbox_supply_if_due() {
        changed = true;
    }
    if drain_runtime_updates(state) {
        changed = true;
    }
    if sync_runtime_snapshot(state) {
        changed = true;
    }
    changed
}

fn drain_runtime_updates(state: &mut RenderState) -> bool {
    let updates = match state.provider_admin_runtime.as_mut() {
        Some(runtime) => runtime.drain_updates(),
        None => return false,
    };

    let mut changed = false;
    for update in updates {
        match update {
            ProviderAdminUpdate::ControlEvent(event) => {
                apply_control_event(state, event);
                changed = true;
            }
            ProviderAdminUpdate::WorkerError(error) => {
                state.provider_admin_last_error = Some(error);
                changed = true;
            }
        }
    }
    changed
}

fn sync_runtime_snapshot(state: &mut RenderState) -> bool {
    let Some(runtime) = state.provider_admin_runtime.as_ref() else {
        return false;
    };
    let snapshot = snapshot_for_state(state);
    let signature = match snapshot_signature(&snapshot) {
        Ok(signature) => signature,
        Err(error) => {
            state.provider_admin_last_error = Some(error);
            return false;
        }
    };
    let should_sync = state.provider_admin_last_sync_signature.as_deref()
        != Some(signature.as_str())
        || state
            .provider_admin_last_sync_at
            .is_none_or(|last_sync_at| last_sync_at.elapsed() >= PROVIDER_ADMIN_SYNC_INTERVAL);
    if !should_sync {
        return false;
    }
    if let Err(error) = runtime.sync_snapshot(snapshot) {
        state.provider_admin_last_error = Some(error);
        return false;
    }
    state.provider_admin_last_error = None;
    state.provider_admin_last_sync_signature = Some(signature);
    state.provider_admin_last_sync_at = Some(Instant::now());
    true
}

fn apply_control_event(state: &mut RenderState, event: ProviderControlEvent) {
    match event.desired_mode {
        ProviderDesiredMode::Online => crate::input::apply_provider_mode_target(
            state,
            true,
            ProviderDesiredMode::Online,
            "provider admin api requested online",
        ),
        ProviderDesiredMode::Offline => crate::input::apply_provider_mode_target(
            state,
            false,
            ProviderDesiredMode::Offline,
            "provider admin api requested offline",
        ),
        ProviderDesiredMode::Paused => crate::input::apply_provider_mode_target(
            state,
            false,
            ProviderDesiredMode::Paused,
            "provider admin api requested pause",
        ),
    };
}

fn snapshot_for_state(state: &RenderState) -> ProviderPersistedSnapshot {
    let captured_at_ms = now_epoch_ms();
    assemble_provider_persisted_snapshot(ProviderSnapshotParts {
        captured_at_ms,
        config_metadata: config_metadata_for_state(state),
        identity: identity_metadata_for_state(state),
        runtime: runtime_status_for_state(state),
        availability: state.provider_runtime.availability(),
        inventory_rows: state.provider_runtime.inventory_rows.clone(),
        recent_jobs: recent_jobs_for_state(state),
        receipts: receipt_summaries_for_state(state),
        payouts: payout_summaries_for_state(state),
        health_events: health_events_for_state(state, captured_at_ms),
        earnings: Some(earnings_summary_for_state(state)),
    })
}

fn config_metadata_for_state(state: &RenderState) -> Vec<ProviderJsonEntry> {
    let mut entries = vec![
        ProviderJsonEntry {
            key: "relay_urls".to_string(),
            value: json!(state.configured_provider_relay_urls()),
        },
        ProviderJsonEntry {
            key: "execution_lane".to_string(),
            value: json!(state.provider_runtime.execution_lane_label()),
        },
        ProviderJsonEntry {
            key: "settlement_truth".to_string(),
            value: json!(state.provider_runtime.settlement_truth_label()),
        },
        ProviderJsonEntry {
            key: "control_authority".to_string(),
            value: json!(state.provider_runtime.control_authority_label(false)),
        },
    ];
    if let Some(base_url) = state.hosted_control_base_url.as_deref() {
        entries.push(ProviderJsonEntry {
            key: "hosted_control_base_url".to_string(),
            value: json!(base_url),
        });
    }
    if let Some(listen_addr) = state.provider_admin_listen_addr.as_deref() {
        entries.push(ProviderJsonEntry {
            key: "provider_admin_listen_addr".to_string(),
            value: json!(listen_addr),
        });
    }
    entries
}

fn identity_metadata_for_state(state: &RenderState) -> Option<ProviderIdentityMetadata> {
    state
        .nostr_identity
        .as_ref()
        .map(|identity| ProviderIdentityMetadata {
            npub: Some(identity.npub.clone()),
            public_key_hex: Some(identity.public_key_hex.clone()),
            display_name: Some("Autopilot Desktop".to_string()),
            node_label: Some("autopilot-desktop".to_string()),
        })
}

fn runtime_status_for_state(state: &RenderState) -> ProviderRuntimeStatusSnapshot {
    ProviderRuntimeStatusSnapshot {
        mode: state.provider_runtime.mode,
        last_action: state
            .provider_runtime
            .last_result
            .clone()
            .or_else(|| state.provider_runtime.inventory_last_action.clone()),
        last_error: state
            .provider_runtime
            .last_error_detail
            .clone()
            .or_else(|| state.provider_admin_last_error.clone()),
        degraded_reason_code: state.provider_runtime.degraded_reason_code.clone(),
        authoritative_status: state.provider_runtime.last_authoritative_status.clone(),
        authoritative_error_class: state.provider_runtime.last_authoritative_error_class,
        queue_depth: state.provider_runtime.queue_depth,
        online_uptime_seconds: state.provider_runtime.uptime_seconds(Instant::now()),
        inventory_session_started_at_ms: state.provider_runtime.inventory_session_started_at_ms,
        last_completed_job_at_epoch_ms: state
            .provider_runtime
            .last_completed_job_at
            .and_then(approx_epoch_ms_for_instant),
        last_authoritative_event_id: state.provider_runtime.last_authoritative_event_id.clone(),
        execution_backend_label: state.provider_runtime.execution_backend_label().to_string(),
        provider_blocker_codes: state
            .provider_blockers()
            .into_iter()
            .map(|blocker| blocker.code().to_string())
            .collect(),
    }
}

fn recent_jobs_for_state(state: &RenderState) -> Vec<ProviderRecentJob> {
    state
        .job_history
        .rows
        .iter()
        .take(PROVIDER_ADMIN_ROW_LIMIT)
        .map(recent_job_from_row)
        .collect()
}

fn recent_job_from_row(row: &JobHistoryReceiptRow) -> ProviderRecentJob {
    let product_id = infer_product_id_for_history_row(row);
    let descriptor = product_id.as_deref().and_then(describe_provider_product_id);
    ProviderRecentJob {
        job_id: row.job_id.clone(),
        request_id: Some(infer_request_id_from_job_id(row.job_id.as_str())),
        status: row.status.label().to_string(),
        demand_source: row.demand_source.label().to_string(),
        product_id,
        compute_family: descriptor
            .as_ref()
            .map(|descriptor| descriptor.compute_family.clone()),
        backend_family: descriptor
            .as_ref()
            .map(|descriptor| descriptor.backend_family.clone()),
        sandbox_execution_class: descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.sandbox_execution_class.clone()),
        sandbox_profile_id: None,
        sandbox_profile_digest: None,
        sandbox_termination_reason: None,
        completed_at_epoch_seconds: row.completed_at_epoch_seconds,
        payout_sats: row.payout_sats,
        payment_pointer: row.payment_pointer.clone(),
        failure_reason: row.failure_reason.clone(),
        delivery_proof_id: row.delivery_proof_id.clone(),
    }
}

fn infer_product_id_for_history_row(row: &JobHistoryReceiptRow) -> Option<String> {
    if row
        .delivery_metering_rule_id
        .as_deref()
        .is_some_and(|rule_id| rule_id == "meter.gpt_oss.embeddings.v1")
    {
        return Some("gpt_oss.embeddings".to_string());
    }
    match row
        .execution_provenance
        .as_ref()
        .map(|provenance| provenance.backend.as_str())
    {
        Some("apple_foundation_models") => {
            Some("apple_foundation_models.text_generation".to_string())
        }
        Some("gpt_oss") | Some("psionic") | Some("ollama") => {
            Some("gpt_oss.text_generation".to_string())
        }
        _ => None,
    }
}

fn receipt_summaries_for_state(state: &RenderState) -> Vec<ProviderReceiptSummary> {
    state
        .earn_kernel_receipts
        .receipts
        .iter()
        .take(PROVIDER_ADMIN_ROW_LIMIT)
        .map(receipt_summary)
        .collect()
}

fn receipt_summary(receipt: &Receipt) -> ProviderReceiptSummary {
    ProviderReceiptSummary {
        receipt_id: receipt.receipt_id.clone(),
        receipt_type: receipt.receipt_type.clone(),
        created_at_ms: receipt.created_at_ms,
        canonical_hash: receipt.canonical_hash.clone(),
        compute_family: None,
        backend_family: None,
        sandbox_execution_class: None,
        sandbox_profile_id: None,
        sandbox_profile_digest: None,
        sandbox_termination_reason: None,
        reason_code: receipt.hints.reason_code.clone(),
        failure_reason: None,
        severity: receipt
            .hints
            .severity
            .map(|severity| severity.label().to_string()),
        notional_sats: receipt.hints.notional.as_ref().and_then(money_sats),
        liability_premium_sats: receipt
            .hints
            .liability_premium
            .as_ref()
            .and_then(money_sats),
        work_unit_id: receipt.trace.work_unit_id.clone(),
    }
}

fn payout_summaries_for_state(state: &RenderState) -> Vec<ProviderPayoutSummary> {
    state
        .job_history
        .wallet_reconciled_payout_rows(&state.spark_wallet)
        .into_iter()
        .take(PROVIDER_ADMIN_ROW_LIMIT)
        .map(|row| ProviderPayoutSummary {
            payout_id: row.payment_pointer.clone(),
            amount_sats: row.payout_sats,
            direction: "receive".to_string(),
            status: "settled".to_string(),
            created_at_epoch_seconds: row.wallet_received_at_epoch_seconds,
            payment_pointer: Some(row.payment_pointer),
        })
        .collect()
}

fn earnings_summary_for_state(state: &RenderState) -> ProviderEarningsSummary {
    ProviderEarningsSummary {
        sats_today: state.earnings_scoreboard.sats_today,
        lifetime_sats: state.earnings_scoreboard.lifetime_sats,
        jobs_today: state.earnings_scoreboard.jobs_today,
        online_uptime_seconds: state.earnings_scoreboard.online_uptime_seconds,
        last_job_result: state.earnings_scoreboard.last_job_result.clone(),
        first_job_latency_seconds: state.earnings_scoreboard.first_job_latency_seconds,
        completion_ratio_bps: state.earnings_scoreboard.completion_ratio_bps,
        payout_success_ratio_bps: state.earnings_scoreboard.payout_success_ratio_bps,
        avg_wallet_confirmation_latency_seconds: state
            .earnings_scoreboard
            .avg_wallet_confirmation_latency_seconds,
    }
}

fn health_events_for_state(state: &RenderState, captured_at_ms: i64) -> Vec<ProviderHealthEvent> {
    let mut events = Vec::new();

    if let Some(error) = state.provider_admin_last_error.as_deref() {
        events.push(ProviderHealthEvent {
            event_id: "provider_admin_runtime_error".to_string(),
            occurred_at_ms: captured_at_ms,
            severity: "error".to_string(),
            code: "PROVIDER_ADMIN_RUNTIME_ERROR".to_string(),
            detail: error.to_string(),
            source: "provider_admin".to_string(),
        });
    }

    for blocker in state.provider_blockers() {
        events.push(ProviderHealthEvent {
            event_id: format!("provider_blocker:{}", blocker.code()),
            occurred_at_ms: captured_at_ms,
            severity: "warn".to_string(),
            code: blocker.code().to_string(),
            detail: blocker.detail().to_string(),
            source: "provider_runtime".to_string(),
        });
    }

    if let Some(error) = state.provider_runtime.last_error_detail.as_deref() {
        events.push(ProviderHealthEvent {
            event_id: "provider_runtime_last_error".to_string(),
            occurred_at_ms: captured_at_ms,
            severity: "error".to_string(),
            code: state
                .provider_runtime
                .degraded_reason_code
                .clone()
                .unwrap_or_else(|| "PROVIDER_RUNTIME_ERROR".to_string()),
            detail: error.to_string(),
            source: "provider_runtime".to_string(),
        });
    }

    if let Some(error) = state.provider_runtime.gpt_oss.last_error.as_deref() {
        events.push(ProviderHealthEvent {
            event_id: "local_inference_runtime_error".to_string(),
            occurred_at_ms: captured_at_ms,
            severity: "warn".to_string(),
            code: "LOCAL_INFERENCE_RUNTIME_ERROR".to_string(),
            detail: error.to_string(),
            source: "gpt_oss".to_string(),
        });
    }

    if let Some(error) = state.provider_runtime.apple_fm.last_error.as_deref() {
        events.push(ProviderHealthEvent {
            event_id: "apple_fm_runtime_error".to_string(),
            occurred_at_ms: captured_at_ms,
            severity: "warn".to_string(),
            code: "APPLE_FM_RUNTIME_ERROR".to_string(),
            detail: error.to_string(),
            source: "apple_foundation_models".to_string(),
        });
    }

    for runtime in &state.provider_runtime.sandbox.runtimes {
        if let Some(error) = runtime.last_error.as_deref() {
            events.push(ProviderHealthEvent {
                event_id: format!("sandbox_runtime_error:{:?}", runtime.runtime_kind),
                occurred_at_ms: captured_at_ms,
                severity: "warn".to_string(),
                code: "SANDBOX_RUNTIME_ERROR".to_string(),
                detail: error.to_string(),
                source: "sandbox_runtime".to_string(),
            });
        }
    }

    if let Some(error) = state.spark_wallet.last_error.as_deref() {
        events.push(ProviderHealthEvent {
            event_id: "spark_wallet_error".to_string(),
            occurred_at_ms: captured_at_ms,
            severity: "warn".to_string(),
            code: "SPARK_WALLET_ERROR".to_string(),
            detail: error.to_string(),
            source: "spark_wallet".to_string(),
        });
    }

    events
}

fn money_sats(money: &Money) -> Option<u64> {
    if money.asset != Asset::Btc {
        return None;
    }
    match money.amount {
        MoneyAmount::AmountMsats(msats) => Some(msats / 1_000),
        MoneyAmount::AmountSats(sats) => Some(sats),
    }
}

fn snapshot_signature(snapshot: &ProviderPersistedSnapshot) -> Result<String, String> {
    let mut stable_snapshot = snapshot.clone();
    stable_snapshot.captured_at_ms = 0;
    for event in &mut stable_snapshot.health_events {
        event.occurred_at_ms = 0;
    }
    serde_json::to_string(&stable_snapshot)
        .map_err(|error| format!("Failed to encode provider admin sync signature: {error}"))
}

fn provider_admin_db_path() -> PathBuf {
    if let Ok(path) = std::env::var("OPENAGENTS_PROVIDER_ADMIN_DB_PATH") {
        return PathBuf::from(path);
    }
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("provider-admin-v1.sqlite")
}

fn provider_admin_listen_addr() -> Result<std::net::SocketAddr, String> {
    let raw = std::env::var("OPENAGENTS_PROVIDER_ADMIN_LISTEN_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:0".to_string());
    raw.parse::<std::net::SocketAddr>()
        .map_err(|error| format!("Invalid provider admin listen addr '{raw}': {error}"))
}

fn now_epoch_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

fn approx_epoch_ms_for_instant(target: Instant) -> Option<i64> {
    let now = Instant::now();
    let age = now.checked_duration_since(target)?;
    let system_now = std::time::SystemTime::now();
    let observed = system_now.checked_sub(age)?;
    observed
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

fn infer_request_id_from_job_id(job_id: &str) -> String {
    job_id
        .strip_prefix("job-")
        .map(ToString::to_string)
        .unwrap_or_else(|| job_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::{infer_product_id_for_history_row, snapshot_signature};
    use crate::app_state::{JobDemandSource, JobHistoryReceiptRow, JobHistoryStatus};
    use crate::local_inference_runtime::LocalInferenceExecutionProvenance;
    use openagents_provider_substrate::{
        ProviderAvailability, ProviderPersistedSnapshot, ProviderRuntimeStatusSnapshot,
        ProviderSandboxAvailability, ProviderSandboxExecutionClass, ProviderSandboxProfile,
        ProviderSandboxRuntimeHealth, ProviderSandboxRuntimeKind,
    };

    #[test]
    fn product_id_inference_prefers_embedding_metering_rule() {
        let row = JobHistoryReceiptRow {
            job_id: "job-req-1".to_string(),
            status: JobHistoryStatus::Succeeded,
            demand_source: JobDemandSource::OpenNetwork,
            completed_at_epoch_seconds: 1,
            requester_nostr_pubkey: Some("npub1buyer".to_string()),
            provider_nostr_pubkey: Some("npub1provider".to_string()),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            delivery_proof_id: None,
            delivery_metering_rule_id: Some("meter.gpt_oss.embeddings.v1".to_string()),
            delivery_proof_status_label: None,
            delivery_metered_quantity: None,
            delivery_accepted_quantity: None,
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            payout_sats: 0,
            result_hash: "sha256:test".to_string(),
            payment_pointer: "pending:req-1".to_string(),
            failure_reason: None,
            execution_provenance: Some(LocalInferenceExecutionProvenance {
                backend: "gpt_oss".to_string(),
                requested_model: Some("nomic-embed-text".to_string()),
                served_model: "nomic-embed-text".to_string(),
                normalized_prompt_digest: "sha256:prompt".to_string(),
                normalized_options_json: "{}".to_string(),
                normalized_options_digest: "sha256:opts".to_string(),
                base_url: "http://127.0.0.1:11434".to_string(),
                total_duration_ns: None,
                load_duration_ns: None,
                prompt_token_count: None,
                generated_token_count: None,
                warm_start: Some(true),
            }),
        };

        assert_eq!(
            infer_product_id_for_history_row(&row).as_deref(),
            Some("gpt_oss.embeddings")
        );
    }

    #[test]
    fn snapshot_signature_ignores_capture_timestamps() {
        let first = ProviderPersistedSnapshot {
            captured_at_ms: 100,
            config_metadata: Vec::new(),
            identity: None,
            runtime: ProviderRuntimeStatusSnapshot::default(),
            availability: ProviderAvailability::default(),
            inventory_rows: Vec::new(),
            recent_jobs: Vec::new(),
            receipts: Vec::new(),
            payouts: Vec::new(),
            health_events: Vec::new(),
            earnings: None,
        };
        let second = ProviderPersistedSnapshot {
            captured_at_ms: 200,
            ..first.clone()
        };

        assert_eq!(
            snapshot_signature(&first).ok(),
            snapshot_signature(&second).ok()
        );
    }

    #[test]
    fn snapshot_signature_changes_when_sandbox_truth_changes() {
        let mut first = ProviderPersistedSnapshot {
            captured_at_ms: 100,
            config_metadata: Vec::new(),
            identity: None,
            runtime: ProviderRuntimeStatusSnapshot::default(),
            availability: ProviderAvailability {
                sandbox: ProviderSandboxAvailability {
                    runtimes: vec![ProviderSandboxRuntimeHealth {
                        runtime_kind: ProviderSandboxRuntimeKind::Python,
                        detected: true,
                        ready: true,
                        binary_name: Some("python3".to_string()),
                        binary_path: Some("/usr/bin/python3".to_string()),
                        runtime_version: Some("Python 3.11.8".to_string()),
                        supported_execution_classes: vec![
                            ProviderSandboxExecutionClass::PythonExec,
                        ],
                        last_error: None,
                    }],
                    profiles: vec![ProviderSandboxProfile {
                        profile_id: "python-batch".to_string(),
                        profile_digest: "sha256:profile-a".to_string(),
                        execution_class: ProviderSandboxExecutionClass::PythonExec,
                        runtime_family: "python3".to_string(),
                        runtime_version: "Python 3.11.8".to_string(),
                        sandbox_engine: "local_subprocess".to_string(),
                        os_family: "linux".to_string(),
                        arch: "x86_64".to_string(),
                        cpu_limit: 2,
                        memory_limit_mb: 2048,
                        disk_limit_mb: 4096,
                        timeout_limit_s: 120,
                        network_mode: "none".to_string(),
                        filesystem_mode: "workspace_only".to_string(),
                        workspace_mode: "ephemeral".to_string(),
                        artifact_output_mode: "declared_paths_only".to_string(),
                        secrets_mode: "none".to_string(),
                        allowed_binaries: vec!["python3".to_string()],
                        toolchain_inventory: vec!["python3".to_string()],
                        container_image: None,
                        runtime_image_digest: None,
                        accelerator_policy: None,
                        runtime_kind: ProviderSandboxRuntimeKind::Python,
                        runtime_ready: true,
                        runtime_binary_path: Some("/usr/bin/python3".to_string()),
                        capability_summary: "backend=sandbox execution=sandbox.python.exec family=sandbox_execution profile_id=python-batch".to_string(),
                    }],
                    last_scan_error: None,
                },
                ..ProviderAvailability::default()
            },
            inventory_rows: Vec::new(),
            recent_jobs: Vec::new(),
            receipts: Vec::new(),
            payouts: Vec::new(),
            health_events: Vec::new(),
            earnings: None,
        };
        let mut second = first.clone();
        second.availability.sandbox.profiles[0].profile_digest = "sha256:profile-b".to_string();
        first.captured_at_ms = 100;
        second.captured_at_ms = 200;

        assert_ne!(
            snapshot_signature(&first).ok(),
            snapshot_signature(&second).ok()
        );
    }
}
