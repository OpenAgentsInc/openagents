use std::{collections::BTreeMap, fs, path::Path};

use psionic_data::TassadarSequenceSplit;
use psionic_eval::{
    EvalArtifact, TassadarExecutorStructuralSupervisionMetric, TassadarExecutorStructuralSupervisionReport,
    TassadarSequenceEvalError, TassadarSequenceWorkload,
    build_tassadar_sequence_dataset_with_trace_family,
};
use psionic_models::{
    TassadarExecutorLongTraceContract, TassadarExecutorTrainableSurface,
    TassadarSequenceTraceFamily, TassadarStructuralSupervisionFamily,
};
use psionic_runtime::TassadarClaimClass;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE, TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE,
    TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE, TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE,
    TASSADAR_EXECUTOR_STRUCTURAL_SUPERVISION_REPORT_FILE,
    TassadarExecutorContextStageFit, TassadarExecutorModelArtifact, TassadarExecutorReferenceRunBundle,
    TassadarExecutorRunError, TassadarExecutorStructuralSupervisionConfig,
    TassadarExecutorTeacherForcedTrainingStrategy, TassadarExecutorTelemetryError,
    TassadarExecutorTrainingConfig, TassadarExecutorTrainingEpochReport,
    TassadarExecutorTrainingReport, augment_tassadar_training_run_with_telemetry,
    execute_tassadar_training_run_without_benchmark,
};

const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const TRAINING_REPORT_FILE: &str = "training_report.json";
const MODEL_ARTIFACT_FILE: &str = "model_artifact.json";
const LEARNED_LANE_REPORT_FILE: &str = "learned_lane_report.json";

/// Stable run identifier for the first bounded learned Hungarian-v0 lane.
pub const TASSADAR_EXECUTOR_HUNGARIAN_V0_LEARNED_RUN_ID: &str =
    "tassadar-executor-transformer-hungarian-v0-learned-run-v0";
/// Canonical repo path for the first bounded learned Hungarian-v0 lane.
pub const TASSADAR_EXECUTOR_HUNGARIAN_V0_LEARNED_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/hungarian_v0_learned_executor_v0";

const fn default_learned_bounded_claim_class() -> TassadarClaimClass {
    TassadarClaimClass::LearnedBounded
}

/// Machine-readable fit analysis for the bounded learned Hungarian-v0 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianLearnedFitReport {
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

impl TassadarHungarianLearnedFitReport {
    fn new(
        config: &TassadarExecutorTrainingConfig,
        model_artifact: &TassadarExecutorModelArtifact,
    ) -> Result<Self, TassadarHungarianLearnedRunError> {
        let dataset_bundle = build_tassadar_sequence_dataset_with_trace_family(
            config.workload,
            config.dataset_version.as_str(),
            config.trace_family,
        )
        .map_err(TassadarHungarianLearnedRunError::SequenceEval)?;
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
            stable_digest(b"psionic_tassadar_hungarian_learned_fit_report|", &report);
        Ok(report)
    }
}

/// Machine-readable summary for the first bounded learned Hungarian-v0 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianLearnedLaneReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Coarse claim class for this learned lane.
    #[serde(default = "default_learned_bounded_claim_class")]
    pub claim_class: TassadarClaimClass,
    /// Stable workload family identifier.
    pub workload_family_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Stable trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Active trainable surface.
    pub trainable_surface: String,
    /// Structural supervision profile used by the run.
    pub structural_supervision_profile_id: String,
    /// Workload-specific structured-state loss weight used by the run.
    pub workload_specific_state_weight: String,
    /// Selected checkpoint identifier.
    pub checkpoint_id: String,
    /// Stage that produced the selected checkpoint.
    pub selected_stage_id: String,
    /// Aggregate token exactness over the selected checkpoint.
    pub aggregate_target_token_exactness_bps: u32,
    /// First-target exactness over the selected checkpoint.
    pub first_target_exactness_bps: u32,
    /// First-8 exactness over the selected checkpoint.
    pub first_8_token_exactness_bps: u32,
    /// First-32 exactness over the selected checkpoint.
    pub first_32_token_exactness_bps: u32,
    /// Exact validation trace count over the selected checkpoint.
    pub exact_trace_case_count: u32,
    /// Exact final-result case count over the selected checkpoint.
    pub final_output_exact_case_count: u32,
    /// Exact halt case count over the selected checkpoint.
    pub halt_exact_case_count: u32,
    /// Aggregate instruction-pointer exactness over the selected validation split.
    pub instruction_pointer_exactness_bps: u32,
    /// Aggregate branch-outcome exactness over the selected validation split.
    pub branch_outcome_exactness_bps: u32,
    /// Aggregate stack-delta exactness over the selected validation split.
    pub stack_delta_exactness_bps: u32,
    /// Aggregate memory-diff exactness over the selected validation split.
    pub memory_diff_exactness_bps: u32,
    /// Aggregate workload-specific-state exactness over the selected validation split.
    pub workload_specific_state_exactness_bps: u32,
    /// Number of workload-specific-state target tokens in the selected validation split.
    pub workload_specific_state_target_token_count: u32,
    /// Whether the full bounded Hungarian-v0 sequences fit model context.
    pub full_sequence_fits_model_context: bool,
    /// Relative fit report artifact.
    pub sequence_fit_report_ref: String,
    /// Relative structural-supervision report artifact.
    pub structural_supervision_report_ref: String,
    /// Relative exactness-curve artifact.
    pub exactness_curve_ref: String,
    /// Relative failure-samples artifact.
    pub failure_samples_ref: String,
    /// Relative exact-trace-samples artifact.
    pub exact_trace_samples_ref: String,
    /// Relative run bundle artifact.
    pub run_bundle_ref: String,
    /// Honest detail for the learned-vs-compiled boundary.
    pub verdict: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarHungarianLearnedLaneReport {
    fn new(
        run_bundle: &TassadarExecutorReferenceRunBundle,
        training_report: &TassadarExecutorTrainingReport,
        model_artifact: &TassadarExecutorModelArtifact,
        fit_report: &TassadarHungarianLearnedFitReport,
        structural_supervision_report: &TassadarExecutorStructuralSupervisionReport,
        selected_epoch: &TassadarExecutorTrainingEpochReport,
    ) -> Self {
        let workload_specific_metric = structural_metric(
            structural_supervision_report,
            TassadarStructuralSupervisionFamily::WorkloadSpecificState,
        );
        let mut report = Self {
            run_id: run_bundle.run_id.clone(),
            claim_class: TassadarClaimClass::LearnedBounded,
            workload_family_id: String::from("tassadar.wasm.hungarian_v0_matching.v1.learned_executor"),
            dataset_storage_key: run_bundle.dataset_storage_key.clone(),
            dataset_digest: run_bundle.dataset_digest.clone(),
            model_id: model_artifact.descriptor.model.model_id.clone(),
            trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
            trainable_surface: run_bundle.trainable_surface.label().to_string(),
            structural_supervision_profile_id: training_report
                .config
                .structural_supervision
                .profile_id
                .clone(),
            workload_specific_state_weight: format!(
                "{:.2}",
                training_report
                    .config
                    .structural_supervision
                    .workload_specific_state_weight
            ),
            checkpoint_id: selected_epoch.checkpoint_id.clone(),
            selected_stage_id: selected_epoch.stage_id.clone(),
            aggregate_target_token_exactness_bps: selected_epoch
                .evaluation
                .aggregate_target_token_exactness_bps,
            first_target_exactness_bps: selected_epoch.evaluation.first_target_exactness_bps,
            first_8_token_exactness_bps: selected_epoch.evaluation.first_8_token_exactness_bps,
            first_32_token_exactness_bps: selected_epoch.evaluation.first_32_token_exactness_bps,
            exact_trace_case_count: selected_epoch.evaluation.exact_trace_case_count,
            final_output_exact_case_count: selected_epoch.evaluation.final_output_exact_case_count,
            halt_exact_case_count: selected_epoch.evaluation.halt_exact_case_count,
            instruction_pointer_exactness_bps: structural_metric(
                structural_supervision_report,
                TassadarStructuralSupervisionFamily::InstructionPointer,
            )
            .exactness_bps,
            branch_outcome_exactness_bps: structural_metric(
                structural_supervision_report,
                TassadarStructuralSupervisionFamily::BranchOutcome,
            )
            .exactness_bps,
            stack_delta_exactness_bps: structural_metric(
                structural_supervision_report,
                TassadarStructuralSupervisionFamily::StackDelta,
            )
            .exactness_bps,
            memory_diff_exactness_bps: structural_metric(
                structural_supervision_report,
                TassadarStructuralSupervisionFamily::MemoryDiff,
            )
            .exactness_bps,
            workload_specific_state_exactness_bps: workload_specific_metric.exactness_bps,
            workload_specific_state_target_token_count: workload_specific_metric.target_token_count,
            full_sequence_fits_model_context: fit_report.full_sequence_fits_model_context,
            sequence_fit_report_ref: String::from(TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE),
            structural_supervision_report_ref: String::from(
                TASSADAR_EXECUTOR_STRUCTURAL_SUPERVISION_REPORT_FILE,
            ),
            exactness_curve_ref: String::from(TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE),
            failure_samples_ref: String::from(TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE),
            exact_trace_samples_ref: String::from(TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE),
            run_bundle_ref: String::from(RUN_BUNDLE_FILE),
            verdict: format!(
                "bounded learned Hungarian-v0 lane exists with workload-specific state supervision profile `{}` and keeps token exactness, state exactness, and final-result exactness separate; compiled Hungarian exactness remains the only promoted closure",
                training_report.config.structural_supervision.profile_id
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_hungarian_learned_lane_report|", &report);
        report
    }
}

/// Failure while materializing or augmenting the bounded learned Hungarian-v0 lane.
#[derive(Debug, Error)]
pub enum TassadarHungarianLearnedRunError {
    /// Base run execution failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Telemetry augmentation failed.
    #[error(transparent)]
    Telemetry(#[from] TassadarExecutorTelemetryError),
    /// Dataset loading failed.
    #[error(transparent)]
    SequenceEval(#[from] TassadarSequenceEvalError),
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

/// Returns the canonical bounded learned Hungarian-v0 run config.
#[must_use]
pub fn tassadar_executor_hungarian_v0_learned_run_config() -> TassadarExecutorTrainingConfig {
    TassadarExecutorTrainingConfig {
        run_id: String::from(TASSADAR_EXECUTOR_HUNGARIAN_V0_LEARNED_RUN_ID),
        workload: TassadarSequenceWorkload::HungarianV0,
        dataset_version: String::from("v0"),
        epochs: 1,
        learning_rate: 0.05,
        max_train_target_tokens_per_example: None,
        max_eval_target_tokens_per_example: None,
        terminal_stage_learning_rate_scale: None,
        trainable_surface: TassadarExecutorTrainableSurface::OutputHeadOnly,
        teacher_forced_training_strategy:
            TassadarExecutorTeacherForcedTrainingStrategy::FullForwardWindow,
        long_trace_contract: TassadarExecutorLongTraceContract::FlatPrefixFullForward,
        structural_supervision:
            TassadarExecutorStructuralSupervisionConfig::hungarian_dual_state_reference(),
        trace_family: TassadarSequenceTraceFamily::SequentialCpuReference,
        train_split_scope: vec![TassadarSequenceSplit::Train],
        train_relative_target_trace_schema_output_bias: false,
        relative_target_trace_schema_output_bias_learning_rate_scale: 1.0,
        train_prompt_summary_embeddings: false,
        prompt_summary_embeddings_learning_rate_scale: 1.0,
        train_prompt_summary_target_output_bias: false,
        prompt_summary_target_output_bias_learning_rate_scale: 1.0,
        seed_prompt_summary_target_output_bias_from_reference_targets: false,
        prompt_summary_target_output_bias_reference_seed_logit: 0.0,
        curriculum_stages: vec![
            crate::TassadarExecutorCurriculumStage::new("prompt_to_first_8_tokens", Some(8), 1),
            crate::TassadarExecutorCurriculumStage::new("prompt_to_first_64_tokens", Some(64), 1),
            crate::TassadarExecutorCurriculumStage::new("full_trace_supervision", None, 1),
        ],
        validate_every_epoch: true,
        select_best_checkpoint_by_boundary: true,
    }
}

/// Executes the bounded learned Hungarian-v0 run and augments it with fit and
/// learned-lane review artifacts.
pub fn execute_tassadar_hungarian_v0_learned_run(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarHungarianLearnedRunError> {
    execute_tassadar_training_run_without_benchmark(
        output_dir,
        &tassadar_executor_hungarian_v0_learned_run_config(),
    )?;
    augment_tassadar_training_run_with_telemetry(output_dir)?;
    augment_tassadar_hungarian_v0_learned_run_with_review(output_dir)
}

/// Augments one persisted learned Hungarian-v0 run with fit and summary artifacts.
pub fn augment_tassadar_hungarian_v0_learned_run_with_review(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarHungarianLearnedRunError> {
    let run_bundle: TassadarExecutorReferenceRunBundle = read_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
    )?;
    let training_report: TassadarExecutorTrainingReport = read_json(
        output_dir.join(TRAINING_REPORT_FILE),
        "tassadar_training_report",
    )?;
    let model_artifact: TassadarExecutorModelArtifact = read_json(
        output_dir.join(MODEL_ARTIFACT_FILE),
        "tassadar_model_artifact",
    )?;
    let structural_supervision_report: TassadarExecutorStructuralSupervisionReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_STRUCTURAL_SUPERVISION_REPORT_FILE),
        "tassadar_structural_supervision_report",
    )?;
    let fit_report =
        TassadarHungarianLearnedFitReport::new(&training_report.config, &model_artifact)?;
    let selected_epoch = training_report
        .epoch_reports
        .iter()
        .find(|epoch| epoch.checkpoint_id == training_report.best_checkpoint_id)
        .ok_or_else(|| TassadarHungarianLearnedRunError::MissingSelectedCheckpoint {
            run_id: training_report.config.run_id.clone(),
            checkpoint_id: training_report.best_checkpoint_id.clone(),
        })?;
    let learned_lane_report = TassadarHungarianLearnedLaneReport::new(
        &run_bundle,
        &training_report,
        &model_artifact,
        &fit_report,
        &structural_supervision_report,
        selected_epoch,
    );

    let mut artifact_map = run_bundle
        .artifacts
        .iter()
        .cloned()
        .map(|artifact| (artifact.artifact_ref.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    for artifact in [
        write_json_artifact(
            output_dir,
            TASSADAR_EXECUTOR_SEQUENCE_FIT_REPORT_FILE,
            "tassadar_hungarian_learned_fit_report",
            &fit_report,
        )?,
        write_json_artifact(
            output_dir,
            LEARNED_LANE_REPORT_FILE,
            "tassadar_hungarian_learned_lane_report",
            &learned_lane_report,
        )?,
    ] {
        artifact_map.insert(artifact.artifact_ref.clone(), artifact);
    }

    let mut updated_bundle = run_bundle;
    updated_bundle.artifacts = artifact_map.into_values().collect();
    updated_bundle.sequence_fit_report_digest = Some(fit_report.report_digest.clone());
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
    Ok(updated_bundle)
}

fn structural_metric(
    report: &TassadarExecutorStructuralSupervisionReport,
    family: TassadarStructuralSupervisionFamily,
) -> TassadarExecutorStructuralSupervisionMetric {
    report
        .aggregate_metrics
        .iter()
        .find(|metric| metric.family == family)
        .cloned()
        .unwrap_or(TassadarExecutorStructuralSupervisionMetric {
            family,
            target_token_count: 0,
            matched_token_count: 0,
            exactness_bps: 0,
        })
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarHungarianLearnedRunError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarHungarianLearnedRunError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarHungarianLearnedRunError::Deserialize {
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
) -> Result<EvalArtifact, TassadarHungarianLearnedRunError>
where
    T: Serialize,
{
    let path = output_dir.join(relative_path);
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarHungarianLearnedRunError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(&path, &bytes).map_err(|error| TassadarHungarianLearnedRunError::Write {
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
) -> Result<(), TassadarHungarianLearnedRunError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarHungarianLearnedRunError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, &bytes).map_err(|error| TassadarHungarianLearnedRunError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar Hungarian learned value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}
