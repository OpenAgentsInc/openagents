use std::collections::BTreeSet;

use psionic_eval::build_psion_actual_pretraining_checkpoint_eval_benchmark_package;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PSION_ACTUAL_PRETRAINING_LANE_ID, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingDataBundle,
};

/// Stable schema version for the actual-lane checkpoint eval decision receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_DECISION_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_checkpoint_eval_decision.v1";

/// Stable schema version for the actual-lane checkpoint eval failure receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_FAILURE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_checkpoint_eval_failure.v1";

/// Stable schema version for the actual-lane redacted alert receipt.
pub const PSION_ACTUAL_PRETRAINING_REDACTED_ALERT_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_redacted_alert.v1";

/// Canonical fixture path for the actual-lane checkpoint eval decision receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_DECISION_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_eval_decision_v1.json";

/// Canonical fixture path for the actual-lane checkpoint eval failure receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_FAILURE_FIXTURE_PATH: &str = "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_eval_failure_worker_unavailable_v1.json";

/// Canonical fixture path for the actual-lane redacted alert receipt.
pub const PSION_ACTUAL_PRETRAINING_REDACTED_ALERT_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_redacted_alert_v1.json";

/// Canonical retained latest checkpoint eval decision path under the run root.
pub const PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH: &str =
    "evals/latest_checkpoint_eval_decision.json";

/// Canonical retained latest checkpoint eval failure path under the run root.
pub const PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_FAILURE_PATH: &str =
    "evals/latest_checkpoint_eval_failure.json";

/// Canonical retained latest redacted alert path under the run root.
pub const PSION_ACTUAL_PRETRAINING_LATEST_REDACTED_ALERT_PATH: &str =
    "alerts/latest_redacted_alert.json";

/// One metric gate row inside the actual-lane checkpoint eval decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointEvalMetricGate {
    /// Stable gate identifier.
    pub gate_id: String,
    /// Package family consumed by this gate.
    pub package_family: String,
    /// Acceptance family consumed by this gate.
    pub acceptance_family: String,
    /// Frozen benchmark case id.
    pub benchmark_case_id: String,
    /// Retained benchmark receipt id from the actual-lane data bundle.
    pub receipt_id: String,
    /// Metric kind scored by the gate.
    pub metric_kind: String,
    /// Required threshold in basis points.
    pub threshold_bps: u32,
    /// Observed metric in basis points.
    pub observed_bps: u32,
    /// Gate status.
    pub status: String,
    /// Short detail.
    pub detail: String,
}

/// Machine-readable eval decision retained for one accepted checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointEvalDecision {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Trigger surface that emitted the decision.
    pub trigger_surface_id: String,
    /// Selected git ref.
    pub selected_git_ref: String,
    /// Exact git commit SHA.
    pub git_commit_sha: String,
    /// Dirty-tree admission posture.
    pub dirty_tree_admission: String,
    /// Optional status digest when dirty-tree override is used.
    pub workspace_status_sha256: Option<String>,
    /// Accepted checkpoint label.
    pub checkpoint_label: String,
    /// Accepted optimizer step.
    pub optimizer_step: u64,
    /// Accepted checkpoint ref.
    pub checkpoint_ref: String,
    /// Retained manifest ref for the accepted checkpoint.
    pub checkpoint_manifest: PsionActualPretrainingArtifactRef,
    /// Retained committed benchmark-pack fixture consumed by the eval.
    pub benchmark_package_fixture: PsionActualPretrainingArtifactRef,
    /// Stable `benchmark_ref@version` key for the checkpoint eval pack.
    pub benchmark_package_storage_key: String,
    /// Fixed execution mode for this bounded actual-lane eval surface.
    pub evaluation_mode: String,
    /// Metric gates scored for this checkpoint.
    pub metric_gates: Vec<PsionActualPretrainingCheckpointEvalMetricGate>,
    /// Aggregate pass rate in basis points.
    pub aggregate_pass_rate_bps: u32,
    /// Aggregate score in basis points.
    pub aggregate_score_bps: u32,
    /// Retained decision state for later continue-vs-restart logic.
    pub decision_state: String,
    /// Short reason for the retained decision.
    pub decision_reason: String,
    /// Narrow claim boundary.
    pub claim_boundary: String,
    /// Short detail.
    pub detail: String,
    /// Stable digest over the decision receipt.
    pub decision_digest: String,
}

/// Machine-readable eval failure receipt retained for one accepted checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointEvalFailure {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Trigger surface that attempted the eval.
    pub trigger_surface_id: String,
    /// Selected git ref.
    pub selected_git_ref: String,
    /// Exact git commit SHA.
    pub git_commit_sha: String,
    /// Dirty-tree admission posture.
    pub dirty_tree_admission: String,
    /// Optional status digest when dirty-tree override is used.
    pub workspace_status_sha256: Option<String>,
    /// Accepted checkpoint label.
    pub checkpoint_label: String,
    /// Accepted optimizer step.
    pub optimizer_step: u64,
    /// Accepted checkpoint ref.
    pub checkpoint_ref: String,
    /// Retained manifest ref for the accepted checkpoint.
    pub checkpoint_manifest: PsionActualPretrainingArtifactRef,
    /// Retained committed benchmark-pack fixture consumed by the eval.
    pub benchmark_package_fixture: PsionActualPretrainingArtifactRef,
    /// Stable `benchmark_ref@version` key for the checkpoint eval pack.
    pub benchmark_package_storage_key: String,
    /// Failure kind.
    pub failure_kind: String,
    /// Retained operator posture after the failure.
    pub resolution_state: String,
    /// Required retry delay for the next eval attempt.
    pub retry_after_seconds: u32,
    /// Relative alert path written for this failure.
    pub alert_relative_path: String,
    /// Narrow claim boundary.
    pub claim_boundary: String,
    /// Short detail.
    pub detail: String,
    /// Stable digest over the failure receipt.
    pub failure_digest: String,
}

/// Redacted alert emitted for a retained actual-lane operator failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRedactedAlert {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable alert identifier.
    pub alert_id: String,
    /// Alert kind.
    pub alert_kind: String,
    /// Severity label.
    pub severity: String,
    /// Surface that emitted the alert.
    pub emitted_from_surface_id: String,
    /// Relative path to the failure receipt that caused this alert.
    pub source_receipt_relative_path: String,
    /// Short redaction policy label.
    pub retained_redaction: String,
    /// Short operator summary.
    pub summary: String,
    /// Short detail.
    pub detail: String,
    /// Stable digest over the alert.
    pub alert_digest: String,
}

impl PsionActualPretrainingCheckpointEvalMetricGate {
    fn validate(&self) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
        ensure_nonempty(
            self.gate_id.as_str(),
            "checkpoint_eval.metric_gates[].gate_id",
        )?;
        ensure_nonempty(
            self.package_family.as_str(),
            "checkpoint_eval.metric_gates[].package_family",
        )?;
        ensure_nonempty(
            self.acceptance_family.as_str(),
            "checkpoint_eval.metric_gates[].acceptance_family",
        )?;
        ensure_nonempty(
            self.benchmark_case_id.as_str(),
            "checkpoint_eval.metric_gates[].benchmark_case_id",
        )?;
        ensure_nonempty(
            self.receipt_id.as_str(),
            "checkpoint_eval.metric_gates[].receipt_id",
        )?;
        ensure_nonempty(
            self.metric_kind.as_str(),
            "checkpoint_eval.metric_gates[].metric_kind",
        )?;
        if self.threshold_bps == 0 || self.observed_bps == 0 {
            return Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
                field: String::from("checkpoint_eval.metric_gates[].bps"),
                detail: String::from("threshold and observed bps must be positive"),
            });
        }
        match self.status.as_str() {
            "passed" | "failed" => {}
            _ => {
                return Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
                    field: String::from("checkpoint_eval.metric_gates[].status"),
                    detail: String::from("metric gate status must be passed or failed"),
                });
            }
        }
        ensure_nonempty(
            self.detail.as_str(),
            "checkpoint_eval.metric_gates[].detail",
        )?;
        Ok(())
    }
}

impl PsionActualPretrainingCheckpointEvalDecision {
    /// Validates the retained checkpoint eval decision.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "checkpoint_eval.schema_version",
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_DECISION_SCHEMA_VERSION,
        )?;
        validate_receipt_common(
            self.lane_id.as_str(),
            self.run_id.as_str(),
            self.trigger_surface_id.as_str(),
            self.selected_git_ref.as_str(),
            self.git_commit_sha.as_str(),
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            self.checkpoint_label.as_str(),
            self.optimizer_step,
            self.checkpoint_ref.as_str(),
            &self.checkpoint_manifest,
            &self.benchmark_package_fixture,
            self.benchmark_package_storage_key.as_str(),
        )?;
        ensure_exact(
            self.evaluation_mode.as_str(),
            "checkpoint_eval.evaluation_mode",
            "operator_simulation",
        )?;
        if self.metric_gates.len() != 4 {
            return Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
                field: String::from("checkpoint_eval.metric_gates"),
                detail: String::from("actual-lane checkpoint eval must retain four metric gates"),
            });
        }
        let mut gate_ids = BTreeSet::new();
        let mut package_families = BTreeSet::new();
        let mut passed_count = 0u32;
        for gate in &self.metric_gates {
            gate.validate()?;
            if !gate_ids.insert(gate.gate_id.as_str()) {
                return Err(PsionActualPretrainingCheckpointEvalError::DuplicateGate {
                    gate_id: gate.gate_id.clone(),
                });
            }
            package_families.insert(gate.package_family.as_str());
            if gate.status == "passed" {
                passed_count += 1;
            }
        }
        for family in [
            "architecture_reasoning",
            "normative_spec_reading",
            "engineering_spec_interpretation",
            "memorization_versus_reasoning",
        ] {
            if !package_families.contains(family) {
                return Err(PsionActualPretrainingCheckpointEvalError::MissingField {
                    field: format!("checkpoint_eval.metric_gates[{family}]"),
                });
            }
        }
        if self.aggregate_pass_rate_bps == 0 || self.aggregate_score_bps == 0 {
            return Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
                field: String::from("checkpoint_eval.aggregate_bps"),
                detail: String::from("aggregate pass rate and score must be positive"),
            });
        }
        let expected_decision_state = if passed_count == 4 {
            "continue"
        } else if self.aggregate_score_bps >= 8000 {
            "hold_and_review"
        } else {
            "restart_from_last_accepted_checkpoint"
        };
        ensure_exact(
            self.decision_state.as_str(),
            "checkpoint_eval.decision_state",
            expected_decision_state,
        )?;
        ensure_nonempty(
            self.decision_reason.as_str(),
            "checkpoint_eval.decision_reason",
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "checkpoint_eval.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "checkpoint_eval.detail")?;
        if stable_checkpoint_eval_decision_digest(self) != self.decision_digest {
            return Err(PsionActualPretrainingCheckpointEvalError::DigestMismatch {
                field: String::from("checkpoint_eval.decision_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingCheckpointEvalFailure {
    /// Validates the retained checkpoint eval failure receipt.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "checkpoint_eval_failure.schema_version",
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_FAILURE_SCHEMA_VERSION,
        )?;
        validate_receipt_common(
            self.lane_id.as_str(),
            self.run_id.as_str(),
            self.trigger_surface_id.as_str(),
            self.selected_git_ref.as_str(),
            self.git_commit_sha.as_str(),
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            self.checkpoint_label.as_str(),
            self.optimizer_step,
            self.checkpoint_ref.as_str(),
            &self.checkpoint_manifest,
            &self.benchmark_package_fixture,
            self.benchmark_package_storage_key.as_str(),
        )?;
        match self.failure_kind.as_str() {
            "eval_worker_unavailable" | "eval_trigger_failed" => {}
            _ => {
                return Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
                    field: String::from("checkpoint_eval_failure.failure_kind"),
                    detail: String::from(
                        "failure_kind must be eval_worker_unavailable or eval_trigger_failed",
                    ),
                });
            }
        }
        ensure_exact(
            self.resolution_state.as_str(),
            "checkpoint_eval_failure.resolution_state",
            "retry_required",
        )?;
        if self.retry_after_seconds == 0 {
            return Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
                field: String::from("checkpoint_eval_failure.retry_after_seconds"),
                detail: String::from("retry_after_seconds must be positive"),
            });
        }
        ensure_exact(
            self.alert_relative_path.as_str(),
            "checkpoint_eval_failure.alert_relative_path",
            PSION_ACTUAL_PRETRAINING_LATEST_REDACTED_ALERT_PATH,
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "checkpoint_eval_failure.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "checkpoint_eval_failure.detail")?;
        if stable_checkpoint_eval_failure_digest(self) != self.failure_digest {
            return Err(PsionActualPretrainingCheckpointEvalError::DigestMismatch {
                field: String::from("checkpoint_eval_failure.failure_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingRedactedAlert {
    /// Validates the retained redacted alert.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "redacted_alert.schema_version",
            PSION_ACTUAL_PRETRAINING_REDACTED_ALERT_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "redacted_alert.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(self.run_id.as_str(), "redacted_alert.run_id")?;
        ensure_nonempty(self.alert_id.as_str(), "redacted_alert.alert_id")?;
        ensure_exact(
            self.alert_kind.as_str(),
            "redacted_alert.alert_kind",
            "checkpoint_eval_retry_required",
        )?;
        ensure_exact(self.severity.as_str(), "redacted_alert.severity", "warning")?;
        ensure_exact(
            self.emitted_from_surface_id.as_str(),
            "redacted_alert.emitted_from_surface_id",
            "psion_actual_pretraining.record_checkpoint",
        )?;
        ensure_nonempty(
            self.source_receipt_relative_path.as_str(),
            "redacted_alert.source_receipt_relative_path",
        )?;
        ensure_exact(
            self.retained_redaction.as_str(),
            "redacted_alert.retained_redaction",
            "declared_source_names_only",
        )?;
        ensure_nonempty(self.summary.as_str(), "redacted_alert.summary")?;
        ensure_nonempty(self.detail.as_str(), "redacted_alert.detail")?;
        if stable_redacted_alert_digest(self) != self.alert_digest {
            return Err(PsionActualPretrainingCheckpointEvalError::DigestMismatch {
                field: String::from("redacted_alert.alert_digest"),
            });
        }
        Ok(())
    }
}

/// Derives the checkpoint eval decision for one accepted checkpoint.
pub fn record_psion_actual_pretraining_checkpoint_eval_decision(
    run_id: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    checkpoint_label: &str,
    optimizer_step: u64,
    checkpoint_ref: &str,
    checkpoint_manifest: PsionActualPretrainingArtifactRef,
    benchmark_package_fixture: PsionActualPretrainingArtifactRef,
    data_bundle: &PsionActualPretrainingDataBundle,
    claim_boundary: &str,
    detail: &str,
) -> Result<PsionActualPretrainingCheckpointEvalDecision, PsionActualPretrainingCheckpointEvalError>
{
    data_bundle.validate().map_err(|error| {
        PsionActualPretrainingCheckpointEvalError::InvalidValue {
            field: String::from("checkpoint_eval.data_bundle"),
            detail: error.to_string(),
        }
    })?;
    let benchmark_package = build_psion_actual_pretraining_checkpoint_eval_benchmark_package()?;
    let benchmark_package_storage_key = benchmark_package.key.storage_key();
    let metric_gates = derive_metric_gates(data_bundle)?;
    let aggregate_score_bps = metric_gates
        .iter()
        .map(|gate| gate.observed_bps as u64)
        .sum::<u64>() as u32
        / metric_gates.len() as u32;
    let aggregate_pass_rate_bps = metric_gates
        .iter()
        .filter(|gate| gate.status == "passed")
        .count() as u32
        * 10_000
        / metric_gates.len() as u32;
    let decision_state = if aggregate_pass_rate_bps == 10_000 {
        "continue"
    } else if aggregate_score_bps >= 8_000 {
        "hold_and_review"
    } else {
        "restart_from_last_accepted_checkpoint"
    };
    let decision_reason = match decision_state {
        "continue" => {
            "All frozen checkpoint-eval gates remained above threshold, so later continue-vs-restart logic may treat this checkpoint as eval-green."
        }
        "hold_and_review" => {
            "At least one checkpoint-eval gate fell below threshold while aggregate score stayed above the bounded hold floor, so later continue-vs-restart logic must hold and review before continuing."
        }
        _ => {
            "Checkpoint-eval score fell below the bounded restart floor, so later continue-vs-restart logic should treat this checkpoint as restart-only."
        }
    };
    let mut receipt = PsionActualPretrainingCheckpointEvalDecision {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_DECISION_SCHEMA_VERSION,
        ),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        trigger_surface_id: String::from("psion_actual_pretraining.record_checkpoint"),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        checkpoint_label: String::from(checkpoint_label),
        optimizer_step,
        checkpoint_ref: String::from(checkpoint_ref),
        checkpoint_manifest,
        benchmark_package_fixture,
        benchmark_package_storage_key,
        evaluation_mode: String::from("operator_simulation"),
        metric_gates,
        aggregate_pass_rate_bps,
        aggregate_score_bps,
        decision_state: String::from(decision_state),
        decision_reason: String::from(decision_reason),
        claim_boundary: String::from(claim_boundary),
        detail: String::from(detail),
        decision_digest: String::new(),
    };
    receipt.decision_digest = stable_checkpoint_eval_decision_digest(&receipt);
    receipt.validate()?;
    Ok(receipt)
}

/// Derives the retained checkpoint eval failure receipt.
pub fn record_psion_actual_pretraining_checkpoint_eval_failure(
    run_id: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    checkpoint_label: &str,
    optimizer_step: u64,
    checkpoint_ref: &str,
    checkpoint_manifest: PsionActualPretrainingArtifactRef,
    benchmark_package_fixture: PsionActualPretrainingArtifactRef,
    failure_kind: &str,
    claim_boundary: &str,
    detail: &str,
) -> Result<PsionActualPretrainingCheckpointEvalFailure, PsionActualPretrainingCheckpointEvalError>
{
    let benchmark_package = build_psion_actual_pretraining_checkpoint_eval_benchmark_package()?;
    let mut receipt = PsionActualPretrainingCheckpointEvalFailure {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_FAILURE_SCHEMA_VERSION,
        ),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        trigger_surface_id: String::from("psion_actual_pretraining.record_checkpoint"),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        checkpoint_label: String::from(checkpoint_label),
        optimizer_step,
        checkpoint_ref: String::from(checkpoint_ref),
        checkpoint_manifest,
        benchmark_package_fixture,
        benchmark_package_storage_key: benchmark_package.key.storage_key(),
        failure_kind: String::from(failure_kind),
        resolution_state: String::from("retry_required"),
        retry_after_seconds: 300,
        alert_relative_path: String::from(PSION_ACTUAL_PRETRAINING_LATEST_REDACTED_ALERT_PATH),
        claim_boundary: String::from(claim_boundary),
        detail: String::from(detail),
        failure_digest: String::new(),
    };
    receipt.failure_digest = stable_checkpoint_eval_failure_digest(&receipt);
    receipt.validate()?;
    Ok(receipt)
}

/// Derives the redacted alert tied to a retained checkpoint eval failure.
pub fn record_psion_actual_pretraining_redacted_alert(
    run_id: &str,
    optimizer_step: u64,
    failure_relative_path: &str,
    detail: &str,
) -> Result<PsionActualPretrainingRedactedAlert, PsionActualPretrainingCheckpointEvalError> {
    let mut alert = PsionActualPretrainingRedactedAlert {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_REDACTED_ALERT_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        alert_id: format!("psion_actual_pretraining_checkpoint_eval_alert::{optimizer_step}"),
        alert_kind: String::from("checkpoint_eval_retry_required"),
        severity: String::from("warning"),
        emitted_from_surface_id: String::from("psion_actual_pretraining.record_checkpoint"),
        source_receipt_relative_path: String::from(failure_relative_path),
        retained_redaction: String::from("declared_source_names_only"),
        summary: format!(
            "Checkpoint eval needs retry for `{run_id}` step {optimizer_step}; retained artifacts keep source names and digests only."
        ),
        detail: String::from(detail),
        alert_digest: String::new(),
    };
    alert.alert_digest = stable_redacted_alert_digest(&alert);
    alert.validate()?;
    Ok(alert)
}

/// Relative path for a per-checkpoint eval decision receipt.
#[must_use]
pub fn checkpoint_eval_decision_relative_path(optimizer_step: u64) -> String {
    format!("evals/checkpoint_eval_step-{optimizer_step}.json")
}

/// Relative path for a per-checkpoint eval failure receipt.
#[must_use]
pub fn checkpoint_eval_failure_relative_path(optimizer_step: u64) -> String {
    format!("evals/checkpoint_eval_failure_step-{optimizer_step}.json")
}

fn derive_metric_gates(
    data_bundle: &PsionActualPretrainingDataBundle,
) -> Result<
    Vec<PsionActualPretrainingCheckpointEvalMetricGate>,
    PsionActualPretrainingCheckpointEvalError,
> {
    let mut gates = Vec::new();
    let mut seen = BTreeSet::new();
    for binding in &data_bundle.recipe_change_eval_package.eval_bindings {
        if !seen.insert(binding.package_family.as_str()) {
            continue;
        }
        let threshold_bps = threshold_bps_for_family(binding.package_family.as_str())?;
        let status = if binding.observed_bps >= threshold_bps {
            "passed"
        } else {
            "failed"
        };
        gates.push(PsionActualPretrainingCheckpointEvalMetricGate {
            gate_id: format!("{}_gate", binding.package_family),
            package_family: binding.package_family.clone(),
            acceptance_family: binding.acceptance_family.clone(),
            benchmark_case_id: format!("{}_gate", binding.package_family),
            receipt_id: binding.receipt_id.clone(),
            metric_kind: binding.metric_kind.clone(),
            threshold_bps,
            observed_bps: binding.observed_bps,
            status: String::from(status),
            detail: format!(
                "Checkpoint eval keeps the saved actual-lane checkpoint bound to the frozen `{}` benchmark family before later continue-vs-restart policy consumes the retained decision.",
                binding.package_family
            ),
        });
    }
    gates.sort_by(|left, right| left.gate_id.cmp(&right.gate_id));
    if gates.len() != 4 {
        return Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
            field: String::from("checkpoint_eval.metric_gates"),
            detail: format!(
                "expected four frozen package families from the actual-lane data bundle, found {}",
                gates.len()
            ),
        });
    }
    Ok(gates)
}

fn threshold_bps_for_family(
    package_family: &str,
) -> Result<u32, PsionActualPretrainingCheckpointEvalError> {
    match package_family {
        "architecture_reasoning" => Ok(8200),
        "normative_spec_reading" => Ok(8600),
        "engineering_spec_interpretation" => Ok(8500),
        "memorization_versus_reasoning" => Ok(7900),
        _ => Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
            field: String::from("checkpoint_eval.package_family"),
            detail: format!("unsupported package family `{package_family}`"),
        }),
    }
}

fn validate_receipt_common(
    lane_id: &str,
    run_id: &str,
    trigger_surface_id: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<&str>,
    checkpoint_label: &str,
    optimizer_step: u64,
    checkpoint_ref: &str,
    checkpoint_manifest: &PsionActualPretrainingArtifactRef,
    benchmark_package_fixture: &PsionActualPretrainingArtifactRef,
    benchmark_package_storage_key: &str,
) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
    ensure_exact(lane_id, "lane_id", PSION_ACTUAL_PRETRAINING_LANE_ID)?;
    ensure_nonempty(run_id, "run_id")?;
    ensure_exact(
        trigger_surface_id,
        "trigger_surface_id",
        "psion_actual_pretraining.record_checkpoint",
    )?;
    ensure_nonempty(selected_git_ref, "selected_git_ref")?;
    ensure_git_sha(git_commit_sha, "git_commit_sha")?;
    ensure_dirty_tree_admission(
        dirty_tree_admission,
        workspace_status_sha256,
        "dirty_tree_admission",
    )?;
    ensure_nonempty(checkpoint_label, "checkpoint_label")?;
    if optimizer_step == 0 {
        return Err(PsionActualPretrainingCheckpointEvalError::MissingField {
            field: String::from("optimizer_step"),
        });
    }
    ensure_nonempty(checkpoint_ref, "checkpoint_ref")?;
    ensure_artifact_ref(checkpoint_manifest, "checkpoint_manifest")?;
    ensure_artifact_ref(benchmark_package_fixture, "benchmark_package_fixture")?;
    ensure_nonempty(
        benchmark_package_storage_key,
        "benchmark_package_storage_key",
    )?;
    Ok(())
}

fn stable_checkpoint_eval_decision_digest(
    decision: &PsionActualPretrainingCheckpointEvalDecision,
) -> String {
    let mut copy = decision.clone();
    copy.decision_digest.clear();
    stable_digest(b"psion_actual_pretraining_checkpoint_eval_decision|", &copy)
}

fn stable_checkpoint_eval_failure_digest(
    failure: &PsionActualPretrainingCheckpointEvalFailure,
) -> String {
    let mut copy = failure.clone();
    copy.failure_digest.clear();
    stable_digest(b"psion_actual_pretraining_checkpoint_eval_failure|", &copy)
}

fn stable_redacted_alert_digest(alert: &PsionActualPretrainingRedactedAlert) -> String {
    let mut copy = alert.clone();
    copy.alert_digest.clear();
    stable_digest(b"psion_actual_pretraining_redacted_alert|", &copy)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let canonical = serde_json::to_vec(value).expect("checkpoint eval receipt should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&canonical);
    hex::encode(hasher.finalize())
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
    if actual != expected {
        return Err(PsionActualPretrainingCheckpointEvalError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingCheckpointEvalError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_git_sha(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
    ensure_nonempty(value, field)?;
    if value.len() != 40 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
            field: String::from(field),
            detail: String::from("git commit SHA must be a 40-character hex string"),
        });
    }
    Ok(())
}

fn ensure_dirty_tree_admission(
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<&str>,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
    match dirty_tree_admission {
        "refuse_by_default" => Ok(()),
        "allowed_by_operator_override" => ensure_nonempty_option(
            workspace_status_sha256,
            &format!("{field_prefix}.workspace_status_sha256"),
        ),
        _ => Err(PsionActualPretrainingCheckpointEvalError::InvalidValue {
            field: String::from(field_prefix),
            detail: String::from(
                "dirty-tree admission must be refuse_by_default or allowed_by_operator_override",
            ),
        }),
    }
}

fn ensure_nonempty_option(
    value: Option<&str>,
    field: &str,
) -> Result<(), PsionActualPretrainingCheckpointEvalError> {
    match value {
        Some(value) => ensure_nonempty(value, field),
        None => Err(PsionActualPretrainingCheckpointEvalError::MissingField {
            field: String::from(field),
        }),
    }
}

/// Validation errors for actual-lane checkpoint eval receipts.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingCheckpointEvalError {
    #[error("psion actual-pretraining checkpoint eval is missing field `{field}`")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining checkpoint eval field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining checkpoint eval field `{field}` is invalid: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("psion actual-pretraining checkpoint eval has duplicate gate `{gate_id}`")]
    DuplicateGate { gate_id: String },
    #[error("psion actual-pretraining checkpoint eval digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error(transparent)]
    EvalRuntime(#[from] psionic_eval::EvalRuntimeError),
}

#[cfg(test)]
mod tests {
    use super::{
        PsionActualPretrainingCheckpointEvalDecision, PsionActualPretrainingCheckpointEvalFailure,
        PsionActualPretrainingRedactedAlert,
    };

    fn decision_fixture() -> PsionActualPretrainingCheckpointEvalDecision {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_eval_decision_v1.json"
        ))
        .expect("checkpoint eval decision fixture should parse")
    }

    fn failure_fixture() -> PsionActualPretrainingCheckpointEvalFailure {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_eval_failure_worker_unavailable_v1.json"
        ))
        .expect("checkpoint eval failure fixture should parse")
    }

    fn alert_fixture() -> PsionActualPretrainingRedactedAlert {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_redacted_alert_v1.json"
        ))
        .expect("redacted alert fixture should parse")
    }

    #[test]
    fn actual_pretraining_checkpoint_eval_decision_fixture_validates() {
        decision_fixture()
            .validate()
            .expect("checkpoint eval decision fixture should validate");
    }

    #[test]
    fn actual_pretraining_checkpoint_eval_failure_fixture_validates() {
        failure_fixture()
            .validate()
            .expect("checkpoint eval failure fixture should validate");
    }

    #[test]
    fn actual_pretraining_redacted_alert_fixture_validates() {
        alert_fixture()
            .validate()
            .expect("redacted alert fixture should validate");
    }
}
