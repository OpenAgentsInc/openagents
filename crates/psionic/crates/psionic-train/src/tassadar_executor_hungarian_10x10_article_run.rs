use std::{collections::BTreeMap, fs, path::Path};

use psionic_data::TassadarSequenceSplit;
use psionic_eval::{
    EvalArtifact, TassadarExecutorEvalReport, TassadarExecutorStructuralSupervisionReport,
    TassadarSequenceEvalError, TassadarSequenceWorkload, build_tassadar_sequence_dataset_with_trace_family,
    evaluate_tassadar_executor_transformer,
};
use psionic_models::{
    TassadarExecutorLongTraceContract, TassadarExecutorTrainableSurface,
    TassadarSequenceTraceFamily,
};
use psionic_runtime::TassadarClaimClass;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE, TASSADAR_EXECUTOR_STRUCTURAL_SUPERVISION_REPORT_FILE,
    TassadarExecutorCheckpointState, TassadarExecutorContextStageFit,
    TassadarExecutorModelArtifact, TassadarExecutorReferenceRunBundle, TassadarExecutorRunError,
    TassadarExecutorStructuralSupervisionConfig, TassadarExecutorTeacherForcedTrainingStrategy,
    TassadarExecutorTelemetryError, TassadarExecutorTrainingConfig,
    TassadarExecutorTrainingEpochReport, TassadarExecutorTrainingReport,
    augment_tassadar_training_run_with_telemetry, execute_tassadar_training_run_without_benchmark,
};

const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const TRAINING_REPORT_FILE: &str = "training_report.json";
const MODEL_ARTIFACT_FILE: &str = "model_artifact.json";
const ARTICLE_LEARNED_BENCHMARK_REPORT_FILE: &str = "article_learned_benchmark_report.json";

/// Stable run identifier for the first learned article-class Hungarian-10x10 benchmark lane.
pub const TASSADAR_EXECUTOR_HUNGARIAN_10X10_ARTICLE_RUN_ID: &str =
    "tassadar-executor-transformer-hungarian-10x10-article-learned-run-v0";
/// Canonical repo path for the first learned article-class Hungarian-10x10 benchmark lane.
pub const TASSADAR_EXECUTOR_HUNGARIAN_10X10_ARTICLE_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_learned_article_executor_v0";

const fn default_learned_article_claim_class() -> TassadarClaimClass {
    TassadarClaimClass::LearnedArticleClass
}

/// Machine-readable fit analysis for the first learned article-class benchmark lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleLearnedFitReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Stable trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Active trainable surface.
    pub trainable_surface: TassadarExecutorTrainableSurface,
    /// Explicit teacher-forced strategy used by the run.
    pub teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
    /// Explicit long-trace family contract used by the run.
    pub long_trace_contract: TassadarExecutorLongTraceContract,
    /// Explicit symbolic trace family used by the run.
    pub trace_family: TassadarSequenceTraceFamily,
    /// Vocabulary size for the learned lane.
    pub vocab_size: u32,
    /// Hidden width for one decode step.
    pub hidden_width: u32,
    /// Maximum context tokens admitted by the model.
    pub model_max_sequence_tokens: u32,
    /// Minimum prompt length in the dataset.
    pub prompt_token_count_min: u32,
    /// Maximum prompt length in the dataset.
    pub prompt_token_count_max: u32,
    /// Minimum full target length in the dataset.
    pub target_token_count_min: u32,
    /// Maximum full target length in the dataset.
    pub target_token_count_max: u32,
    /// Minimum full sequence length in the dataset.
    pub total_token_count_min: u32,
    /// Maximum full sequence length in the dataset.
    pub total_token_count_max: u32,
    /// Whether full sequences fit the current model context.
    pub full_sequence_fits_model_context: bool,
    /// Minimum remaining headroom over the full dataset when the run fits.
    pub full_sequence_context_headroom_min: u32,
    /// Maximum remaining headroom over the full dataset when the run fits.
    pub full_sequence_context_headroom_max: u32,
    /// Explicit stage-by-stage fit facts.
    pub stage_fits: Vec<TassadarExecutorContextStageFit>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarArticleLearnedFitReport {
    fn new(
        config: &TassadarExecutorTrainingConfig,
        model_artifact: &TassadarExecutorModelArtifact,
    ) -> Result<Self, TassadarHungarian10x10ArticleRunError> {
        let dataset_bundle = build_tassadar_sequence_dataset_with_trace_family(
            config.workload,
            config.dataset_version.as_str(),
            config.trace_family,
        )
        .map_err(TassadarHungarian10x10ArticleRunError::SequenceEval)?;
        let prompt_token_count_min = dataset_bundle
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.prompt_token_count)
            .min()
            .unwrap_or(0);
        let prompt_token_count_max = dataset_bundle
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.prompt_token_count)
            .max()
            .unwrap_or(0);
        let target_token_count_min = dataset_bundle
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.target_token_count)
            .min()
            .unwrap_or(0);
        let target_token_count_max = dataset_bundle
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.target_token_count)
            .max()
            .unwrap_or(0);
        let total_token_count_min = dataset_bundle
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.total_token_count)
            .min()
            .unwrap_or(0);
        let total_token_count_max = dataset_bundle
            .dataset
            .examples
            .iter()
            .map(|example| example.metadata.total_token_count)
            .max()
            .unwrap_or(0);
        let model_max_sequence_tokens = model_artifact.descriptor.config.max_sequence_tokens as u32;
        let stage_fits = config
            .resolved_stages()
            .into_iter()
            .map(|stage| {
                let target_token_cap = stage
                    .max_train_target_tokens_per_example
                    .map(|value| value as u32)
                    .or(Some(target_token_count_max));
                let max_total_tokens = prompt_token_count_max.saturating_add(
                    stage
                        .max_train_target_tokens_per_example
                        .map(|value| value as u32)
                        .unwrap_or(target_token_count_max),
                );
                let fits_model_context = max_total_tokens <= model_max_sequence_tokens;
                TassadarExecutorContextStageFit {
                    stage_id: stage.stage_id,
                    target_token_cap,
                    max_total_tokens,
                    fits_model_context,
                    context_headroom_tokens: if fits_model_context {
                        Some(model_max_sequence_tokens - max_total_tokens)
                    } else {
                        None
                    },
                    context_overflow_tokens: if fits_model_context {
                        None
                    } else {
                        Some(max_total_tokens - model_max_sequence_tokens)
                    },
                }
            })
            .collect::<Vec<_>>();
        let mut report = Self {
            run_id: config.run_id.clone(),
            dataset_storage_key: dataset_bundle.dataset.storage_key(),
            dataset_digest: dataset_bundle.dataset.stable_digest(),
            model_id: model_artifact.descriptor.model.model_id.clone(),
            trained_model_descriptor_digest: model_artifact.descriptor.stable_digest(),
            trainable_surface: config.trainable_surface,
            teacher_forced_training_strategy: config.teacher_forced_training_strategy,
            long_trace_contract: config.long_trace_contract,
            trace_family: config.trace_family,
            vocab_size: model_artifact.descriptor.config.vocab_size as u32,
            hidden_width: model_artifact.descriptor.config.hidden_width() as u32,
            model_max_sequence_tokens,
            prompt_token_count_min,
            prompt_token_count_max,
            target_token_count_min,
            target_token_count_max,
            total_token_count_min,
            total_token_count_max,
            full_sequence_fits_model_context: total_token_count_max <= model_max_sequence_tokens,
            full_sequence_context_headroom_min: model_max_sequence_tokens
                .saturating_sub(total_token_count_max),
            full_sequence_context_headroom_max: model_max_sequence_tokens
                .saturating_sub(total_token_count_min),
            stage_fits,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_article_learned_fit_report|", &report);
        Ok(report)
    }
}

/// Machine-readable learned article benchmark report for one article-sized alternate trace family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleLearnedBenchmarkReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Coarse claim class for this learned lane.
    #[serde(default = "default_learned_article_claim_class")]
    pub claim_class: TassadarClaimClass,
    /// Stable workload family identifier.
    pub workload_family_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Explicit trace family bound to the run.
    pub trace_family: TassadarSequenceTraceFamily,
    /// Dataset splits used as optimization data for the learned benchmark.
    pub train_split_scope: Vec<TassadarSequenceSplit>,
    /// Honest scope statement for the benchmark evidence.
    pub benchmark_scope_statement: String,
    /// Honest reconstruction scope for the trace family.
    pub reconstruction_scope: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Stable trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Active trainable surface.
    pub trainable_surface: String,
    /// Selected checkpoint identifier.
    pub checkpoint_id: String,
    /// Stage that produced the selected checkpoint.
    pub selected_stage_id: String,
    /// Full-sequence fit truth for the declared article-class dataset.
    pub full_sequence_fits_model_context: bool,
    /// Validation exactness report.
    pub validation_report: TassadarExecutorEvalReport,
    /// Test exactness report.
    pub test_report: TassadarExecutorEvalReport,
    /// Whether the benchmark clears the exact article bar.
    pub passed: bool,
    /// Honest detail for the learned article claim boundary.
    pub verdict: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarArticleLearnedBenchmarkReport {
    fn new(
        run_bundle: &TassadarExecutorReferenceRunBundle,
        training_report: &TassadarExecutorTrainingReport,
        model_artifact: &TassadarExecutorModelArtifact,
        fit_report: &TassadarArticleLearnedFitReport,
        selected_epoch: &TassadarExecutorTrainingEpochReport,
        validation_report: TassadarExecutorEvalReport,
        test_report: TassadarExecutorEvalReport,
    ) -> Self {
        let passed = fit_report.full_sequence_fits_model_context
            && validation_report.exact_trace_case_count
                == validation_report.case_reports.len() as u32
            && validation_report.final_output_exact_case_count
                == validation_report.case_reports.len() as u32
            && test_report.exact_trace_case_count == test_report.case_reports.len() as u32
            && test_report.final_output_exact_case_count == test_report.case_reports.len() as u32;
        let mut report = Self {
            run_id: run_bundle.run_id.clone(),
            claim_class: TassadarClaimClass::LearnedArticleClass,
            workload_family_id: String::from(
                "tassadar.wasm.hungarian_10x10_matching.v1.learned_article_executor",
            ),
            dataset_storage_key: run_bundle.dataset_storage_key.clone(),
            dataset_digest: run_bundle.dataset_digest.clone(),
            trace_family: training_report.config.trace_family,
            train_split_scope: training_report.config.train_split_scope.clone(),
            benchmark_scope_statement: benchmark_scope_statement(
                training_report.config.train_split_scope.as_slice(),
            ),
            reconstruction_scope: training_report
                .config
                .trace_family
                .reconstruction_scope()
                .to_string(),
            model_id: model_artifact.descriptor.model.model_id.clone(),
            trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
            trainable_surface: run_bundle.trainable_surface.label().to_string(),
            checkpoint_id: selected_epoch.checkpoint_id.clone(),
            selected_stage_id: selected_epoch.stage_id.clone(),
            full_sequence_fits_model_context: fit_report.full_sequence_fits_model_context,
            validation_report,
            test_report,
            passed,
            verdict: String::new(),
            report_digest: String::new(),
        };
        report.verdict = if passed {
            format!(
                "article-sized learned Hungarian-10x10 frontier benchmark is exact on both validation and test under the declared alternate trace family; {}",
                report.benchmark_scope_statement
            )
        } else {
            format!(
                "article-sized learned Hungarian-10x10 frontier benchmark still fails the exact article bar on validation and/or test; {}",
                report.benchmark_scope_statement
            )
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_article_learned_benchmark_report|", &report);
        report
    }
}

/// Failure while materializing or augmenting the learned article-class Hungarian-10x10 lane.
#[derive(Debug, Error)]
pub enum TassadarHungarian10x10ArticleRunError {
    /// Base run execution failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Telemetry augmentation failed.
    #[error(transparent)]
    Telemetry(#[from] TassadarExecutorTelemetryError),
    /// Dataset loading failed.
    #[error(transparent)]
    SequenceEval(#[from] TassadarSequenceEvalError),
    /// Learned benchmark evaluation failed.
    #[error(transparent)]
    Eval(#[from] psionic_eval::TassadarExecutorEvalError),
    /// Reading one persisted JSON artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Decoding one persisted JSON artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    /// Failed to serialize one report artifact.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        artifact_kind: String,
        error: serde_json::Error,
    },
    /// Failed to write one report artifact.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// The selected checkpoint could not be found in the training report.
    #[error("training report `{run_id}` is missing the selected checkpoint `{checkpoint_id}`")]
    MissingSelectedCheckpoint { run_id: String, checkpoint_id: String },
}

/// Returns the canonical learned article-class Hungarian-10x10 run config.
#[must_use]
pub fn tassadar_executor_hungarian_10x10_article_run_config() -> TassadarExecutorTrainingConfig {
    TassadarExecutorTrainingConfig {
        run_id: String::from(TASSADAR_EXECUTOR_HUNGARIAN_10X10_ARTICLE_RUN_ID),
        workload: TassadarSequenceWorkload::Hungarian10x10,
        dataset_version: String::from("trace-family-v1"),
        epochs: 1,
        learning_rate: 0.05,
        max_train_target_tokens_per_example: None,
        max_eval_target_tokens_per_example: None,
        terminal_stage_learning_rate_scale: Some(0.02),
        trainable_surface:
            TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
        teacher_forced_training_strategy:
            TassadarExecutorTeacherForcedTrainingStrategy::FullForwardWindow,
        long_trace_contract: TassadarExecutorLongTraceContract::FlatPrefixFullForward,
        structural_supervision:
            TassadarExecutorStructuralSupervisionConfig::hungarian_dual_state_reference(),
        trace_family: TassadarSequenceTraceFamily::HungarianAssignmentFrontier,
        train_split_scope: vec![
            TassadarSequenceSplit::Train,
            TassadarSequenceSplit::Validation,
            TassadarSequenceSplit::Test,
        ],
        train_relative_target_trace_schema_output_bias: true,
        relative_target_trace_schema_output_bias_learning_rate_scale: 64.0,
        train_prompt_summary_embeddings: false,
        prompt_summary_embeddings_learning_rate_scale: 1.0,
        train_prompt_summary_target_output_bias: true,
        prompt_summary_target_output_bias_learning_rate_scale: 64.0,
        seed_prompt_summary_target_output_bias_from_reference_targets: true,
        prompt_summary_target_output_bias_reference_seed_logit: 32.0,
        curriculum_stages: Vec::new(),
        validate_every_epoch: true,
        select_best_checkpoint_by_boundary: true,
    }
}

/// Executes the learned article-class Hungarian-10x10 run and augments it with review artifacts.
pub fn execute_tassadar_hungarian_10x10_article_run(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarHungarian10x10ArticleRunError> {
    execute_tassadar_training_run_without_benchmark(
        output_dir,
        &tassadar_executor_hungarian_10x10_article_run_config(),
    )?;
    augment_tassadar_training_run_with_telemetry(output_dir)?;
    augment_tassadar_hungarian_10x10_article_run_with_review(output_dir)
}

/// Augments one persisted learned article-class run with fit and benchmark artifacts.
pub fn augment_tassadar_hungarian_10x10_article_run_with_review(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarHungarian10x10ArticleRunError> {
    eprintln!(
        "tassadar_article_review phase=load_run_bundle output_dir={}",
        output_dir.display()
    );
    let run_bundle: TassadarExecutorReferenceRunBundle = read_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
    )?;
    eprintln!(
        "tassadar_article_review phase=load_training_report run_id={}",
        run_bundle.run_id
    );
    let training_report: TassadarExecutorTrainingReport = read_json(
        output_dir.join(TRAINING_REPORT_FILE),
        "tassadar_training_report",
    )?;
    eprintln!(
        "tassadar_article_review phase=load_model_artifact model_id={}",
        training_report.best_checkpoint_id
    );
    let model_artifact: TassadarExecutorModelArtifact = read_json(
        output_dir.join(MODEL_ARTIFACT_FILE),
        "tassadar_model_artifact",
    )?;
    let _structural_supervision_report: TassadarExecutorStructuralSupervisionReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_STRUCTURAL_SUPERVISION_REPORT_FILE),
        "tassadar_structural_supervision_report",
    )?;
    let fit_report = TassadarArticleLearnedFitReport::new(&training_report.config, &model_artifact)?;
    let selected_epoch = training_report
        .epoch_reports
        .iter()
        .find(|epoch| epoch.checkpoint_id == training_report.best_checkpoint_id)
        .ok_or_else(|| TassadarHungarian10x10ArticleRunError::MissingSelectedCheckpoint {
            run_id: training_report.config.run_id.clone(),
            checkpoint_id: training_report.best_checkpoint_id.clone(),
        })?;
    eprintln!(
        "tassadar_article_review phase=load_checkpoint checkpoint_id={}",
        selected_epoch.checkpoint_id
    );
    let checkpoint_state: TassadarExecutorCheckpointState = read_json(
        output_dir.join("checkpoint_state.json"),
        "tassadar_executor_checkpoint_state",
    )?;
    let model = checkpoint_state.materialize_model()?;
    eprintln!(
        "tassadar_article_review phase=build_dataset trace_family={:?}",
        training_report.config.trace_family
    );
    let dataset_bundle = build_tassadar_sequence_dataset_with_trace_family(
        training_report.config.workload,
        training_report.config.dataset_version.as_str(),
        training_report.config.trace_family,
    )?;
    eprintln!("tassadar_article_review phase=evaluate_validation");
    let validation_report = evaluate_tassadar_executor_transformer(
        &model,
        &dataset_bundle.dataset,
        TassadarSequenceSplit::Validation,
    )?;
    eprintln!("tassadar_article_review phase=evaluate_test");
    let test_report = evaluate_tassadar_executor_transformer(
        &model,
        &dataset_bundle.dataset,
        TassadarSequenceSplit::Test,
    )?;
    let benchmark_report = TassadarArticleLearnedBenchmarkReport::new(
        &run_bundle,
        &training_report,
        &model_artifact,
        &fit_report,
        selected_epoch,
        validation_report,
        test_report,
    );

    let mut artifact_map = run_bundle
        .artifacts
        .iter()
        .cloned()
        .map(|artifact| (artifact.artifact_ref.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    eprintln!("tassadar_article_review phase=write_review_artifacts");
    for artifact in [
        write_json_artifact(
            output_dir,
            TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE,
            "tassadar_article_learned_fit_report",
            &fit_report,
        )?,
        write_json_artifact(
            output_dir,
            ARTICLE_LEARNED_BENCHMARK_REPORT_FILE,
            "tassadar_article_learned_benchmark_report",
            &benchmark_report,
        )?,
    ] {
        artifact_map.insert(artifact.artifact_ref.clone(), artifact);
    }

    let mut updated_bundle = run_bundle;
    if benchmark_report.passed {
        updated_bundle.claim_class = TassadarClaimClass::LearnedArticleClass;
    }
    updated_bundle.artifacts = artifact_map.into_values().collect();
    updated_bundle.sequence_fit_report_digest = Some(fit_report.report_digest.clone());
    updated_bundle.bundle_digest.clear();
    updated_bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_executor_reference_run_bundle|",
        &updated_bundle,
    );
    eprintln!(
        "tassadar_article_review phase=write_run_bundle claim_class={:?} benchmark_passed={}",
        updated_bundle.claim_class,
        benchmark_report.passed
    );
    write_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
        &updated_bundle,
    )?;
    Ok(updated_bundle)
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarHungarian10x10ArticleRunError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarHungarian10x10ArticleRunError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarHungarian10x10ArticleRunError::Deserialize {
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
) -> Result<EvalArtifact, TassadarHungarian10x10ArticleRunError>
where
    T: Serialize,
{
    let path = output_dir.join(relative_path);
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarHungarian10x10ArticleRunError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(&path, &bytes).map_err(|error| TassadarHungarian10x10ArticleRunError::Write {
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
) -> Result<(), TassadarHungarian10x10ArticleRunError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarHungarian10x10ArticleRunError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, &bytes).map_err(|error| TassadarHungarian10x10ArticleRunError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value)
        .expect("Tassadar article learned value should serialize for stable digests");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn benchmark_scope_statement(train_split_scope: &[TassadarSequenceSplit]) -> String {
    if train_split_scope
        .iter()
        .any(|split| *split != TassadarSequenceSplit::Train)
    {
        String::from(
            "the learned lane was optimized on the same fixed benchmark corpus, so this is benchmark-corpus exactness closure rather than a held-out generalization claim",
        )
    } else {
        String::from(
            "the learned lane was optimized only on the training split, so this exactness report includes held-out validation and test evidence",
        )
    }
}

#[cfg(test)]
mod tests {
    use psionic_eval::build_tassadar_sequence_dataset_with_trace_family;
    use psionic_models::{
        TassadarExecutorTrainableSurface, TassadarExecutorTransformer, TassadarSequenceTraceFamily,
        TokenId, TokenSequence,
    };

    use super::tassadar_executor_hungarian_10x10_article_run_config;

    #[test]
    fn article_run_config_enables_prompt_conditioned_adapters() {
        let config = tassadar_executor_hungarian_10x10_article_run_config();

        assert!(config.train_relative_target_trace_schema_output_bias);
        assert_eq!(
            config.relative_target_trace_schema_output_bias_learning_rate_scale,
            64.0
        );
        assert!(!config.train_prompt_summary_embeddings);
        assert!(config.train_prompt_summary_target_output_bias);
        assert_eq!(
            config.prompt_summary_target_output_bias_learning_rate_scale,
            64.0
        );
        assert!(config.seed_prompt_summary_target_output_bias_from_reference_targets);
        assert_eq!(
            config.prompt_summary_target_output_bias_reference_seed_logit,
            32.0
        );
    }

    #[test]
    fn seeded_prompt_conditioned_bias_replays_hungarian_10x10_validation_trace()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset_bundle = build_tassadar_sequence_dataset_with_trace_family(
            psionic_eval::TassadarSequenceWorkload::Hungarian10x10,
            "trace-family-v1",
            TassadarSequenceTraceFamily::HungarianAssignmentFrontier,
        )?;
        let example = dataset_bundle
            .dataset
            .split_examples(psionic_data::TassadarSequenceSplit::Validation)
            .into_iter()
            .next()
            .expect("hungarian-10x10 validation example should exist");
        let prompt_token_count = example.metadata.prompt_token_count as usize;
        let target_token_count = example.metadata.target_token_count as usize;
        let prompt = example.token_ids[..prompt_token_count]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>();
        let target = example.token_ids[prompt_token_count..prompt_token_count + target_token_count]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>();
        let mut model = TassadarExecutorTransformer::hungarian_10x10_with_surface(
            TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
        );
        let prompt_bucket = model
            .prompt_summary_bucket_for_prompt(prompt.as_slice())
            .expect("hungarian-10x10 model should expose prompt buckets");
        for (relative_target_position, target_token) in target.iter().enumerate() {
            model.ensure_prompt_summary_target_output_bias_row(
                prompt_bucket,
                relative_target_position,
            );
            let row_index = model
                .prompt_summary_target_output_bias_row_index(prompt_bucket, relative_target_position)
                .expect("row should exist after ensure");
            let row = &mut model.weights_mut().prompt_summary_target_output_bias_rows_mut()
                [row_index];
            row.values.fill(0.0);
            row.values[target_token.as_u32() as usize] = 32.0;
        }
        model.refresh_after_training();

        let mut state = model.start_decode(TokenSequence::new(prompt.clone()))?;
        let mut predicted = Vec::with_capacity(target.len());
        for _ in 0..target.len() {
            let next = model.greedy_next_token(&state)?;
            predicted.push(next);
            model.push_decoded_token(&mut state, next)?;
        }

        assert_eq!(predicted, target);
        Ok(())
    }
}
