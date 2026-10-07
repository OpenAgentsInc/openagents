use std::{fs, path::{Path, PathBuf}};

use psionic_models::{
    TassadarExecutorLongTraceContract, TassadarExecutorTrainableSurface,
    TassadarExecutorTransformerClaimBoundary,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE, TassadarExecutorCheckpointState,
    TassadarExecutorRunError, TassadarExecutorTeacherForcedTrainingStrategy,
    TassadarExecutorTrainingConfig, TassadarExecutorTrainingReport,
    TassadarExecutorReferenceRunBundle, TassadarExecutorSequenceFitReport,
    TassadarSudoku9x9ReferenceRunError,
    augment_tassadar_sudoku_9x9_run_with_fit_and_review,
    augment_tassadar_training_run_with_telemetry,
    execute_tassadar_training_run_without_benchmark,
    tassadar_executor_sudoku_9x9_reference_run_config,
};

const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const TRAINING_REPORT_FILE: &str = "training_report.json";
const CHECKPOINT_STATE_FILE: &str = "checkpoint_state.json";
const FLAT_PREFIX_DIR: &str = "flat_prefix_full_forward";
const WINDOWED_INCREMENTAL_DIR: &str = "windowed_incremental";

/// Canonical output root for the first honest 9x9 flat-prefix-vs-windowed comparison.
pub const TASSADAR_EXECUTOR_WINDOWED_FAMILY_COMPARISON_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_windowed_family_comparison_v1";
/// Canonical machine-readable comparison artifact.
pub const TASSADAR_EXECUTOR_WINDOWED_FAMILY_COMPARISON_REPORT_FILE: &str =
    "windowed_family_comparison_report.json";

/// One persisted family summary inside the 9x9 comparison root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorLongTraceFamilySummary {
    /// Stable family label inside the comparison root.
    pub family_id: String,
    /// Relative family directory under the comparison root.
    pub run_directory: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Stable model-family label.
    pub model_family: String,
    /// Stable trained descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Stable trained weight digest.
    pub trained_weight_digest: String,
    /// Trainable surface used by the run.
    pub trainable_surface: TassadarExecutorTrainableSurface,
    /// Teacher-forced strategy used by the run.
    pub teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
    /// Explicit long-trace contract advertised by the family.
    pub long_trace_contract: TassadarExecutorLongTraceContract,
    /// Explicit model claim boundary.
    pub claim_boundary: TassadarExecutorTransformerClaimBoundary,
    /// Whether the full 9x9 prompt-plus-target sequence fits in one pass.
    pub full_sequence_fits_model_context: bool,
    /// Smallest full-sequence overflow over the current context.
    pub full_sequence_context_overflow_min: u32,
    /// Largest full-sequence overflow over the current context.
    pub full_sequence_context_overflow_max: u32,
    /// Bounded eval cap used by the family run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounded_eval_target_token_cap: Option<u32>,
    /// Aggregate target-token exactness over the bounded validation slice.
    pub aggregate_target_token_exactness_bps: u32,
    /// Aggregate exactness over the first target token.
    pub first_target_exactness_bps: u32,
    /// Aggregate exactness over the first 32 target tokens.
    pub first_32_token_exactness_bps: u32,
    /// Exact-trace validation case count over the bounded slice.
    pub exact_trace_case_count: u32,
    /// Estimated bytes for a naive full forward pass on the longest full sequence.
    pub estimated_full_forward_buffer_bytes_max: u64,
    /// Estimated live bytes for the family's declared long-trace contract.
    pub estimated_contract_live_bytes_max: u64,
    /// Honest outcome statement carried forward from the fit report.
    pub outcome_statement: String,
    /// Relative run-bundle path.
    pub run_bundle_ref: String,
    /// Relative training-report path.
    pub training_report_ref: String,
    /// Relative fit-report path.
    pub fit_report_ref: String,
}

impl TassadarExecutorLongTraceFamilySummary {
    fn new(
        family_id: &str,
        run_directory: &str,
        run_bundle: &TassadarExecutorReferenceRunBundle,
        training_report: &TassadarExecutorTrainingReport,
        fit_report: &TassadarExecutorSequenceFitReport,
        checkpoint: &TassadarExecutorCheckpointState,
    ) -> Result<Self, TassadarExecutorWindowedFamilyComparisonError> {
        let model = checkpoint.materialize_model()?;
        let estimated_contract_live_bytes_max = match fit_report.long_trace_contract {
            TassadarExecutorLongTraceContract::FlatPrefixFullForward => {
                fit_report.estimated_bounded_forward_buffer_bytes_max
            }
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow => {
                fit_report.estimated_incremental_decode_live_bytes_max
            }
        };
        Ok(Self {
            family_id: family_id.to_string(),
            run_directory: run_directory.to_string(),
            run_id: run_bundle.run_id.clone(),
            model_id: model.descriptor().model.model_id.clone(),
            model_family: model.descriptor().model.family.clone(),
            trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
            trained_weight_digest: run_bundle.trained_weight_digest.clone(),
            trainable_surface: run_bundle.trainable_surface,
            teacher_forced_training_strategy: training_report.config.teacher_forced_training_strategy,
            long_trace_contract: fit_report.long_trace_contract,
            claim_boundary: model.descriptor().claim_boundary,
            full_sequence_fits_model_context: fit_report.full_sequence_fits_model_context,
            full_sequence_context_overflow_min: fit_report.full_sequence_context_overflow_min,
            full_sequence_context_overflow_max: fit_report.full_sequence_context_overflow_max,
            bounded_eval_target_token_cap: fit_report.eval_target_token_cap,
            aggregate_target_token_exactness_bps: training_report
                .evaluation
                .aggregate_target_token_exactness_bps,
            first_target_exactness_bps: training_report.evaluation.first_target_exactness_bps,
            first_32_token_exactness_bps: training_report.evaluation.first_32_token_exactness_bps,
            exact_trace_case_count: training_report.evaluation.exact_trace_case_count,
            estimated_full_forward_buffer_bytes_max: fit_report
                .estimated_full_forward_buffer_bytes_max,
            estimated_contract_live_bytes_max,
            outcome_statement: fit_report.outcome_statement.clone(),
            run_bundle_ref: format!("{run_directory}/{RUN_BUNDLE_FILE}"),
            training_report_ref: format!("{run_directory}/{TRAINING_REPORT_FILE}"),
            fit_report_ref: format!(
                "{run_directory}/{TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE}"
            ),
        })
    }
}

/// Top-level same-corpus comparison for the first flat-prefix-vs-windowed 9x9 pair.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorWindowedFamilyComparisonReport {
    /// Stable dataset storage key shared by both families.
    pub dataset_storage_key: String,
    /// Stable dataset digest shared by both families.
    pub dataset_digest: String,
    /// Flat-prefix bounded family summary.
    pub flat_prefix_full_forward: TassadarExecutorLongTraceFamilySummary,
    /// Windowed bounded family summary.
    pub windowed_incremental: TassadarExecutorLongTraceFamilySummary,
    /// Whether the windowed family advertises a smaller live-state budget.
    pub windowed_reduces_live_state_bytes: bool,
    /// Whether the windowed family matches or beats the flat-prefix bounded boundary rank.
    pub windowed_matches_or_beats_flat_prefix_bounded_exactness: bool,
    /// Plain-language summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarExecutorWindowedFamilyComparisonReport {
    fn new(
        flat_prefix_full_forward: TassadarExecutorLongTraceFamilySummary,
        windowed_incremental: TassadarExecutorLongTraceFamilySummary,
    ) -> Self {
        let windowed_reduces_live_state_bytes =
            windowed_incremental.estimated_contract_live_bytes_max
                < flat_prefix_full_forward.estimated_contract_live_bytes_max;
        let windowed_matches_or_beats_flat_prefix_bounded_exactness =
            bounded_rank(&windowed_incremental) >= bounded_rank(&flat_prefix_full_forward);
        let mut report = Self {
            dataset_storage_key: String::from("oa.tassadar.sudoku_9x9.sequence@scale-v0"),
            dataset_digest: String::from(
                "62ff118b9980cccc0bfeb49fa86fc9b2703846e6a344c5207ad44a704d7a172a",
            ),
            flat_prefix_full_forward,
            windowed_incremental,
            windowed_reduces_live_state_bytes,
            windowed_matches_or_beats_flat_prefix_bounded_exactness,
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Windowed long-trace contract is now explicit on the same 9x9 corpus: flat_prefix_live_bytes={}, windowed_live_bytes={}, flat_prefix_first_32_bps={}, windowed_first_32_bps={}, full_sequence_fits_flat_prefix={}, full_sequence_fits_windowed={}.",
            report.flat_prefix_full_forward.estimated_contract_live_bytes_max,
            report.windowed_incremental.estimated_contract_live_bytes_max,
            report.flat_prefix_full_forward.first_32_token_exactness_bps,
            report.windowed_incremental.first_32_token_exactness_bps,
            report.flat_prefix_full_forward.full_sequence_fits_model_context,
            report.windowed_incremental.full_sequence_fits_model_context,
        );
        report.report_digest =
            stable_digest(b"psionic_tassadar_executor_windowed_family_comparison_report|", &report);
        report
    }
}

/// Errors while materializing the 9x9 flat-prefix-vs-windowed comparison.
#[derive(Debug, Error)]
pub enum TassadarExecutorWindowedFamilyComparisonError {
    /// One 9x9 run or fit augmentation failed.
    #[error(transparent)]
    ReferenceRun(#[from] TassadarSudoku9x9ReferenceRunError),
    /// Materializing a trained model from checkpoint state failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Reading one persisted artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding one persisted artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// File path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Writing one comparison artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
}

/// Returns the bounded flat-prefix comparison config on the 9x9 corpus.
#[must_use]
pub fn tassadar_executor_sudoku_9x9_flat_prefix_comparison_config() -> TassadarExecutorTrainingConfig
{
    let mut config = tassadar_executor_sudoku_9x9_reference_run_config();
    config.run_id = String::from("tassadar-executor-transformer-sudoku-9x9-flat-prefix-v1");
    config.teacher_forced_training_strategy =
        TassadarExecutorTeacherForcedTrainingStrategy::FullForwardWindow;
    config.long_trace_contract = TassadarExecutorLongTraceContract::FlatPrefixFullForward;
    config
}

/// Returns the bounded windowed comparison config on the 9x9 corpus.
#[must_use]
pub fn tassadar_executor_sudoku_9x9_windowed_comparison_config() -> TassadarExecutorTrainingConfig {
    let mut config = tassadar_executor_sudoku_9x9_reference_run_config();
    config.run_id = String::from("tassadar-executor-transformer-sudoku-9x9-windowed-v1");
    config.long_trace_contract = TassadarExecutorLongTraceContract::IncrementalDecodeWindow;
    config
}

/// Executes the first honest 9x9 flat-prefix-vs-windowed comparison and writes the report.
pub fn execute_tassadar_sudoku_9x9_windowed_family_comparison(
    output_dir: &Path,
) -> Result<TassadarExecutorWindowedFamilyComparisonReport, TassadarExecutorWindowedFamilyComparisonError>
{
    fs::create_dir_all(output_dir).map_err(|error| TassadarExecutorWindowedFamilyComparisonError::Write {
        path: output_dir.display().to_string(),
        error,
    })?;
    execute_family_run(
        &output_dir.join(FLAT_PREFIX_DIR),
        &tassadar_executor_sudoku_9x9_flat_prefix_comparison_config(),
    )?;
    execute_family_run(
        &output_dir.join(WINDOWED_INCREMENTAL_DIR),
        &tassadar_executor_sudoku_9x9_windowed_comparison_config(),
    )?;

    let flat_summary = read_family_summary(FLAT_PREFIX_DIR, output_dir)?;
    let windowed_summary = read_family_summary(WINDOWED_INCREMENTAL_DIR, output_dir)?;
    let report = TassadarExecutorWindowedFamilyComparisonReport::new(flat_summary, windowed_summary);
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_WINDOWED_FAMILY_COMPARISON_REPORT_FILE),
        "tassadar_executor_windowed_family_comparison_report",
        &report,
    )?;
    Ok(report)
}

fn execute_family_run(
    output_dir: &Path,
    config: &TassadarExecutorTrainingConfig,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorWindowedFamilyComparisonError> {
    execute_tassadar_training_run_without_benchmark(output_dir, config)
        .map_err(TassadarSudoku9x9ReferenceRunError::from)?;
    augment_tassadar_training_run_with_telemetry(output_dir)
        .map_err(TassadarSudoku9x9ReferenceRunError::from)?;
    Ok(augment_tassadar_sudoku_9x9_run_with_fit_and_review(output_dir)?)
}

fn read_family_summary(
    run_directory: &str,
    output_dir: &Path,
) -> Result<TassadarExecutorLongTraceFamilySummary, TassadarExecutorWindowedFamilyComparisonError>
{
    let run_root = output_dir.join(run_directory);
    let run_bundle: TassadarExecutorReferenceRunBundle =
        read_json(run_root.join(RUN_BUNDLE_FILE), "tassadar_reference_run_bundle")?;
    let training_report: TassadarExecutorTrainingReport =
        read_json(run_root.join(TRAINING_REPORT_FILE), "tassadar_training_report")?;
    let fit_report: TassadarExecutorSequenceFitReport = read_json(
        run_root.join(TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE),
        "tassadar_sequence_fit_report",
    )?;
    let checkpoint: TassadarExecutorCheckpointState =
        read_json(run_root.join(CHECKPOINT_STATE_FILE), "tassadar_checkpoint_state")?;
    TassadarExecutorLongTraceFamilySummary::new(
        run_directory,
        run_directory,
        &run_bundle,
        &training_report,
        &fit_report,
        &checkpoint,
    )
}

fn read_json<T>(
    path: PathBuf,
    artifact_kind: &str,
) -> Result<T, TassadarExecutorWindowedFamilyComparisonError>
where
    T: DeserializeOwned,
{
    let bytes = fs::read(&path).map_err(|error| TassadarExecutorWindowedFamilyComparisonError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarExecutorWindowedFamilyComparisonError::Deserialize {
            artifact_kind: artifact_kind.to_string(),
            path: path.display().to_string(),
            error,
        }
    })
}

fn write_json<T: Serialize>(
    path: PathBuf,
    artifact_kind: &str,
    value: &T,
) -> Result<(), TassadarExecutorWindowedFamilyComparisonError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorWindowedFamilyComparisonError::Deserialize {
            artifact_kind: artifact_kind.to_string(),
            path: path.display().to_string(),
            error,
        }
    })?;
    fs::write(&path, bytes).map_err(|error| TassadarExecutorWindowedFamilyComparisonError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn bounded_rank(summary: &TassadarExecutorLongTraceFamilySummary) -> (u32, u32, u32, u32) {
    (
        summary.first_target_exactness_bps,
        summary.first_32_token_exactness_bps,
        summary.exact_trace_case_count,
        summary.aggregate_target_token_exactness_bps,
    )
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    let bytes =
        serde_json::to_vec(value).expect("Tassadar windowed family comparison value should serialize");
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
