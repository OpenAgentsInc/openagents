use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use psionic_eval::{EvalArtifact, TassadarExecutorEvalWindowMode, TassadarExecutorWindowedEvalReport};
use psionic_runtime::TassadarClaimClass;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_EXECUTOR_BEST_CHECKPOINT_MANIFEST_FILE, TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE,
    TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE, TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE,
    TASSADAR_EXECUTOR_LATER_WINDOW_EXACTNESS_FILE, TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE,
    TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE, TASSADAR_EXECUTOR_SUFFIX_WINDOW_FAILURE_FILE,
    TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE,
    TassadarExecutorBestCheckpointManifest, TassadarExecutorCheckpointArtifact,
    TassadarExecutorModelArtifact, TassadarExecutorReferenceRunBundle,
    TassadarExecutorSequenceFitDisposition, TassadarExecutorSequenceFitReport,
    TassadarExecutorTrainingEpochReport, TassadarExecutorTrainingReport,
    TassadarSudoku9x9LaterWindowExactnessReport, TassadarSudoku9x9SuffixWindowFailureReport,
};

const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const TRAINING_REPORT_FILE: &str = "training_report.json";
const CHECKPOINT_ARTIFACT_FILE: &str = "checkpoint_artifact.json";
const MODEL_ARTIFACT_FILE: &str = "model_artifact.json";
const PROMOTION_BUNDLE_FILE: &str = "promotion_bundle.json";

const REQUIRED_WINDOW_FIRST_TARGET_EXACTNESS_BPS: u32 = 10_000;
const REQUIRED_WINDOW_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE: u32 = 9_000;
const REQUIRED_EXACT_WINDOW_CASE_COUNT: u32 = 1;
const REQUIRED_FULL_TRACE_EXACT_CASE_COUNT: u32 = 1;

/// Stable schema version for the learned 9x9 promotion-gate checker report.
pub const TASSADAR_EXECUTOR_9X9_PROMOTION_GATE_CHECK_SCHEMA_VERSION: u16 = 1;

const fn default_learned_bounded_claim_class() -> TassadarClaimClass {
    TassadarClaimClass::LearnedBounded
}

/// Canonical bundle that freezes the learned 9x9 promotion-gate evidence set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9PromotionBundle {
    /// Stable run identifier.
    pub run_id: String,
    /// Coarse claim class for the learned 9x9 lane.
    #[serde(default = "default_learned_bounded_claim_class")]
    pub claim_class: TassadarClaimClass,
    /// Stable model identifier from the underlying learned run.
    pub model_id: String,
    /// Repo-relative output directory.
    pub output_dir: String,
    /// Relative persisted reference run bundle.
    pub reference_run_bundle_file: String,
    /// Relative persisted training report.
    pub training_report_file: String,
    /// Relative persisted checkpoint artifact.
    pub checkpoint_artifact_file: String,
    /// Relative persisted model artifact.
    pub model_artifact_file: String,
    /// Relative persisted best-checkpoint manifest.
    pub best_checkpoint_manifest_file: String,
    /// Relative persisted sequence-fit report.
    pub sequence_fit_report_file: String,
    /// Relative persisted exactness-curve artifact.
    pub exactness_curve_file: String,
    /// Relative persisted failure-sample artifact.
    pub failure_samples_file: String,
    /// Relative persisted exact-trace-sample artifact.
    pub exact_trace_samples_file: String,
    /// Relative persisted trace-divergence artifact.
    pub trace_divergence_report_file: String,
    /// Relative persisted later-window exactness artifact.
    pub later_window_exactness_file: String,
    /// Relative persisted suffix-window failure artifact.
    pub suffix_window_failure_file: String,
    /// Relative persisted promotion-gate report.
    pub promotion_gate_report_file: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl TassadarSudoku9x9PromotionBundle {
    fn new(run_bundle: &TassadarExecutorReferenceRunBundle, output_dir: &Path) -> Self {
        let mut bundle = Self {
            run_id: run_bundle.run_id.clone(),
            claim_class: TassadarClaimClass::LearnedBounded,
            model_id: String::from("tassadar-executor-transformer-sudoku-9x9-windowed-v0"),
            output_dir: repo_relative_path(output_dir),
            reference_run_bundle_file: String::from(RUN_BUNDLE_FILE),
            training_report_file: String::from(TRAINING_REPORT_FILE),
            checkpoint_artifact_file: String::from(CHECKPOINT_ARTIFACT_FILE),
            model_artifact_file: String::from(MODEL_ARTIFACT_FILE),
            best_checkpoint_manifest_file: String::from(
                TASSADAR_EXECUTOR_BEST_CHECKPOINT_MANIFEST_FILE,
            ),
            sequence_fit_report_file: String::from(TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE),
            exactness_curve_file: String::from(TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE),
            failure_samples_file: String::from(TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE),
            exact_trace_samples_file: String::from(TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE),
            trace_divergence_report_file: String::from(TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE),
            later_window_exactness_file: String::from(TASSADAR_EXECUTOR_LATER_WINDOW_EXACTNESS_FILE),
            suffix_window_failure_file: String::from(TASSADAR_EXECUTOR_SUFFIX_WINDOW_FAILURE_FILE),
            promotion_gate_report_file: String::from(TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest =
            stable_digest(b"psionic_tassadar_sudoku_9x9_promotion_bundle|", &bundle);
        bundle
    }
}

/// One promotion-gate window summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9PromotionGateWindowSummary {
    /// Stable window identifier.
    pub window_id: String,
    /// Window mode.
    pub mode: TassadarExecutorEvalWindowMode,
    /// Requested start offset when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_target_token_index: Option<u32>,
    /// Target-token count evaluated for the window.
    pub target_token_count: u32,
    /// Aggregate token exactness across the window.
    pub aggregate_target_token_exactness_bps: u32,
    /// Boundary exactness over the first target token inside the window.
    pub first_target_exactness_bps: u32,
    /// Boundary exactness over the first eight target tokens inside the window.
    pub first_8_token_exactness_bps: u32,
    /// Boundary exactness over the first 32 target tokens inside the window.
    pub first_32_token_exactness_bps: u32,
    /// Exact-window case count.
    pub exact_window_case_count: u32,
    /// Full-trace exact case count surfaced by the windowed eval.
    pub full_trace_exact_case_count: u32,
    /// Final-output exact case count surfaced by the windowed eval.
    pub final_output_exact_case_count: u32,
    /// Halt exact case count surfaced by the windowed eval.
    pub halt_exact_case_count: u32,
}

impl TassadarSudoku9x9PromotionGateWindowSummary {
    fn from_report(report: &TassadarExecutorWindowedEvalReport) -> Self {
        Self {
            window_id: report.window.window_id.clone(),
            mode: report.window.mode,
            start_target_token_index: report.window.start_target_token_index,
            target_token_count: report.window.target_token_count,
            aggregate_target_token_exactness_bps: report.aggregate_target_token_exactness_bps,
            first_target_exactness_bps: report.first_target_exactness_bps,
            first_8_token_exactness_bps: report.first_8_token_exactness_bps,
            first_32_token_exactness_bps: report.first_32_token_exactness_bps,
            exact_window_case_count: report.exact_window_case_count,
            full_trace_exact_case_count: report.full_trace_exact_case_count,
            final_output_exact_case_count: report.final_output_exact_case_count,
            halt_exact_case_count: report.halt_exact_case_count,
        }
    }
}

/// One failed learned 9x9 promotion-gate threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSudoku9x9PromotionGateFailureKind {
    /// The declared learned 9x9 lane still cannot fit full honest sequences.
    FullSequenceFitUnavailable,
    /// The early prefix window missed the first-target exactness bar.
    EarlyPrefixFirstTargetExactnessBelowThreshold,
    /// The early prefix window missed the first-32 exactness bar.
    EarlyPrefixFirst32TokenExactnessBelowThreshold,
    /// The early prefix window still has zero exact validation windows.
    EarlyPrefixExactWindowCountBelowThreshold,
    /// The later offset window missed the first-target exactness bar.
    LaterOffsetFirstTargetExactnessBelowThreshold,
    /// The later offset window missed the first-32 exactness bar.
    LaterOffsetFirst32TokenExactnessBelowThreshold,
    /// The later offset window still has zero exact validation windows.
    LaterOffsetExactWindowCountBelowThreshold,
    /// The furthest fittable suffix window missed the first-target exactness bar.
    SuffixWindowFirstTargetExactnessBelowThreshold,
    /// The furthest fittable suffix window missed the first-32 exactness bar.
    SuffixWindowFirst32TokenExactnessBelowThreshold,
    /// The furthest fittable suffix window still has zero exact validation windows.
    SuffixWindowExactWindowCountBelowThreshold,
    /// No declared gate window preserved a full exact trace.
    FullTraceExactCaseCountBelowThreshold,
}

/// One failed promotion threshold with concrete observed values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9PromotionGateFailure {
    /// Stable failure kind.
    pub kind: TassadarSudoku9x9PromotionGateFailureKind,
    /// Observed numeric value.
    pub actual: u32,
    /// Required numeric threshold.
    pub required: u32,
}

/// Machine-readable learned 9x9 promotion-gate report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9PromotionGateReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Coarse claim class for the learned 9x9 lane.
    #[serde(default = "default_learned_bounded_claim_class")]
    pub claim_class: TassadarClaimClass,
    /// Active trainable surface.
    pub trainable_surface: String,
    /// Selected checkpoint identifier.
    pub checkpoint_id: String,
    /// Stage that produced the selected checkpoint.
    pub selected_stage_id: String,
    /// Honest fit disposition for the learned 9x9 lane.
    pub fit_disposition: TassadarExecutorSequenceFitDisposition,
    /// Whether the declared contract requires one-pass full-sequence fit.
    pub one_pass_full_sequence_required_for_declared_contract: bool,
    /// Whether full honest 9x9 sequences fit the model context.
    pub full_sequence_fits_model_context: bool,
    /// Largest observed full-sequence overflow over model context.
    pub full_sequence_context_overflow_max: u32,
    /// Exact statement describing the current learned 9x9 scope.
    pub scope_statement: String,
    /// Early prefix window summary.
    pub early_prefix_window: TassadarSudoku9x9PromotionGateWindowSummary,
    /// Later fixed-offset window summary.
    pub later_offset_window: TassadarSudoku9x9PromotionGateWindowSummary,
    /// Furthest fittable suffix window summary.
    pub furthest_fittable_suffix_window: TassadarSudoku9x9PromotionGateWindowSummary,
    /// Maximum exact full-trace case count surfaced across the declared gate windows.
    pub full_trace_exact_case_count_across_gate_windows: u32,
    /// Required full-sequence fit flag for a green learned 9x9 gate.
    pub required_full_sequence_fit: bool,
    /// Required first-target exactness for every declared gate window.
    pub required_window_first_target_exactness_bps: u32,
    /// Required strict lower bound for first-32 exactness for every declared gate window.
    pub required_window_first_32_token_exactness_bps_strictly_greater_than: u32,
    /// Required exact-window case count for every declared gate window.
    pub required_exact_window_case_count: u32,
    /// Required full-trace exact case count across the declared gate windows.
    pub required_full_trace_exact_case_count: u32,
    /// Whether the learned 9x9 lane clears the declared promotion gate.
    pub passed: bool,
    /// Concrete threshold failures when the gate stays red.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<TassadarSudoku9x9PromotionGateFailure>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarSudoku9x9PromotionGateReport {
    fn new(
        run_bundle: &TassadarExecutorReferenceRunBundle,
        selected_epoch: &TassadarExecutorTrainingEpochReport,
        fit_report: &TassadarExecutorSequenceFitReport,
        later_window: &TassadarSudoku9x9LaterWindowExactnessReport,
    ) -> Self {
        let early_prefix_window =
            TassadarSudoku9x9PromotionGateWindowSummary::from_report(&later_window.early_prefix_window);
        let later_offset_window =
            TassadarSudoku9x9PromotionGateWindowSummary::from_report(&later_window.later_offset_window);
        let furthest_fittable_suffix_window =
            TassadarSudoku9x9PromotionGateWindowSummary::from_report(
                &later_window.furthest_fittable_suffix_window,
            );
        let full_trace_exact_case_count_across_gate_windows = [
            early_prefix_window.full_trace_exact_case_count,
            later_offset_window.full_trace_exact_case_count,
            furthest_fittable_suffix_window.full_trace_exact_case_count,
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        let failures = promotion_gate_failures(
            fit_report.fit_disposition,
            fit_report.full_sequence_fits_model_context,
            &early_prefix_window,
            &later_offset_window,
            &furthest_fittable_suffix_window,
            full_trace_exact_case_count_across_gate_windows,
        );
        let mut report = Self {
            run_id: run_bundle.run_id.clone(),
            claim_class: TassadarClaimClass::LearnedBounded,
            trainable_surface: run_bundle.trainable_surface.label().to_string(),
            checkpoint_id: selected_epoch.checkpoint_id.clone(),
            selected_stage_id: selected_epoch.stage_id.clone(),
            fit_disposition: fit_report.fit_disposition,
            one_pass_full_sequence_required_for_declared_contract:
                fit_report.one_pass_full_sequence_required_for_declared_contract,
            full_sequence_fits_model_context: fit_report.full_sequence_fits_model_context,
            full_sequence_context_overflow_max: fit_report.full_sequence_context_overflow_max,
            scope_statement: fit_report.scope_statement.clone(),
            early_prefix_window,
            later_offset_window,
            furthest_fittable_suffix_window,
            full_trace_exact_case_count_across_gate_windows,
            required_full_sequence_fit: true,
            required_window_first_target_exactness_bps:
                REQUIRED_WINDOW_FIRST_TARGET_EXACTNESS_BPS,
            required_window_first_32_token_exactness_bps_strictly_greater_than:
                REQUIRED_WINDOW_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE,
            required_exact_window_case_count: REQUIRED_EXACT_WINDOW_CASE_COUNT,
            required_full_trace_exact_case_count: REQUIRED_FULL_TRACE_EXACT_CASE_COUNT,
            passed: failures.is_empty(),
            failures,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_sudoku_9x9_promotion_gate_report|", &report);
        report
    }
}

/// Machine-readable verification report for one persisted learned 9x9
/// promotion-gate artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9PromotionGateCheckReport {
    /// Stable checker schema version.
    pub schema_version: u16,
    /// Checked report path.
    pub report_path: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Selected checkpoint identifier.
    pub checkpoint_id: String,
    /// Stage that produced the selected checkpoint.
    pub selected_stage_id: String,
    /// Honest fit disposition surfaced by the checked report.
    pub fit_disposition: TassadarExecutorSequenceFitDisposition,
    /// Stored full-sequence-fit fact.
    pub full_sequence_fits_model_context: bool,
    /// Early prefix window summary.
    pub early_prefix_window: TassadarSudoku9x9PromotionGateWindowSummary,
    /// Later offset window summary.
    pub later_offset_window: TassadarSudoku9x9PromotionGateWindowSummary,
    /// Furthest fittable suffix window summary.
    pub furthest_fittable_suffix_window: TassadarSudoku9x9PromotionGateWindowSummary,
    /// Maximum full-trace exact case count across the declared gate windows.
    pub full_trace_exact_case_count_across_gate_windows: u32,
    /// Revalidated promotion result.
    pub passed: bool,
    /// Revalidated threshold failures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<TassadarSudoku9x9PromotionGateFailure>,
    /// Stored pass/fail flag carried by the checked artifact.
    pub stored_passed: bool,
    /// Stored failures carried by the checked artifact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stored_failures: Vec<TassadarSudoku9x9PromotionGateFailure>,
    /// Digest read from the checked artifact.
    pub observed_report_digest: String,
    /// Digest recomputed from the revalidated report.
    pub recomputed_report_digest: String,
    /// Whether the stored digest matches the recomputed report digest.
    pub report_digest_matches: bool,
    /// Whether the stored `passed` field matches the revalidated gate result.
    pub passed_field_matches: bool,
    /// Whether the stored failure list matches the revalidated threshold failures.
    pub failures_match: bool,
    /// Stable digest over the checker report.
    pub check_digest: String,
}

impl TassadarSudoku9x9PromotionGateCheckReport {
    fn new(report_path: &Path, report: &TassadarSudoku9x9PromotionGateReport) -> Self {
        let failures = promotion_gate_failures(
            report.fit_disposition,
            report.full_sequence_fits_model_context,
            &report.early_prefix_window,
            &report.later_offset_window,
            &report.furthest_fittable_suffix_window,
            report.full_trace_exact_case_count_across_gate_windows,
        );
        let passed = failures.is_empty();
        let mut recomputed_report = report.clone();
        recomputed_report.passed = passed;
        recomputed_report.failures = failures.clone();
        recomputed_report.report_digest = String::new();
        recomputed_report.report_digest = stable_digest(
            b"psionic_tassadar_sudoku_9x9_promotion_gate_report|",
            &recomputed_report,
        );
        let recomputed_report_digest = recomputed_report.report_digest.clone();
        let mut check = Self {
            schema_version: TASSADAR_EXECUTOR_9X9_PROMOTION_GATE_CHECK_SCHEMA_VERSION,
            report_path: report_path.display().to_string(),
            run_id: report.run_id.clone(),
            checkpoint_id: report.checkpoint_id.clone(),
            selected_stage_id: report.selected_stage_id.clone(),
            fit_disposition: report.fit_disposition,
            full_sequence_fits_model_context: report.full_sequence_fits_model_context,
            early_prefix_window: report.early_prefix_window.clone(),
            later_offset_window: report.later_offset_window.clone(),
            furthest_fittable_suffix_window: report.furthest_fittable_suffix_window.clone(),
            full_trace_exact_case_count_across_gate_windows: report
                .full_trace_exact_case_count_across_gate_windows,
            passed,
            failures,
            stored_passed: report.passed,
            stored_failures: report.failures.clone(),
            observed_report_digest: report.report_digest.clone(),
            recomputed_report_digest: recomputed_report_digest.clone(),
            report_digest_matches: report.report_digest == recomputed_report_digest,
            passed_field_matches: report.passed == passed,
            failures_match: report.failures == recomputed_report.failures,
            check_digest: String::new(),
        };
        check.check_digest = stable_digest(
            b"psionic_tassadar_sudoku_9x9_promotion_gate_check_report|",
            &check,
        );
        check
    }

    /// Returns whether the checked report is internally consistent.
    #[must_use]
    pub const fn consistent(&self) -> bool {
        self.report_digest_matches && self.passed_field_matches && self.failures_match
    }
}

/// Errors while materializing the learned 9x9 promotion-gate bundle.
#[derive(Debug, Error)]
pub enum TassadarSudoku9x9PromotionError {
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
    /// Failed to serialize a JSON artifact.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Failed to write one persisted JSON artifact.
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

/// Augments one persisted learned 9x9 run with promotion-gate artifacts.
pub fn augment_tassadar_sudoku_9x9_run_with_promotion_artifacts(
    output_dir: &Path,
) -> Result<TassadarSudoku9x9PromotionBundle, TassadarSudoku9x9PromotionError> {
    let run_bundle: TassadarExecutorReferenceRunBundle = read_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
    )?;
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
    let fit_report: TassadarExecutorSequenceFitReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE),
        "tassadar_sequence_fit_report",
    )?;
    let later_window: TassadarSudoku9x9LaterWindowExactnessReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_LATER_WINDOW_EXACTNESS_FILE),
        "tassadar_sudoku_9x9_later_window_exactness_report",
    )?;
    let _suffix_failure: TassadarSudoku9x9SuffixWindowFailureReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_SUFFIX_WINDOW_FAILURE_FILE),
        "tassadar_sudoku_9x9_suffix_window_failure_report",
    )?;
    let selected_epoch = training_report
        .epoch_reports
        .iter()
        .find(|epoch| epoch.checkpoint_id == training_report.best_checkpoint_id)
        .ok_or_else(
            || TassadarSudoku9x9PromotionError::MissingSelectedCheckpoint {
                run_id: training_report.config.run_id.clone(),
                checkpoint_id: training_report.best_checkpoint_id.clone(),
            },
        )?;

    let best_checkpoint_manifest = build_best_checkpoint_manifest(
        &run_bundle,
        &training_report,
        &checkpoint_artifact,
        selected_epoch,
    );
    let promotion_gate_report = TassadarSudoku9x9PromotionGateReport::new(
        &run_bundle,
        selected_epoch,
        &fit_report,
        &later_window,
    );
    let promotion_bundle = TassadarSudoku9x9PromotionBundle::new(&run_bundle, output_dir);

    let mut artifact_map = run_bundle
        .artifacts
        .iter()
        .cloned()
        .map(|artifact| (artifact.artifact_ref.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    for artifact in [
        write_json_artifact(
            output_dir,
            TASSADAR_EXECUTOR_BEST_CHECKPOINT_MANIFEST_FILE,
            "tassadar_best_checkpoint_manifest",
            &best_checkpoint_manifest,
        )?,
        write_json_artifact(
            output_dir,
            TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE,
            "tassadar_sudoku_9x9_promotion_gate_report",
            &promotion_gate_report,
        )?,
        write_json_artifact(
            output_dir,
            PROMOTION_BUNDLE_FILE,
            "tassadar_sudoku_9x9_promotion_bundle",
            &promotion_bundle,
        )?,
    ] {
        artifact_map.insert(artifact.artifact_ref.clone(), artifact);
    }

    let mut updated_run_bundle = run_bundle;
    updated_run_bundle.artifacts = artifact_map.into_values().collect();
    updated_run_bundle.best_checkpoint_manifest_digest =
        Some(best_checkpoint_manifest.report_digest.clone());
    updated_run_bundle.promotion_gate_report_digest =
        Some(promotion_gate_report.report_digest.clone());
    updated_run_bundle.bundle_digest.clear();
    updated_run_bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_executor_reference_run_bundle|",
        &updated_run_bundle,
    );
    write_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
        &updated_run_bundle,
    )?;
    Ok(promotion_bundle)
}

/// Revalidates one persisted learned 9x9 promotion-gate report.
pub fn check_tassadar_sudoku_9x9_promotion_gate_report(
    report_path: &Path,
) -> Result<TassadarSudoku9x9PromotionGateCheckReport, TassadarSudoku9x9PromotionError> {
    let report: TassadarSudoku9x9PromotionGateReport =
        read_json(report_path, "tassadar_sudoku_9x9_promotion_gate_report")?;
    Ok(TassadarSudoku9x9PromotionGateCheckReport::new(
        report_path, &report,
    ))
}

fn build_best_checkpoint_manifest(
    run_bundle: &TassadarExecutorReferenceRunBundle,
    training_report: &TassadarExecutorTrainingReport,
    checkpoint_artifact: &TassadarExecutorCheckpointArtifact,
    selected_epoch: &TassadarExecutorTrainingEpochReport,
) -> TassadarExecutorBestCheckpointManifest {
    let mut manifest = TassadarExecutorBestCheckpointManifest {
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

fn push_window_failures(
    failures: &mut Vec<TassadarSudoku9x9PromotionGateFailure>,
    window: &TassadarSudoku9x9PromotionGateWindowSummary,
    first_target_kind: TassadarSudoku9x9PromotionGateFailureKind,
    first_32_kind: TassadarSudoku9x9PromotionGateFailureKind,
    exact_window_kind: TassadarSudoku9x9PromotionGateFailureKind,
) {
    if window.first_target_exactness_bps < REQUIRED_WINDOW_FIRST_TARGET_EXACTNESS_BPS {
        failures.push(TassadarSudoku9x9PromotionGateFailure {
            kind: first_target_kind,
            actual: window.first_target_exactness_bps,
            required: REQUIRED_WINDOW_FIRST_TARGET_EXACTNESS_BPS,
        });
    }
    if window.first_32_token_exactness_bps <= REQUIRED_WINDOW_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE
    {
        failures.push(TassadarSudoku9x9PromotionGateFailure {
            kind: first_32_kind,
            actual: window.first_32_token_exactness_bps,
            required: REQUIRED_WINDOW_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE + 1,
        });
    }
    if window.exact_window_case_count < REQUIRED_EXACT_WINDOW_CASE_COUNT {
        failures.push(TassadarSudoku9x9PromotionGateFailure {
            kind: exact_window_kind,
            actual: window.exact_window_case_count,
            required: REQUIRED_EXACT_WINDOW_CASE_COUNT,
        });
    }
}

fn promotion_gate_failures(
    fit_disposition: TassadarExecutorSequenceFitDisposition,
    full_sequence_fits_model_context: bool,
    early_prefix_window: &TassadarSudoku9x9PromotionGateWindowSummary,
    later_offset_window: &TassadarSudoku9x9PromotionGateWindowSummary,
    furthest_fittable_suffix_window: &TassadarSudoku9x9PromotionGateWindowSummary,
    full_trace_exact_case_count_across_gate_windows: u32,
) -> Vec<TassadarSudoku9x9PromotionGateFailure> {
    let mut failures = Vec::new();
    if fit_disposition != TassadarExecutorSequenceFitDisposition::FullSequenceFit
        || !full_sequence_fits_model_context
    {
        failures.push(TassadarSudoku9x9PromotionGateFailure {
            kind: TassadarSudoku9x9PromotionGateFailureKind::FullSequenceFitUnavailable,
            actual: u32::from(full_sequence_fits_model_context),
            required: 1,
        });
    }
    push_window_failures(
        &mut failures,
        early_prefix_window,
        TassadarSudoku9x9PromotionGateFailureKind::EarlyPrefixFirstTargetExactnessBelowThreshold,
        TassadarSudoku9x9PromotionGateFailureKind::EarlyPrefixFirst32TokenExactnessBelowThreshold,
        TassadarSudoku9x9PromotionGateFailureKind::EarlyPrefixExactWindowCountBelowThreshold,
    );
    push_window_failures(
        &mut failures,
        later_offset_window,
        TassadarSudoku9x9PromotionGateFailureKind::LaterOffsetFirstTargetExactnessBelowThreshold,
        TassadarSudoku9x9PromotionGateFailureKind::LaterOffsetFirst32TokenExactnessBelowThreshold,
        TassadarSudoku9x9PromotionGateFailureKind::LaterOffsetExactWindowCountBelowThreshold,
    );
    push_window_failures(
        &mut failures,
        furthest_fittable_suffix_window,
        TassadarSudoku9x9PromotionGateFailureKind::SuffixWindowFirstTargetExactnessBelowThreshold,
        TassadarSudoku9x9PromotionGateFailureKind::SuffixWindowFirst32TokenExactnessBelowThreshold,
        TassadarSudoku9x9PromotionGateFailureKind::SuffixWindowExactWindowCountBelowThreshold,
    );
    if full_trace_exact_case_count_across_gate_windows < REQUIRED_FULL_TRACE_EXACT_CASE_COUNT {
        failures.push(TassadarSudoku9x9PromotionGateFailure {
            kind: TassadarSudoku9x9PromotionGateFailureKind::FullTraceExactCaseCountBelowThreshold,
            actual: full_trace_exact_case_count_across_gate_windows,
            required: REQUIRED_FULL_TRACE_EXACT_CASE_COUNT,
        });
    }
    failures
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn repo_relative_path(path: &Path) -> String {
    let root = repo_root();
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarSudoku9x9PromotionError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarSudoku9x9PromotionError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSudoku9x9PromotionError::Deserialize {
            artifact_kind: artifact_kind.to_string(),
            path: path.display().to_string(),
            error,
        }
    })
}

fn write_json_artifact<T>(
    output_dir: &Path,
    relative_path: &str,
    artifact_kind: &str,
    value: &T,
) -> Result<EvalArtifact, TassadarSudoku9x9PromotionError>
where
    T: Serialize,
{
    let path = output_dir.join(relative_path);
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarSudoku9x9PromotionError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(&path, &bytes).map_err(|error| TassadarSudoku9x9PromotionError::Write {
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
) -> Result<(), TassadarSudoku9x9PromotionError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarSudoku9x9PromotionError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, &bytes).map_err(|error| TassadarSudoku9x9PromotionError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar Sudoku-9x9 promotion value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        TASSADAR_EXECUTOR_9X9_PROMOTION_GATE_CHECK_SCHEMA_VERSION,
        TassadarSudoku9x9PromotionGateFailureKind, check_tassadar_sudoku_9x9_promotion_gate_report,
    };
    use crate::TASSADAR_EXECUTOR_SUDOKU_9X9_REFERENCE_RUN_OUTPUT_DIR;

    #[test]
    fn committed_9x9_promotion_gate_report_stays_consistent_and_red() {
        let report_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(TASSADAR_EXECUTOR_SUDOKU_9X9_REFERENCE_RUN_OUTPUT_DIR)
            .join("promotion_gate_report.json");
        let check = check_tassadar_sudoku_9x9_promotion_gate_report(&report_path)
            .expect("committed 9x9 promotion gate report should deserialize");
        assert_eq!(
            check.schema_version,
            TASSADAR_EXECUTOR_9X9_PROMOTION_GATE_CHECK_SCHEMA_VERSION
        );
        assert!(check.consistent());
        assert!(!check.passed);
        assert!(
            check.failures.iter().any(|failure| {
                failure.kind == TassadarSudoku9x9PromotionGateFailureKind::FullSequenceFitUnavailable
            }),
            "fit failure should stay explicit while the learned 9x9 lane remains bounded"
        );
        assert!(
            check.failures.iter().any(|failure| {
                failure.kind
                    == TassadarSudoku9x9PromotionGateFailureKind::FullTraceExactCaseCountBelowThreshold
            }),
            "full-trace failure should remain explicit while no gate window is exact end to end"
        );
    }
}
