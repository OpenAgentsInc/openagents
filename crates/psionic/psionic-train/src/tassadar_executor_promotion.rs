use std::{collections::BTreeMap, env, fs, path::Path, time::Instant};

use psionic_eval::{EvalArtifact, TassadarExecutorBoundaryExactnessReport};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_EXECUTOR_BOUNDARY_EXACTNESS_REPORT_FILE, TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE,
    TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE, TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE,
    TASSADAR_EXECUTOR_PROMOTION_RUN_OUTPUT_DIR,
    TassadarExecutorCheckpointArtifact, TassadarExecutorExactTraceSampleReport,
    TassadarExecutorModelArtifact, TassadarExecutorReferenceRunBundle, TassadarExecutorRunError,
    TassadarExecutorTelemetryError, TassadarExecutorTrainingReport,
    augment_tassadar_training_run_with_telemetry, execute_tassadar_promotion_training_run,
    execute_tassadar_promotion_v2_training_run,
};

const TRAINING_REPORT_FILE: &str = "training_report.json";
const CHECKPOINT_ARTIFACT_FILE: &str = "checkpoint_artifact.json";
const MODEL_ARTIFACT_FILE: &str = "model_artifact.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";

/// Canonical best-checkpoint manifest artifact file for promotion review.
pub const TASSADAR_EXECUTOR_BEST_CHECKPOINT_MANIFEST_FILE: &str = "best_checkpoint_manifest.json";
/// Canonical promotion-gate report artifact file.
pub const TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE: &str = "promotion_gate_report.json";

const REQUIRED_FIRST_TARGET_EXACTNESS_BPS: u32 = 10_000;
const REQUIRED_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE: u32 = 9_000;
const REQUIRED_EXACT_TRACE_CASE_COUNT: u32 = 1;

fn tassadar_progress_updates_enabled() -> bool {
    match env::var("OPENAGENTS_TASSADAR_PROGRESS") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "off" | "no")
        }
        Err(_) => !cfg!(test),
    }
}

fn emit_tassadar_progress(message: impl AsRef<str>) {
    if tassadar_progress_updates_enabled() {
        eprintln!("{}", message.as_ref());
    }
}

/// First-class best-checkpoint manifest for promotion review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBestCheckpointManifest {
    /// Stable run identifier.
    pub run_id: String,
    /// Active trainable surface.
    pub trainable_surface: String,
    /// Selected checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Stable checkpoint reference.
    pub checkpoint_ref: String,
    /// Selection basis recorded by the trainer.
    pub selection_basis: String,
    /// Curriculum stage that produced the checkpoint.
    pub selected_stage_id: String,
    /// Zero-based global epoch index for the checkpoint.
    pub global_epoch_index: u32,
    /// First-target exactness at the selected checkpoint.
    pub first_target_exactness_bps: u32,
    /// First-32 exactness at the selected checkpoint.
    pub first_32_token_exactness_bps: u32,
    /// Exact validation trace count at the selected checkpoint.
    pub exact_trace_case_count: u32,
    /// Relative path to the checkpoint artifact.
    pub checkpoint_artifact_ref: String,
    /// Relative path to the model artifact.
    pub model_artifact_ref: String,
    /// Relative path to the exactness curve artifact.
    pub exactness_curve_ref: String,
    /// Relative path to the failure-sample artifact.
    pub failure_samples_ref: String,
    /// Relative path to the exact-trace-sample artifact.
    pub exact_trace_samples_ref: String,
    /// Stable manifest digest.
    pub report_digest: String,
}

impl TassadarExecutorBestCheckpointManifest {
    fn new(
        run_bundle: &TassadarExecutorReferenceRunBundle,
        training_report: &TassadarExecutorTrainingReport,
        checkpoint_artifact: &TassadarExecutorCheckpointArtifact,
        selected_epoch: &crate::TassadarExecutorTrainingEpochReport,
    ) -> Self {
        let mut manifest = Self {
            run_id: run_bundle.run_id.clone(),
            trainable_surface: run_bundle.trainable_surface.label().to_string(),
            checkpoint_id: checkpoint_artifact.checkpoint_id.clone(),
            checkpoint_family: checkpoint_artifact.checkpoint_family.clone(),
            checkpoint_ref: checkpoint_artifact.checkpoint_ref.clone(),
            selection_basis: training_report.checkpoint_selection_basis.clone(),
            selected_stage_id: selected_epoch.stage_id.clone(),
            global_epoch_index: selected_epoch.global_epoch_index,
            first_target_exactness_bps: selected_epoch.evaluation.first_target_exactness_bps,
            first_32_token_exactness_bps: selected_epoch.evaluation.first_32_token_exactness_bps,
            exact_trace_case_count: selected_epoch.evaluation.exact_trace_case_count,
            checkpoint_artifact_ref: String::from(CHECKPOINT_ARTIFACT_FILE),
            model_artifact_ref: String::from(MODEL_ARTIFACT_FILE),
            exactness_curve_ref: String::from(TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE),
            failure_samples_ref: String::from(TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE),
            exact_trace_samples_ref: String::from(TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE),
            report_digest: String::new(),
        };
        manifest.report_digest = stable_digest(
            b"psionic_tassadar_executor_best_checkpoint_manifest|",
            &manifest,
        );
        manifest
    }
}

/// One failed promotion threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorPromotionGateFailureKind {
    /// First target exactness missed the required `10000` bps bar.
    FirstTargetExactnessBelowThreshold,
    /// First-32 exactness did not exceed the required bar.
    First32TokenExactnessBelowThreshold,
    /// Exact validation trace count stayed below the required count.
    ExactTraceCountBelowThreshold,
}

/// One failed promotion threshold with concrete observed values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorPromotionGateFailure {
    /// Stable failure kind.
    pub kind: TassadarExecutorPromotionGateFailureKind,
    /// Observed value.
    pub actual: u32,
    /// Required threshold value.
    pub required: u32,
}

/// Machine-readable report for the 4x4 learned-lane promotion gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorPromotionGateReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Active trainable surface.
    pub trainable_surface: String,
    /// Selected checkpoint identifier.
    pub checkpoint_id: String,
    /// Stage that produced the selected checkpoint.
    pub selected_stage_id: String,
    /// First-target exactness at the selected checkpoint.
    pub first_target_exactness_bps: u32,
    /// First-32 exactness at the selected checkpoint.
    pub first_32_token_exactness_bps: u32,
    /// Exact validation trace count at the selected checkpoint.
    pub exact_trace_case_count: u32,
    /// Required first-target exactness.
    pub required_first_target_exactness_bps: u32,
    /// Required strict-lower bound for first-32 exactness.
    pub required_first_32_token_exactness_bps_strictly_greater_than: u32,
    /// Required exact validation trace count.
    pub required_exact_trace_case_count: u32,
    /// Whether the candidate cleared all promotion thresholds.
    pub passed: bool,
    /// Concrete threshold failures when the gate did not pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<TassadarExecutorPromotionGateFailure>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarExecutorPromotionGateReport {
    fn new(
        run_bundle: &TassadarExecutorReferenceRunBundle,
        selected_epoch: &crate::TassadarExecutorTrainingEpochReport,
        boundary_report: &TassadarExecutorBoundaryExactnessReport,
    ) -> Self {
        let mut failures = Vec::new();
        if boundary_report.first_target_exactness_bps < REQUIRED_FIRST_TARGET_EXACTNESS_BPS {
            failures.push(TassadarExecutorPromotionGateFailure {
                kind: TassadarExecutorPromotionGateFailureKind::FirstTargetExactnessBelowThreshold,
                actual: boundary_report.first_target_exactness_bps,
                required: REQUIRED_FIRST_TARGET_EXACTNESS_BPS,
            });
        }
        if boundary_report.first_32_token_exactness_bps
            <= REQUIRED_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE
        {
            failures.push(TassadarExecutorPromotionGateFailure {
                kind: TassadarExecutorPromotionGateFailureKind::First32TokenExactnessBelowThreshold,
                actual: boundary_report.first_32_token_exactness_bps,
                required: REQUIRED_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE + 1,
            });
        }
        if boundary_report.exact_trace_case_count < REQUIRED_EXACT_TRACE_CASE_COUNT {
            failures.push(TassadarExecutorPromotionGateFailure {
                kind: TassadarExecutorPromotionGateFailureKind::ExactTraceCountBelowThreshold,
                actual: boundary_report.exact_trace_case_count,
                required: REQUIRED_EXACT_TRACE_CASE_COUNT,
            });
        }
        let mut report = Self {
            run_id: run_bundle.run_id.clone(),
            trainable_surface: run_bundle.trainable_surface.label().to_string(),
            checkpoint_id: selected_epoch.checkpoint_id.clone(),
            selected_stage_id: selected_epoch.stage_id.clone(),
            first_target_exactness_bps: boundary_report.first_target_exactness_bps,
            first_32_token_exactness_bps: boundary_report.first_32_token_exactness_bps,
            exact_trace_case_count: boundary_report.exact_trace_case_count,
            required_first_target_exactness_bps: REQUIRED_FIRST_TARGET_EXACTNESS_BPS,
            required_first_32_token_exactness_bps_strictly_greater_than:
                REQUIRED_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE,
            required_exact_trace_case_count: REQUIRED_EXACT_TRACE_CASE_COUNT,
            passed: failures.is_empty(),
            failures,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_executor_promotion_gate_report|", &report);
        report
    }
}

/// Errors while materializing promotion artifacts for one persisted Tassadar run.
#[derive(Debug, Error)]
pub enum TassadarExecutorPromotionError {
    /// Base run execution failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Telemetry augmentation failed.
    #[error(transparent)]
    Telemetry(#[from] TassadarExecutorTelemetryError),
    /// Reading a persisted JSON artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// Path read.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding a persisted JSON artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Path read.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Writing one persisted JSON artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// Path written.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// The selected checkpoint could not be found in the training report.
    #[error("training report `{run_id}` is missing the selected checkpoint `{checkpoint_id}`")]
    MissingSelectedCheckpoint {
        /// Stable run identifier.
        run_id: String,
        /// Stable checkpoint identifier.
        checkpoint_id: String,
    },
}

/// Executes the canonical Phase 14 training run and augments it with promotion artifacts.
pub fn execute_tassadar_promotion_training_run_with_artifacts(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorPromotionError> {
    let started_at = Instant::now();
    emit_tassadar_progress(format!(
        "tassadar_progress phase=promotion_run_start output_dir={} elapsed_ms=0",
        output_dir.display(),
    ));
    execute_tassadar_promotion_training_run(output_dir)?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=promotion_artifact_augment_start output_dir={} elapsed_ms={}",
        output_dir.display(),
        started_at.elapsed().as_millis(),
    ));
    let bundle = augment_tassadar_training_run_with_promotion_artifacts(output_dir)?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=promotion_run_complete run={} bundle_digest={} elapsed_ms={}",
        bundle.run_id,
        bundle.bundle_digest,
        started_at.elapsed().as_millis(),
    ));
    Ok(bundle)
}

/// Executes the teacher-forced promotion follow-on run and augments it with the
/// same promotion artifacts as Phase 14.
pub fn execute_tassadar_promotion_v2_training_run_with_artifacts(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorPromotionError> {
    let started_at = Instant::now();
    emit_tassadar_progress(format!(
        "tassadar_progress phase=promotion_run_start output_dir={} elapsed_ms=0",
        output_dir.display(),
    ));
    execute_tassadar_promotion_v2_training_run(output_dir)?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=promotion_artifact_augment_start output_dir={} elapsed_ms={}",
        output_dir.display(),
        started_at.elapsed().as_millis(),
    ));
    let bundle = augment_tassadar_training_run_with_promotion_artifacts(output_dir)?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=promotion_run_complete run={} bundle_digest={} elapsed_ms={}",
        bundle.run_id,
        bundle.bundle_digest,
        started_at.elapsed().as_millis(),
    ));
    Ok(bundle)
}

/// Augments one persisted Tassadar run with best-checkpoint and promotion-gate artifacts.
pub fn augment_tassadar_training_run_with_promotion_artifacts(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorPromotionError> {
    let started_at = Instant::now();
    emit_tassadar_progress(format!(
        "tassadar_progress phase=telemetry_augment_start output_dir={} elapsed_ms=0",
        output_dir.display(),
    ));
    let run_bundle = augment_tassadar_training_run_with_telemetry(output_dir)?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=telemetry_augment_complete run={} output_dir={} elapsed_ms={}",
        run_bundle.run_id,
        output_dir.display(),
        started_at.elapsed().as_millis(),
    ));
    let training_report: TassadarExecutorTrainingReport = read_json(
        output_dir.join(TRAINING_REPORT_FILE),
        "tassadar_training_report",
    )?;
    let checkpoint_artifact: TassadarExecutorCheckpointArtifact = read_json(
        output_dir.join(CHECKPOINT_ARTIFACT_FILE),
        "tassadar_checkpoint_artifact",
    )?;
    let _model_artifact: TassadarExecutorModelArtifact = read_json(
        output_dir.join(MODEL_ARTIFACT_FILE),
        "tassadar_model_artifact",
    )?;
    let boundary_report: TassadarExecutorBoundaryExactnessReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_BOUNDARY_EXACTNESS_REPORT_FILE),
        "tassadar_boundary_exactness_report",
    )?;
    let _exact_trace_samples: TassadarExecutorExactTraceSampleReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE),
        "tassadar_exact_trace_samples",
    )?;
    let selected_epoch = training_report
        .epoch_reports
        .iter()
        .find(|epoch| epoch.checkpoint_id == training_report.best_checkpoint_id)
        .ok_or_else(
            || TassadarExecutorPromotionError::MissingSelectedCheckpoint {
                run_id: training_report.config.run_id.clone(),
                checkpoint_id: training_report.best_checkpoint_id.clone(),
            },
        )?;
    let best_checkpoint_manifest = TassadarExecutorBestCheckpointManifest::new(
        &run_bundle,
        &training_report,
        &checkpoint_artifact,
        selected_epoch,
    );
    let promotion_gate_report =
        TassadarExecutorPromotionGateReport::new(&run_bundle, selected_epoch, &boundary_report);
    emit_tassadar_progress(format!(
        "tassadar_progress phase=promotion_gate_evaluated run={} checkpoint_id={} stage_id={} passed={} first_target_bps={} first_32_bps={} exact_traces={} failure_count={} elapsed_ms={}",
        run_bundle.run_id,
        selected_epoch.checkpoint_id,
        selected_epoch.stage_id,
        promotion_gate_report.passed,
        promotion_gate_report.first_target_exactness_bps,
        promotion_gate_report.first_32_token_exactness_bps,
        promotion_gate_report.exact_trace_case_count,
        promotion_gate_report.failures.len(),
        started_at.elapsed().as_millis(),
    ));

    let mut artifact_map = run_bundle
        .artifacts
        .iter()
        .cloned()
        .map(|artifact| (artifact.artifact_ref.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    let best_checkpoint_artifact = write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_BEST_CHECKPOINT_MANIFEST_FILE,
        "tassadar_best_checkpoint_manifest",
        &best_checkpoint_manifest,
    )?;
    artifact_map.insert(
        best_checkpoint_artifact.artifact_ref.clone(),
        best_checkpoint_artifact,
    );
    let promotion_gate_artifact = write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE,
        "tassadar_promotion_gate_report",
        &promotion_gate_report,
    )?;
    artifact_map.insert(
        promotion_gate_artifact.artifact_ref.clone(),
        promotion_gate_artifact,
    );

    let mut updated_bundle = run_bundle;
    updated_bundle.artifacts = artifact_map.into_values().collect();
    updated_bundle.best_checkpoint_manifest_digest =
        Some(best_checkpoint_manifest.report_digest.clone());
    updated_bundle.promotion_gate_report_digest = Some(promotion_gate_report.report_digest.clone());
    updated_bundle.bundle_digest.clear();
    updated_bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_executor_reference_run_bundle|",
        &updated_bundle,
    );
    write_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
        &updated_bundle,
    )?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=promotion_artifacts_written run={} bundle_digest={} artifact_count={} elapsed_ms={}",
        updated_bundle.run_id,
        updated_bundle.bundle_digest,
        updated_bundle.artifacts.len(),
        started_at.elapsed().as_millis(),
    ));
    Ok(updated_bundle)
}

/// Augments the canonical Phase 14 run location with promotion artifacts.
pub fn augment_tassadar_promotion_run_with_artifacts(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorPromotionError> {
    augment_tassadar_training_run_with_promotion_artifacts(output_dir)
}

/// Returns the canonical repo-relative output root for the Phase 14 promotion run.
#[must_use]
pub const fn tassadar_promotion_run_output_dir() -> &'static str {
    TASSADAR_EXECUTOR_PROMOTION_RUN_OUTPUT_DIR
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarExecutorPromotionError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarExecutorPromotionError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarExecutorPromotionError::Deserialize {
        artifact_kind: artifact_kind.to_string(),
        path: path.display().to_string(),
        error,
    })
}

fn write_json_artifact<T>(
    output_dir: &Path,
    relative_path: &str,
    artifact_kind: &str,
    value: &T,
) -> Result<EvalArtifact, TassadarExecutorPromotionError>
where
    T: Serialize,
{
    let path = output_dir.join(relative_path);
    let bytes = serde_json::to_vec_pretty(value).expect("promotion artifact should serialize");
    fs::write(&path, &bytes).map_err(|error| TassadarExecutorPromotionError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(EvalArtifact::new(
        artifact_kind,
        relative_path,
        bytes.as_slice(),
    ))
}

fn write_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
    value: &T,
) -> Result<(), TassadarExecutorPromotionError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).expect("promotion bundle should serialize");
    fs::write(path, &bytes).map_err(|error| TassadarExecutorPromotionError::Write {
        path: path.display().to_string(),
        error,
    })?;
    let _ = artifact_kind;
    Ok(())
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let bytes = serde_json::to_vec(value).expect("promotion value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes.as_slice());
    format!("{:x}", hasher.finalize())
}
