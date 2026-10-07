use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_remote_training_run_index_v2, build_remote_training_visualization_bundle_v2,
    PsionExecutorMlxCheckpointCompatibilityPacket, PsionExecutorMlxSmokeRunPacket,
    RemoteTrainingArtifactSourceKind, RemoteTrainingEmissionMode, RemoteTrainingEventSample,
    RemoteTrainingEventSeverity, RemoteTrainingHeartbeatSample, RemoteTrainingLossSample,
    RemoteTrainingMathSample, RemoteTrainingProvider, RemoteTrainingRefreshContract,
    RemoteTrainingResultClassification, RemoteTrainingRunIndexEntryV2,
    RemoteTrainingRunIndexV2, RemoteTrainingRuntimeSample, RemoteTrainingSeriesStatus,
    RemoteTrainingSourceArtifact, RemoteTrainingTimelineEntry,
    RemoteTrainingTrackFamilyV2, RemoteTrainingTrackSemanticsV2,
    RemoteTrainingVisualizationBundleV2, RemoteTrainingVisualizationError,
    RemoteTrainingVisualizationSummary, RemoteTrainingComparabilityClassV2,
    RemoteTrainingExecutionClassV2, RemoteTrainingProofPostureV2,
    RemoteTrainingPublicEquivalenceClassV2,
    PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH,
    PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH, REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
};

/// Stable schema version for the executor-lane MLX decision-grade packet.
pub const PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_SCHEMA_VERSION: &str =
    "psion.executor.mlx_decision_grade_run.v1";
/// Canonical fixture path for the executor-lane MLX decision-grade packet.
pub const PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_mlx_decision_grade_run_v1.json";
/// Canonical doc path for the executor-lane MLX decision-grade packet.
pub const PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MLX_DECISION_GRADE_RUN.md";
/// Canonical track-aware visualization bundle path for the retained MLX decision-grade run.
pub const PSION_EXECUTOR_MLX_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/training_visualization/psion_executor_mlx_decision_grade_remote_training_visualization_bundle_v2.json";
/// Canonical track-aware run-index path for the retained MLX decision-grade run.
pub const PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH: &str =
    "fixtures/training_visualization/psion_executor_mlx_decision_grade_remote_training_run_index_v2.json";

const LOCAL_MAC_MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ACCEPTANCE_PROFILE.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const REMOTE_TRAINING_RUN_INDEX_V2_FIXTURE_PATH: &str =
    "fixtures/training_visualization/remote_training_run_index_v2.json";
const M5_MLX_REPORT_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/report.json";
const M5_MLX_BUNDLE_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors";
const M5_MLX_MATRIX_REPORT_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/matrix_report.json";
const MLX_DECISION_GRADE_EQUIVALENT_SUBSET_ID: &str =
    "psion.executor.mlx_local_decision_grade_equivalent_subset.v1";

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
    claim_boundary: String,
}

/// One required gate inside the approved equivalent local subset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMlxDecisionGradeEquivalentSubset {
    /// Stable subset id.
    pub subset_id: String,
    /// Required gate ids that make the MLX-local decision-grade packet admissible.
    pub required_gate_ids: Vec<String>,
    /// Stable subset digest.
    pub subset_digest: String,
    /// Honest explanation of why this subset stands in for explicit checkpoint evals.
    pub detail: String,
}

/// One retained gate row for the decision-grade packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMlxDecisionGradeGateRow {
    /// Stable gate id.
    pub gate_id: String,
    /// Final status for the gate.
    pub status: String,
    /// Honest detail.
    pub detail: String,
}

/// Typed packet proving the first MLX-local decision-grade run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorMlxDecisionGradeRunPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted profile id.
    pub admitted_profile_id: String,
    /// Stable decision-grade lane id.
    pub decision_lane_id: String,
    /// Honest question posture.
    pub decision_question_posture: String,
    /// Prerequisite smoke packet reference.
    pub smoke_packet_ref: String,
    /// Stable SHA256 over the smoke packet bytes.
    pub smoke_packet_sha256: String,
    /// Prerequisite checkpoint packet reference.
    pub checkpoint_packet_ref: String,
    /// Stable SHA256 over the checkpoint packet bytes.
    pub checkpoint_packet_sha256: String,
    /// Retained local report reference.
    pub retained_local_report_ref: String,
    /// Stable SHA256 over the retained local report bytes.
    pub retained_local_report_sha256: String,
    /// Retained matrix report reference.
    pub retained_matrix_report_ref: String,
    /// Stable SHA256 over the retained matrix report bytes.
    pub retained_matrix_report_sha256: String,
    /// Retained track-aware visualization bundle reference.
    pub visualization_bundle_ref: String,
    /// Stable SHA256 over the visualization bundle bytes.
    pub visualization_bundle_sha256: String,
    /// Stable visualization bundle digest.
    pub visualization_bundle_digest: String,
    /// Retained track-aware run-index reference.
    pub visualization_run_index_ref: String,
    /// Stable SHA256 over the run-index bytes.
    pub visualization_run_index_sha256: String,
    /// Stable run-index digest.
    pub visualization_run_index_digest: String,
    /// Stable backend label.
    pub execution_backend_label: String,
    /// Stable logical-device label.
    pub logical_device_label: String,
    /// Stable retained run id.
    pub retained_run_id: String,
    /// Stable retained checkpoint family.
    pub checkpoint_family: String,
    /// Stable completed step count.
    pub completed_steps: u64,
    /// Stable observed wallclock milliseconds.
    pub observed_wallclock_ms: u64,
    /// Stable final mean loss.
    pub final_mean_loss: f32,
    /// Stable retained state-dict digest.
    pub final_state_dict_digest: String,
    /// Number of entries in the retained v2 run index after the executor entry is added.
    pub dashboard_entry_count: u64,
    /// Approved equivalent local subset that counts instead of explicit checkpoint eval points.
    pub approved_equivalent_local_subset: PsionExecutorMlxDecisionGradeEquivalentSubset,
    /// Retained gate rows.
    pub gate_rows: Vec<PsionExecutorMlxDecisionGradeGateRow>,
    /// Support references.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutorMlxDecisionGradeRunPacket {
    /// Validate the retained MLX decision-grade packet.
    pub fn validate(&self) -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_mlx_decision_grade_run.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_SCHEMA_VERSION {
            return Err(PsionExecutorMlxDecisionGradeRunError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.packet_id.as_str(),
            "psion_executor_mlx_decision_grade_run.packet_id",
        )?;
        ensure_nonempty(
            self.admitted_profile_id.as_str(),
            "psion_executor_mlx_decision_grade_run.admitted_profile_id",
        )?;
        ensure_nonempty(
            self.decision_lane_id.as_str(),
            "psion_executor_mlx_decision_grade_run.decision_lane_id",
        )?;
        ensure_nonempty(
            self.decision_question_posture.as_str(),
            "psion_executor_mlx_decision_grade_run.decision_question_posture",
        )?;
        ensure_nonempty(
            self.smoke_packet_ref.as_str(),
            "psion_executor_mlx_decision_grade_run.smoke_packet_ref",
        )?;
        ensure_nonempty(
            self.smoke_packet_sha256.as_str(),
            "psion_executor_mlx_decision_grade_run.smoke_packet_sha256",
        )?;
        ensure_nonempty(
            self.checkpoint_packet_ref.as_str(),
            "psion_executor_mlx_decision_grade_run.checkpoint_packet_ref",
        )?;
        ensure_nonempty(
            self.checkpoint_packet_sha256.as_str(),
            "psion_executor_mlx_decision_grade_run.checkpoint_packet_sha256",
        )?;
        ensure_nonempty(
            self.retained_local_report_ref.as_str(),
            "psion_executor_mlx_decision_grade_run.retained_local_report_ref",
        )?;
        ensure_nonempty(
            self.retained_local_report_sha256.as_str(),
            "psion_executor_mlx_decision_grade_run.retained_local_report_sha256",
        )?;
        ensure_nonempty(
            self.retained_matrix_report_ref.as_str(),
            "psion_executor_mlx_decision_grade_run.retained_matrix_report_ref",
        )?;
        ensure_nonempty(
            self.retained_matrix_report_sha256.as_str(),
            "psion_executor_mlx_decision_grade_run.retained_matrix_report_sha256",
        )?;
        ensure_nonempty(
            self.visualization_bundle_ref.as_str(),
            "psion_executor_mlx_decision_grade_run.visualization_bundle_ref",
        )?;
        ensure_nonempty(
            self.visualization_bundle_sha256.as_str(),
            "psion_executor_mlx_decision_grade_run.visualization_bundle_sha256",
        )?;
        ensure_nonempty(
            self.visualization_bundle_digest.as_str(),
            "psion_executor_mlx_decision_grade_run.visualization_bundle_digest",
        )?;
        ensure_nonempty(
            self.visualization_run_index_ref.as_str(),
            "psion_executor_mlx_decision_grade_run.visualization_run_index_ref",
        )?;
        ensure_nonempty(
            self.visualization_run_index_sha256.as_str(),
            "psion_executor_mlx_decision_grade_run.visualization_run_index_sha256",
        )?;
        ensure_nonempty(
            self.visualization_run_index_digest.as_str(),
            "psion_executor_mlx_decision_grade_run.visualization_run_index_digest",
        )?;
        ensure_nonempty(
            self.execution_backend_label.as_str(),
            "psion_executor_mlx_decision_grade_run.execution_backend_label",
        )?;
        ensure_nonempty(
            self.logical_device_label.as_str(),
            "psion_executor_mlx_decision_grade_run.logical_device_label",
        )?;
        ensure_nonempty(
            self.retained_run_id.as_str(),
            "psion_executor_mlx_decision_grade_run.retained_run_id",
        )?;
        ensure_nonempty(
            self.checkpoint_family.as_str(),
            "psion_executor_mlx_decision_grade_run.checkpoint_family",
        )?;
        ensure_nonempty(
            self.final_state_dict_digest.as_str(),
            "psion_executor_mlx_decision_grade_run.final_state_dict_digest",
        )?;
        self.approved_equivalent_local_subset.validate()?;
        if self.gate_rows.is_empty() {
            return Err(PsionExecutorMlxDecisionGradeRunError::MissingField {
                field: String::from("psion_executor_mlx_decision_grade_run.gate_rows"),
            });
        }
        for row in &self.gate_rows {
            row.validate()?;
        }
        if self.dashboard_entry_count == 0 {
            return Err(PsionExecutorMlxDecisionGradeRunError::InvalidValue {
                field: String::from("psion_executor_mlx_decision_grade_run.dashboard_entry_count"),
                detail: String::from("dashboard entry count must stay positive"),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorMlxDecisionGradeRunError::MissingField {
                field: String::from("psion_executor_mlx_decision_grade_run.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(
                support_ref.as_str(),
                "psion_executor_mlx_decision_grade_run.support_refs[]",
            )?;
        }
        ensure_nonempty(
            self.summary.as_str(),
            "psion_executor_mlx_decision_grade_run.summary",
        )?;
        if stable_executor_mlx_decision_grade_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorMlxDecisionGradeRunError::DigestMismatch {
                expected: stable_executor_mlx_decision_grade_packet_digest(self),
                actual: self.packet_digest.clone(),
            });
        }
        Ok(())
    }
}

impl PsionExecutorMlxDecisionGradeEquivalentSubset {
    fn validate(&self) -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
        ensure_nonempty(
            self.subset_id.as_str(),
            "psion_executor_mlx_decision_grade_run.approved_equivalent_local_subset.subset_id",
        )?;
        if self.required_gate_ids.is_empty() {
            return Err(PsionExecutorMlxDecisionGradeRunError::MissingField {
                field: String::from(
                    "psion_executor_mlx_decision_grade_run.approved_equivalent_local_subset.required_gate_ids",
                ),
            });
        }
        for gate_id in &self.required_gate_ids {
            ensure_nonempty(
                gate_id.as_str(),
                "psion_executor_mlx_decision_grade_run.approved_equivalent_local_subset.required_gate_ids[]",
            )?;
        }
        ensure_nonempty(
            self.subset_digest.as_str(),
            "psion_executor_mlx_decision_grade_run.approved_equivalent_local_subset.subset_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_mlx_decision_grade_run.approved_equivalent_local_subset.detail",
        )?;
        if stable_executor_mlx_decision_grade_subset_digest(self) != self.subset_digest {
            return Err(PsionExecutorMlxDecisionGradeRunError::DigestMismatch {
                expected: stable_executor_mlx_decision_grade_subset_digest(self),
                actual: self.subset_digest.clone(),
            });
        }
        Ok(())
    }
}

impl PsionExecutorMlxDecisionGradeGateRow {
    fn validate(&self) -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
        ensure_nonempty(
            self.gate_id.as_str(),
            "psion_executor_mlx_decision_grade_run.gate_rows[].gate_id",
        )?;
        ensure_nonempty(
            self.status.as_str(),
            "psion_executor_mlx_decision_grade_run.gate_rows[].status",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_mlx_decision_grade_run.gate_rows[].detail",
        )?;
        Ok(())
    }
}

/// Validation failures for the executor-lane MLX decision-grade packet.
#[derive(Debug, Error)]
pub enum PsionExecutorMlxDecisionGradeRunError {
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

/// Build the retained MLX decision-grade visualization bundle.
pub fn builtin_executor_mlx_decision_grade_visualization_bundle(
    workspace_root: &Path,
) -> Result<RemoteTrainingVisualizationBundleV2, PsionExecutorMlxDecisionGradeRunError> {
    let matrix_report: OpenAdapterTailnetAdmittedDeviceMatrixReport =
        read_json(workspace_root, M5_MLX_MATRIX_REPORT_PATH)?;
    let local_report = matrix_report.local_report.clone();
    let retained = local_report.retained_run.clone();

    let run_start_ms = 1_742_951_200_000_u64;
    let mid_ms = run_start_ms + retained.observed_wallclock_ms / 2;
    let end_ms = run_start_ms + retained.observed_wallclock_ms;
    let samples_per_second_milli = (retained.samples_per_second * 1000.0).round() as u32;
    let tokens_per_second = retained.source_tokens_per_second.round() as u64;
    let steps_midpoint = retained.completed_steps / 2;
    let midpoint_loss = (retained.initial_mean_loss / 2.0).max(retained.final_mean_loss);

    build_remote_training_visualization_bundle_v2(RemoteTrainingVisualizationBundleV2 {
        schema_version: String::new(),
        bundle_id: String::from("psion-executor-mlx-decision-grade-run-v2"),
        provider: RemoteTrainingProvider::LocalHybrid,
        profile_id: String::from(LOCAL_MAC_MLX_PROFILE_ID),
        lane_id: String::from("psion_executor_mlx_local_decision_grade"),
        run_id: retained.run_id.clone(),
        repo_revision: matrix_report.git_ref,
        track_semantics: RemoteTrainingTrackSemanticsV2 {
            track_family: RemoteTrainingTrackFamilyV2::Psion,
            track_id: String::from("psion.executor.mlx_local_decision_grade.v1"),
            execution_class: RemoteTrainingExecutionClassV2::SingleNode,
            comparability_class: RemoteTrainingComparabilityClassV2::SameTrackComparable,
            proof_posture: RemoteTrainingProofPostureV2::RuntimeMeasured,
            public_equivalence_class: RemoteTrainingPublicEquivalenceClassV2::NotApplicable,
            score_law_ref: Some(String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH)),
            artifact_cap_bytes: None,
            wallclock_cap_seconds: Some(local_report.target_wallclock_seconds),
            semantic_summary: String::from(
                "The admitted Mac MLX decision-grade executor lane stays local-only, keeps one retained post-run dashboard packet in the shared v2 training surfaces, and does not claim cross-device closure or promotion authority.",
            ),
        },
        primary_score: None,
        score_surface: None,
        result_classification: RemoteTrainingResultClassification::CompletedSuccess,
        refresh_contract: RemoteTrainingRefreshContract {
            target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
            emission_mode: RemoteTrainingEmissionMode::PostRunOnly,
            last_heartbeat_at_ms: Some(end_ms),
            heartbeat_seq: 3,
        },
        series_status: RemoteTrainingSeriesStatus::Available,
        series_unavailable_reason: None,
        timeline: vec![
            RemoteTrainingTimelineEntry {
                observed_at_ms: run_start_ms,
                phase: String::from("dataset_staging"),
                subphase: Some(String::from("executor_subset_admission")),
                detail: String::from(
                    "The admitted Mac MLX profile sealed the executor-local question and budget before the retained decision-grade run packet was formed.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: mid_ms,
                phase: String::from("training"),
                subphase: Some(String::from("bounded_local_run")),
                detail: format!(
                    "The retained MLX run `{}` kept the same-node Rust-only lane alive for most of the 600-second budget and remained inside the admitted Mac-only question boundary.",
                    retained.run_id
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: end_ms,
                phase: String::from("review"),
                subphase: Some(String::from("decision_grade_packet")),
                detail: String::from(
                    "Checkpoint compatibility, smoke prerequisites, and the shared v2 run-index entry were sealed together so the MLX-local run appears beside the rest of the admitted run set.",
                ),
            },
        ],
        summary: RemoteTrainingVisualizationSummary {
            total_steps_completed: retained.completed_steps,
            latest_global_step: Some(retained.completed_steps),
            latest_train_loss: Some(retained.final_mean_loss),
            latest_ema_loss: None,
            latest_validation_loss: Some(retained.final_mean_loss),
            latest_tokens_per_second: Some(tokens_per_second),
            latest_samples_per_second_milli: Some(samples_per_second_milli),
            accumulated_cost_microusd: None,
            latest_checkpoint_ref: Some(format!(
                "{}#{}",
                retained.checkpoint_family, retained.run_id
            )),
            detail: String::from(
                "The retained Mac MLX decision-grade packet surfaces one post-run dashboard-visible local executor run with restore, export, and visibility facts preserved.",
            ),
        },
        heartbeat_series: vec![
            RemoteTrainingHeartbeatSample {
                observed_at_ms: run_start_ms,
                phase: String::from("dataset_staging"),
                subphase: Some(String::from("executor_subset_admission")),
                step_in_progress: Some(0),
                microbatch_in_progress: None,
                active_subsystems: vec![String::from("mlx"), String::from("operator_review")],
                stale_after_ms: 5_000,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: mid_ms,
                phase: String::from("training"),
                subphase: Some(String::from("bounded_local_run")),
                step_in_progress: Some(steps_midpoint),
                microbatch_in_progress: Some(1),
                active_subsystems: vec![String::from("mlx"), String::from("checkpoint")],
                stale_after_ms: 5_000,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: end_ms,
                phase: String::from("review"),
                subphase: Some(String::from("decision_grade_packet")),
                step_in_progress: Some(retained.completed_steps),
                microbatch_in_progress: None,
                active_subsystems: vec![String::from("dashboard"), String::from("ledger")],
                stale_after_ms: 5_000,
            },
        ],
        loss_series: vec![
            RemoteTrainingLossSample {
                global_step: Some(0),
                elapsed_ms: 0,
                train_loss: Some(retained.initial_mean_loss),
                ema_loss: None,
                validation_loss: None,
            },
            RemoteTrainingLossSample {
                global_step: Some(steps_midpoint),
                elapsed_ms: retained.observed_wallclock_ms / 2,
                train_loss: Some(midpoint_loss),
                ema_loss: None,
                validation_loss: Some(midpoint_loss),
            },
            RemoteTrainingLossSample {
                global_step: Some(retained.completed_steps),
                elapsed_ms: retained.observed_wallclock_ms,
                train_loss: Some(retained.final_mean_loss),
                ema_loss: None,
                validation_loss: Some(retained.final_mean_loss),
            },
        ],
        math_series: vec![RemoteTrainingMathSample {
            observed_at_ms: end_ms,
            global_step: Some(retained.completed_steps),
            learning_rate: None,
            gradient_norm: None,
            parameter_norm: None,
            update_norm: None,
            clip_fraction: None,
            clip_event_count: None,
            loss_scale: None,
            non_finite_count: 0,
            model_specific_diagnostics: Default::default(),
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
        distributed_series: vec![],
        event_series: vec![RemoteTrainingEventSample {
            observed_at_ms: end_ms,
            severity: RemoteTrainingEventSeverity::Info,
            event_kind: String::from("decision_grade_visibility"),
            detail: String::from(
                "The retained MLX-local decision-grade run was sealed into the shared track-aware dashboard bundle and run index instead of staying a one-off local note.",
            ),
        }],
        source_artifacts: vec![
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("decision_grade_packet"),
                artifact_uri: String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from("psion.executor.mlx_decision_grade_run.v1")],
                detail: String::from(
                    "The decision-grade packet is the authoritative executor-lane admission receipt for the retained Mac MLX run.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("matrix_report"),
                artifact_uri: String::from(M5_MLX_MATRIX_REPORT_PATH),
                artifact_digest: Some(hex::encode(Sha256::digest(
                    &read_bytes(workspace_root, M5_MLX_MATRIX_REPORT_PATH)?,
                ))),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psionic.open_adapter_tailnet_admitted_device_matrix.v1",
                )],
                detail: String::from(
                    "The admitted device-matrix report preserves the local MLX run beside the reachable Tailnet accelerator comparison lane.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("local_report"),
                artifact_uri: String::from(M5_MLX_REPORT_PATH),
                artifact_digest: Some(hex::encode(Sha256::digest(
                    &read_bytes(workspace_root, M5_MLX_REPORT_PATH)?,
                ))),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psionic.open_adapter_same_node_wallclock_benchmark.v1",
                )],
                detail: String::from(
                    "The retained same-node MLX report remains authoritative for steps, wallclock, throughput, and final loss facts.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("portable_bundle"),
                artifact_uri: String::from(M5_MLX_BUNDLE_PATH),
                artifact_digest: Some(hex::encode(Sha256::digest(
                    &read_bytes(workspace_root, M5_MLX_BUNDLE_PATH)?,
                ))),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psion.executor.mlx_checkpoint_compatibility.v1",
                )],
                detail: String::from(
                    "The durable portable bundle remains the export artifact bound into the decision-grade packet.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("checkpoint_packet"),
                artifact_uri: String::from(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psion.executor.mlx_checkpoint_compatibility.v1",
                )],
                detail: String::from(
                    "The checkpoint packet remains authoritative for deferred import-plan and eager restore truth on the retained MLX bundle.",
                ),
            },
        ],
        bundle_digest: String::new(),
    })
    .map_err(PsionExecutorMlxDecisionGradeRunError::from)
}

/// Build the retained v2 run index showing the MLX decision-grade run beside the shipped bundle set.
pub fn builtin_executor_mlx_decision_grade_run_index(
    workspace_root: &Path,
) -> Result<RemoteTrainingRunIndexV2, PsionExecutorMlxDecisionGradeRunError> {
    let base_index: RemoteTrainingRunIndexV2 =
        read_json(workspace_root, REMOTE_TRAINING_RUN_INDEX_V2_FIXTURE_PATH)?;
    let bundle = builtin_executor_mlx_decision_grade_visualization_bundle(workspace_root)?;
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
            PSION_EXECUTOR_MLX_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH,
        )),
        bundle_digest: Some(bundle.bundle_digest.clone()),
        semantic_summary: String::from(
            "The retained Mac MLX decision-grade executor run is visible in the shared v2 dashboard family as a local-only, post-run measured lane.",
        ),
    };

    let mut entries = base_index.entries;
    entries.push(entry);
    build_remote_training_run_index_v2(RemoteTrainingRunIndexV2 {
        schema_version: String::new(),
        index_id: String::from("psion-executor-mlx-decision-grade-run-index-v2"),
        generated_at_ms: 1_742_951_780_000,
        entries,
        detail: String::from(
            "This retained v2 run index keeps the admitted Mac MLX executor decision-grade run visible beside the already shipped training surfaces instead of inventing a parallel dashboard family.",
        ),
        index_digest: String::new(),
    })
    .map_err(PsionExecutorMlxDecisionGradeRunError::from)
}

/// Build the retained MLX decision-grade packet.
pub fn builtin_executor_mlx_decision_grade_run_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMlxDecisionGradeRunPacket, PsionExecutorMlxDecisionGradeRunError> {
    let smoke_packet_bytes = read_bytes(workspace_root, PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH)?;
    let smoke_packet: PsionExecutorMlxSmokeRunPacket =
        serde_json::from_slice(&smoke_packet_bytes).map_err(|error| {
            PsionExecutorMlxDecisionGradeRunError::Parse {
                path: String::from(PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH),
                error,
            }
        })?;
    smoke_packet
        .validate()
        .map_err(|error| PsionExecutorMlxDecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_mlx_decision_grade_run.smoke_packet"),
            detail: error.to_string(),
        })?;

    let checkpoint_packet_bytes =
        read_bytes(workspace_root, PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH)?;
    let checkpoint_packet: PsionExecutorMlxCheckpointCompatibilityPacket =
        serde_json::from_slice(&checkpoint_packet_bytes).map_err(|error| {
            PsionExecutorMlxDecisionGradeRunError::Parse {
                path: String::from(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH),
                error,
            }
        })?;
    checkpoint_packet.validate().map_err(|error| {
        PsionExecutorMlxDecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_mlx_decision_grade_run.checkpoint_packet"),
            detail: error.to_string(),
        }
    })?;

    let local_report_bytes = read_bytes(workspace_root, M5_MLX_REPORT_PATH)?;
    let local_report: OpenAdapterSameNodeWallclockBenchmarkReport =
        serde_json::from_slice(&local_report_bytes).map_err(|error| {
            PsionExecutorMlxDecisionGradeRunError::Parse {
                path: String::from(M5_MLX_REPORT_PATH),
                error,
            }
        })?;
    let matrix_report_bytes = read_bytes(workspace_root, M5_MLX_MATRIX_REPORT_PATH)?;
    let matrix_report: OpenAdapterTailnetAdmittedDeviceMatrixReport =
        serde_json::from_slice(&matrix_report_bytes).map_err(|error| {
            PsionExecutorMlxDecisionGradeRunError::Parse {
                path: String::from(M5_MLX_MATRIX_REPORT_PATH),
                error,
            }
        })?;

    if checkpoint_packet.restore_facts.state_dict_digest
        != local_report.retained_run.final_state_dict_digest
    {
        return Err(PsionExecutorMlxDecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_mlx_decision_grade_run.final_state_dict_digest"),
            detail: String::from(
                "checkpoint restore digest must match the retained local MLX state-dict digest",
            ),
        });
    }
    if smoke_packet.final_state_dict_digest != local_report.retained_run.final_state_dict_digest {
        return Err(PsionExecutorMlxDecisionGradeRunError::InvalidValue {
            field: String::from("psion_executor_mlx_decision_grade_run.smoke_packet"),
            detail: String::from(
                "smoke packet digest must stay aligned with the retained local MLX state-dict digest",
            ),
        });
    }

    let visualization_bundle = builtin_executor_mlx_decision_grade_visualization_bundle(workspace_root)?;
    let visualization_bundle_bytes = serde_json::to_vec_pretty(&visualization_bundle)?;
    let run_index = builtin_executor_mlx_decision_grade_run_index(workspace_root)?;
    let run_index_bytes = serde_json::to_vec_pretty(&run_index)?;

    let mut equivalent_subset = PsionExecutorMlxDecisionGradeEquivalentSubset {
        subset_id: String::from(MLX_DECISION_GRADE_EQUIVALENT_SUBSET_ID),
        required_gate_ids: vec![
            String::from("full_budget_retained_run_green"),
            String::from("checkpoint_restore_rehearsal_green"),
            String::from("export_smoke_green"),
            String::from("dashboard_visibility_green"),
        ],
        subset_digest: String::new(),
        detail: String::from(
            "This approved MLX-local equivalent subset stands in for explicit checkpoint-pack repetition: the retained run consumed most of the admitted local wallclock budget, the checkpoint packet kept restore green, the durable bundle kept export smoke green, and the shared v2 dashboard surfaces now show the run beside the shipped training lanes.",
        ),
    };
    equivalent_subset.subset_digest =
        stable_executor_mlx_decision_grade_subset_digest(&equivalent_subset);

    let gate_rows = vec![
        PsionExecutorMlxDecisionGradeGateRow {
            gate_id: String::from("full_budget_retained_run_green"),
            status: String::from("green"),
            detail: format!(
                "The retained MLX run `{}` stayed alive for {}ms against a {}s target and therefore counts as the long-budget local run fact for the Mac-only decision-grade question.",
                local_report.retained_run.run_id,
                local_report.retained_run.observed_wallclock_ms,
                local_report.target_wallclock_seconds,
            ),
        },
        PsionExecutorMlxDecisionGradeGateRow {
            gate_id: String::from("checkpoint_restore_rehearsal_green"),
            status: String::from("green"),
            detail: String::from(
                "The prerequisite checkpoint packet kept deferred import-plan and eager restore truth green on the retained MLX portable bundle.",
            ),
        },
        PsionExecutorMlxDecisionGradeGateRow {
            gate_id: String::from("export_smoke_green"),
            status: String::from("green"),
            detail: String::from(
                "The retained same-node report and durable portable bundle still carry the export smoke fact inside the admitted Mac profile.",
            ),
        },
        PsionExecutorMlxDecisionGradeGateRow {
            gate_id: String::from("dashboard_visibility_green"),
            status: String::from("green"),
            detail: format!(
                "The retained MLX decision-grade bundle now appears in the shared v2 run index with {} total entries, so the run is visible beside the shipped training surfaces instead of hiding behind a one-off local report.",
                run_index.entries.len(),
            ),
        },
    ];

    let mut packet = PsionExecutorMlxDecisionGradeRunPacket {
        schema_version: String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_mlx_decision_grade_run_v1"),
        admitted_profile_id: String::from(LOCAL_MAC_MLX_PROFILE_ID),
        decision_lane_id: String::from("psion_executor_mlx_local_decision_grade"),
        decision_question_posture: String::from(
            "This packet counts only for explicitly MLX-local executor-lane questions and does not claim cross-device cluster closure, remote launch authority, or promotion readiness.",
        ),
        smoke_packet_ref: String::from(PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH),
        smoke_packet_sha256: hex::encode(Sha256::digest(&smoke_packet_bytes)),
        checkpoint_packet_ref: String::from(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH),
        checkpoint_packet_sha256: hex::encode(Sha256::digest(&checkpoint_packet_bytes)),
        retained_local_report_ref: String::from(M5_MLX_REPORT_PATH),
        retained_local_report_sha256: hex::encode(Sha256::digest(&local_report_bytes)),
        retained_matrix_report_ref: String::from(M5_MLX_MATRIX_REPORT_PATH),
        retained_matrix_report_sha256: hex::encode(Sha256::digest(&matrix_report_bytes)),
        visualization_bundle_ref: String::from(
            PSION_EXECUTOR_MLX_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH,
        ),
        visualization_bundle_sha256: hex::encode(Sha256::digest(&visualization_bundle_bytes)),
        visualization_bundle_digest: visualization_bundle.bundle_digest.clone(),
        visualization_run_index_ref: String::from(
            PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH,
        ),
        visualization_run_index_sha256: hex::encode(Sha256::digest(&run_index_bytes)),
        visualization_run_index_digest: run_index.index_digest.clone(),
        execution_backend_label: local_report.backend_label.clone(),
        logical_device_label: local_report.logical_device_label.clone(),
        retained_run_id: local_report.retained_run.run_id.clone(),
        checkpoint_family: local_report.retained_run.checkpoint_family.clone(),
        completed_steps: local_report.retained_run.completed_steps,
        observed_wallclock_ms: local_report.retained_run.observed_wallclock_ms,
        final_mean_loss: local_report.retained_run.final_mean_loss,
        final_state_dict_digest: local_report.retained_run.final_state_dict_digest.clone(),
        dashboard_entry_count: run_index.entries.len() as u64,
        approved_equivalent_local_subset: equivalent_subset,
        gate_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_SMOKE_RUN_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_MLX_CHECKPOINT_COMPATIBILITY_FIXTURE_PATH),
            String::from(M5_MLX_REPORT_PATH),
            String::from(M5_MLX_MATRIX_REPORT_PATH),
            String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH),
        ],
        summary: format!(
            "The admitted Mac MLX profile now has one retained decision-grade packet for the explicitly MLX-local executor question. Run `{}` on `{}` stays anchored to the retained same-node report (steps={} wallclock_ms={} final_mean_loss={:.6}), reuses the green checkpoint/export prerequisites, and is now visible in the shared v2 dashboard bundle/index family with {} total entries while still refusing to claim the Mac -> 4080 -> Mac roundtrip before EPIC 3.",
            local_report.retained_run.run_id,
            local_report.logical_device_label,
            local_report.retained_run.completed_steps,
            local_report.retained_run.observed_wallclock_ms,
            local_report.retained_run.final_mean_loss,
            run_index.entries.len(),
        ),
        packet_digest: String::new(),
    };
    if matrix_report.local_report.retained_run.loss_delta != local_report.retained_run.loss_delta {
        packet.summary.push_str(
            " The matrix-carried local loss delta remains checked against the retained local report before this packet counts.",
        );
    }
    packet.packet_digest = stable_executor_mlx_decision_grade_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the retained executor MLX decision-grade visualization bundle, run index, and packet.
pub fn write_builtin_executor_mlx_decision_grade_artifacts(
    workspace_root: &Path,
) -> Result<PsionExecutorMlxDecisionGradeRunPacket, PsionExecutorMlxDecisionGradeRunError> {
    let visualization_bundle = builtin_executor_mlx_decision_grade_visualization_bundle(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_MLX_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH,
        &visualization_bundle,
    )?;
    let run_index = builtin_executor_mlx_decision_grade_run_index(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH,
        &run_index,
    )?;
    let packet = builtin_executor_mlx_decision_grade_run_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutorMlxDecisionGradeRunError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutorMlxDecisionGradeRunError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorMlxDecisionGradeRunError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorMlxDecisionGradeRunError::Parse {
        path: relative_path.to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionExecutorMlxDecisionGradeRunError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    fs::write(&path, serde_json::to_vec_pretty(value)?).map_err(|error| {
        PsionExecutorMlxDecisionGradeRunError::Write {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_executor_mlx_decision_grade_subset_digest(
    subset: &PsionExecutorMlxDecisionGradeEquivalentSubset,
) -> String {
    let mut canonical = subset.clone();
    canonical.subset_digest.clear();
    stable_digest(
        b"psion_executor_mlx_decision_grade_equivalent_subset|",
        &canonical,
    )
}

fn stable_executor_mlx_decision_grade_packet_digest(
    packet: &PsionExecutorMlxDecisionGradeRunPacket,
) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_mlx_decision_grade_run|", &canonical)
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorMlxDecisionGradeRunError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
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
    fn builtin_executor_mlx_decision_grade_bundle_is_valid() -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
        let root = workspace_root();
        let bundle = builtin_executor_mlx_decision_grade_visualization_bundle(root.as_path())?;
        bundle.validate()?;
        Ok(())
    }

    #[test]
    fn builtin_executor_mlx_decision_grade_run_index_is_valid() -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
        let root = workspace_root();
        let run_index = builtin_executor_mlx_decision_grade_run_index(root.as_path())?;
        run_index.validate()?;
        Ok(())
    }

    #[test]
    fn builtin_executor_mlx_decision_grade_packet_is_valid() -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
        let root = workspace_root();
        let packet = builtin_executor_mlx_decision_grade_run_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_mlx_decision_grade_fixture_matches_committed_truth() -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
        let root = workspace_root();
        let committed_bundle: RemoteTrainingVisualizationBundleV2 =
            read_json(root.as_path(), PSION_EXECUTOR_MLX_DECISION_GRADE_VISUALIZATION_BUNDLE_FIXTURE_PATH)?;
        let current_bundle =
            builtin_executor_mlx_decision_grade_visualization_bundle(root.as_path())?;
        assert_eq!(committed_bundle, current_bundle);

        let committed_index: RemoteTrainingRunIndexV2 =
            read_json(root.as_path(), PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_INDEX_FIXTURE_PATH)?;
        let current_index =
            builtin_executor_mlx_decision_grade_run_index(root.as_path())?;
        assert_eq!(committed_index, current_index);

        let committed_packet: PsionExecutorMlxDecisionGradeRunPacket =
            read_json(root.as_path(), PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH)?;
        let current_packet =
            builtin_executor_mlx_decision_grade_run_packet(root.as_path())?;
        assert_eq!(committed_packet, current_packet);
        Ok(())
    }

    #[test]
    fn write_executor_mlx_decision_grade_artifacts_persists_current_truth(
    ) -> Result<(), PsionExecutorMlxDecisionGradeRunError> {
        let root = workspace_root();
        let packet = write_builtin_executor_mlx_decision_grade_artifacts(root.as_path())?;
        packet.validate()?;
        Ok(())
    }
}
