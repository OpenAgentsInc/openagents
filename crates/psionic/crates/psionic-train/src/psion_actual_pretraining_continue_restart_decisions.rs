use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PSION_ACTUAL_PRETRAINING_LANE_ID, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingCheckpointBackupReceipt, PsionActualPretrainingCheckpointEvalDecision,
    PsionActualPretrainingCheckpointEvalFailure, PsionActualPretrainingCheckpointPointer,
    PsionActualPretrainingHardwareQualification, PsionActualPretrainingRunShapeQualification,
    PsionActualPretrainingSystemsBundle,
};

/// Stable schema version for one retained actual-lane checkpoint comparison receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_COMPARISON_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_checkpoint_comparison.v1";

/// Stable schema version for one retained actual-lane continue-restart decision receipt.
pub const PSION_ACTUAL_PRETRAINING_CONTINUE_RESTART_DECISION_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_continue_restart_decision.v1";

/// Canonical fixture path for the retained actual-lane checkpoint comparison receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_COMPARISON_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_comparison_v1.json";

/// Canonical fixture path for the retained actual-lane continue-restart decision receipt.
pub const PSION_ACTUAL_PRETRAINING_CONTINUE_RESTART_DECISION_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_continue_restart_decision_v1.json";

/// Canonical retained latest checkpoint comparison path under the run root.
pub const PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_COMPARISON_PATH: &str =
    "decisions/latest_checkpoint_comparison.json";

/// Canonical retained latest continue-restart decision path under the run root.
pub const PSION_ACTUAL_PRETRAINING_LATEST_CONTINUE_RESTART_DECISION_PATH: &str =
    "decisions/latest_continue_restart_decision.json";

const CONTINUE_RESTART_TRIGGER_SURFACE_ID: &str =
    "psion_actual_pretraining.decide_continue_restart";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointComparisonRow {
    pub row_id: String,
    pub signal_kind: String,
    pub comparison_kind: String,
    pub expected_value: String,
    pub observed_value: String,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointComparison {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub trigger_surface_id: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub workspace_status_sha256: Option<String>,
    pub checkpoint_label: String,
    pub optimizer_step: u64,
    pub checkpoint_ref: String,
    pub checkpoint_pointer: PsionActualPretrainingArtifactRef,
    pub checkpoint_manifest: PsionActualPretrainingArtifactRef,
    pub checkpoint_backup_receipt: Option<PsionActualPretrainingArtifactRef>,
    pub checkpoint_eval_decision: Option<PsionActualPretrainingArtifactRef>,
    pub checkpoint_eval_failure: Option<PsionActualPretrainingArtifactRef>,
    pub hardware_qualification: PsionActualPretrainingArtifactRef,
    pub run_shape_qualification: PsionActualPretrainingArtifactRef,
    pub systems_bundle: PsionActualPretrainingArtifactRef,
    pub comparison_rows: Vec<PsionActualPretrainingCheckpointComparisonRow>,
    pub checkpoint_readiness_state: String,
    pub claim_boundary: String,
    pub detail: String,
    pub comparison_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingContinueRestartDecision {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub trigger_surface_id: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub workspace_status_sha256: Option<String>,
    pub checkpoint_label: String,
    pub optimizer_step: u64,
    pub checkpoint_ref: String,
    pub checkpoint_comparison: PsionActualPretrainingArtifactRef,
    pub checkpoint_eval_decision: Option<PsionActualPretrainingArtifactRef>,
    pub checkpoint_eval_failure: Option<PsionActualPretrainingArtifactRef>,
    pub checkpoint_backup_receipt: Option<PsionActualPretrainingArtifactRef>,
    pub hardware_qualification: PsionActualPretrainingArtifactRef,
    pub run_shape_qualification: PsionActualPretrainingArtifactRef,
    pub systems_bundle: PsionActualPretrainingArtifactRef,
    pub decision_state: String,
    pub operator_action: String,
    pub blocking_row_ids: Vec<String>,
    pub decision_reason: String,
    pub claim_boundary: String,
    pub detail: String,
    pub decision_digest: String,
}

impl PsionActualPretrainingCheckpointComparisonRow {
    fn validate(&self) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
        ensure_nonempty(self.row_id.as_str(), "checkpoint_comparison.rows[].row_id")?;
        ensure_nonempty(
            self.signal_kind.as_str(),
            "checkpoint_comparison.rows[].signal_kind",
        )?;
        match self.comparison_kind.as_str() {
            "exact_match" | "min_floor" | "max_ceiling" => {}
            _ => {
                return Err(
                    PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                        field: String::from("checkpoint_comparison.rows[].comparison_kind"),
                        detail: String::from(
                            "comparison kind must be exact_match, min_floor, or max_ceiling",
                        ),
                    },
                );
            }
        }
        ensure_nonempty(
            self.expected_value.as_str(),
            "checkpoint_comparison.rows[].expected_value",
        )?;
        ensure_nonempty(
            self.observed_value.as_str(),
            "checkpoint_comparison.rows[].observed_value",
        )?;
        match self.status.as_str() {
            "green" | "red" => {}
            _ => {
                return Err(
                    PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                        field: String::from("checkpoint_comparison.rows[].status"),
                        detail: String::from("comparison row status must be green or red"),
                    },
                );
            }
        }
        ensure_nonempty(self.detail.as_str(), "checkpoint_comparison.rows[].detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingCheckpointComparison {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
        ensure_exact(
            self.schema_version.as_str(),
            "checkpoint_comparison.schema_version",
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_COMPARISON_SCHEMA_VERSION,
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
        )?;
        ensure_artifact_ref(
            &self.checkpoint_pointer,
            "checkpoint_comparison.checkpoint_pointer",
        )?;
        ensure_artifact_ref(
            &self.checkpoint_manifest,
            "checkpoint_comparison.checkpoint_manifest",
        )?;
        ensure_optional_artifact_ref(
            self.checkpoint_backup_receipt.as_ref(),
            "checkpoint_comparison.checkpoint_backup_receipt",
        )?;
        ensure_optional_artifact_ref(
            self.checkpoint_eval_decision.as_ref(),
            "checkpoint_comparison.checkpoint_eval_decision",
        )?;
        ensure_optional_artifact_ref(
            self.checkpoint_eval_failure.as_ref(),
            "checkpoint_comparison.checkpoint_eval_failure",
        )?;
        ensure_artifact_ref(
            &self.hardware_qualification,
            "checkpoint_comparison.hardware_qualification",
        )?;
        ensure_artifact_ref(
            &self.run_shape_qualification,
            "checkpoint_comparison.run_shape_qualification",
        )?;
        ensure_artifact_ref(&self.systems_bundle, "checkpoint_comparison.systems_bundle")?;
        if self.comparison_rows.is_empty() {
            return Err(
                PsionActualPretrainingContinueRestartDecisionError::MissingField {
                    field: String::from("checkpoint_comparison.comparison_rows"),
                },
            );
        }
        let mut row_ids = BTreeSet::new();
        for row in &self.comparison_rows {
            row.validate()?;
            if !row_ids.insert(row.row_id.as_str()) {
                return Err(
                    PsionActualPretrainingContinueRestartDecisionError::DuplicateRow {
                        row_id: row.row_id.clone(),
                    },
                );
            }
        }
        match self.checkpoint_readiness_state.as_str() {
            "ready_to_continue" | "ready_to_restart" | "hold_for_investigation" => {}
            _ => {
                return Err(
                    PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                        field: String::from("checkpoint_comparison.checkpoint_readiness_state"),
                        detail: String::from(
                            "checkpoint readiness state must be ready_to_continue, ready_to_restart, or hold_for_investigation",
                        ),
                    },
                );
            }
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "checkpoint_comparison.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "checkpoint_comparison.detail")?;
        if stable_checkpoint_comparison_digest(self) != self.comparison_digest {
            return Err(
                PsionActualPretrainingContinueRestartDecisionError::DigestMismatch {
                    field: String::from("checkpoint_comparison.comparison_digest"),
                },
            );
        }
        Ok(())
    }
}

impl PsionActualPretrainingContinueRestartDecision {
    pub fn validate(&self) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
        ensure_exact(
            self.schema_version.as_str(),
            "continue_restart_decision.schema_version",
            PSION_ACTUAL_PRETRAINING_CONTINUE_RESTART_DECISION_SCHEMA_VERSION,
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
        )?;
        ensure_artifact_ref(
            &self.checkpoint_comparison,
            "continue_restart_decision.checkpoint_comparison",
        )?;
        ensure_optional_artifact_ref(
            self.checkpoint_eval_decision.as_ref(),
            "continue_restart_decision.checkpoint_eval_decision",
        )?;
        ensure_optional_artifact_ref(
            self.checkpoint_eval_failure.as_ref(),
            "continue_restart_decision.checkpoint_eval_failure",
        )?;
        ensure_optional_artifact_ref(
            self.checkpoint_backup_receipt.as_ref(),
            "continue_restart_decision.checkpoint_backup_receipt",
        )?;
        ensure_artifact_ref(
            &self.hardware_qualification,
            "continue_restart_decision.hardware_qualification",
        )?;
        ensure_artifact_ref(
            &self.run_shape_qualification,
            "continue_restart_decision.run_shape_qualification",
        )?;
        ensure_artifact_ref(
            &self.systems_bundle,
            "continue_restart_decision.systems_bundle",
        )?;
        match self.decision_state.as_str() {
            "continue" | "hold_and_investigate" | "restart_from_last_accepted_checkpoint" => {}
            _ => {
                return Err(
                    PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                        field: String::from("continue_restart_decision.decision_state"),
                        detail: String::from(
                            "decision state must be continue, hold_and_investigate, or restart_from_last_accepted_checkpoint",
                        ),
                    },
                );
            }
        }
        let expected_action = match self.decision_state.as_str() {
            "continue" => "continue_long_run",
            "hold_and_investigate" => "pause_and_review",
            _ => "restart_from_latest_accepted_checkpoint",
        };
        ensure_exact(
            self.operator_action.as_str(),
            "continue_restart_decision.operator_action",
            expected_action,
        )?;
        ensure_unique_nonempty_strings(
            self.blocking_row_ids.as_slice(),
            "continue_restart_decision.blocking_row_ids[]",
        )?;
        if self.decision_state == "continue" && !self.blocking_row_ids.is_empty() {
            return Err(
                PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                    field: String::from("continue_restart_decision.blocking_row_ids"),
                    detail: String::from("continue decisions must not retain blocking row ids"),
                },
            );
        }
        if self.decision_state != "continue" && self.blocking_row_ids.is_empty() {
            return Err(
                PsionActualPretrainingContinueRestartDecisionError::MissingField {
                    field: String::from("continue_restart_decision.blocking_row_ids"),
                },
            );
        }
        ensure_nonempty(
            self.decision_reason.as_str(),
            "continue_restart_decision.decision_reason",
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "continue_restart_decision.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "continue_restart_decision.detail")?;
        if stable_continue_restart_decision_digest(self) != self.decision_digest {
            return Err(
                PsionActualPretrainingContinueRestartDecisionError::DigestMismatch {
                    field: String::from("continue_restart_decision.decision_digest"),
                },
            );
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn record_psion_actual_pretraining_checkpoint_comparison(
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    checkpoint_pointer_artifact: PsionActualPretrainingArtifactRef,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    checkpoint_manifest_artifact: PsionActualPretrainingArtifactRef,
    checkpoint_backup_receipt_artifact: Option<PsionActualPretrainingArtifactRef>,
    checkpoint_backup_receipt: Option<&PsionActualPretrainingCheckpointBackupReceipt>,
    checkpoint_eval_decision_artifact: Option<PsionActualPretrainingArtifactRef>,
    checkpoint_eval_decision: Option<&PsionActualPretrainingCheckpointEvalDecision>,
    checkpoint_eval_failure_artifact: Option<PsionActualPretrainingArtifactRef>,
    checkpoint_eval_failure: Option<&PsionActualPretrainingCheckpointEvalFailure>,
    hardware_qualification_artifact: PsionActualPretrainingArtifactRef,
    hardware_qualification: &PsionActualPretrainingHardwareQualification,
    run_shape_qualification_artifact: PsionActualPretrainingArtifactRef,
    run_shape_qualification: &PsionActualPretrainingRunShapeQualification,
    systems_bundle_artifact: PsionActualPretrainingArtifactRef,
    systems_bundle: &PsionActualPretrainingSystemsBundle,
    claim_boundary: &str,
    detail: &str,
) -> Result<
    PsionActualPretrainingCheckpointComparison,
    PsionActualPretrainingContinueRestartDecisionError,
> {
    checkpoint_pointer.validate().map_err(|error| {
        PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
            surface: String::from("checkpoint_pointer"),
            detail: error.to_string(),
        }
    })?;
    if checkpoint_pointer.pointer_state != "accepted" {
        return Err(
            PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                field: String::from("checkpoint_pointer.pointer_state"),
                detail: String::from(
                    "continue-restart comparison requires an accepted checkpoint pointer",
                ),
            },
        );
    }
    if checkpoint_pointer.optimizer_step == 0 {
        return Err(
            PsionActualPretrainingContinueRestartDecisionError::MissingField {
                field: String::from("checkpoint_pointer.optimizer_step"),
            },
        );
    }
    let checkpoint_ref = checkpoint_pointer.checkpoint_ref.clone().ok_or_else(|| {
        PsionActualPretrainingContinueRestartDecisionError::MissingField {
            field: String::from("checkpoint_pointer.checkpoint_ref"),
        }
    })?;
    hardware_qualification.validate().map_err(|error| {
        PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
            surface: String::from("hardware_qualification"),
            detail: error.to_string(),
        }
    })?;
    run_shape_qualification.validate().map_err(|error| {
        PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
            surface: String::from("run_shape_qualification"),
            detail: error.to_string(),
        }
    })?;
    systems_bundle.validate().map_err(|error| {
        PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
            surface: String::from("systems_bundle"),
            detail: error.to_string(),
        }
    })?;
    if let Some(receipt) = checkpoint_backup_receipt {
        receipt.validate().map_err(|error| {
            PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
                surface: String::from("checkpoint_backup_receipt"),
                detail: error.to_string(),
            }
        })?;
    }
    if let Some(decision) = checkpoint_eval_decision {
        decision.validate().map_err(|error| {
            PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
                surface: String::from("checkpoint_eval_decision"),
                detail: error.to_string(),
            }
        })?;
    }
    if let Some(failure) = checkpoint_eval_failure {
        failure.validate().map_err(|error| {
            PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
                surface: String::from("checkpoint_eval_failure"),
                detail: error.to_string(),
            }
        })?;
    }

    let throughput_anchor = systems_bundle
        .throughput_baselines
        .iter()
        .find(|baseline| baseline.baseline_kind == "trusted_cluster_anchor")
        .ok_or_else(
            || PsionActualPretrainingContinueRestartDecisionError::MissingField {
                field: String::from("systems_bundle.throughput_baselines[trusted_cluster_anchor]"),
            },
        )?;
    let continue_tokens_floor_bps = 9_000;
    let continue_step_latency_ceiling_bps = 11_500;
    let continue_checkpoint_write_floor_bps = 9_000;
    let continue_dataloader_stall_ceiling = 1_u64;

    let throughput_ratio_bps = ratio_bps(
        run_shape_qualification
            .throughput_probe
            .observed_tokens_per_second,
        throughput_anchor.mean_tokens_per_second,
    );
    let step_latency_ratio_bps = ratio_bps(
        run_shape_qualification
            .throughput_probe
            .observed_step_latency_ms,
        throughput_anchor.mean_step_latency_ms,
    );
    let checkpoint_write_ratio_bps = ratio_bps(
        run_shape_qualification
            .throughput_probe
            .observed_checkpoint_write_throughput_bytes_per_second,
        throughput_anchor.checkpoint_write_throughput_bytes_per_second,
    );

    let mut comparison_rows = Vec::new();
    comparison_rows.push(comparison_row(
        "checkpoint_eval_receipt",
        "checkpoint_eval_receipt",
        "exact_match",
        String::from("decision_available"),
        match (checkpoint_eval_decision, checkpoint_eval_failure) {
            (Some(_), None) => String::from("decision_available"),
            (None, Some(failure)) => format!("failure:{}", failure.failure_kind),
            (Some(_), Some(_)) => String::from("conflicting_decision_and_failure_receipts"),
            (None, None) => String::from("missing"),
        },
        checkpoint_eval_decision.is_some() && checkpoint_eval_failure.is_none(),
        "Actual-lane continue-restart decisions require one clear retained checkpoint-eval outcome before the operator should trust the latest checkpoint.",
    ));
    if let Some(eval_decision) = checkpoint_eval_decision {
        comparison_rows.push(comparison_row(
            "checkpoint_eval_decision_state",
            "checkpoint_eval_decision_state",
            "exact_match",
            String::from("continue"),
            eval_decision.decision_state.clone(),
            eval_decision.decision_state == "continue",
            "Automatic checkpoint review must stay green for uninterrupted long-run continuation.",
        ));
        comparison_rows.push(comparison_row(
            "checkpoint_eval_pass_rate_bps",
            "checkpoint_eval_pass_rate_bps",
            "min_floor",
            String::from("10000"),
            eval_decision.aggregate_pass_rate_bps.to_string(),
            eval_decision.aggregate_pass_rate_bps >= 10_000,
            "The continue threshold requires every frozen checkpoint-eval gate to stay above its admitted floor.",
        ));
        comparison_rows.push(comparison_row(
            "checkpoint_eval_score_bps",
            "checkpoint_eval_score_bps",
            "min_floor",
            String::from("8000"),
            eval_decision.aggregate_score_bps.to_string(),
            eval_decision.aggregate_score_bps >= 8_000,
            "The restart threshold stays tied to the bounded aggregate score floor already emitted by the checkpoint-eval surface.",
        ));
    }
    comparison_rows.push(comparison_row(
        "checkpoint_backup_state",
        "checkpoint_backup_state",
        "exact_match",
        String::from("backed_up"),
        checkpoint_backup_receipt
            .map(|receipt| receipt.backup_state.clone())
            .unwrap_or_else(|| String::from("missing")),
        checkpoint_backup_receipt.is_some_and(|receipt| receipt.backup_state == "backed_up"),
        "The operator should not continue or restart from a checkpoint that lacks one durable backup receipt.",
    ));
    comparison_rows.push(comparison_row(
        "hardware_admission_state",
        "hardware_admission_state",
        "exact_match",
        String::from("admitted"),
        hardware_qualification.admission_state.clone(),
        hardware_qualification.admission_state == "admitted",
        "The long-run decision path reuses the frozen hardware admission receipt rather than assuming the machine stayed healthy.",
    ));
    comparison_rows.push(comparison_row(
        "run_shape_admission_state",
        "run_shape_admission_state",
        "exact_match",
        String::from("admitted"),
        run_shape_qualification.admission_state.clone(),
        run_shape_qualification.admission_state == "admitted",
        "The long-run decision path reuses the retained run-shape qualification instead of drifting away from the admitted topology and dataloader contract.",
    ));
    comparison_rows.push(comparison_row(
        "throughput_anchor_ratio_bps",
        "throughput_anchor_ratio_bps",
        "min_floor",
        continue_tokens_floor_bps.to_string(),
        throughput_ratio_bps.to_string(),
        throughput_ratio_bps >= continue_tokens_floor_bps,
        "Continue decisions keep throughput within 90% of the trusted-cluster anchor before the operator treats the run as healthy.",
    ));
    comparison_rows.push(comparison_row(
        "step_latency_anchor_ratio_bps",
        "step_latency_anchor_ratio_bps",
        "max_ceiling",
        continue_step_latency_ceiling_bps.to_string(),
        step_latency_ratio_bps.to_string(),
        step_latency_ratio_bps <= continue_step_latency_ceiling_bps,
        "Continue decisions require step latency to stay within 115% of the trusted-cluster anchor.",
    ));
    comparison_rows.push(comparison_row(
        "checkpoint_write_ratio_bps",
        "checkpoint_write_ratio_bps",
        "min_floor",
        continue_checkpoint_write_floor_bps.to_string(),
        checkpoint_write_ratio_bps.to_string(),
        checkpoint_write_ratio_bps >= continue_checkpoint_write_floor_bps,
        "Continue decisions keep checkpoint write throughput inside the healthy band so backup and resume remain credible.",
    ));
    comparison_rows.push(comparison_row(
        "dataloader_stall_count",
        "dataloader_stall_count",
        "max_ceiling",
        continue_dataloader_stall_ceiling.to_string(),
        run_shape_qualification
            .dataloader_probe
            .observed_stall_count
            .to_string(),
        run_shape_qualification.dataloader_probe.observed_stall_count
            <= continue_dataloader_stall_ceiling,
        "Continue decisions keep dataloader stalls below the bounded investigation ceiling instead of tolerating slow degradation.",
    ));

    let comparison_readiness_state = derive_comparison_readiness_state(
        checkpoint_eval_decision,
        checkpoint_eval_failure,
        &comparison_rows,
    );

    let mut comparison = PsionActualPretrainingCheckpointComparison {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CHECKPOINT_COMPARISON_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: checkpoint_pointer.run_id.clone(),
        trigger_surface_id: String::from(CONTINUE_RESTART_TRIGGER_SURFACE_ID),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        checkpoint_label: checkpoint_pointer.checkpoint_label.clone(),
        optimizer_step: checkpoint_pointer.optimizer_step,
        checkpoint_ref,
        checkpoint_pointer: checkpoint_pointer_artifact,
        checkpoint_manifest: checkpoint_manifest_artifact,
        checkpoint_backup_receipt: checkpoint_backup_receipt_artifact,
        checkpoint_eval_decision: checkpoint_eval_decision_artifact,
        checkpoint_eval_failure: checkpoint_eval_failure_artifact,
        hardware_qualification: hardware_qualification_artifact,
        run_shape_qualification: run_shape_qualification_artifact,
        systems_bundle: systems_bundle_artifact,
        comparison_rows,
        checkpoint_readiness_state: String::from(comparison_readiness_state),
        claim_boundary: String::from(claim_boundary),
        detail: String::from(detail),
        comparison_digest: String::new(),
    };
    comparison.comparison_digest = stable_checkpoint_comparison_digest(&comparison);
    comparison.validate()?;
    Ok(comparison)
}

#[allow(clippy::too_many_arguments)]
pub fn record_psion_actual_pretraining_continue_restart_decision(
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    checkpoint_comparison_artifact: PsionActualPretrainingArtifactRef,
    checkpoint_comparison: &PsionActualPretrainingCheckpointComparison,
    checkpoint_backup_receipt_artifact: Option<PsionActualPretrainingArtifactRef>,
    checkpoint_eval_decision_artifact: Option<PsionActualPretrainingArtifactRef>,
    checkpoint_eval_decision: Option<&PsionActualPretrainingCheckpointEvalDecision>,
    checkpoint_eval_failure_artifact: Option<PsionActualPretrainingArtifactRef>,
    checkpoint_eval_failure: Option<&PsionActualPretrainingCheckpointEvalFailure>,
    hardware_qualification_artifact: PsionActualPretrainingArtifactRef,
    run_shape_qualification_artifact: PsionActualPretrainingArtifactRef,
    systems_bundle_artifact: PsionActualPretrainingArtifactRef,
    claim_boundary: &str,
    detail: &str,
) -> Result<
    PsionActualPretrainingContinueRestartDecision,
    PsionActualPretrainingContinueRestartDecisionError,
> {
    checkpoint_pointer.validate().map_err(|error| {
        PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
            surface: String::from("checkpoint_pointer"),
            detail: error.to_string(),
        }
    })?;
    checkpoint_comparison.validate().map_err(|error| {
        PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
            surface: String::from("checkpoint_comparison"),
            detail: error.to_string(),
        }
    })?;
    if let Some(decision) = checkpoint_eval_decision {
        decision.validate().map_err(|error| {
            PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
                surface: String::from("checkpoint_eval_decision"),
                detail: error.to_string(),
            }
        })?;
    }
    if let Some(failure) = checkpoint_eval_failure {
        failure.validate().map_err(|error| {
            PsionActualPretrainingContinueRestartDecisionError::UpstreamValidation {
                surface: String::from("checkpoint_eval_failure"),
                detail: error.to_string(),
            }
        })?;
    }
    let blocking_row_ids: Vec<String> = checkpoint_comparison
        .comparison_rows
        .iter()
        .filter(|row| row.status == "red")
        .map(|row| row.row_id.clone())
        .collect();
    let (decision_state, operator_action, decision_reason) = derive_continue_restart_outcome(
        checkpoint_comparison,
        checkpoint_eval_decision,
        checkpoint_eval_failure,
        &blocking_row_ids,
    );
    let checkpoint_ref = checkpoint_pointer.checkpoint_ref.clone().ok_or_else(|| {
        PsionActualPretrainingContinueRestartDecisionError::MissingField {
            field: String::from("checkpoint_pointer.checkpoint_ref"),
        }
    })?;
    let mut decision = PsionActualPretrainingContinueRestartDecision {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CONTINUE_RESTART_DECISION_SCHEMA_VERSION,
        ),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: checkpoint_pointer.run_id.clone(),
        trigger_surface_id: String::from(CONTINUE_RESTART_TRIGGER_SURFACE_ID),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        checkpoint_label: checkpoint_pointer.checkpoint_label.clone(),
        optimizer_step: checkpoint_pointer.optimizer_step,
        checkpoint_ref,
        checkpoint_comparison: checkpoint_comparison_artifact,
        checkpoint_eval_decision: checkpoint_eval_decision_artifact,
        checkpoint_eval_failure: checkpoint_eval_failure_artifact,
        checkpoint_backup_receipt: checkpoint_backup_receipt_artifact,
        hardware_qualification: hardware_qualification_artifact,
        run_shape_qualification: run_shape_qualification_artifact,
        systems_bundle: systems_bundle_artifact,
        decision_state: String::from(decision_state),
        operator_action: String::from(operator_action),
        blocking_row_ids: if decision_state == "continue" {
            Vec::new()
        } else {
            blocking_row_ids
        },
        decision_reason,
        claim_boundary: String::from(claim_boundary),
        detail: String::from(detail),
        decision_digest: String::new(),
    };
    decision.decision_digest = stable_continue_restart_decision_digest(&decision);
    decision.validate()?;
    Ok(decision)
}

#[must_use]
pub fn checkpoint_comparison_relative_path(optimizer_step: u64) -> String {
    format!("decisions/checkpoint_comparison_step-{optimizer_step}.json")
}

#[must_use]
pub fn continue_restart_decision_relative_path(optimizer_step: u64) -> String {
    format!("decisions/continue_restart_decision_step-{optimizer_step}.json")
}

fn comparison_row(
    row_id: &str,
    signal_kind: &str,
    comparison_kind: &str,
    expected_value: String,
    observed_value: String,
    is_green: bool,
    detail: &str,
) -> PsionActualPretrainingCheckpointComparisonRow {
    PsionActualPretrainingCheckpointComparisonRow {
        row_id: String::from(row_id),
        signal_kind: String::from(signal_kind),
        comparison_kind: String::from(comparison_kind),
        expected_value,
        observed_value,
        status: String::from(if is_green { "green" } else { "red" }),
        detail: String::from(detail),
    }
}

fn derive_comparison_readiness_state(
    checkpoint_eval_decision: Option<&PsionActualPretrainingCheckpointEvalDecision>,
    checkpoint_eval_failure: Option<&PsionActualPretrainingCheckpointEvalFailure>,
    comparison_rows: &[PsionActualPretrainingCheckpointComparisonRow],
) -> &'static str {
    let red_row_ids: BTreeSet<_> = comparison_rows
        .iter()
        .filter(|row| row.status == "red")
        .map(|row| row.row_id.as_str())
        .collect();
    if checkpoint_eval_failure.is_some()
        || checkpoint_eval_decision.is_none()
        || red_row_ids.contains("checkpoint_backup_state")
        || red_row_ids.contains("hardware_admission_state")
        || red_row_ids.contains("run_shape_admission_state")
        || red_row_ids.contains("throughput_anchor_ratio_bps")
        || red_row_ids.contains("step_latency_anchor_ratio_bps")
        || red_row_ids.contains("checkpoint_write_ratio_bps")
        || red_row_ids.contains("dataloader_stall_count")
    {
        return "hold_for_investigation";
    }
    match checkpoint_eval_decision.map(|decision| decision.decision_state.as_str()) {
        Some("continue") => "ready_to_continue",
        Some("restart_from_last_accepted_checkpoint") => "ready_to_restart",
        _ => "hold_for_investigation",
    }
}

fn derive_continue_restart_outcome(
    checkpoint_comparison: &PsionActualPretrainingCheckpointComparison,
    checkpoint_eval_decision: Option<&PsionActualPretrainingCheckpointEvalDecision>,
    checkpoint_eval_failure: Option<&PsionActualPretrainingCheckpointEvalFailure>,
    blocking_row_ids: &[String],
) -> (&'static str, &'static str, String) {
    match checkpoint_comparison.checkpoint_readiness_state.as_str() {
        "ready_to_continue" => (
            "continue",
            "continue_long_run",
            String::from(
                "Automatic checkpoint review stayed green, durable backup is present, and the retained hardware and run-shape receipts remain inside the continue threshold.",
            ),
        ),
        "ready_to_restart" => (
            "restart_from_last_accepted_checkpoint",
            "restart_from_latest_accepted_checkpoint",
            String::from(
                "Checkpoint eval explicitly fell onto the restart branch while durable backup and retained system receipts stayed green enough to trust the latest accepted checkpoint for restart.",
            ),
        ),
        _ => {
            if let Some(failure) = checkpoint_eval_failure {
                return (
                    "hold_and_investigate",
                    "pause_and_review",
                    format!(
                        "Checkpoint eval evidence is not trustworthy yet because the retained failure `{}` is still active; operator review must land before continuing or restarting.",
                        failure.failure_kind
                    ),
                );
            }
            if checkpoint_eval_decision.is_none() {
                return (
                    "hold_and_investigate",
                    "pause_and_review",
                    String::from(
                        "Continue-restart posture is blocked because the latest accepted checkpoint has no retained eval decision yet.",
                    ),
                );
            }
            if checkpoint_eval_decision
                .is_some_and(|decision| decision.decision_state == "hold_and_review")
            {
                return (
                    "hold_and_investigate",
                    "pause_and_review",
                    String::from(
                        "Checkpoint eval explicitly requested hold-and-review, so the actual lane must pause for operator investigation instead of guessing.",
                    ),
                );
            }
            (
                "hold_and_investigate",
                "pause_and_review",
                format!(
                    "Continue-restart posture is blocked by retained comparison rows: {}.",
                    blocking_row_ids.join(", ")
                ),
            )
        }
    }
}

fn ratio_bps(observed: u64, baseline: u64) -> u32 {
    ((observed as u128).saturating_mul(10_000) / baseline as u128) as u32
}

fn stable_checkpoint_comparison_digest(
    comparison: &PsionActualPretrainingCheckpointComparison,
) -> String {
    let mut copy = comparison.clone();
    copy.comparison_digest.clear();
    stable_digest(b"psion_actual_pretraining_checkpoint_comparison|", &copy)
}

fn stable_continue_restart_decision_digest(
    decision: &PsionActualPretrainingContinueRestartDecision,
) -> String {
    let mut copy = decision.clone();
    copy.decision_digest.clear();
    stable_digest(
        b"psion_actual_pretraining_continue_restart_decision|",
        &copy,
    )
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let canonical =
        serde_json::to_vec(value).expect("continue-restart decision receipts should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&canonical);
    hex::encode(hasher.finalize())
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
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    ensure_exact(lane_id, "lane_id", PSION_ACTUAL_PRETRAINING_LANE_ID)?;
    ensure_nonempty(run_id, "run_id")?;
    ensure_exact(
        trigger_surface_id,
        "trigger_surface_id",
        CONTINUE_RESTART_TRIGGER_SURFACE_ID,
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
        return Err(
            PsionActualPretrainingContinueRestartDecisionError::MissingField {
                field: String::from("optimizer_step"),
            },
        );
    }
    ensure_nonempty(checkpoint_ref, "checkpoint_ref")?;
    Ok(())
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_optional_artifact_ref(
    artifact: Option<&PsionActualPretrainingArtifactRef>,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    if let Some(artifact) = artifact {
        ensure_artifact_ref(artifact, field_prefix)?;
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    if actual != expected {
        return Err(
            PsionActualPretrainingContinueRestartDecisionError::FieldMismatch {
                field: String::from(field),
                expected: String::from(expected),
                actual: String::from(actual),
            },
        );
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    if value.trim().is_empty() {
        return Err(
            PsionActualPretrainingContinueRestartDecisionError::MissingField {
                field: String::from(field),
            },
        );
    }
    Ok(())
}

fn ensure_git_sha(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    ensure_nonempty(value, field)?;
    if value.len() != 40 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(
            PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                field: String::from(field),
                detail: String::from("git commit SHA must be a 40-character hex string"),
            },
        );
    }
    Ok(())
}

fn ensure_dirty_tree_admission(
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<&str>,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    match dirty_tree_admission {
        "refuse_by_default" => Ok(()),
        "allowed_by_operator_override" => ensure_nonempty_option(
            workspace_status_sha256,
            &format!("{field_prefix}.workspace_status_sha256"),
        ),
        _ => Err(
            PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                field: String::from(field_prefix),
                detail: String::from(
                    "dirty-tree admission must be refuse_by_default or allowed_by_operator_override",
                ),
            },
        ),
    }
}

fn ensure_nonempty_option(
    value: Option<&str>,
    field: &str,
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    match value {
        Some(value) => ensure_nonempty(value, field),
        None => Err(
            PsionActualPretrainingContinueRestartDecisionError::MissingField {
                field: String::from(field),
            },
        ),
    }
}

fn ensure_unique_nonempty_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionActualPretrainingContinueRestartDecisionError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(
                PsionActualPretrainingContinueRestartDecisionError::InvalidValue {
                    field: String::from(field),
                    detail: format!("duplicate value `{value}`"),
                },
            );
        }
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingContinueRestartDecisionError {
    #[error("psion actual-pretraining continue-restart receipt is missing field `{field}`")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining continue-restart field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining continue-restart field `{field}` is invalid: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("psion actual-pretraining continue-restart digest drifted for `{field}`")]
    DigestMismatch { field: String },
    #[error("psion actual-pretraining continue-restart has duplicate row `{row_id}`")]
    DuplicateRow { row_id: String },
    #[error("psion actual-pretraining continue-restart upstream `{surface}` is invalid: {detail}")]
    UpstreamValidation { surface: String, detail: String },
}

#[cfg(test)]
mod tests {
    use super::{
        PsionActualPretrainingCheckpointComparison, PsionActualPretrainingContinueRestartDecision,
    };

    fn checkpoint_comparison_fixture() -> PsionActualPretrainingCheckpointComparison {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_comparison_v1.json"
        ))
        .expect("checkpoint comparison fixture should parse")
    }

    fn continue_restart_decision_fixture() -> PsionActualPretrainingContinueRestartDecision {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_continue_restart_decision_v1.json"
        ))
        .expect("continue-restart decision fixture should parse")
    }

    #[test]
    fn checkpoint_comparison_fixture_validates() {
        checkpoint_comparison_fixture()
            .validate()
            .expect("checkpoint comparison fixture should validate");
    }

    #[test]
    fn continue_restart_decision_fixture_validates() {
        continue_restart_decision_fixture()
            .validate()
            .expect("continue-restart decision fixture should validate");
    }
}
