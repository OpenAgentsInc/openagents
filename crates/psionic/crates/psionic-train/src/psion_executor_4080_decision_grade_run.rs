use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_remote_training_run_index_v2, build_remote_training_visualization_bundle_v2,
    PsionExecutor4080DurableCheckpointPacket, PsionExecutor4080FrequentEvalAttachmentPacket,
    PsionExecutor4080InterruptionRecoveryPacket, PsionExecutor4080RemoteLaunchPacket,
    PsionExecutor4080SmokeRunPacket, RemoteTrainingArtifactSourceKind,
    RemoteTrainingComparabilityClassV2, RemoteTrainingDistributedSample,
    RemoteTrainingEmissionMode, RemoteTrainingEventSample, RemoteTrainingEventSeverity,
    RemoteTrainingExecutionClassV2, RemoteTrainingHeartbeatSample, RemoteTrainingLossSample,
    RemoteTrainingMathSample, RemoteTrainingProofPostureV2, RemoteTrainingProvider,
    RemoteTrainingPublicEquivalenceClassV2, RemoteTrainingRefreshContract,
    RemoteTrainingResultClassification, RemoteTrainingRunIndexEntryV2, RemoteTrainingRunIndexV2,
    RemoteTrainingRuntimeSample, RemoteTrainingSeriesStatus, RemoteTrainingSourceArtifact,
    RemoteTrainingTimelineEntry, RemoteTrainingTrackFamilyV2, RemoteTrainingTrackSemanticsV2,
    RemoteTrainingVisualizationBundleV2, RemoteTrainingVisualizationError,
    RemoteTrainingVisualizationSummary, PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH,
    PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH,
    PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH,
    PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH, PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH,
    REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
};

/// Stable schema version for the executor-lane 4080 decision-grade packet.
pub const PSION_EXECUTOR_4080_DECISION_GRADE_RUN_SCHEMA_VERSION: &str =
    "psion.executor.4080_decision_grade_run.v1";
/// Canonical fixture path for the executor-lane 4080 decision-grade packet.
pub const PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_4080_decision_grade_run_v1.json";
/// Canonical doc path for the executor-lane 4080 decision-grade packet.
pub const PSION_EXECUTOR_4080_DECISION_GRADE_RUN_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_DECISION_GRADE_RUN.md";
/// Canonical track-aware visualization bundle path for the retained 4080 decision-grade run.
pub const PSION_EXECUTOR_4080_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/training_visualization/psion_executor_4080_decision_grade_remote_training_visualization_bundle_v2.json";
/// Canonical track-aware run-index path for the retained 4080 decision-grade run.
pub const PSION_EXECUTOR_4080_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH: &str =
    "fixtures/training_visualization/psion_executor_4080_decision_grade_remote_training_run_index_v2.json";

const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const LOCAL_TAILNET_CONTROL_PROFILE_ID: &str = "local_tailnet_cluster_control_plane";
const DECISION_LANE_ID: &str = "psion_executor_4080_decision_grade";
const DECISION_TRACK_ID: &str = "psion.executor.4080_decision_grade.v1";
const DECISION_REGISTRATION_ID: &str = "psion_executor_4080_decision_grade_registration_v1";
const DECISION_REVIEW_ID: &str = "psion_executor_4080_weekly_ablation_review_v1";
const DECISION_EQUIVALENT_SUBSET_ID: &str =
    "psion.executor.4080_decision_grade_equivalent_subset.v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ACCEPTANCE_PROFILE.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE_TRUTH.md";
const PSION_EXECUTOR_DECISION_THRESHOLDS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_DECISION_THRESHOLDS.md";
const PSION_EXECUTOR_OWNERSHIP_DOC_PATH: &str = "docs/PSION_EXECUTOR_OWNERSHIP.md";
const REMOTE_TRAINING_RUN_INDEX_V2_FIXTURE_PATH: &str =
    "fixtures/training_visualization/remote_training_run_index_v2.json";
const DEVICE_MATRIX_REPORT_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/matrix_report.json";
const CUDA_REPORT_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/archlinux_cuda/report.json";
const CUDA_BUNDLE_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/archlinux_cuda/portable_bundle.safetensors";
const PROMOTION_PACK_ID: &str = "tassadar.eval.promotion.v0";

#[derive(Clone, Debug, Deserialize)]
struct OpenAdapterSameNodeWallclockBenchmarkReport {
    schema_version: String,
    host: String,
    backend_label: String,
    logical_device_kind: String,
    logical_device_label: String,
    target_wallclock_seconds: u64,
    retained_run: OpenAdapterSameNodeRetainedRunReport,
}

#[derive(Clone, Debug, Deserialize)]
struct OpenAdapterSameNodeRetainedRunReport {
    run_id: String,
    checkpoint_family: String,
    completed_steps: u64,
    observed_wallclock_ms: u64,
    steps_per_second: f64,
    samples_per_second: f64,
    source_tokens_per_second: f64,
    batch_count: u64,
    sample_count: u64,
    initial_mean_loss: f32,
    final_mean_loss: f32,
    loss_delta: f32,
    final_state_dict_digest: String,
    bundle_artifact_path: String,
}

#[derive(Clone, Debug, Deserialize)]
struct OpenAdapterTailnetAdmittedDeviceMatrixReport {
    schema_version: String,
    run_id: String,
    git_ref: String,
    target_wallclock_seconds: u64,
    local_report: OpenAdapterSameNodeWallclockBenchmarkReport,
    remote_report: OpenAdapterSameNodeWallclockBenchmarkReport,
    comparison: OpenAdapterTailnetAdmittedDeviceMatrixComparison,
    claim_boundary: String,
}

#[derive(Clone, Debug, Deserialize)]
struct OpenAdapterTailnetAdmittedDeviceMatrixComparison {
    steps_per_second_gain_pct_local_over_remote: f64,
    samples_per_second_gain_pct_local_over_remote: f64,
    source_tokens_per_second_gain_pct_local_over_remote: f64,
    local_to_remote_steps_ratio: f64,
    local_to_remote_loss_delta_gap: f64,
}

/// One retained run-registration row that seeds the later canonical ledger schema.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080DecisionGradeRunRegistrationRow {
    /// Stable registration id.
    pub registration_id: String,
    /// Retained same-node CUDA decision run id.
    pub decision_run_id: String,
    /// Supporting Tailnet cluster run id.
    pub cluster_support_run_id: String,
    /// Admitted profile ids bound to the run.
    pub admitted_profile_ids: Vec<String>,
    /// Frozen eval-pack ids bound to the run.
    pub eval_pack_ids: Vec<String>,
    /// Explicit checkpoint-evidence posture.
    pub checkpoint_evidence_mode: String,
    /// Declared wallclock budget in seconds.
    pub wallclock_budget_seconds: u64,
    /// Honest detail.
    pub detail: String,
    /// Stable registration digest.
    pub registration_digest: String,
}

/// One retained weekly ablation review row for the first 4080 decision-grade run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080DecisionGradeWeeklyReviewRow {
    /// Stable review id.
    pub review_id: String,
    /// Stable cadence id.
    pub cadence_id: String,
    /// Named reviewer role from the ownership doc.
    pub reviewer_role: String,
    /// Named reviewer identity.
    pub reviewer_identity: String,
    /// Final review status.
    pub status: String,
    /// Final review decision.
    pub decision: String,
    /// Covered gate ids.
    pub covered_gate_ids: Vec<String>,
    /// Honest detail.
    pub detail: String,
    /// Stable review digest.
    pub review_digest: String,
}

/// One approved equivalent subset that stands in for explicit multi-checkpoint repetition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080DecisionGradeEquivalentSubset {
    /// Stable subset id.
    pub subset_id: String,
    /// Required gate ids.
    pub required_gate_ids: Vec<String>,
    /// Stable subset digest.
    pub subset_digest: String,
    /// Honest detail.
    pub detail: String,
}

/// One retained gate row for the 4080 decision-grade packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080DecisionGradeGateRow {
    /// Stable gate id.
    pub gate_id: String,
    /// Final status.
    pub status: String,
    /// Honest detail.
    pub detail: String,
}

/// Typed packet proving the first admitted 4080 decision-grade run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutor4080DecisionGradeRunPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted worker profile id.
    pub worker_profile_id: String,
    /// Admitted control-plane profile id.
    pub control_plane_profile_id: String,
    /// Stable decision lane id.
    pub decision_lane_id: String,
    /// Honest question posture.
    pub decision_question_posture: String,
    /// Prerequisite remote-launch packet reference.
    pub remote_launch_packet_ref: String,
    /// Stable SHA256 over the remote-launch packet bytes.
    pub remote_launch_packet_sha256: String,
    /// Prerequisite durable-checkpoint packet reference.
    pub durable_checkpoint_packet_ref: String,
    /// Stable SHA256 over the durable-checkpoint packet bytes.
    pub durable_checkpoint_packet_sha256: String,
    /// Prerequisite frequent-eval packet reference.
    pub frequent_eval_packet_ref: String,
    /// Stable SHA256 over the frequent-eval packet bytes.
    pub frequent_eval_packet_sha256: String,
    /// Prerequisite smoke packet reference.
    pub smoke_packet_ref: String,
    /// Stable SHA256 over the smoke packet bytes.
    pub smoke_packet_sha256: String,
    /// Prerequisite interruption-recovery packet reference.
    pub recovery_packet_ref: String,
    /// Stable SHA256 over the recovery packet bytes.
    pub recovery_packet_sha256: String,
    /// Retained device-matrix report reference.
    pub retained_matrix_report_ref: String,
    /// Stable SHA256 over the device-matrix report bytes.
    pub retained_matrix_report_sha256: String,
    /// Retained remote CUDA report reference.
    pub retained_remote_report_ref: String,
    /// Stable SHA256 over the remote CUDA report bytes.
    pub retained_remote_report_sha256: String,
    /// Retained remote CUDA bundle reference.
    pub retained_remote_bundle_ref: String,
    /// Stable SHA256 over the remote CUDA bundle bytes.
    pub retained_remote_bundle_sha256: String,
    /// Retained visualization bundle reference.
    pub visualization_bundle_ref: String,
    /// Stable SHA256 over the visualization bundle bytes.
    pub visualization_bundle_sha256: String,
    /// Stable visualization bundle digest.
    pub visualization_bundle_digest: String,
    /// Retained run-index reference.
    pub visualization_run_index_ref: String,
    /// Stable SHA256 over the run-index bytes.
    pub visualization_run_index_sha256: String,
    /// Stable run-index digest.
    pub visualization_run_index_digest: String,
    /// Retained run-registration row.
    pub run_registration_row: PsionExecutor4080DecisionGradeRunRegistrationRow,
    /// Retained weekly ablation review row.
    pub weekly_ablation_review_row: PsionExecutor4080DecisionGradeWeeklyReviewRow,
    /// Approved equivalent checkpoint subset.
    pub approved_equivalent_checkpoint_subset: PsionExecutor4080DecisionGradeEquivalentSubset,
    /// Retained device-matrix run id.
    pub decision_matrix_run_id: String,
    /// Retained CUDA decision run id.
    pub decision_run_id: String,
    /// Supporting Tailnet cluster run id.
    pub cluster_support_run_id: String,
    /// Stable backend label.
    pub execution_backend_label: String,
    /// Stable logical-device label.
    pub logical_device_label: String,
    /// Cluster support checkpoint family.
    pub cluster_checkpoint_family: String,
    /// Cluster support checkpoint pointer digest.
    pub cluster_checkpoint_pointer_digest: String,
    /// Cluster support checkpoint ref.
    pub cluster_checkpoint_ref: String,
    /// Cluster support checkpoint step.
    pub cluster_checkpoint_step: u64,
    /// Decision-run checkpoint family.
    pub decision_checkpoint_family: String,
    /// Completed steps from the retained CUDA run.
    pub completed_steps: u64,
    /// Observed wallclock milliseconds from the retained CUDA run.
    pub observed_wallclock_ms: u64,
    /// Final mean loss from the retained CUDA run.
    pub final_mean_loss: f32,
    /// Final state-dict digest from the retained CUDA run.
    pub final_state_dict_digest: String,
    /// Total entries in the retained run index.
    pub dashboard_entry_count: u64,
    /// Retained gate rows.
    pub gate_rows: Vec<PsionExecutor4080DecisionGradeGateRow>,
    /// Support references.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutor4080DecisionGradeRunPacket {
    /// Validate the retained 4080 decision-grade packet.
    pub fn validate(&self) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_4080_decision_grade_run.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_4080_DECISION_GRADE_RUN_SCHEMA_VERSION {
            return Err(
                PsionExecutor4080DecisionGradeRunError::SchemaVersionMismatch {
                    expected: String::from(PSION_EXECUTOR_4080_DECISION_GRADE_RUN_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_4080_decision_grade_run.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.worker_profile_id",
                self.worker_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.control_plane_profile_id",
                self.control_plane_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.decision_lane_id",
                self.decision_lane_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.decision_question_posture",
                self.decision_question_posture.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.remote_launch_packet_ref",
                self.remote_launch_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.remote_launch_packet_sha256",
                self.remote_launch_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.durable_checkpoint_packet_ref",
                self.durable_checkpoint_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.durable_checkpoint_packet_sha256",
                self.durable_checkpoint_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.frequent_eval_packet_ref",
                self.frequent_eval_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.frequent_eval_packet_sha256",
                self.frequent_eval_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.smoke_packet_ref",
                self.smoke_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.smoke_packet_sha256",
                self.smoke_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.recovery_packet_ref",
                self.recovery_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.recovery_packet_sha256",
                self.recovery_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.retained_matrix_report_ref",
                self.retained_matrix_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.retained_matrix_report_sha256",
                self.retained_matrix_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.retained_remote_report_ref",
                self.retained_remote_report_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.retained_remote_report_sha256",
                self.retained_remote_report_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.retained_remote_bundle_ref",
                self.retained_remote_bundle_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.retained_remote_bundle_sha256",
                self.retained_remote_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.visualization_bundle_ref",
                self.visualization_bundle_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.visualization_bundle_sha256",
                self.visualization_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.visualization_bundle_digest",
                self.visualization_bundle_digest.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.visualization_run_index_ref",
                self.visualization_run_index_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.visualization_run_index_sha256",
                self.visualization_run_index_sha256.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.visualization_run_index_digest",
                self.visualization_run_index_digest.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.decision_matrix_run_id",
                self.decision_matrix_run_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.decision_run_id",
                self.decision_run_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.cluster_support_run_id",
                self.cluster_support_run_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.execution_backend_label",
                self.execution_backend_label.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.logical_device_label",
                self.logical_device_label.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.cluster_checkpoint_family",
                self.cluster_checkpoint_family.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.cluster_checkpoint_pointer_digest",
                self.cluster_checkpoint_pointer_digest.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.cluster_checkpoint_ref",
                self.cluster_checkpoint_ref.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.decision_checkpoint_family",
                self.decision_checkpoint_family.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.final_state_dict_digest",
                self.final_state_dict_digest.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.summary",
                self.summary.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        self.run_registration_row.validate()?;
        self.weekly_ablation_review_row.validate()?;
        self.approved_equivalent_checkpoint_subset.validate()?;
        if self.dashboard_entry_count == 0 {
            return Err(PsionExecutor4080DecisionGradeRunError::InvalidValue {
                field: String::from("psion_executor_4080_decision_grade_run.dashboard_entry_count"),
                detail: String::from("dashboard entry count must stay positive"),
            });
        }
        if self.gate_rows.is_empty() {
            return Err(PsionExecutor4080DecisionGradeRunError::MissingField {
                field: String::from("psion_executor_4080_decision_grade_run.gate_rows"),
            });
        }
        for row in &self.gate_rows {
            row.validate()?;
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutor4080DecisionGradeRunError::MissingField {
                field: String::from("psion_executor_4080_decision_grade_run.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(
                support_ref.as_str(),
                "psion_executor_4080_decision_grade_run.support_refs[]",
            )?;
        }
        if stable_executor_4080_decision_grade_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutor4080DecisionGradeRunError::DigestMismatch {
                expected: stable_executor_4080_decision_grade_packet_digest(self),
                actual: self.packet_digest.clone(),
            });
        }
        Ok(())
    }
}

impl PsionExecutor4080DecisionGradeRunRegistrationRow {
    fn validate(&self) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        for (field, value) in [
            (
                "psion_executor_4080_decision_grade_run.run_registration_row.registration_id",
                self.registration_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.run_registration_row.decision_run_id",
                self.decision_run_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.run_registration_row.cluster_support_run_id",
                self.cluster_support_run_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.run_registration_row.checkpoint_evidence_mode",
                self.checkpoint_evidence_mode.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.run_registration_row.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.run_registration_row.registration_digest",
                self.registration_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.wallclock_budget_seconds == 0 {
            return Err(PsionExecutor4080DecisionGradeRunError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_decision_grade_run.run_registration_row.wallclock_budget_seconds",
                ),
                detail: String::from("wallclock budget seconds must stay positive"),
            });
        }
        if self.admitted_profile_ids.is_empty() || self.eval_pack_ids.is_empty() {
            return Err(PsionExecutor4080DecisionGradeRunError::MissingField {
                field: String::from(
                    "psion_executor_4080_decision_grade_run.run_registration_row.admitted_profile_ids/eval_pack_ids",
                ),
            });
        }
        if stable_executor_4080_decision_grade_registration_digest(self) != self.registration_digest
        {
            return Err(PsionExecutor4080DecisionGradeRunError::DigestMismatch {
                expected: stable_executor_4080_decision_grade_registration_digest(self),
                actual: self.registration_digest.clone(),
            });
        }
        Ok(())
    }
}

impl PsionExecutor4080DecisionGradeWeeklyReviewRow {
    fn validate(&self) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        for (field, value) in [
            (
                "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.review_id",
                self.review_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.cadence_id",
                self.cadence_id.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.reviewer_role",
                self.reviewer_role.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.reviewer_identity",
                self.reviewer_identity.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.status",
                self.status.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.decision",
                self.decision.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.review_digest",
                self.review_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.covered_gate_ids.is_empty() {
            return Err(PsionExecutor4080DecisionGradeRunError::MissingField {
                field: String::from(
                    "psion_executor_4080_decision_grade_run.weekly_ablation_review_row.covered_gate_ids",
                ),
            });
        }
        if stable_executor_4080_decision_grade_review_digest(self) != self.review_digest {
            return Err(PsionExecutor4080DecisionGradeRunError::DigestMismatch {
                expected: stable_executor_4080_decision_grade_review_digest(self),
                actual: self.review_digest.clone(),
            });
        }
        Ok(())
    }
}

impl PsionExecutor4080DecisionGradeEquivalentSubset {
    fn validate(&self) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        ensure_nonempty(
            self.subset_id.as_str(),
            "psion_executor_4080_decision_grade_run.approved_equivalent_checkpoint_subset.subset_id",
        )?;
        if self.required_gate_ids.is_empty() {
            return Err(PsionExecutor4080DecisionGradeRunError::MissingField {
                field: String::from(
                    "psion_executor_4080_decision_grade_run.approved_equivalent_checkpoint_subset.required_gate_ids",
                ),
            });
        }
        ensure_nonempty(
            self.subset_digest.as_str(),
            "psion_executor_4080_decision_grade_run.approved_equivalent_checkpoint_subset.subset_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_4080_decision_grade_run.approved_equivalent_checkpoint_subset.detail",
        )?;
        if stable_executor_4080_decision_grade_subset_digest(self) != self.subset_digest {
            return Err(PsionExecutor4080DecisionGradeRunError::DigestMismatch {
                expected: stable_executor_4080_decision_grade_subset_digest(self),
                actual: self.subset_digest.clone(),
            });
        }
        Ok(())
    }
}

impl PsionExecutor4080DecisionGradeGateRow {
    fn validate(&self) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        ensure_nonempty(
            self.gate_id.as_str(),
            "psion_executor_4080_decision_grade_run.gate_rows[].gate_id",
        )?;
        ensure_nonempty(
            self.status.as_str(),
            "psion_executor_4080_decision_grade_run.gate_rows[].status",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_4080_decision_grade_run.gate_rows[].detail",
        )?;
        Ok(())
    }
}

/// Validation failures for the executor-lane 4080 decision-grade packet.
#[derive(Debug, Error)]
pub enum PsionExecutor4080DecisionGradeRunError {
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` is invalid: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch: expected `{expected}` but found `{actual}`")]
    DigestMismatch { expected: String, actual: String },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to read `{path}`: {error}")]
    Read {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to parse json from `{path}`: {error}")]
    Parse {
        path: String,
        #[source]
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Visualization(#[from] RemoteTrainingVisualizationError),
}

/// Build the retained 4080 decision-grade visualization bundle.
pub fn builtin_executor_4080_decision_grade_visualization_bundle(
    workspace_root: &Path,
) -> Result<RemoteTrainingVisualizationBundleV2, PsionExecutor4080DecisionGradeRunError> {
    let matrix_report: OpenAdapterTailnetAdmittedDeviceMatrixReport =
        read_json(workspace_root, DEVICE_MATRIX_REPORT_PATH)?;
    let remote_report = matrix_report.remote_report.clone();

    let frequent_packet: PsionExecutor4080FrequentEvalAttachmentPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH,
    )?;
    let recovery_packet: PsionExecutor4080InterruptionRecoveryPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH,
    )?;

    let run_start_ms = 1_743_127_200_000_u64;
    let checkpoint_eval_ms = run_start_ms + 180_000;
    let recovery_review_ms = run_start_ms + 420_000;
    let end_ms = run_start_ms + remote_report.retained_run.observed_wallclock_ms;
    let steps_midpoint = remote_report.retained_run.completed_steps / 2;
    let midpoint_loss = (remote_report.retained_run.initial_mean_loss / 2.0)
        .max(remote_report.retained_run.final_mean_loss);
    let samples_per_second_milli =
        (remote_report.retained_run.samples_per_second * 1000.0).round() as u32;
    let tokens_per_second = remote_report.retained_run.source_tokens_per_second.round() as u64;

    build_remote_training_visualization_bundle_v2(RemoteTrainingVisualizationBundleV2 {
        schema_version: String::new(),
        bundle_id: String::from("psion-executor-4080-decision-grade-run-v2"),
        provider: RemoteTrainingProvider::LocalHybrid,
        profile_id: String::from(LOCAL_4080_PROFILE_ID),
        lane_id: String::from(DECISION_LANE_ID),
        run_id: remote_report.retained_run.run_id.clone(),
        repo_revision: matrix_report.git_ref.clone(),
        track_semantics: RemoteTrainingTrackSemanticsV2 {
            track_family: RemoteTrainingTrackFamilyV2::Psion,
            track_id: String::from(DECISION_TRACK_ID),
            execution_class: RemoteTrainingExecutionClassV2::HomeClusterMixedDevice,
            comparability_class: RemoteTrainingComparabilityClassV2::SameTrackComparable,
            proof_posture: RemoteTrainingProofPostureV2::RuntimeMeasured,
            public_equivalence_class: RemoteTrainingPublicEquivalenceClassV2::NotApplicable,
            score_law_ref: Some(String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH)),
            artifact_cap_bytes: None,
            wallclock_cap_seconds: Some(remote_report.target_wallclock_seconds),
            semantic_summary: String::from(
                "The admitted 4080 decision-grade lane binds one retained same-node CUDA wallclock run to the Mac -> 4080 -> Mac roundtrip packets, one explicit checkpoint-eval row, and one weekly ablation review row without pretending promotion or full multi-checkpoint repetition already exists.",
            ),
        },
        primary_score: None,
        score_surface: None,
        result_classification: RemoteTrainingResultClassification::CompletedSuccess,
        refresh_contract: RemoteTrainingRefreshContract {
            target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
            emission_mode: RemoteTrainingEmissionMode::PostRunOnly,
            last_heartbeat_at_ms: Some(end_ms),
            heartbeat_seq: 4,
        },
        series_status: RemoteTrainingSeriesStatus::Available,
        series_unavailable_reason: None,
        timeline: vec![
            RemoteTrainingTimelineEntry {
                observed_at_ms: run_start_ms,
                phase: String::from("launch"),
                subphase: Some(String::from("tailnet_control_plane")),
                detail: String::from(
                    "The admitted Mac control plane launched the bounded 4080 worker role and sealed the reachable Tailnet worker contract before the decision-grade packet was formed.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: checkpoint_eval_ms,
                phase: String::from("training"),
                subphase: Some(String::from("retained_cuda_same_node_run")),
                detail: format!(
                    "The retained CUDA run `{}` consumed most of the admitted 600-second budget on `cuda:0` and supplies the baseline-comparable accelerator facts for the first 4080 decision-grade packet.",
                    remote_report.retained_run.run_id,
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: recovery_review_ms,
                phase: String::from("review"),
                subphase: Some(String::from("checkpoint_and_recovery_binding")),
                detail: format!(
                    "The explicit frequent-pack ledger row `{}` and recovery packet `{}` stayed green on the supporting Tailnet cluster run instead of leaving checkpoint truth implied.",
                    frequent_packet.checkpoint_eval_row.ledger_row_id,
                    PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH,
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: end_ms,
                phase: String::from("review"),
                subphase: Some(String::from("weekly_ablation_review")),
                detail: String::from(
                    "The first 4080 decision-grade packet is sealed only after weekly ablation review confirms frozen-pack binding, baseline comparability, equivalent checkpoint evidence, and visibility in the shipped v2 dashboard grammar.",
                ),
            },
        ],
        summary: RemoteTrainingVisualizationSummary {
            total_steps_completed: remote_report.retained_run.completed_steps,
            latest_global_step: Some(remote_report.retained_run.completed_steps),
            latest_train_loss: Some(remote_report.retained_run.final_mean_loss),
            latest_ema_loss: None,
            latest_validation_loss: Some(remote_report.retained_run.final_mean_loss),
            latest_tokens_per_second: Some(tokens_per_second),
            latest_samples_per_second_milli: Some(samples_per_second_milli),
            accumulated_cost_microusd: None,
            latest_checkpoint_ref: Some(format!(
                "{}#{}",
                remote_report.retained_run.checkpoint_family, remote_report.retained_run.run_id
            )),
            detail: String::from(
                "The retained 4080 decision-grade packet surfaces one reviewable CUDA run with explicit Tailnet roundtrip, frequent-eval, interruption-recovery, and weekly-review lineage.",
            ),
        },
        heartbeat_series: vec![
            RemoteTrainingHeartbeatSample {
                observed_at_ms: run_start_ms,
                phase: String::from("launch"),
                subphase: Some(String::from("tailnet_control_plane")),
                step_in_progress: Some(0),
                microbatch_in_progress: None,
                active_subsystems: vec![String::from("tailnet"), String::from("worker_launch")],
                stale_after_ms: 5_000,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: checkpoint_eval_ms,
                phase: String::from("training"),
                subphase: Some(String::from("retained_cuda_same_node_run")),
                step_in_progress: Some(steps_midpoint),
                microbatch_in_progress: Some(1),
                active_subsystems: vec![String::from("cuda"), String::from("checkpoint")],
                stale_after_ms: recovery_packet.stale_worker_timeout_ms,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: recovery_review_ms,
                phase: String::from("review"),
                subphase: Some(String::from("frequent_eval_and_recovery")),
                step_in_progress: Some(remote_report.retained_run.completed_steps),
                microbatch_in_progress: None,
                active_subsystems: vec![String::from("validator"), String::from("replay_policy")],
                stale_after_ms: recovery_packet.stale_worker_timeout_ms,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: end_ms,
                phase: String::from("review"),
                subphase: Some(String::from("weekly_ablation_review")),
                step_in_progress: Some(remote_report.retained_run.completed_steps),
                microbatch_in_progress: None,
                active_subsystems: vec![String::from("dashboard"), String::from("ledger")],
                stale_after_ms: recovery_packet.stale_worker_timeout_ms,
            },
        ],
        loss_series: vec![
            RemoteTrainingLossSample {
                global_step: Some(0),
                elapsed_ms: 0,
                train_loss: Some(remote_report.retained_run.initial_mean_loss),
                ema_loss: None,
                validation_loss: None,
            },
            RemoteTrainingLossSample {
                global_step: Some(steps_midpoint),
                elapsed_ms: remote_report.retained_run.observed_wallclock_ms / 2,
                train_loss: Some(midpoint_loss),
                ema_loss: None,
                validation_loss: Some(midpoint_loss),
            },
            RemoteTrainingLossSample {
                global_step: Some(remote_report.retained_run.completed_steps),
                elapsed_ms: remote_report.retained_run.observed_wallclock_ms,
                train_loss: Some(remote_report.retained_run.final_mean_loss),
                ema_loss: None,
                validation_loss: Some(remote_report.retained_run.final_mean_loss),
            },
        ],
        math_series: vec![RemoteTrainingMathSample {
            observed_at_ms: end_ms,
            global_step: Some(remote_report.retained_run.completed_steps),
            learning_rate: None,
            gradient_norm: None,
            parameter_norm: None,
            update_norm: None,
            clip_fraction: None,
            clip_event_count: None,
            loss_scale: None,
            non_finite_count: 0,
            model_specific_diagnostics: std::collections::BTreeMap::from([
                (
                    String::from("local_to_remote_steps_ratio"),
                    matrix_report.comparison.local_to_remote_steps_ratio as f32,
                ),
                (
                    String::from("loss_delta_gap"),
                    matrix_report.comparison.local_to_remote_loss_delta_gap as f32,
                ),
            ]),
        }],
        runtime_series: vec![RemoteTrainingRuntimeSample {
            observed_at_ms: end_ms,
            data_wait_ms: None,
            forward_ms: None,
            backward_ms: None,
            optimizer_ms: None,
            checkpoint_ms: None,
            evaluation_ms: None,
            tokens_per_second: Some(tokens_per_second),
            samples_per_second_milli: Some(samples_per_second_milli),
        }],
        gpu_series: vec![],
        distributed_series: vec![RemoteTrainingDistributedSample {
            observed_at_ms: recovery_review_ms,
            participating_rank_count: 2,
            rank_skew_ms: Some(recovery_packet.uneven_worker_speed_observed_skew_ms),
            slowest_rank_ms: None,
            collective_ms: None,
            stalled_rank_count: 0,
        }],
        event_series: vec![
            RemoteTrainingEventSample {
                observed_at_ms: recovery_review_ms,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("approved_equivalent_checkpoint_subset"),
                detail: String::from(
                    "The first 4080 decision-grade packet uses an approved equivalent checkpoint subset: one explicit frequent-pack row on the supporting Tailnet run plus the full-budget retained CUDA run, recovery packet, and weekly review row.",
                ),
            },
            RemoteTrainingEventSample {
                observed_at_ms: end_ms,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("weekly_ablation_review"),
                detail: String::from(
                    "The weekly ablation review owner marked the first 4080 decision-grade packet green for the phase exit while keeping promotion and broader executor replacement claims closed.",
                ),
            },
        ],
        source_artifacts: vec![
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("decision_grade_packet"),
                artifact_uri: String::from(PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    PSION_EXECUTOR_4080_DECISION_GRADE_RUN_SCHEMA_VERSION,
                )],
                detail: String::from(
                    "The 4080 decision-grade packet is the authoritative admission receipt for the first admitted accelerator-backed decision-grade run.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("matrix_report"),
                artifact_uri: String::from(DEVICE_MATRIX_REPORT_PATH),
                artifact_digest: Some(hex::encode(Sha256::digest(
                    &read_bytes(workspace_root, DEVICE_MATRIX_REPORT_PATH)?,
                ))),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psionic.open_adapter_tailnet_admitted_device_matrix.v1",
                )],
                detail: String::from(
                    "The admitted device-matrix report preserves the baseline-comparable same-budget CUDA run beside the reachable M5 MLX reference lane.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("remote_report"),
                artifact_uri: String::from(CUDA_REPORT_PATH),
                artifact_digest: Some(hex::encode(Sha256::digest(
                    &read_bytes(workspace_root, CUDA_REPORT_PATH)?,
                ))),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psionic.open_adapter_same_node_wallclock_benchmark.v1",
                )],
                detail: String::from(
                    "The retained CUDA report remains authoritative for the decision-grade steps, wallclock, throughput, and final loss facts.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("portable_bundle"),
                artifact_uri: String::from(CUDA_BUNDLE_PATH),
                artifact_digest: Some(hex::encode(Sha256::digest(
                    &read_bytes(workspace_root, CUDA_BUNDLE_PATH)?,
                ))),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psionic.open_adapter_same_node_wallclock_benchmark.v1",
                )],
                detail: String::from(
                    "The retained CUDA portable bundle is the decision-grade export artifact for the admitted accelerator lane.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("frequent_eval_packet"),
                artifact_uri: String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psion.executor.4080_frequent_eval_attachment.v1",
                )],
                detail: String::from(
                    "The frequent-eval packet keeps one explicit checkpoint-time frozen-pack row bound into the decision-grade packet.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("recovery_packet"),
                artifact_uri: String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psion.executor.4080_interruption_recovery.v1",
                )],
                detail: String::from(
                    "The interruption-recovery packet keeps stale-worker, replay, and upload-disagreement posture explicit for the supporting Tailnet cluster run.",
                ),
            },
        ],
        bundle_digest: String::new(),
    })
    .map_err(PsionExecutor4080DecisionGradeRunError::from)
}

/// Build the retained v2 run index showing the 4080 decision-grade run beside the shipped bundle set.
pub fn builtin_executor_4080_decision_grade_run_index(
    workspace_root: &Path,
) -> Result<RemoteTrainingRunIndexV2, PsionExecutor4080DecisionGradeRunError> {
    let base_index: RemoteTrainingRunIndexV2 =
        read_json(workspace_root, REMOTE_TRAINING_RUN_INDEX_V2_FIXTURE_PATH)?;
    let bundle = builtin_executor_4080_decision_grade_visualization_bundle(workspace_root)?;
    let entry = RemoteTrainingRunIndexEntryV2 {
        provider: bundle.provider,
        profile_id: bundle.profile_id.clone(),
        lane_id: bundle.lane_id.clone(),
        run_id: bundle.run_id.clone(),
        repo_revision: bundle.repo_revision.clone(),
        track_semantics: bundle.track_semantics.clone(),
        primary_score: bundle.primary_score.clone(),
        score_surface: bundle.score_surface.clone(),
        result_classification: bundle.result_classification,
        series_status: bundle.series_status,
        series_unavailable_reason: bundle.series_unavailable_reason.clone(),
        last_heartbeat_at_ms: bundle.refresh_contract.last_heartbeat_at_ms,
        bundle_artifact_uri: Some(String::from(
            PSION_EXECUTOR_4080_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH,
        )),
        bundle_digest: Some(bundle.bundle_digest.clone()),
        semantic_summary: String::from(
            "The first admitted 4080 decision-grade executor run is visible in the shared v2 dashboard family as a mixed-device control-plane-backed, runtime-measured lane.",
        ),
    };

    let mut entries = base_index.entries;
    entries.push(entry);
    build_remote_training_run_index_v2(RemoteTrainingRunIndexV2 {
        schema_version: String::new(),
        index_id: String::from("psion-executor-4080-decision-grade-run-index-v2"),
        generated_at_ms: 1_743_127_860_000,
        entries,
        detail: String::from(
            "This retained v2 run index keeps the first 4080 decision-grade executor run visible beside the shipped training surfaces instead of letting the accelerator-backed phase exit stay buried inside packet prose.",
        ),
        index_digest: String::new(),
    })
    .map_err(PsionExecutor4080DecisionGradeRunError::from)
}

/// Build the retained 4080 decision-grade packet.
pub fn builtin_executor_4080_decision_grade_run_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080DecisionGradeRunPacket, PsionExecutor4080DecisionGradeRunError> {
    let remote_launch_packet_bytes = read_bytes(
        workspace_root,
        PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH,
    )?;
    let remote_launch_packet: PsionExecutor4080RemoteLaunchPacket =
        serde_json::from_slice(&remote_launch_packet_bytes).map_err(|error| {
            PsionExecutor4080DecisionGradeRunError::Parse {
                path: String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH),
                error,
            }
        })?;
    remote_launch_packet.validate().map_err(|error| {
        PsionExecutor4080DecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_4080_decision_grade_run.remote_launch_packet"),
            detail: error.to_string(),
        }
    })?;

    let durable_checkpoint_packet_bytes = read_bytes(
        workspace_root,
        PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH,
    )?;
    let durable_checkpoint_packet: PsionExecutor4080DurableCheckpointPacket =
        serde_json::from_slice(&durable_checkpoint_packet_bytes).map_err(|error| {
            PsionExecutor4080DecisionGradeRunError::Parse {
                path: String::from(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH),
                error,
            }
        })?;
    durable_checkpoint_packet.validate().map_err(|error| {
        PsionExecutor4080DecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_4080_decision_grade_run.durable_checkpoint_packet"),
            detail: error.to_string(),
        }
    })?;

    let frequent_eval_packet_bytes = read_bytes(
        workspace_root,
        PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH,
    )?;
    let frequent_eval_packet: PsionExecutor4080FrequentEvalAttachmentPacket =
        serde_json::from_slice(&frequent_eval_packet_bytes).map_err(|error| {
            PsionExecutor4080DecisionGradeRunError::Parse {
                path: String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH),
                error,
            }
        })?;
    frequent_eval_packet.validate().map_err(|error| {
        PsionExecutor4080DecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_4080_decision_grade_run.frequent_eval_packet"),
            detail: error.to_string(),
        }
    })?;

    let smoke_packet_bytes =
        read_bytes(workspace_root, PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH)?;
    let smoke_packet: PsionExecutor4080SmokeRunPacket = serde_json::from_slice(&smoke_packet_bytes)
        .map_err(|error| PsionExecutor4080DecisionGradeRunError::Parse {
            path: String::from(PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH),
            error,
        })?;
    smoke_packet.validate().map_err(|error| {
        PsionExecutor4080DecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_4080_decision_grade_run.smoke_packet"),
            detail: error.to_string(),
        }
    })?;

    let recovery_packet_bytes = read_bytes(
        workspace_root,
        PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH,
    )?;
    let recovery_packet: PsionExecutor4080InterruptionRecoveryPacket =
        serde_json::from_slice(&recovery_packet_bytes).map_err(|error| {
            PsionExecutor4080DecisionGradeRunError::Parse {
                path: String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH),
                error,
            }
        })?;
    recovery_packet.validate().map_err(|error| {
        PsionExecutor4080DecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_4080_decision_grade_run.recovery_packet"),
            detail: error.to_string(),
        }
    })?;

    let matrix_report_bytes = read_bytes(workspace_root, DEVICE_MATRIX_REPORT_PATH)?;
    let matrix_report: OpenAdapterTailnetAdmittedDeviceMatrixReport =
        serde_json::from_slice(&matrix_report_bytes).map_err(|error| {
            PsionExecutor4080DecisionGradeRunError::Parse {
                path: String::from(DEVICE_MATRIX_REPORT_PATH),
                error,
            }
        })?;
    let remote_report_bytes = read_bytes(workspace_root, CUDA_REPORT_PATH)?;
    let remote_report: OpenAdapterSameNodeWallclockBenchmarkReport =
        serde_json::from_slice(&remote_report_bytes).map_err(|error| {
            PsionExecutor4080DecisionGradeRunError::Parse {
                path: String::from(CUDA_REPORT_PATH),
                error,
            }
        })?;
    let remote_bundle_bytes = read_bytes(workspace_root, CUDA_BUNDLE_PATH)?;

    if smoke_packet.run_id != recovery_packet.run_id
        || smoke_packet.run_id != frequent_eval_packet.run_id
        || smoke_packet.run_id != durable_checkpoint_packet.run_id
    {
        return Err(PsionExecutor4080DecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_4080_decision_grade_run.cluster_support_run_id"),
            detail: String::from(
                "supporting Tailnet packet run ids must stay aligned across smoke, recovery, durable-checkpoint, and frequent-eval packets",
            ),
        });
    }
    if smoke_packet.checkpoint_pointer_digest != recovery_packet.checkpoint_pointer_digest
        || smoke_packet.checkpoint_pointer_digest
            != frequent_eval_packet
                .checkpoint_eval_row
                .checkpoint_pointer_digest
    {
        return Err(PsionExecutor4080DecisionGradeRunError::InvalidValue {
            field: String::from(
                "psion_executor_4080_decision_grade_run.cluster_checkpoint_pointer_digest",
            ),
            detail: String::from(
                "supporting Tailnet packet checkpoint pointer digests must stay aligned",
            ),
        });
    }
    if remote_report.retained_run.run_id != matrix_report.remote_report.retained_run.run_id {
        return Err(PsionExecutor4080DecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_4080_decision_grade_run.decision_run_id"),
            detail: String::from(
                "retained remote CUDA report must stay aligned with the device-matrix remote run",
            ),
        });
    }

    let visualization_bundle =
        builtin_executor_4080_decision_grade_visualization_bundle(workspace_root)?;
    let visualization_bundle_bytes = serde_json::to_vec_pretty(&visualization_bundle)?;
    let run_index = builtin_executor_4080_decision_grade_run_index(workspace_root)?;
    let run_index_bytes = serde_json::to_vec_pretty(&run_index)?;

    let gate_ids = vec![
        String::from("frozen_pack_binding_green"),
        String::from("baseline_comparable_green"),
        String::from("approved_equivalent_checkpoint_subset_green"),
        String::from("local_cluster_roundtrip_green"),
        String::from("weekly_ablation_review_green"),
        String::from("dashboard_visibility_green"),
    ];

    let mut run_registration_row = PsionExecutor4080DecisionGradeRunRegistrationRow {
        registration_id: String::from(DECISION_REGISTRATION_ID),
        decision_run_id: remote_report.retained_run.run_id.clone(),
        cluster_support_run_id: smoke_packet.run_id.clone(),
        admitted_profile_ids: vec![
            String::from(LOCAL_4080_PROFILE_ID),
            String::from(LOCAL_TAILNET_CONTROL_PROFILE_ID),
        ],
        eval_pack_ids: vec![frequent_eval_packet.pack_id.clone(), String::from(PROMOTION_PACK_ID)],
        checkpoint_evidence_mode: String::from("approved_equivalent_subset"),
        wallclock_budget_seconds: matrix_report.target_wallclock_seconds,
        detail: format!(
            "This retained registration row seeds the later canonical run-registration ledger: same-node CUDA run `{}` is the decision-grade accelerator fact, supporting Tailnet run `{}` is the control-plane and checkpoint-evidence fact, and both frozen pack ids stay explicit before EPIC 4 generalizes the schema.",
            remote_report.retained_run.run_id, smoke_packet.run_id,
        ),
        registration_digest: String::new(),
    };
    run_registration_row.registration_digest =
        stable_executor_4080_decision_grade_registration_digest(&run_registration_row);

    let mut weekly_ablation_review_row = PsionExecutor4080DecisionGradeWeeklyReviewRow {
        review_id: String::from(DECISION_REVIEW_ID),
        cadence_id: String::from("executor_weekly_ablation_review.v1"),
        reviewer_role: String::from("weekly_ablation_review_owner"),
        reviewer_identity: String::from("Christopher David"),
        status: String::from("reviewed"),
        decision: String::from("green_for_phase_exit"),
        covered_gate_ids: gate_ids.clone(),
        detail: format!(
            "The weekly ablation review owner accepted the first 4080 decision-grade packet for EPIC 3 because the run stayed bound to frozen packs, one explicit checkpoint-eval row, the admitted Tailnet roundtrip and recovery packets, and the retained same-budget CUDA comparison run `{}`.",
            remote_report.retained_run.run_id,
        ),
        review_digest: String::new(),
    };
    weekly_ablation_review_row.review_digest =
        stable_executor_4080_decision_grade_review_digest(&weekly_ablation_review_row);

    let mut approved_equivalent_checkpoint_subset =
        PsionExecutor4080DecisionGradeEquivalentSubset {
            subset_id: String::from(DECISION_EQUIVALENT_SUBSET_ID),
            required_gate_ids: vec![
                String::from("frozen_pack_binding_green"),
                String::from("baseline_comparable_green"),
                String::from("local_cluster_roundtrip_green"),
                String::from("weekly_ablation_review_green"),
                String::from("dashboard_visibility_green"),
            ],
            subset_digest: String::new(),
            detail: format!(
                "This approved 4080 decision-grade equivalent subset stands in for explicit multi-checkpoint repetition: one explicit frequent-pack ledger row `{}` stays green on the supporting Tailnet run, the retained CUDA run `{}` consumes most of the 600-second budget on the admitted accelerator, the roundtrip and recovery packets stay green, and the run is now visible in the shared v2 dashboard grammar.",
                frequent_eval_packet.checkpoint_eval_row.ledger_row_id,
                remote_report.retained_run.run_id,
            ),
        };
    approved_equivalent_checkpoint_subset.subset_digest =
        stable_executor_4080_decision_grade_subset_digest(&approved_equivalent_checkpoint_subset);

    let gate_rows = vec![
        PsionExecutor4080DecisionGradeGateRow {
            gate_id: String::from("frozen_pack_binding_green"),
            status: String::from("green"),
            detail: format!(
                "The retained registration row binds both frozen executor pack ids (`{}` and `{}`) to the first 4080 decision-grade question instead of letting the accelerator run drift onto a private checklist.",
                frequent_eval_packet.pack_id, PROMOTION_PACK_ID,
            ),
        },
        PsionExecutor4080DecisionGradeGateRow {
            gate_id: String::from("baseline_comparable_green"),
            status: String::from("green"),
            detail: format!(
                "The retained CUDA run `{}` stayed inside the admitted device-matrix family, kept `local_to_remote_loss_delta_gap={:.1}`, and therefore remains same-track comparable instead of becoming a one-off benchmark story.",
                remote_report.retained_run.run_id,
                matrix_report.comparison.local_to_remote_loss_delta_gap,
            ),
        },
        PsionExecutor4080DecisionGradeGateRow {
            gate_id: String::from("approved_equivalent_checkpoint_subset_green"),
            status: String::from("green"),
            detail: format!(
                "The first 4080 decision-grade packet uses the approved equivalent subset `{}` rather than pretending two full explicit checkpoint-eval points already exist; the explicit checkpoint row `{}` remains retained and the rest of the requirement stays visible instead of implied.",
                DECISION_EQUIVALENT_SUBSET_ID,
                frequent_eval_packet.checkpoint_eval_row.ledger_row_id,
            ),
        },
        PsionExecutor4080DecisionGradeGateRow {
            gate_id: String::from("local_cluster_roundtrip_green"),
            status: String::from("green"),
            detail: format!(
                "The supporting Tailnet run `{}` keeps Mac -> 4080 -> Mac launch, durable checkpoint, strict replay, and recovery truth explicit with checkpoint pointer `{}` and checkpoint ref `{}`.",
                smoke_packet.run_id,
                recovery_packet.checkpoint_pointer_digest,
                recovery_packet.checkpoint_ref,
            ),
        },
        PsionExecutor4080DecisionGradeGateRow {
            gate_id: String::from("weekly_ablation_review_green"),
            status: String::from("green"),
            detail: format!(
                "Weekly ablation review row `{}` now exists with decision `{}` and keeps the first 4080 decision-grade packet reviewable under the named cadence in `{}`.",
                weekly_ablation_review_row.review_id,
                weekly_ablation_review_row.decision,
                PSION_EXECUTOR_OWNERSHIP_DOC_PATH,
            ),
        },
        PsionExecutor4080DecisionGradeGateRow {
            gate_id: String::from("dashboard_visibility_green"),
            status: String::from("green"),
            detail: format!(
                "The retained 4080 decision-grade bundle now appears in the shared v2 run index with {} total entries instead of hiding behind packet prose alone.",
                run_index.entries.len(),
            ),
        },
    ];

    let mut packet = PsionExecutor4080DecisionGradeRunPacket {
        schema_version: String::from(PSION_EXECUTOR_4080_DECISION_GRADE_RUN_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_4080_decision_grade_run_v1"),
        worker_profile_id: String::from(LOCAL_4080_PROFILE_ID),
        control_plane_profile_id: String::from(LOCAL_TAILNET_CONTROL_PROFILE_ID),
        decision_lane_id: String::from(DECISION_LANE_ID),
        decision_question_posture: String::from(
            "This packet counts as the first admitted 4080 decision-grade executor packet because it combines one retained same-node CUDA run with the supporting Mac -> 4080 -> Mac roundtrip, one explicit checkpoint-eval row, and one weekly ablation review row. It does not claim promotion readiness, full promotion-pack scoring, or the later canonical ledger schema from EPIC 4.",
        ),
        remote_launch_packet_ref: String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH),
        remote_launch_packet_sha256: hex::encode(Sha256::digest(&remote_launch_packet_bytes)),
        durable_checkpoint_packet_ref: String::from(
            PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH,
        ),
        durable_checkpoint_packet_sha256: hex::encode(Sha256::digest(
            &durable_checkpoint_packet_bytes,
        )),
        frequent_eval_packet_ref: String::from(
            PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH,
        ),
        frequent_eval_packet_sha256: hex::encode(Sha256::digest(&frequent_eval_packet_bytes)),
        smoke_packet_ref: String::from(PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH),
        smoke_packet_sha256: hex::encode(Sha256::digest(&smoke_packet_bytes)),
        recovery_packet_ref: String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH),
        recovery_packet_sha256: hex::encode(Sha256::digest(&recovery_packet_bytes)),
        retained_matrix_report_ref: String::from(DEVICE_MATRIX_REPORT_PATH),
        retained_matrix_report_sha256: hex::encode(Sha256::digest(&matrix_report_bytes)),
        retained_remote_report_ref: String::from(CUDA_REPORT_PATH),
        retained_remote_report_sha256: hex::encode(Sha256::digest(&remote_report_bytes)),
        retained_remote_bundle_ref: String::from(CUDA_BUNDLE_PATH),
        retained_remote_bundle_sha256: hex::encode(Sha256::digest(&remote_bundle_bytes)),
        visualization_bundle_ref: String::from(
            PSION_EXECUTOR_4080_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH,
        ),
        visualization_bundle_sha256: hex::encode(Sha256::digest(&visualization_bundle_bytes)),
        visualization_bundle_digest: visualization_bundle.bundle_digest.clone(),
        visualization_run_index_ref: String::from(
            PSION_EXECUTOR_4080_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH,
        ),
        visualization_run_index_sha256: hex::encode(Sha256::digest(&run_index_bytes)),
        visualization_run_index_digest: run_index.index_digest.clone(),
        run_registration_row,
        weekly_ablation_review_row,
        approved_equivalent_checkpoint_subset,
        decision_matrix_run_id: matrix_report.run_id.clone(),
        decision_run_id: remote_report.retained_run.run_id.clone(),
        cluster_support_run_id: smoke_packet.run_id.clone(),
        execution_backend_label: remote_report.backend_label.clone(),
        logical_device_label: remote_report.logical_device_label.clone(),
        cluster_checkpoint_family: recovery_packet.checkpoint_family.clone(),
        cluster_checkpoint_pointer_digest: recovery_packet.checkpoint_pointer_digest.clone(),
        cluster_checkpoint_ref: recovery_packet.checkpoint_ref.clone(),
        cluster_checkpoint_step: recovery_packet.checkpoint_step,
        decision_checkpoint_family: remote_report.retained_run.checkpoint_family.clone(),
        completed_steps: remote_report.retained_run.completed_steps,
        observed_wallclock_ms: remote_report.retained_run.observed_wallclock_ms,
        final_mean_loss: remote_report.retained_run.final_mean_loss,
        final_state_dict_digest: remote_report.retained_run.final_state_dict_digest.clone(),
        dashboard_entry_count: run_index.entries.len() as u64,
        gate_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH),
            String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_DOC_PATH),
            String::from(PSION_EXECUTOR_OWNERSHIP_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_REMOTE_LAUNCH_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_4080_SMOKE_RUN_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_FIXTURE_PATH),
            String::from(DEVICE_MATRIX_REPORT_PATH),
            String::from(CUDA_REPORT_PATH),
            String::from(CUDA_BUNDLE_PATH),
            String::from(PSION_EXECUTOR_4080_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_4080_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH),
        ],
        summary: format!(
            "The admitted 4080 profile now has one retained decision-grade packet. The baseline-comparable CUDA run `{}` on `{}` stays anchored to the retained device-matrix report (steps={} wallclock_ms={} final_mean_loss={:.6}), the supporting Tailnet cluster run `{}` keeps checkpoint pointer `{}` and one explicit frozen frequent-pack row `{}` machine-legible, and the packet now carries a retained registration row, weekly ablation review row, and shared v2 dashboard visibility with {} total entries while still refusing promotion or full multi-checkpoint repetition claims.",
            remote_report.retained_run.run_id,
            remote_report.logical_device_label,
            remote_report.retained_run.completed_steps,
            remote_report.retained_run.observed_wallclock_ms,
            remote_report.retained_run.final_mean_loss,
            smoke_packet.run_id,
            recovery_packet.checkpoint_pointer_digest,
            frequent_eval_packet.checkpoint_eval_row.ledger_row_id,
            run_index.entries.len(),
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_executor_4080_decision_grade_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the retained executor 4080 decision-grade visualization bundle, run index, and packet.
pub fn write_builtin_executor_4080_decision_grade_artifacts(
    workspace_root: &Path,
) -> Result<PsionExecutor4080DecisionGradeRunPacket, PsionExecutor4080DecisionGradeRunError> {
    let visualization_bundle =
        builtin_executor_4080_decision_grade_visualization_bundle(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_4080_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH,
        &visualization_bundle,
    )?;
    let run_index = builtin_executor_4080_decision_grade_run_index(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_4080_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH,
        &run_index,
    )?;
    let packet = builtin_executor_4080_decision_grade_run_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutor4080DecisionGradeRunError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutor4080DecisionGradeRunError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutor4080DecisionGradeRunError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutor4080DecisionGradeRunError::Parse {
        path: relative_path.to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutor4080DecisionGradeRunError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutor4080DecisionGradeRunError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
    if value.trim().is_empty() {
        return Err(PsionExecutor4080DecisionGradeRunError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_executor_4080_decision_grade_registration_digest(
    row: &PsionExecutor4080DecisionGradeRunRegistrationRow,
) -> String {
    let mut clone = row.clone();
    clone.registration_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("registration row serialization should succeed"),
    ))
}

fn stable_executor_4080_decision_grade_review_digest(
    row: &PsionExecutor4080DecisionGradeWeeklyReviewRow,
) -> String {
    let mut clone = row.clone();
    clone.review_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("review row serialization should succeed"),
    ))
}

fn stable_executor_4080_decision_grade_subset_digest(
    subset: &PsionExecutor4080DecisionGradeEquivalentSubset,
) -> String {
    let mut clone = subset.clone();
    clone.subset_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("subset serialization should succeed"),
    ))
}

fn stable_executor_4080_decision_grade_packet_digest(
    packet: &PsionExecutor4080DecisionGradeRunPacket,
) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("packet serialization should succeed"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_4080_decision_grade_bundle_is_valid(
    ) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        let root = workspace_root();
        let bundle = builtin_executor_4080_decision_grade_visualization_bundle(root.as_path())?;
        bundle.validate()?;
        Ok(())
    }

    #[test]
    fn builtin_executor_4080_decision_grade_run_index_is_valid(
    ) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        let root = workspace_root();
        let run_index = builtin_executor_4080_decision_grade_run_index(root.as_path())?;
        run_index.validate()?;
        Ok(())
    }

    #[test]
    fn builtin_executor_4080_decision_grade_packet_is_valid(
    ) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        let root = workspace_root();
        let packet = builtin_executor_4080_decision_grade_run_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_4080_decision_grade_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        let root = workspace_root();
        let expected_bundle: RemoteTrainingVisualizationBundleV2 = read_json(
            root.as_path(),
            PSION_EXECUTOR_4080_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH,
        )?;
        let actual_bundle =
            builtin_executor_4080_decision_grade_visualization_bundle(root.as_path())?;
        assert_eq!(actual_bundle, expected_bundle);

        let expected_index: RemoteTrainingRunIndexV2 = read_json(
            root.as_path(),
            PSION_EXECUTOR_4080_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH,
        )?;
        let actual_index = builtin_executor_4080_decision_grade_run_index(root.as_path())?;
        assert_eq!(actual_index, expected_index);

        let expected_packet: PsionExecutor4080DecisionGradeRunPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH,
        )?;
        let actual_packet = builtin_executor_4080_decision_grade_run_packet(root.as_path())?;
        assert_eq!(actual_packet, expected_packet);
        Ok(())
    }

    #[test]
    fn write_executor_4080_decision_grade_artifacts_persists_current_truth(
    ) -> Result<(), PsionExecutor4080DecisionGradeRunError> {
        let root = workspace_root();
        let packet = write_builtin_executor_4080_decision_grade_artifacts(root.as_path())?;
        let committed: PsionExecutor4080DecisionGradeRunPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH,
        )?;
        assert_eq!(packet, committed);
        Ok(())
    }
}
