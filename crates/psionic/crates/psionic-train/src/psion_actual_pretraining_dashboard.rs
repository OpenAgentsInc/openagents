use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionActualPretrainingCheckpointBackupReceipt, PsionActualPretrainingCheckpointEvalDecision,
    PsionActualPretrainingCheckpointEvalFailure, PsionActualPretrainingCheckpointPointer,
    PsionActualPretrainingCurrentRunStatus, PsionActualPretrainingHardwareQualification,
    PsionActualPretrainingRedactedAlert, PsionActualPretrainingRetainedSummary,
    PsionActualPretrainingRunShapeQualification, PsionActualPretrainingSystemsBundle,
    PSION_ACTUAL_PRETRAINING_LANE_ID,
};

/// Stable schema version for the retained actual-lane dashboard packet.
pub const PSION_ACTUAL_PRETRAINING_DASHBOARD_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_dashboard.v1";

/// Stable schema version for the retained actual-lane active-alert feed.
pub const PSION_ACTUAL_PRETRAINING_ALERT_FEED_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_alert_feed.v1";

/// Canonical fixture path for the retained actual-lane dashboard packet.
pub const PSION_ACTUAL_PRETRAINING_DASHBOARD_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_dashboard_v1.json";

/// Canonical fixture path for the retained actual-lane active-alert feed.
pub const PSION_ACTUAL_PRETRAINING_ALERT_FEED_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_alert_feed_v1.json";

/// Canonical focused doc path for the retained actual-lane dashboard and alerts.
pub const PSION_ACTUAL_PRETRAINING_DASHBOARD_DOC_PATH: &str =
    "docs/PSION_ACTUAL_PRETRAINING_DASHBOARD_AND_ALERTS.md";

/// Canonical retained dashboard path under the run root.
pub const PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH: &str =
    "dashboard/current_dashboard.json";

/// Canonical retained active-alert feed path under the run root.
pub const PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH: &str = "alerts/active_alerts.json";

const DASHBOARD_RUNBOOK_PATH: &str = "docs/PSION_ACTUAL_PRETRAINING_DASHBOARD_AND_ALERTS.md";
const HARDWARE_QUALIFICATION_PATH: &str = "preflight/hardware_qualification.json";
const RUN_SHAPE_QUALIFICATION_PATH: &str = "preflight/run_shape_qualification.json";
const BACKUP_RECEIPT_PATH: &str = "checkpoints/latest_accepted_checkpoint_backup_receipt.json";
const CHECKPOINT_POINTER_PATH: &str = "checkpoints/latest_accepted_checkpoint_pointer.json";
const CONTINUATION_HANDOFF_PATH: &str = "continuation/accepted_checkpoint_handoff.json";
const CHECKPOINT_EVAL_DECISION_PATH: &str = "evals/latest_checkpoint_eval_decision.json";
const CHECKPOINT_EVAL_FAILURE_PATH: &str = "evals/latest_checkpoint_eval_failure.json";
const LATEST_REDACTED_ALERT_PATH: &str = "alerts/latest_redacted_alert.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDashboardThroughputCard {
    pub baseline_tokens_per_second: u64,
    pub observed_tokens_per_second: Option<u64>,
    pub min_healthy_tokens_per_second: u64,
    pub observed_step_latency_ms: Option<u64>,
    pub max_healthy_step_latency_ms: u64,
    pub observed_checkpoint_write_throughput_bytes_per_second: Option<u64>,
    pub min_checkpoint_write_throughput_bytes_per_second: u64,
    pub observed_batches_per_second: Option<u64>,
    pub observed_stall_count: u64,
    pub degradation_state: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDashboardLossCard {
    pub visibility_state: String,
    pub latest_mean_loss_milli: Option<u64>,
    pub source_relative_path: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDashboardGradientCard {
    pub visibility_state: String,
    pub collective_kind: String,
    pub runtime_backend: String,
    pub transport: String,
    pub collective_benchmark_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDashboardCheckpointCard {
    pub checkpoint_label: String,
    pub optimizer_step: u64,
    pub checkpoint_ref: String,
    pub checkpoint_pointer_path: String,
    pub checkpoint_backup_state: String,
    pub checkpoint_eval_state: String,
    pub checkpoint_eval_receipt_path: Option<String>,
    pub continuation_handoff_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDashboardHardwareCard {
    pub required_backend: String,
    pub required_worker_count: u64,
    pub observed_worker_count: u64,
    pub required_min_free_memory_bytes_per_worker: u64,
    pub min_observed_free_memory_bytes: Option<u64>,
    pub max_observed_temperature_celsius: Option<u64>,
    pub max_observed_ecc_uncorrected_error_count: Option<u64>,
    pub any_throttling_observed: bool,
    pub checkpoint_restore_ready: bool,
    pub health_state: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDashboardAlertSummary {
    pub active_alert_count: u64,
    pub highest_severity: String,
    pub alert_feed_path: String,
    pub response_runbook_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingActiveAlert {
    pub alert_id: String,
    pub alert_kind: String,
    pub severity: String,
    pub state: String,
    pub response_posture: String,
    pub source_relative_path: String,
    pub retained_redaction: String,
    pub summary: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingAlertFeed {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub alert_feed_path: String,
    pub active_alerts: Vec<PsionActualPretrainingActiveAlert>,
    pub summary: String,
    pub alert_feed_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingDashboardPacket {
    pub schema_version: String,
    pub dashboard_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub current_phase: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub dashboard_path: String,
    pub alert_feed_path: String,
    pub throughput: PsionActualPretrainingDashboardThroughputCard,
    pub loss: PsionActualPretrainingDashboardLossCard,
    pub gradient: PsionActualPretrainingDashboardGradientCard,
    pub checkpoint: PsionActualPretrainingDashboardCheckpointCard,
    pub hardware: PsionActualPretrainingDashboardHardwareCard,
    pub alerts: PsionActualPretrainingDashboardAlertSummary,
    pub support_refs: Vec<String>,
    pub claim_boundary: String,
    pub detail: String,
    pub dashboard_digest: String,
}

#[derive(Debug, Error)]
pub enum PsionActualPretrainingDashboardError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl PsionActualPretrainingDashboardPacket {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        ensure_exact(
            self.schema_version.as_str(),
            "dashboard.schema_version",
            PSION_ACTUAL_PRETRAINING_DASHBOARD_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "dashboard.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        for (field, value) in [
            ("dashboard.dashboard_id", self.dashboard_id.as_str()),
            ("dashboard.run_id", self.run_id.as_str()),
            ("dashboard.current_phase", self.current_phase.as_str()),
            ("dashboard.selected_git_ref", self.selected_git_ref.as_str()),
            ("dashboard.git_commit_sha", self.git_commit_sha.as_str()),
            (
                "dashboard.dirty_tree_admission",
                self.dirty_tree_admission.as_str(),
            ),
            ("dashboard.claim_boundary", self.claim_boundary.as_str()),
            ("dashboard.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        ensure_exact(
            self.dashboard_path.as_str(),
            "dashboard.dashboard_path",
            PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH,
        )?;
        ensure_exact(
            self.alert_feed_path.as_str(),
            "dashboard.alert_feed_path",
            PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH,
        )?;
        self.throughput.validate()?;
        self.loss.validate()?;
        self.gradient.validate()?;
        self.checkpoint.validate()?;
        self.hardware.validate()?;
        self.alerts.validate()?;
        if self.support_refs.is_empty() {
            return Err(PsionActualPretrainingDashboardError::MissingField {
                field: String::from("dashboard.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(support_ref.as_str(), "dashboard.support_refs[]")?;
        }
        if self.dashboard_digest != stable_dashboard_digest(self)? {
            return Err(PsionActualPretrainingDashboardError::DigestMismatch {
                field: String::from("dashboard.dashboard_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingAlertFeed {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        ensure_exact(
            self.schema_version.as_str(),
            "alert_feed.schema_version",
            PSION_ACTUAL_PRETRAINING_ALERT_FEED_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "alert_feed.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(self.run_id.as_str(), "alert_feed.run_id")?;
        ensure_exact(
            self.alert_feed_path.as_str(),
            "alert_feed.alert_feed_path",
            PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH,
        )?;
        for alert in &self.active_alerts {
            alert.validate()?;
        }
        ensure_nonempty(self.summary.as_str(), "alert_feed.summary")?;
        if self.alert_feed_digest != stable_alert_feed_digest(self)? {
            return Err(PsionActualPretrainingDashboardError::DigestMismatch {
                field: String::from("alert_feed.alert_feed_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingDashboardThroughputCard {
    fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        if self.baseline_tokens_per_second == 0 {
            return Err(PsionActualPretrainingDashboardError::InvalidValue {
                field: String::from("dashboard.throughput.baseline_tokens_per_second"),
                detail: String::from("baseline tokens per second must stay positive"),
            });
        }
        if self.min_healthy_tokens_per_second == 0
            || self.max_healthy_step_latency_ms == 0
            || self.min_checkpoint_write_throughput_bytes_per_second == 0
        {
            return Err(PsionActualPretrainingDashboardError::InvalidValue {
                field: String::from("dashboard.throughput.thresholds"),
                detail: String::from("throughput thresholds must stay positive"),
            });
        }
        match self.degradation_state.as_str() {
            "healthy" | "degraded" | "not_measured" => {}
            _ => {
                return Err(PsionActualPretrainingDashboardError::InvalidValue {
                    field: String::from("dashboard.throughput.degradation_state"),
                    detail: String::from(
                        "throughput degradation state must be healthy, degraded, or not_measured",
                    ),
                });
            }
        }
        ensure_nonempty(self.detail.as_str(), "dashboard.throughput.detail")
    }
}

impl PsionActualPretrainingDashboardLossCard {
    fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        match self.visibility_state.as_str() {
            "not_started" | "not_emitted_by_actual_lane_operator" | "retained_metric_present" => {}
            _ => {
                return Err(PsionActualPretrainingDashboardError::InvalidValue {
                    field: String::from("dashboard.loss.visibility_state"),
                    detail: String::from("unsupported loss visibility state"),
                });
            }
        }
        if let Some(value) = self.latest_mean_loss_milli {
            if value == 0 {
                return Err(PsionActualPretrainingDashboardError::InvalidValue {
                    field: String::from("dashboard.loss.latest_mean_loss_milli"),
                    detail: String::from("loss must stay positive when present"),
                });
            }
        }
        ensure_nonempty(self.detail.as_str(), "dashboard.loss.detail")
    }
}

impl PsionActualPretrainingDashboardGradientCard {
    fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        match self.visibility_state.as_str() {
            "qualified_reference_only" | "retained_live_signal" => {}
            _ => {
                return Err(PsionActualPretrainingDashboardError::InvalidValue {
                    field: String::from("dashboard.gradient.visibility_state"),
                    detail: String::from("unsupported gradient visibility state"),
                });
            }
        }
        for (field, value) in [
            (
                "dashboard.gradient.collective_kind",
                self.collective_kind.as_str(),
            ),
            (
                "dashboard.gradient.runtime_backend",
                self.runtime_backend.as_str(),
            ),
            ("dashboard.gradient.transport", self.transport.as_str()),
            (
                "dashboard.gradient.collective_benchmark_digest",
                self.collective_benchmark_digest.as_str(),
            ),
            ("dashboard.gradient.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        Ok(())
    }
}

impl PsionActualPretrainingDashboardCheckpointCard {
    fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        for (field, value) in [
            (
                "dashboard.checkpoint.checkpoint_label",
                self.checkpoint_label.as_str(),
            ),
            (
                "dashboard.checkpoint.checkpoint_ref",
                self.checkpoint_ref.as_str(),
            ),
            (
                "dashboard.checkpoint.checkpoint_pointer_path",
                self.checkpoint_pointer_path.as_str(),
            ),
            (
                "dashboard.checkpoint.checkpoint_backup_state",
                self.checkpoint_backup_state.as_str(),
            ),
            (
                "dashboard.checkpoint.checkpoint_eval_state",
                self.checkpoint_eval_state.as_str(),
            ),
            (
                "dashboard.checkpoint.continuation_handoff_path",
                self.continuation_handoff_path.as_str(),
            ),
            ("dashboard.checkpoint.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        Ok(())
    }
}

impl PsionActualPretrainingDashboardHardwareCard {
    fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        ensure_nonempty(
            self.required_backend.as_str(),
            "dashboard.hardware.required_backend",
        )?;
        if self.required_worker_count == 0 || self.required_min_free_memory_bytes_per_worker == 0 {
            return Err(PsionActualPretrainingDashboardError::InvalidValue {
                field: String::from("dashboard.hardware.worker_counts"),
                detail: String::from("hardware counts and memory thresholds must stay positive"),
            });
        }
        match self.health_state.as_str() {
            "healthy" | "degraded" | "refused" => {}
            _ => {
                return Err(PsionActualPretrainingDashboardError::InvalidValue {
                    field: String::from("dashboard.hardware.health_state"),
                    detail: String::from("unsupported hardware health state"),
                });
            }
        }
        if self.health_state != "refused" && self.observed_worker_count == 0 {
            return Err(PsionActualPretrainingDashboardError::InvalidValue {
                field: String::from("dashboard.hardware.worker_counts"),
                detail: String::from(
                    "non-refused hardware cards must retain at least one observed worker",
                ),
            });
        }
        ensure_nonempty(self.detail.as_str(), "dashboard.hardware.detail")
    }
}

impl PsionActualPretrainingDashboardAlertSummary {
    fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        match self.highest_severity.as_str() {
            "none" | "info" | "warning" | "critical" => {}
            _ => {
                return Err(PsionActualPretrainingDashboardError::InvalidValue {
                    field: String::from("dashboard.alerts.highest_severity"),
                    detail: String::from("unsupported alert severity"),
                });
            }
        }
        ensure_exact(
            self.alert_feed_path.as_str(),
            "dashboard.alerts.alert_feed_path",
            PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH,
        )?;
        ensure_exact(
            self.response_runbook_path.as_str(),
            "dashboard.alerts.response_runbook_path",
            DASHBOARD_RUNBOOK_PATH,
        )?;
        ensure_nonempty(self.detail.as_str(), "dashboard.alerts.detail")
    }
}

impl PsionActualPretrainingActiveAlert {
    fn validate(&self) -> Result<(), PsionActualPretrainingDashboardError> {
        for (field, value) in [
            ("alert.alert_id", self.alert_id.as_str()),
            ("alert.alert_kind", self.alert_kind.as_str()),
            ("alert.severity", self.severity.as_str()),
            ("alert.state", self.state.as_str()),
            ("alert.response_posture", self.response_posture.as_str()),
            (
                "alert.source_relative_path",
                self.source_relative_path.as_str(),
            ),
            ("alert.retained_redaction", self.retained_redaction.as_str()),
            ("alert.summary", self.summary.as_str()),
            ("alert.detail", self.detail.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        match self.severity.as_str() {
            "info" | "warning" | "critical" => {}
            _ => {
                return Err(PsionActualPretrainingDashboardError::InvalidValue {
                    field: String::from("alert.severity"),
                    detail: String::from("unsupported alert severity"),
                });
            }
        }
        if self.state != "active" {
            return Err(PsionActualPretrainingDashboardError::InvalidValue {
                field: String::from("alert.state"),
                detail: String::from("retained dashboard alerts must stay active"),
            });
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_psion_actual_pretraining_dashboard_packet(
    current_status: &PsionActualPretrainingCurrentRunStatus,
    retained_summary: &PsionActualPretrainingRetainedSummary,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    hardware_qualification: &PsionActualPretrainingHardwareQualification,
    run_shape_qualification: &PsionActualPretrainingRunShapeQualification,
    systems_bundle: &PsionActualPretrainingSystemsBundle,
    checkpoint_backup_receipt: Option<&PsionActualPretrainingCheckpointBackupReceipt>,
    checkpoint_eval_decision: Option<&PsionActualPretrainingCheckpointEvalDecision>,
    checkpoint_eval_failure: Option<&PsionActualPretrainingCheckpointEvalFailure>,
    latest_redacted_alert: Option<&PsionActualPretrainingRedactedAlert>,
) -> Result<
    (
        PsionActualPretrainingDashboardPacket,
        PsionActualPretrainingAlertFeed,
    ),
    PsionActualPretrainingDashboardError,
> {
    let throughput_baseline = systems_bundle
        .throughput_baselines
        .iter()
        .find(|baseline| baseline.baseline_kind == "trusted_cluster_anchor")
        .ok_or_else(|| PsionActualPretrainingDashboardError::MissingField {
            field: String::from("systems_bundle.throughput_baselines[trusted_cluster_anchor]"),
        })?;

    let throughput_not_measured = run_shape_qualification.throughput_probe.source_receipt_id
        == "local_runtime_probe_unmeasured"
        || run_shape_qualification
            .throughput_probe
            .observed_tokens_per_second
            == 0;
    let throughput_degraded = !throughput_not_measured
        && (run_shape_qualification
            .throughput_probe
            .observed_tokens_per_second
            < run_shape_qualification.min_healthy_tokens_per_second
            || run_shape_qualification
                .throughput_probe
                .observed_step_latency_ms
                > run_shape_qualification.max_healthy_step_latency_ms
            || run_shape_qualification
                .throughput_probe
                .observed_checkpoint_write_throughput_bytes_per_second
                < run_shape_qualification.min_checkpoint_write_throughput_bytes_per_second
            || run_shape_qualification
                .dataloader_probe
                .observed_stall_count
                > run_shape_qualification.max_dataloader_stall_count);
    let throughput = PsionActualPretrainingDashboardThroughputCard {
        baseline_tokens_per_second: throughput_baseline.mean_tokens_per_second,
        observed_tokens_per_second: (!throughput_not_measured).then_some(
            run_shape_qualification
                .throughput_probe
                .observed_tokens_per_second,
        ),
        min_healthy_tokens_per_second: run_shape_qualification.min_healthy_tokens_per_second,
        observed_step_latency_ms: (!throughput_not_measured).then_some(
            run_shape_qualification
                .throughput_probe
                .observed_step_latency_ms,
        ),
        max_healthy_step_latency_ms: run_shape_qualification.max_healthy_step_latency_ms,
        observed_checkpoint_write_throughput_bytes_per_second: (!throughput_not_measured)
            .then_some(
                run_shape_qualification
                    .throughput_probe
                    .observed_checkpoint_write_throughput_bytes_per_second,
            ),
        min_checkpoint_write_throughput_bytes_per_second: run_shape_qualification
            .min_checkpoint_write_throughput_bytes_per_second,
        observed_batches_per_second: (!throughput_not_measured).then_some(
            run_shape_qualification
                .dataloader_probe
                .observed_batches_per_second,
        ),
        observed_stall_count: run_shape_qualification
            .dataloader_probe
            .observed_stall_count,
        degradation_state: String::from(if throughput_not_measured {
            "not_measured"
        } else if throughput_degraded {
            "degraded"
        } else {
            "healthy"
        }),
        detail: String::from(if throughput_not_measured {
            "Actual-lane dashboard exposes that the current operator bundle has no admitted live throughput receipt yet; the frozen systems baseline remains visible without pretending the launcher measured a real run."
        } else if throughput_degraded {
            "Observed run-shape throughput or checkpoint-write metrics are below the admitted actual-lane floor and require operator review before a long run continues."
        } else {
            "Observed run-shape throughput, checkpoint-write bandwidth, and dataloader cadence are above the admitted actual-lane floor."
        }),
    };

    let loss = PsionActualPretrainingDashboardLossCard {
        visibility_state: String::from(if current_status.last_completed_step == 0 {
            "not_started"
        } else {
            "not_emitted_by_actual_lane_operator"
        }),
        latest_mean_loss_milli: None,
        source_relative_path: None,
        detail: String::from(
            "The current actual-lane operator path does not yet retain a live mean-loss stream. The dashboard keeps that absence explicit instead of implying a hidden metric source.",
        ),
    };

    let gradient = PsionActualPretrainingDashboardGradientCard {
        visibility_state: String::from("qualified_reference_only"),
        collective_kind: systems_bundle
            .distributed_qualification
            .collective_kind
            .clone(),
        runtime_backend: systems_bundle
            .distributed_qualification
            .runtime_backend
            .clone(),
        transport: systems_bundle.distributed_qualification.transport.clone(),
        collective_benchmark_digest: systems_bundle
            .distributed_qualification
            .collective_benchmark_digest
            .clone(),
        detail: String::from(
            "Gradient visibility currently reflects the frozen distributed-qualification and collective benchmark surface for the actual lane, not a live per-step gradient stream.",
        ),
    };

    let mut min_free_memory: Option<u64> = None;
    let mut max_temperature: Option<u64> = None;
    let mut max_ecc: Option<u64> = None;
    let mut any_throttling = false;
    let mut unhealthy_workers = 0_u64;
    for worker in &hardware_qualification.observed_workers {
        min_free_memory = Some(match min_free_memory {
            Some(current) => current.min(worker.free_memory_bytes),
            None => worker.free_memory_bytes,
        });
        if let Some(temperature) = worker.temperature_celsius {
            max_temperature = Some(match max_temperature {
                Some(current) => current.max(temperature),
                None => temperature,
            });
            if temperature >= 85 {
                unhealthy_workers += 1;
            }
        }
        if let Some(ecc_count) = worker.ecc_uncorrected_error_count {
            max_ecc = Some(match max_ecc {
                Some(current) => current.max(ecc_count),
                None => ecc_count,
            });
            if ecc_count > 0 {
                unhealthy_workers += 1;
            }
        }
        if worker.free_memory_bytes
            < hardware_qualification.required_min_free_memory_bytes_per_worker
        {
            unhealthy_workers += 1;
        }
        if worker.throttling_observed.unwrap_or(false) {
            any_throttling = true;
            unhealthy_workers += 1;
        }
    }
    let hardware = PsionActualPretrainingDashboardHardwareCard {
        required_backend: hardware_qualification.required_backend.clone(),
        required_worker_count: hardware_qualification.required_worker_count,
        observed_worker_count: hardware_qualification.observed_workers.len() as u64,
        required_min_free_memory_bytes_per_worker: hardware_qualification
            .required_min_free_memory_bytes_per_worker,
        min_observed_free_memory_bytes: min_free_memory,
        max_observed_temperature_celsius: max_temperature,
        max_observed_ecc_uncorrected_error_count: max_ecc,
        any_throttling_observed: any_throttling,
        checkpoint_restore_ready: hardware_qualification.checkpoint_restore_ready,
        health_state: String::from(if hardware_qualification.admission_state != "admitted" {
            "refused"
        } else if unhealthy_workers > 0 {
            "degraded"
        } else {
            "healthy"
        }),
        detail: String::from(if hardware_qualification.admission_state != "admitted" {
            "Hardware qualification refused the actual lane; the retained dashboard keeps that refusal visible instead of collapsing it into a generic launch failure."
        } else if unhealthy_workers > 0 {
            "Observed worker health drifted away from the admitted actual-lane floor even though the retained qualification artifact still exists."
        } else {
            "Observed workers, memory headroom, checkpoint-restore posture, and credential-source checks stay within the admitted actual-lane floor."
        }),
    };

    let checkpoint_eval_state = if let Some(decision) = checkpoint_eval_decision {
        decision.decision_state.clone()
    } else if let Some(failure) = checkpoint_eval_failure {
        failure.resolution_state.clone()
    } else if checkpoint_pointer.optimizer_step == 0 {
        String::from("pending_first_checkpoint")
    } else {
        String::from("pending_eval")
    };
    let checkpoint_eval_receipt_path = checkpoint_eval_decision
        .map(|_| String::from(CHECKPOINT_EVAL_DECISION_PATH))
        .or_else(|| checkpoint_eval_failure.map(|_| String::from(CHECKPOINT_EVAL_FAILURE_PATH)));
    let checkpoint = PsionActualPretrainingDashboardCheckpointCard {
        checkpoint_label: checkpoint_pointer.checkpoint_label.clone(),
        optimizer_step: checkpoint_pointer.optimizer_step,
        checkpoint_ref: checkpoint_pointer
            .checkpoint_ref
            .clone()
            .unwrap_or_else(|| String::from("pending_first_checkpoint")),
        checkpoint_pointer_path: String::from(CHECKPOINT_POINTER_PATH),
        checkpoint_backup_state: checkpoint_backup_receipt.map_or_else(
            || {
                if checkpoint_pointer.optimizer_step == 0 {
                    String::from("pending_first_checkpoint")
                } else {
                    String::from("not_recorded_yet")
                }
            },
            |receipt| receipt.backup_state.clone(),
        ),
        checkpoint_eval_state,
        checkpoint_eval_receipt_path,
        continuation_handoff_path: String::from(CONTINUATION_HANDOFF_PATH),
        detail: String::from(
            "Checkpoint visibility keeps the latest accepted pointer, backup posture, continuation handoff target, and checkpoint-eval state in one retained operator surface.",
        ),
    };

    let mut active_alerts = Vec::new();
    if let Some(failure) = checkpoint_eval_failure {
        active_alerts.push(PsionActualPretrainingActiveAlert {
            alert_id: format!(
                "psion_actual_pretraining_dashboard::checkpoint_eval_retry_required::{}",
                failure.optimizer_step
            ),
            alert_kind: String::from(
                latest_redacted_alert
                    .map(|alert| alert.alert_kind.as_str())
                    .unwrap_or("checkpoint_eval_retry_required"),
            ),
            severity: String::from("warning"),
            state: String::from("active"),
            response_posture: String::from(
                "retry_checkpoint_eval_after_restoring_the_eval_worker",
            ),
            source_relative_path: String::from(
                latest_redacted_alert
                    .map(|_| LATEST_REDACTED_ALERT_PATH)
                    .unwrap_or(CHECKPOINT_EVAL_FAILURE_PATH),
            ),
            retained_redaction: String::from("declared_source_names_only"),
            summary: format!(
                "Checkpoint eval for `{}` step {} needs retry before the operator can rely on a new decision receipt.",
                failure.run_id, failure.optimizer_step
            ),
            detail: failure.detail.clone(),
        });
    }
    if let Some(backup_receipt) = checkpoint_backup_receipt {
        if backup_receipt.backup_state != "backed_up" {
            active_alerts.push(PsionActualPretrainingActiveAlert {
                alert_id: format!(
                    "psion_actual_pretraining_dashboard::checkpoint_backup_refused::{}",
                    backup_receipt.optimizer_step
                ),
                alert_kind: String::from("checkpoint_backup_refused"),
                severity: String::from("critical"),
                state: String::from("active"),
                response_posture: String::from(
                    "replay_backup_after_storage_credential_and_remote_root_review",
                ),
                source_relative_path: String::from(BACKUP_RECEIPT_PATH),
                retained_redaction: String::from("declared_source_names_only"),
                summary: format!(
                    "Checkpoint backup for `{}` step {} is not durable yet.",
                    backup_receipt.run_id, backup_receipt.optimizer_step
                ),
                detail: backup_receipt.detail.clone(),
            });
        }
    }
    if hardware.health_state != "healthy" {
        active_alerts.push(PsionActualPretrainingActiveAlert {
            alert_id: format!(
                "psion_actual_pretraining_dashboard::hardware_health::{}",
                current_status.run_id
            ),
            alert_kind: String::from(if hardware.health_state == "refused" {
                "hardware_health_refused"
            } else {
                "hardware_worker_unhealthy"
            }),
            severity: String::from(if hardware.health_state == "refused" {
                "critical"
            } else {
                "warning"
            }),
            state: String::from("active"),
            response_posture: String::from("inspect_hardware_qualification_before_launch"),
            source_relative_path: String::from(HARDWARE_QUALIFICATION_PATH),
            retained_redaction: String::from("declared_source_names_only"),
            summary: format!(
                "Hardware health for `{}` is `{}` under the retained actual-lane qualification.",
                current_status.run_id, hardware.health_state
            ),
            detail: hardware.detail.clone(),
        });
    }
    if throughput.degradation_state == "degraded" {
        active_alerts.push(PsionActualPretrainingActiveAlert {
            alert_id: format!(
                "psion_actual_pretraining_dashboard::throughput_degraded::{}",
                current_status.run_id
            ),
            alert_kind: String::from("throughput_degraded"),
            severity: String::from("warning"),
            state: String::from("active"),
            response_posture: String::from(
                "inspect_run_shape_qualification_before_continue",
            ),
            source_relative_path: String::from(RUN_SHAPE_QUALIFICATION_PATH),
            retained_redaction: String::from("declared_source_names_only"),
            summary: format!(
                "Throughput or checkpoint-write health for `{}` fell below the admitted actual-lane floor.",
                current_status.run_id
            ),
            detail: throughput.detail.clone(),
        });
    }
    active_alerts.sort_by(|left, right| {
        severity_rank(right.severity.as_str())
            .cmp(&severity_rank(left.severity.as_str()))
            .then_with(|| left.alert_kind.cmp(&right.alert_kind))
    });
    let highest_severity = active_alerts
        .first()
        .map(|alert| alert.severity.clone())
        .unwrap_or_else(|| String::from("none"));
    let mut alert_feed = PsionActualPretrainingAlertFeed {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_ALERT_FEED_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: current_status.run_id.clone(),
        alert_feed_path: String::from(PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH),
        active_alerts,
        summary: String::from(
            "Retained alert feed keeps the actual-lane operator response surface explicit without copying raw secret payloads into dashboards or alerts.",
        ),
        alert_feed_digest: String::new(),
    };
    alert_feed.alert_feed_digest = stable_alert_feed_digest(&alert_feed)?;

    let alerts = PsionActualPretrainingDashboardAlertSummary {
        active_alert_count: alert_feed.active_alerts.len() as u64,
        highest_severity,
        alert_feed_path: String::from(PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH),
        response_runbook_path: String::from(DASHBOARD_RUNBOOK_PATH),
        detail: String::from(
            "Dashboard alert summary points the operator at the retained alert feed plus one response runbook instead of scattering alert semantics across logs.",
        ),
    };
    let support_refs = vec![
        String::from(DASHBOARD_RUNBOOK_PATH),
        String::from("docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md"),
        String::from("docs/PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT.md"),
        String::from("docs/PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVALS.md"),
        String::from("docs/PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE.md"),
        String::from(HARDWARE_QUALIFICATION_PATH),
        String::from(RUN_SHAPE_QUALIFICATION_PATH),
    ];
    let mut dashboard = PsionActualPretrainingDashboardPacket {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_DASHBOARD_SCHEMA_VERSION),
        dashboard_id: format!(
            "psion_actual_pretraining_dashboard::{}",
            current_status.run_id
        ),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: current_status.run_id.clone(),
        current_phase: current_status.phase.clone(),
        selected_git_ref: retained_summary.selected_git_ref.clone(),
        git_commit_sha: retained_summary.git_commit_sha.clone(),
        dirty_tree_admission: retained_summary.dirty_tree_admission.clone(),
        dashboard_path: String::from(PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH),
        alert_feed_path: String::from(PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH),
        throughput,
        loss,
        gradient,
        checkpoint,
        hardware,
        alerts,
        support_refs,
        claim_boundary: String::from(
            "This retained dashboard gives the actual lane one operator-owned visibility surface over run status, preflight health, checkpoint progress, and active alerts. It keeps missing loss or live runtime signals explicit instead of fabricating them. It does not yet claim external alert delivery or a cluster-connected streaming dashboard.",
        ),
        detail: String::from(
            "Actual-lane dashboard binds status, checkpoint, preflight, and alert semantics into one retained operator packet that later hardening work can extend without renaming paths.",
        ),
        dashboard_digest: String::new(),
    };
    dashboard.dashboard_digest = stable_dashboard_digest(&dashboard)?;
    dashboard.validate()?;
    alert_feed.validate()?;
    Ok((dashboard, alert_feed))
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 3,
        "warning" => 2,
        "info" => 1,
        _ => 0,
    }
}

fn stable_dashboard_digest(
    dashboard: &PsionActualPretrainingDashboardPacket,
) -> Result<String, PsionActualPretrainingDashboardError> {
    let mut clone = dashboard.clone();
    clone.dashboard_digest.clear();
    Ok(hex_digest(&serde_json::to_vec(&clone)?))
}

fn stable_alert_feed_digest(
    alert_feed: &PsionActualPretrainingAlertFeed,
) -> Result<String, PsionActualPretrainingDashboardError> {
    let mut clone = alert_feed.clone();
    clone.alert_feed_digest.clear();
    Ok(hex_digest(&serde_json::to_vec(&clone)?))
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_actual_pretraining_dashboard|");
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionActualPretrainingDashboardError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingDashboardError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingDashboardError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionActualPretrainingDashboardError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_fixture_validates() {
        let packet: PsionActualPretrainingDashboardPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_dashboard_v1.json"
        ))
        .expect("dashboard fixture should parse");
        packet
            .validate()
            .expect("dashboard fixture should validate");
    }

    #[test]
    fn alert_feed_fixture_validates() {
        let feed: PsionActualPretrainingAlertFeed = serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_alert_feed_v1.json"
        ))
        .expect("alert feed fixture should parse");
        feed.validate().expect("alert feed fixture should validate");
    }

    #[test]
    fn refused_hardware_card_allows_zero_observed_workers() {
        let card = PsionActualPretrainingDashboardHardwareCard {
            required_backend: String::from("cuda"),
            required_worker_count: 4,
            observed_worker_count: 0,
            required_min_free_memory_bytes_per_worker: 1,
            min_observed_free_memory_bytes: None,
            max_observed_temperature_celsius: None,
            max_observed_ecc_uncorrected_error_count: None,
            any_throttling_observed: false,
            checkpoint_restore_ready: false,
            health_state: String::from("refused"),
            detail: String::from("hardware qualification refused the lane"),
        };

        card.validate()
            .expect("refused hardware card should allow zero observed workers");
    }

    #[test]
    fn healthy_hardware_card_requires_observed_workers() {
        let card = PsionActualPretrainingDashboardHardwareCard {
            required_backend: String::from("cuda"),
            required_worker_count: 4,
            observed_worker_count: 0,
            required_min_free_memory_bytes_per_worker: 1,
            min_observed_free_memory_bytes: None,
            max_observed_temperature_celsius: None,
            max_observed_ecc_uncorrected_error_count: None,
            any_throttling_observed: false,
            checkpoint_restore_ready: true,
            health_state: String::from("healthy"),
            detail: String::from("all workers are admitted"),
        };

        let error = card
            .validate()
            .expect_err("healthy hardware card should refuse zero observed workers");
        assert!(matches!(
            error,
            PsionActualPretrainingDashboardError::InvalidValue { field, .. }
                if field == "dashboard.hardware.worker_counts"
        ));
    }
}
