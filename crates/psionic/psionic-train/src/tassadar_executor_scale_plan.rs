use std::{collections::BTreeMap, fs, path::Path};

use psionic_data::TassadarSequenceSplit;
use psionic_eval::{TassadarSequenceEvalError, build_tassadar_sudoku_9x9_sequence_dataset};
use psionic_models::TassadarExecutorTransformer;
use psionic_runtime::tassadar_sudoku_9x9_corpus;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE, TassadarExecutorReferenceRunBundle,
    TassadarExecutorRunError, TassadarExecutorTraceDivergenceReport,
    TassadarExecutorTrainingConfig, TassadarSequenceTrainingError,
    build_tassadar_sudoku_9x9_sequence_training_manifest,
};

/// Canonical committed fixture directory for the Phase 11 scale plan.
pub const TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_9x9_scale_plan_v0";
/// Canonical machine-readable Phase 11 plan artifact.
pub const TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_FILE: &str = "scale_plan.json";

const RUN_BUNDLE_FILE: &str = "run_bundle.json";

/// Baseline 4x4 gate state that controls whether 9x9 promotion is honest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorScaleGate {
    /// Baseline 4x4 run identifier.
    pub baseline_run_id: String,
    /// Baseline 4x4 dataset storage key.
    pub baseline_dataset_storage_key: String,
    /// Baseline validation case count.
    pub validation_case_count: u32,
    /// Validation cases that stayed exact on the first target token.
    pub first_target_exact_case_count: u32,
    /// First-target exactness in basis points.
    pub first_target_exactness_bps: u32,
    /// Aggregate first-32-target-token exactness in basis points.
    pub first_32_target_exactness_bps: u32,
    /// Validation cases that stayed exact over the full suffix.
    pub exact_trace_case_count: u32,
    /// Whether the 9x9 promotion gate is open.
    pub gate_open: bool,
    /// Plain blockers when the gate remains closed.
    pub blocking_reasons: Vec<String>,
}

/// Compact summary of the real 9x9 workload being planned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9DatasetStats {
    /// Dataset storage key.
    pub dataset_storage_key: String,
    /// Dataset digest.
    pub dataset_digest: String,
    /// Frozen training-manifest digest.
    pub training_manifest_digest: String,
    /// Bound runtime profile.
    pub wasm_profile_id: String,
    /// Bound trace ABI.
    pub trace_abi_id: String,
    /// Total case count.
    pub case_count: u32,
    /// Split-wise case counts.
    pub split_case_counts: BTreeMap<String, u32>,
    /// Minimum givens in the corpus.
    pub given_count_min: u32,
    /// Maximum givens in the corpus.
    pub given_count_max: u32,
    /// Minimum program prompt tokens.
    pub prompt_token_count_min: u32,
    /// Maximum program prompt tokens.
    pub prompt_token_count_max: u32,
    /// Minimum target tokens.
    pub target_token_count_min: u32,
    /// Maximum target tokens.
    pub target_token_count_max: u32,
    /// Minimum total tokens.
    pub total_token_count_min: u32,
    /// Maximum total tokens.
    pub total_token_count_max: u32,
    /// Minimum program instruction count.
    pub program_instruction_count_min: u32,
    /// Maximum program instruction count.
    pub program_instruction_count_max: u32,
}

/// Recommended configs and curriculum for the 9x9 scale-out lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorScaleTrainingPlan {
    /// Small smoke config that proves the 9x9 model can build and train honestly.
    pub smoke_test_config: TassadarExecutorTrainingConfig,
    /// Promotion config to use once the 4x4 gate opens.
    pub gated_promotion_config: TassadarExecutorTrainingConfig,
    /// Ordered curriculum target-token schedule.
    pub curriculum_target_token_schedule: Vec<u32>,
    /// Whether checkpoints should be emitted every epoch.
    pub checkpoint_every_epoch: bool,
}

/// Machine-readable Phase 11 scale-out plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorSudoku9x9ScalePlan {
    /// Baseline 4x4 reference run directory.
    pub baseline_reference_run_dir: String,
    /// Baseline gate facts.
    pub baseline_gate: TassadarExecutorScaleGate,
    /// Real 9x9 workload stats.
    pub dataset_stats: TassadarSudoku9x9DatasetStats,
    /// 9x9 model identity.
    pub model_id: String,
    /// 9x9 model descriptor digest.
    pub model_descriptor_digest: String,
    /// 9x9 weight digest.
    pub model_weight_digest: String,
    /// Recommended training plan.
    pub training_plan: TassadarExecutorScaleTrainingPlan,
    /// Stable plan digest.
    pub plan_digest: String,
}

impl TassadarExecutorSudoku9x9ScalePlan {
    fn new(
        baseline_reference_run_dir: &Path,
        run_bundle: &TassadarExecutorReferenceRunBundle,
        divergence: &TassadarExecutorTraceDivergenceReport,
        dataset_stats: TassadarSudoku9x9DatasetStats,
    ) -> Self {
        let baseline_gate = build_baseline_gate(run_bundle, divergence);
        let model = TassadarExecutorTransformer::sudoku_9x9();
        let training_plan = TassadarExecutorScaleTrainingPlan {
            smoke_test_config: TassadarExecutorTrainingConfig::sudoku_9x9_scale_smoke(),
            gated_promotion_config: TassadarExecutorTrainingConfig {
                run_id: String::from("tassadar-executor-transformer-sudoku-9x9-scale-v0"),
                workload: psionic_eval::TassadarSequenceWorkload::Sudoku9x9,
                dataset_version: String::from("scale-v0"),
                epochs: 8,
                learning_rate: 0.05,
                max_train_target_tokens_per_example: Some(256),
                max_eval_target_tokens_per_example: None,
                curriculum_stages: Vec::new(),
                validate_every_epoch: true,
                select_best_checkpoint_by_boundary: true,
            },
            curriculum_target_token_schedule: vec![1, 8, 32, 128, 512],
            checkpoint_every_epoch: true,
        };

        let mut plan = Self {
            baseline_reference_run_dir: baseline_reference_run_dir.display().to_string(),
            baseline_gate,
            dataset_stats,
            model_id: model.descriptor().model.model_id.clone(),
            model_descriptor_digest: model.descriptor().stable_digest(),
            model_weight_digest: model.descriptor().weights.digest.clone(),
            training_plan,
            plan_digest: String::new(),
        };
        plan.plan_digest =
            stable_digest(b"psionic_tassadar_executor_sudoku_9x9_scale_plan|", &plan);
        plan
    }
}

/// Errors while building or writing the Phase 11 scale plan.
#[derive(Debug, Error)]
pub enum TassadarExecutorScalePlanError {
    /// Reusing the 4x4 reference run surface failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Reading one persisted baseline artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding one persisted baseline artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// File path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Serializing the plan failed.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Writing one artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Sequence dataset generation failed.
    #[error(transparent)]
    SequenceEval(#[from] TassadarSequenceEvalError),
    /// Training-manifest generation failed.
    #[error(transparent)]
    SequenceTraining(#[from] TassadarSequenceTrainingError),
}

/// Builds the machine-readable Phase 11 plan from the committed 4x4 run plus the real 9x9 corpus.
pub fn build_tassadar_sudoku_9x9_scale_plan(
    baseline_reference_run_dir: &Path,
) -> Result<TassadarExecutorSudoku9x9ScalePlan, TassadarExecutorScalePlanError> {
    let run_bundle: TassadarExecutorReferenceRunBundle = read_json(
        baseline_reference_run_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
    )?;
    let divergence: TassadarExecutorTraceDivergenceReport = read_json(
        baseline_reference_run_dir.join(TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE),
        "tassadar_trace_divergence_report",
    )?;
    let dataset = build_tassadar_sudoku_9x9_sequence_dataset("scale-v0")?;
    let manifest = build_tassadar_sudoku_9x9_sequence_training_manifest("scale-v0")?;
    let corpus = tassadar_sudoku_9x9_corpus();
    let mut split_case_counts = BTreeMap::new();
    for example in &dataset.dataset.examples {
        *split_case_counts
            .entry(example.metadata.split.as_str().to_string())
            .or_insert(0) += 1;
    }
    let dataset_stats = TassadarSudoku9x9DatasetStats {
        dataset_storage_key: dataset.dataset.storage_key(),
        dataset_digest: dataset.dataset.stable_digest(),
        training_manifest_digest: manifest.manifest_digest,
        wasm_profile_id: String::from(
            psionic_runtime::TassadarWasmProfile::sudoku_9x9_search_v1().profile_id,
        ),
        trace_abi_id: String::from(
            psionic_runtime::TassadarTraceAbi::sudoku_9x9_search_v1().abi_id,
        ),
        case_count: dataset.dataset.examples.len() as u32,
        split_case_counts,
        given_count_min: corpus
            .iter()
            .map(|case| case.given_count as u32)
            .min()
            .unwrap_or(0),
        given_count_max: corpus
            .iter()
            .map(|case| case.given_count as u32)
            .max()
            .unwrap_or(0),
        prompt_token_count_min: dataset
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.prompt_token_count)
            .min()
            .unwrap_or(0),
        prompt_token_count_max: dataset
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.prompt_token_count)
            .max()
            .unwrap_or(0),
        target_token_count_min: dataset
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.target_token_count)
            .min()
            .unwrap_or(0),
        target_token_count_max: dataset
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.target_token_count)
            .max()
            .unwrap_or(0),
        total_token_count_min: dataset
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.total_token_count)
            .min()
            .unwrap_or(0),
        total_token_count_max: dataset
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.total_token_count)
            .max()
            .unwrap_or(0),
        program_instruction_count_min: corpus
            .iter()
            .map(|case| case.validation_case.program.instructions.len() as u32)
            .min()
            .unwrap_or(0),
        program_instruction_count_max: corpus
            .iter()
            .map(|case| case.validation_case.program.instructions.len() as u32)
            .max()
            .unwrap_or(0),
    };

    Ok(TassadarExecutorSudoku9x9ScalePlan::new(
        baseline_reference_run_dir,
        &run_bundle,
        &divergence,
        dataset_stats,
    ))
}

/// Builds and writes the machine-readable Phase 11 scale plan.
pub fn write_tassadar_sudoku_9x9_scale_plan(
    output_dir: &Path,
    baseline_reference_run_dir: &Path,
) -> Result<TassadarExecutorSudoku9x9ScalePlan, TassadarExecutorScalePlanError> {
    let plan = build_tassadar_sudoku_9x9_scale_plan(baseline_reference_run_dir)?;
    fs::create_dir_all(output_dir).map_err(|error| TassadarExecutorScalePlanError::Write {
        path: output_dir.display().to_string(),
        error,
    })?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_FILE),
        "tassadar_sudoku_9x9_scale_plan",
        &plan,
    )?;
    Ok(plan)
}

fn build_baseline_gate(
    run_bundle: &TassadarExecutorReferenceRunBundle,
    divergence: &TassadarExecutorTraceDivergenceReport,
) -> TassadarExecutorScaleGate {
    let validation_cases = divergence
        .case_reports
        .iter()
        .filter(|case| case.split == TassadarSequenceSplit::Validation)
        .collect::<Vec<_>>();
    let validation_case_count = validation_cases.len() as u32;
    let first_target_exact_case_count = validation_cases
        .iter()
        .filter(|case| case.first_divergence_index.map_or(true, |index| index > 0))
        .count() as u32;
    let first_target_exactness_bps = basis_points(
        first_target_exact_case_count as u64,
        validation_case_count.max(1) as u64,
    );
    let first_32_exact_token_count = validation_cases
        .iter()
        .map(|case| u64::from(case.matched_target_token_count.min(32)))
        .sum::<u64>();
    let first_32_total_token_count = validation_cases
        .iter()
        .map(|case| u64::from(case.target_token_count.min(32)))
        .sum::<u64>()
        .max(1);
    let first_32_target_exactness_bps =
        basis_points(first_32_exact_token_count, first_32_total_token_count);
    let exact_trace_case_count = validation_cases
        .iter()
        .filter(|case| case.exact_trace_match)
        .count() as u32;
    let gate_open = validation_case_count > 0
        && first_target_exact_case_count == validation_case_count
        && first_32_target_exactness_bps > 9_000
        && exact_trace_case_count >= 1;
    let mut blocking_reasons = Vec::new();
    if first_target_exact_case_count != validation_case_count {
        blocking_reasons.push(String::from(
            "4x4 validation first-target exactness is not yet 100%.",
        ));
    }
    if first_32_target_exactness_bps <= 9_000 {
        blocking_reasons.push(String::from(
            "4x4 validation first-32-target exactness is still below the 9,000 bps gate.",
        ));
    }
    if exact_trace_case_count == 0 {
        blocking_reasons.push(String::from(
            "No 4x4 validation case is exact over the full suffix yet.",
        ));
    }

    TassadarExecutorScaleGate {
        baseline_run_id: run_bundle.run_id.clone(),
        baseline_dataset_storage_key: run_bundle.dataset_storage_key.clone(),
        validation_case_count,
        first_target_exact_case_count,
        first_target_exactness_bps,
        first_32_target_exactness_bps,
        exact_trace_case_count,
        gate_open,
        blocking_reasons,
    }
}

fn basis_points(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    ((numerator as f64 / denominator as f64) * 10_000.0).round() as u32
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarExecutorScalePlanError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarExecutorScalePlanError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarExecutorScalePlanError::Deserialize {
        artifact_kind: artifact_kind.to_string(),
        path: path.display().to_string(),
        error,
    })
}

fn write_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
    value: &T,
) -> Result<(), TassadarExecutorScalePlanError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorScalePlanError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, &bytes).map_err(|error| TassadarExecutorScalePlanError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("Tassadar scale plan value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use psionic_data::TassadarSequenceSplit;
    use tempfile::tempdir;

    use super::{
        TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_FILE,
        TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_OUTPUT_DIR, build_tassadar_sudoku_9x9_scale_plan,
        write_tassadar_sudoku_9x9_scale_plan,
    };
    use crate::{
        TassadarExecutorReferenceRunBundle, TassadarExecutorTraceDivergenceCase,
        TassadarExecutorTraceDivergenceReport,
    };

    fn write_baseline_run_fixture(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(dir)?;
        let run_bundle = TassadarExecutorReferenceRunBundle {
            schema_version: 1,
            run_id: String::from("tassadar-executor-transformer-sudoku-v0-reference-run-v0"),
            dataset_version: String::from("train-v0"),
            dataset_storage_key: String::from("oa.tassadar.sudoku_v0.sequence@train-v0"),
            dataset_digest: String::from("dataset-digest"),
            tokenizer_digest: String::from("tokenizer-digest"),
            vocabulary_digest: String::from("vocab-digest"),
            training_manifest_digest: String::from("manifest-digest"),
            trained_model_descriptor_digest: String::from("model-digest"),
            trained_weight_digest: String::from("weight-digest"),
            training_report_digest: String::from("training-report-digest"),
            linear_benchmark_report_digest: String::from("benchmark-digest"),
            boundary_exactness_report_digest: None,
            divergence_histogram_report_digest: None,
            first_token_confusion_report_digest: None,
            checkpoint_leaderboard_report_digest: None,
            neural_hull_benchmark_report_digest: None,
            checkpoint_artifact_digest: String::from("checkpoint-digest"),
            model_artifact_digest: String::from("artifact-digest"),
            artifacts: Vec::new(),
            bundle_digest: String::from("bundle-digest"),
        };
        let divergence = TassadarExecutorTraceDivergenceReport {
            run_id: run_bundle.run_id.clone(),
            dataset_storage_key: run_bundle.dataset_storage_key.clone(),
            dataset_digest: run_bundle.dataset_digest.clone(),
            trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
            checkpoint_manifest_digest: String::from("checkpoint-manifest-digest"),
            case_reports: vec![
                TassadarExecutorTraceDivergenceCase {
                    sequence_id: String::from("validation-a"),
                    case_id: String::from("validation-a"),
                    split: TassadarSequenceSplit::Validation,
                    target_token_count: 64,
                    matched_target_token_count: 0,
                    target_token_exactness_bps: 0,
                    first_divergence_index: Some(0),
                    reference_divergence_token: Some(String::from("<step>")),
                    predicted_divergence_token: Some(String::from("<unk>")),
                    exact_trace_match: false,
                    final_output_match: false,
                    halt_match: false,
                    reference_target_digest: String::from("reference-a"),
                    predicted_target_digest: String::from("predicted-a"),
                },
                TassadarExecutorTraceDivergenceCase {
                    sequence_id: String::from("validation-b"),
                    case_id: String::from("validation-b"),
                    split: TassadarSequenceSplit::Validation,
                    target_token_count: 64,
                    matched_target_token_count: 4,
                    target_token_exactness_bps: 625,
                    first_divergence_index: Some(4),
                    reference_divergence_token: Some(String::from("<step>")),
                    predicted_divergence_token: Some(String::from("<unk>")),
                    exact_trace_match: false,
                    final_output_match: false,
                    halt_match: false,
                    reference_target_digest: String::from("reference-b"),
                    predicted_target_digest: String::from("predicted-b"),
                },
            ],
            report_digest: String::from("divergence-digest"),
        };

        fs::write(
            dir.join("run_bundle.json"),
            serde_json::to_vec_pretty(&run_bundle)?,
        )?;
        fs::write(
            dir.join("trace_divergence_report.json"),
            serde_json::to_vec_pretty(&divergence)?,
        )?;
        Ok(())
    }

    #[test]
    fn scale_plan_is_gated_on_current_4x4_evidence_and_tracks_9x9_stats()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        write_baseline_run_fixture(temp.path())?;
        let plan = build_tassadar_sudoku_9x9_scale_plan(temp.path())?;

        assert!(!plan.baseline_gate.gate_open);
        assert_eq!(plan.dataset_stats.case_count, 4);
        assert_eq!(plan.dataset_stats.split_case_counts.get("train"), Some(&2));
        assert_eq!(
            plan.training_plan.smoke_test_config.workload,
            psionic_eval::TassadarSequenceWorkload::Sudoku9x9
        );
        assert!(plan.dataset_stats.target_token_count_max > 0);
        Ok(())
    }

    #[test]
    fn scale_plan_writer_emits_machine_readable_artifact() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let baseline = tempdir()?;
        write_baseline_run_fixture(baseline.path())?;
        let plan = write_tassadar_sudoku_9x9_scale_plan(temp.path(), baseline.path())?;

        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_FILE)
                .exists()
        );
        assert!(!plan.plan_digest.is_empty());
        Ok(())
    }

    #[test]
    fn scale_plan_reuses_the_repo_relative_fixture_constant() {
        assert_eq!(
            TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_OUTPUT_DIR,
            "crates/psionic/fixtures/tassadar/runs/sudoku_9x9_scale_plan_v0"
        );
    }
}
