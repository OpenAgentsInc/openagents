use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use psionic_core::{DType, Device, Shape, TensorData, TensorSpec};
use psionic_eval::{
    TassadarArticleTransformerCheckpointEvidence, TassadarArticleTransformerGradientCheckEvidence,
    TassadarArticleTransformerWeightProductionCaseEvidence,
    TassadarArticleTransformerWeightProductionCaseMetric,
    TassadarArticleTransformerWeightProductionEvidenceBundle,
    TassadarArticleTransformerWeightProductionSplit,
    TassadarArticleTransformerWeightProductionStepEvidence,
};
use psionic_models::{
    TassadarArticleTransformer, TassadarArticleTransformerEmbeddingStrategy,
    TassadarArticleTransformerError, TassadarArticleTransformerParameterVector,
    TassadarTraceTokenizer, TokenizerBoundary,
};
use psionic_runtime::{
    tassadar_article_class_corpus, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
};
use psionic_transformer::{EncoderDecoderTransformerConfig, TransformerExecutionMode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    restore_tassadar_article_transformer_checkpoint_with_base_model, FixedBudgetTrainingRun,
    TassadarArticleTransformerCheckpointArtifact, TassadarArticleTransformerTrainingError,
    TrainingCoreError, TrainingGradientBatch, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, TrainingParameterClass, TrainingParameterGroupState,
    TrainingSchedulerConfig, TrainingStepInput, TrainingStepReceipt, TrainingTensorBuffer,
};

const MODEL_MODULE_REF: &str = "crates/psionic-models/src/tassadar_article_transformer.rs";
const TRANSFORMER_MODULE_REF: &str = "crates/psionic-transformer/src/encoder_decoder.rs";
const TRAIN_MODULE_REF: &str =
    "crates/psionic-train/src/tassadar_article_transformer_weight_production.rs";
const RUNTIME_MODULE_REF: &str = "crates/psionic-runtime/src/tassadar.rs";
const SOURCE_CORPUS_ID: &str = "tassadar.article_class_corpus.v1";
const SUITE_ID: &str = "tassadar.article_transformer.weight_production_suite.v1";
const MAX_TARGET_WINDOW_TOKENS: usize = 32;
const TRAIN_CASE_IDS: &[&str] = &["hungarian_matching"];
const HELD_OUT_CASE_IDS: &[&str] = &[
    "micro_wasm_kernel",
    "branch_heavy_kernel",
    "memory_heavy_kernel",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionExample {
    pub case_id: String,
    pub split: TassadarArticleTransformerWeightProductionSplit,
    pub summary: String,
    pub profile_id: String,
    pub trace_step_count: usize,
    pub expected_output_count: usize,
    pub source_tokens: Vec<usize>,
    pub target_tokens: Vec<usize>,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub full_target_token_count: usize,
    pub source_token_digest: String,
    pub target_token_digest: String,
    pub sequence_digest: String,
    pub halt_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionSuite {
    pub suite_id: String,
    pub description: String,
    pub source_corpus_id: String,
    pub bos_token_id: usize,
    pub training_examples: Vec<TassadarArticleTransformerWeightProductionExample>,
    pub held_out_examples: Vec<TassadarArticleTransformerWeightProductionExample>,
}

impl TassadarArticleTransformerWeightProductionSuite {
    pub fn reference() -> Result<Self, TassadarArticleTransformerWeightProductionError> {
        let model = TassadarArticleTransformer::article_trace_domain_reference()?;
        Self::for_model(&model)
    }

    pub fn for_model(
        model: &TassadarArticleTransformer,
    ) -> Result<Self, TassadarArticleTransformerWeightProductionError> {
        let bos_token_id = TassadarTraceTokenizer::new().vocabulary().bos_id().as_u32() as usize;
        let corpus = tassadar_article_class_corpus()
            .into_iter()
            .map(|case| (case.case_id.clone(), case))
            .collect::<BTreeMap<_, _>>();
        Ok(Self {
            suite_id: String::from(SUITE_ID),
            description: String::from(
                "bounded article-class trace-production slice over one trained Hungarian article demo plus held-out kernel-family cases on a 32-token trace-prefix window",
            ),
            source_corpus_id: String::from(SOURCE_CORPUS_ID),
            bos_token_id,
            training_examples: build_examples(
                &model,
                &corpus,
                TRAIN_CASE_IDS,
                TassadarArticleTransformerWeightProductionSplit::Train,
            )?,
            held_out_examples: build_examples(
                model,
                &corpus,
                HELD_OUT_CASE_IDS,
                TassadarArticleTransformerWeightProductionSplit::HeldOut,
            )?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionConfig {
    pub run_id: String,
    pub checkpoint_family: String,
    pub started_at_ms: u64,
    pub step_duration_ms: u64,
    pub budget: TrainingLoopBudget,
    pub model_config: EncoderDecoderTransformerConfig,
    pub embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
    pub optimizer: TrainingOptimizerConfig,
    pub scheduler: TrainingSchedulerConfig,
    pub label_smoothing: f32,
    pub finite_difference_epsilon: f32,
    pub trainable_parameter_ids: Vec<String>,
    pub base_model_id: String,
    pub base_model_revision: String,
    pub base_descriptor_ref: String,
    pub base_artifact_ref: String,
    pub produced_descriptor_ref: String,
    pub produced_artifact_ref: String,
    pub produced_model_id: String,
    pub produced_model_revision: String,
}

impl TassadarArticleTransformerWeightProductionConfig {
    pub fn reference() -> Result<Self, TassadarArticleTransformerWeightProductionError> {
        let model = TassadarArticleTransformer::article_trace_domain_reference()?;
        Ok(Self {
            run_id: String::from("tassadar-article-transformer-weight-production-v1"),
            checkpoint_family: String::from("train.tassadar.article_transformer.weight_production"),
            started_at_ms: 1_774_406_400_000,
            step_duration_ms: 250,
            budget: TrainingLoopBudget::new(1, 2, 1)?,
            model_config: model.descriptor().config.clone(),
            embedding_strategy: model.embedding_strategy(),
            optimizer: TrainingOptimizerConfig::adam(0.35, 0.9, 0.98, 1e-8)
                .with_gradient_clip_norm(1.0),
            scheduler: TrainingSchedulerConfig::inverse_square_root_warmup(1, 1.0),
            label_smoothing: 0.01,
            finite_difference_epsilon: 0.01,
            trainable_parameter_ids: vec![String::from(
                TassadarArticleTransformer::LOGITS_PROJECTION_BIAS_PARAMETER_ID,
            )],
            base_model_id: String::from(TassadarArticleTransformer::TRACE_BOUND_MODEL_ID),
            base_model_revision: String::from("v0"),
            base_descriptor_ref: String::from(
                TassadarArticleTransformer::TRACE_BOUND_DESCRIPTOR_REF,
            ),
            base_artifact_ref: String::from(TassadarArticleTransformer::TRACE_BOUND_ARTIFACT_REF),
            produced_descriptor_ref: String::from(
                TassadarArticleTransformer::TRAINED_TRACE_BOUND_DESCRIPTOR_REF,
            ),
            produced_artifact_ref: String::from(
                TassadarArticleTransformer::TRAINED_TRACE_BOUND_ARTIFACT_REF,
            ),
            produced_model_id: String::from(
                TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID,
            ),
            produced_model_revision: String::from("v0"),
        })
    }

    fn validate_against_base_model(
        &self,
        base_model: &TassadarArticleTransformer,
    ) -> Result<(), TassadarArticleTransformerWeightProductionError> {
        if self.run_id.trim().is_empty() {
            return Err(TassadarArticleTransformerWeightProductionError::MissingRunId);
        }
        if self.checkpoint_family.trim().is_empty() {
            return Err(TassadarArticleTransformerWeightProductionError::MissingCheckpointFamily);
        }
        if self.step_duration_ms == 0 {
            return Err(TassadarArticleTransformerWeightProductionError::InvalidStepDuration);
        }
        if self.base_model_id.trim().is_empty() {
            return Err(TassadarArticleTransformerWeightProductionError::MissingBaseModelId);
        }
        if self.base_model_revision.trim().is_empty() {
            return Err(TassadarArticleTransformerWeightProductionError::MissingBaseModelRevision);
        }
        if self.base_descriptor_ref.trim().is_empty() {
            return Err(TassadarArticleTransformerWeightProductionError::MissingBaseDescriptorRef);
        }
        if self.base_artifact_ref.trim().is_empty() {
            return Err(TassadarArticleTransformerWeightProductionError::MissingBaseArtifactRef);
        }
        if self.produced_model_id.trim().is_empty() {
            return Err(TassadarArticleTransformerWeightProductionError::MissingProducedModelId);
        }
        if self.produced_model_revision.trim().is_empty() {
            return Err(
                TassadarArticleTransformerWeightProductionError::MissingProducedModelRevision,
            );
        }
        if self.model_config != base_model.descriptor().config {
            return Err(TassadarArticleTransformerWeightProductionError::ConfigMismatch);
        }
        if self.embedding_strategy != base_model.embedding_strategy() {
            return Err(TassadarArticleTransformerWeightProductionError::EmbeddingStrategyMismatch);
        }
        if !self.label_smoothing.is_finite()
            || self.label_smoothing <= 0.0
            || self.label_smoothing >= 1.0
        {
            return Err(
                TassadarArticleTransformerWeightProductionError::InvalidLabelSmoothing {
                    value: self.label_smoothing,
                },
            );
        }
        if !self.finite_difference_epsilon.is_finite() || self.finite_difference_epsilon <= 0.0 {
            return Err(
                TassadarArticleTransformerWeightProductionError::InvalidFiniteDifferenceEpsilon {
                    epsilon: self.finite_difference_epsilon,
                },
            );
        }
        if self.trainable_parameter_ids.is_empty() {
            return Err(TassadarArticleTransformerWeightProductionError::EmptyTrainableSurface);
        }
        let available_ids = base_model
            .trainable_parameter_vectors()
            .into_iter()
            .map(|parameter| parameter.parameter_id)
            .collect::<BTreeSet<_>>();
        for parameter_id in &self.trainable_parameter_ids {
            if !available_ids.contains(parameter_id) {
                return Err(
                    TassadarArticleTransformerWeightProductionError::UnknownParameter {
                        parameter_id: parameter_id.clone(),
                    },
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionOutcome {
    pub base_model: TassadarArticleTransformer,
    pub trained_model: TassadarArticleTransformer,
    pub restored_checkpoint_model: TassadarArticleTransformer,
    pub produced_model: TassadarArticleTransformer,
    pub reloaded_artifact_model: TassadarArticleTransformer,
    pub gradient_checks: Vec<TassadarArticleTransformerGradientCheckEvidence>,
    pub step_receipts: Vec<TrainingStepReceipt>,
    pub step_evidence: Vec<TassadarArticleTransformerWeightProductionStepEvidence>,
    pub case_metrics: Vec<TassadarArticleTransformerWeightProductionCaseMetric>,
    pub initial_training_mean_loss: f32,
    pub final_training_mean_loss: f32,
    pub initial_training_token_exactness_bps: u32,
    pub final_training_token_exactness_bps: u32,
    pub initial_training_exact_match_count: usize,
    pub final_training_exact_match_count: usize,
    pub final_held_out_mean_loss: f32,
    pub final_held_out_token_exactness_bps: u32,
    pub final_held_out_exact_match_count: usize,
    pub initial_checkpoint: TassadarArticleTransformerCheckpointArtifact,
    pub final_checkpoint: TassadarArticleTransformerCheckpointArtifact,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerWeightProductionError {
    #[error("article Transformer weight production requires a non-empty run id")]
    MissingRunId,
    #[error("article Transformer weight production requires a non-empty checkpoint family")]
    MissingCheckpointFamily,
    #[error("article Transformer weight production requires a non-empty base model id")]
    MissingBaseModelId,
    #[error("article Transformer weight production requires a non-empty base model revision")]
    MissingBaseModelRevision,
    #[error("article Transformer weight production requires a non-empty base descriptor ref")]
    MissingBaseDescriptorRef,
    #[error("article Transformer weight production requires a non-empty base artifact ref")]
    MissingBaseArtifactRef,
    #[error("article Transformer weight production requires a non-empty produced model id")]
    MissingProducedModelId,
    #[error("article Transformer weight production requires a non-empty produced model revision")]
    MissingProducedModelRevision,
    #[error("article Transformer weight production requires a non-zero step duration")]
    InvalidStepDuration,
    #[error(
        "article Transformer weight production requires label_smoothing in the open interval (0, 1), got {value}"
    )]
    InvalidLabelSmoothing { value: f32 },
    #[error(
        "article Transformer weight production requires a positive finite-difference epsilon, got {epsilon}"
    )]
    InvalidFiniteDifferenceEpsilon { epsilon: f32 },
    #[error(
        "article Transformer weight production suite must contain at least one training example"
    )]
    EmptyTrainingExamples,
    #[error(
        "article Transformer weight production suite must contain at least one held-out example"
    )]
    EmptyHeldOutExamples,
    #[error("article Transformer weight production suite requires one BOS token in the canonical tokenizer")]
    MissingBosToken,
    #[error("article Transformer weight production config does not match the committed trace-bound base model config")]
    ConfigMismatch,
    #[error("article Transformer weight production config does not match the committed trace-bound embedding strategy")]
    EmbeddingStrategyMismatch,
    #[error("article Transformer weight production requires a non-empty trainable surface")]
    EmptyTrainableSurface,
    #[error("article Transformer weight production requested unknown trainable parameter `{parameter_id}`")]
    UnknownParameter { parameter_id: String },
    #[error("article Transformer article-class suite omitted expected case `{case_id}`")]
    MissingCase { case_id: String },
    #[error(
        "article Transformer weight production example `{case_id}` has an empty source sequence"
    )]
    EmptySourceSequence { case_id: String },
    #[error(
        "article Transformer weight production example `{case_id}` has an empty target sequence"
    )]
    EmptyTargetSequence { case_id: String },
    #[error(
        "article Transformer token {token_id} in `{case_id}` exceeds vocabulary size {vocab_size}"
    )]
    TokenOutOfRange {
        case_id: String,
        token_id: usize,
        vocab_size: usize,
    },
    #[error("article Transformer weight production run is missing parameter group `{group_id}`")]
    MissingParameterGroup { group_id: String },
    #[error("article Transformer weight production group `{group_id}` is not dense f32")]
    NonDenseGroup { group_id: String },
    #[error("failed to build article-class execution for `{case_id}`: {error}")]
    CpuReference {
        case_id: String,
        error: TassadarExecutionRefusal,
    },
    #[error("{context}: {message}")]
    Serialization {
        context: &'static str,
        message: String,
    },
    #[error(transparent)]
    TrainCore(#[from] TrainingCoreError),
    #[error(transparent)]
    Training(#[from] TassadarArticleTransformerTrainingError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error(transparent)]
    Eval(#[from] psionic_eval::TassadarArticleTransformerWeightProductionError),
}

pub fn run_tassadar_article_transformer_weight_production(
    suite: &TassadarArticleTransformerWeightProductionSuite,
    config: &TassadarArticleTransformerWeightProductionConfig,
) -> Result<
    TassadarArticleTransformerWeightProductionOutcome,
    TassadarArticleTransformerWeightProductionError,
> {
    let base_model = TassadarArticleTransformer::paper_faithful_reference(
        config.model_config.clone(),
        config.embedding_strategy,
    )?
    .with_model_identity(
        config.base_model_id.clone(),
        config.base_model_revision.clone(),
    )?;
    config.validate_against_base_model(&base_model)?;
    validate_suite(suite, &config.model_config)?;
    let base_descriptor_path = resolve_output_path(&config.base_descriptor_ref);
    let base_artifact_path = resolve_output_path(&config.base_artifact_ref);
    base_model.write_artifact_bundle(&base_descriptor_path, &base_artifact_path)?;

    let selected_parameters =
        selected_trainable_parameters(&base_model, config.trainable_parameter_ids.as_slice());
    let gradient_checks = build_gradient_checks(
        &base_model,
        suite,
        config.label_smoothing,
        config.finite_difference_epsilon,
        config.trainable_parameter_ids.as_slice(),
    )?;
    let initial_training_metrics = dataset_metrics(
        &base_model,
        suite,
        &suite.training_examples,
        config.label_smoothing,
    )?;
    let mut run = FixedBudgetTrainingRun::new(
        config.run_id.clone(),
        config.checkpoint_family.clone(),
        config.budget,
        build_training_groups(selected_parameters.as_slice(), config)?,
    )?;
    let initial_checkpoint = export_checkpoint(&base_model, &base_model, suite, config, 0, None)?;

    let mut current_model = base_model.clone();
    let mut step_receipts = Vec::new();
    let mut step_evidence = Vec::new();
    for step_index in 0..config.budget.max_steps {
        let batch = build_gradient_batch(
            &base_model,
            &current_model,
            suite,
            config.label_smoothing,
            config.finite_difference_epsilon,
            config.trainable_parameter_ids.as_slice(),
        )?;
        let started_at_ms = config
            .started_at_ms
            .saturating_add(step_index.saturating_mul(config.step_duration_ms));
        let finished_at_ms = started_at_ms.saturating_add(config.step_duration_ms);
        let receipt =
            run.apply_step(TrainingStepInput::new(batch, started_at_ms, finished_at_ms))?;
        current_model =
            materialize_model(&base_model, &run, config.trainable_parameter_ids.as_slice())?;
        let training_metrics = dataset_metrics(
            &current_model,
            suite,
            &suite.training_examples,
            config.label_smoothing,
        )?;
        let held_out_metrics = dataset_metrics(
            &current_model,
            suite,
            &suite.held_out_examples,
            config.label_smoothing,
        )?;
        step_evidence.push(TassadarArticleTransformerWeightProductionStepEvidence {
            global_step: receipt.schedule.global_step,
            batch_id: receipt.batch_id.clone(),
            training_mean_loss: training_metrics.mean_loss,
            training_token_exactness_bps: training_metrics.token_exactness_bps(),
            training_exact_match_count: training_metrics.exact_match_count,
            held_out_mean_loss: held_out_metrics.mean_loss,
            held_out_token_exactness_bps: held_out_metrics.token_exactness_bps(),
            held_out_exact_match_count: held_out_metrics.exact_match_count,
            effective_learning_rates: receipt
                .group_telemetry
                .iter()
                .map(|row| (row.group_id.clone(), row.effective_learning_rate))
                .collect(),
            scheduler_kinds: receipt
                .group_telemetry
                .iter()
                .map(|row| {
                    (
                        row.group_id.clone(),
                        row.scheduler_kind
                            .map(scheduler_kind_label)
                            .unwrap_or_else(|| String::from("none")),
                    )
                })
                .collect(),
        });
        step_receipts.push(receipt);
    }

    let final_checkpoint = export_checkpoint(
        &base_model,
        &current_model,
        suite,
        config,
        config.budget.max_steps,
        Some(&initial_checkpoint),
    )?;
    let restored_checkpoint_model =
        restore_tassadar_article_transformer_checkpoint_with_base_model(
            &base_model,
            &final_checkpoint.manifest,
            &final_checkpoint.weights_bytes,
        )?;
    let final_training_metrics = dataset_metrics(
        &current_model,
        suite,
        &suite.training_examples,
        config.label_smoothing,
    )?;
    let final_held_out_metrics = dataset_metrics(
        &current_model,
        suite,
        &suite.held_out_examples,
        config.label_smoothing,
    )?;

    let produced_model = current_model.with_model_identity(
        config.produced_model_id.clone(),
        config.produced_model_revision.clone(),
    )?;
    let produced_descriptor_path = resolve_output_path(&config.produced_descriptor_ref);
    let produced_artifact_path = resolve_output_path(&config.produced_artifact_ref);
    produced_model.write_artifact_bundle(&produced_descriptor_path, &produced_artifact_path)?;
    let reloaded_artifact_model =
        TassadarArticleTransformer::load_from_descriptor_path(&produced_descriptor_path)?;

    Ok(TassadarArticleTransformerWeightProductionOutcome {
        base_model,
        trained_model: current_model.clone(),
        restored_checkpoint_model,
        produced_model,
        reloaded_artifact_model,
        gradient_checks,
        step_receipts,
        step_evidence,
        case_metrics: case_metrics(&current_model, suite, config.label_smoothing)?,
        initial_training_mean_loss: initial_training_metrics.mean_loss,
        final_training_mean_loss: final_training_metrics.mean_loss,
        initial_training_token_exactness_bps: initial_training_metrics.token_exactness_bps(),
        final_training_token_exactness_bps: final_training_metrics.token_exactness_bps(),
        initial_training_exact_match_count: initial_training_metrics.exact_match_count,
        final_training_exact_match_count: final_training_metrics.exact_match_count,
        final_held_out_mean_loss: final_held_out_metrics.mean_loss,
        final_held_out_token_exactness_bps: final_held_out_metrics.token_exactness_bps(),
        final_held_out_exact_match_count: final_held_out_metrics.exact_match_count,
        initial_checkpoint,
        final_checkpoint,
    })
}

pub fn build_tassadar_article_transformer_weight_production_evidence_bundle(
    suite: &TassadarArticleTransformerWeightProductionSuite,
    config: &TassadarArticleTransformerWeightProductionConfig,
    outcome: &TassadarArticleTransformerWeightProductionOutcome,
) -> TassadarArticleTransformerWeightProductionEvidenceBundle {
    let final_checkpoint = &outcome.final_checkpoint;
    let trained_digest = outcome.trained_model.trainable_parameter_digest();
    TassadarArticleTransformerWeightProductionEvidenceBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.article_transformer.weight_production_bundle.v1"),
        tied_requirement_id: String::from("TAS-169"),
        model_module_ref: String::from(MODEL_MODULE_REF),
        transformer_module_ref: String::from(TRANSFORMER_MODULE_REF),
        train_module_ref: String::from(TRAIN_MODULE_REF),
        runtime_module_ref: String::from(RUNTIME_MODULE_REF),
        run_id: config.run_id.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        suite_id: suite.suite_id.clone(),
        suite_description: suite.description.clone(),
        source_corpus_id: suite.source_corpus_id.clone(),
        architecture_variant: outcome.produced_model.descriptor().architecture_variant,
        config: config.model_config.clone(),
        embedding_strategy: config.embedding_strategy,
        trainable_parameter_ids: config.trainable_parameter_ids.clone(),
        trainable_parameter_scalar_count: selected_trainable_parameters(
            &outcome.base_model,
            config.trainable_parameter_ids.as_slice(),
        )
        .iter()
        .map(|parameter| parameter.values.len())
        .sum(),
        loss_kind: String::from("label_smoothed_cross_entropy"),
        optimizer_kind: format!("{:?}", config.optimizer.kind).to_lowercase(),
        scheduler_kind: scheduler_kind_label(config.scheduler.kind()),
        warmup_steps: match config.scheduler {
            TrainingSchedulerConfig::InverseSquareRootWarmup { warmup_steps, .. } => warmup_steps,
            _ => 0,
        },
        label_smoothing: config.label_smoothing,
        finite_difference_epsilon: config.finite_difference_epsilon,
        base_descriptor_ref: config.base_descriptor_ref.clone(),
        base_artifact_ref: config.base_artifact_ref.clone(),
        produced_descriptor_ref: config.produced_descriptor_ref.clone(),
        produced_artifact_ref: config.produced_artifact_ref.clone(),
        training_cases: suite_case_evidence(&suite.training_examples),
        held_out_cases: suite_case_evidence(&suite.held_out_examples),
        gradient_checks: outcome.gradient_checks.clone(),
        step_evidence: outcome.step_evidence.clone(),
        case_metrics: outcome.case_metrics.clone(),
        base_model_artifact_binding: outcome.base_model.model_artifact_binding(),
        produced_model_artifact_binding: outcome.reloaded_artifact_model.model_artifact_binding(),
        initial_training_mean_loss: outcome.initial_training_mean_loss,
        final_training_mean_loss: outcome.final_training_mean_loss,
        initial_training_token_exactness_bps: outcome.initial_training_token_exactness_bps,
        final_training_token_exactness_bps: outcome.final_training_token_exactness_bps,
        initial_training_exact_match_count: outcome.initial_training_exact_match_count,
        final_training_exact_match_count: outcome.final_training_exact_match_count,
        final_held_out_mean_loss: outcome.final_held_out_mean_loss,
        final_held_out_token_exactness_bps: outcome.final_held_out_token_exactness_bps,
        final_held_out_exact_match_count: outcome.final_held_out_exact_match_count,
        produced_artifact_differs_from_base: outcome.base_model.model_artifact_binding().weight_bundle_digest
            != outcome
                .reloaded_artifact_model
                .model_artifact_binding()
                .weight_bundle_digest,
        checkpoint: TassadarArticleTransformerCheckpointEvidence {
            checkpoint_ref: final_checkpoint
                .checkpoint
                .checkpoint_ref
                .clone()
                .unwrap_or_else(|| String::from("missing")),
            checkpoint_family: final_checkpoint.manifest.checkpoint_family.clone(),
            stream_id: final_checkpoint.checkpoint.stream_id.clone(),
            manifest_digest: final_checkpoint.manifest.stable_digest(),
            object_digest: final_checkpoint.checkpoint.object_digest.clone(),
            writer_node_id: final_checkpoint.checkpoint.writer_node_id.clone(),
            membership_epoch: final_checkpoint.checkpoint.membership_epoch,
            cluster_state_digest: final_checkpoint.checkpoint.cluster_state_digest.clone(),
            topology_digest: final_checkpoint.checkpoint.topology_digest.clone(),
            started_at_ms: final_checkpoint.checkpoint.started_at_ms,
            step: final_checkpoint.checkpoint.step.unwrap_or(final_checkpoint.manifest.step),
            durable_at_ms: final_checkpoint
                .checkpoint
                .durable_at_ms
                .unwrap_or(final_checkpoint.checkpoint.started_at_ms),
            parent_checkpoint_ref: final_checkpoint.manifest.parent_checkpoint_ref.clone(),
            parent_manifest_digest: final_checkpoint.manifest.parent_manifest_digest.clone(),
            trained_trainable_parameter_digest: trained_digest.clone(),
            restored_trainable_parameter_digest: outcome
                .restored_checkpoint_model
                .trainable_parameter_digest(),
            restore_matches_trained_state: trained_digest
                == outcome
                    .restored_checkpoint_model
                    .trainable_parameter_digest(),
        },
        artifact_reload_matches_trained_state: trained_digest
            == outcome
                .reloaded_artifact_model
                .trainable_parameter_digest(),
        artifact_reload_descriptor_digest: outcome
            .reloaded_artifact_model
            .descriptor()
            .stable_digest(),
        claim_boundary: String::from(
            "this evidence bundle covers the first real Transformer-backed article-model weight production run on a bounded article-class slice. It proves that the owned trace-bound article wrapper can train on canonical trace-domain targets, emit a committed trained safetensors artifact, reload that artifact exactly, and preserve checkpoint restore parity. It does not claim full article-class exactness, benchmark parity, fast-route closure, or final article-equivalence green status.",
        ),
        bundle_digest: String::new(),
    }
    .with_bundle_digest()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DatasetMetrics {
    mean_loss: f32,
    exact_match_count: usize,
    token_exact_count: usize,
    token_count: usize,
}

impl DatasetMetrics {
    fn token_exactness_bps(self) -> u32 {
        if self.token_count == 0 {
            0
        } else {
            ((self.token_exact_count * 10_000) / self.token_count) as u32
        }
    }
}

fn build_examples(
    model: &TassadarArticleTransformer,
    corpus: &BTreeMap<String, psionic_runtime::TassadarValidationCase>,
    case_ids: &[&str],
    split: TassadarArticleTransformerWeightProductionSplit,
) -> Result<
    Vec<TassadarArticleTransformerWeightProductionExample>,
    TassadarArticleTransformerWeightProductionError,
> {
    case_ids
        .iter()
        .map(|case_id| {
            let case = corpus.get(*case_id).ok_or_else(|| {
                TassadarArticleTransformerWeightProductionError::MissingCase {
                    case_id: String::from(*case_id),
                }
            })?;
            let execution = TassadarCpuReferenceRunner::for_program(&case.program)
                .map_err(
                    |error| TassadarArticleTransformerWeightProductionError::CpuReference {
                        case_id: case.case_id.clone(),
                        error,
                    },
                )?
                .execute(&case.program)
                .map_err(
                    |error| TassadarArticleTransformerWeightProductionError::CpuReference {
                        case_id: case.case_id.clone(),
                        error,
                    },
                )?;
            let batch = model.encode_article_trace_domain(&case.program, &execution)?;
            let target_tokens = batch
                .target_token_ids
                .iter()
                .copied()
                .take(MAX_TARGET_WINDOW_TOKENS)
                .collect::<Vec<_>>();
            let sequence_digest = stable_digest(
                b"psionic_tassadar_article_transformer_weight_production_window_sequence|",
                &(batch.source_token_ids.as_slice(), target_tokens.as_slice()),
            );
            let target_token_count = target_tokens.len();
            let target_token_digest = stable_digest(
                b"psionic_tassadar_article_transformer_weight_production_target_tokens|",
                &target_tokens,
            );
            Ok(TassadarArticleTransformerWeightProductionExample {
                case_id: case.case_id.clone(),
                split,
                summary: case.summary.clone(),
                profile_id: case.program.profile_id.clone(),
                trace_step_count: execution.steps.len(),
                expected_output_count: execution.outputs.len(),
                source_tokens: batch.source_token_ids.clone(),
                target_tokens,
                prompt_token_count: batch.prompt_token_count,
                target_token_count,
                full_target_token_count: batch.target_token_count,
                source_token_digest: stable_digest(
                    b"psionic_tassadar_article_transformer_weight_production_source_tokens|",
                    &batch.source_token_ids,
                ),
                target_token_digest,
                sequence_digest,
                halt_reason: format!("{:?}", execution.halt_reason).to_lowercase(),
            })
        })
        .collect()
}

fn suite_case_evidence(
    examples: &[TassadarArticleTransformerWeightProductionExample],
) -> Vec<TassadarArticleTransformerWeightProductionCaseEvidence> {
    examples
        .iter()
        .map(
            |example| TassadarArticleTransformerWeightProductionCaseEvidence {
                case_id: example.case_id.clone(),
                split: example.split,
                summary: example.summary.clone(),
                profile_id: example.profile_id.clone(),
                trace_step_count: example.trace_step_count,
                expected_output_count: example.expected_output_count,
                prompt_token_count: example.prompt_token_count,
                target_token_count: example.target_token_count,
                full_target_token_count: example.full_target_token_count,
                source_token_digest: example.source_token_digest.clone(),
                target_token_digest: example.target_token_digest.clone(),
                sequence_digest: example.sequence_digest.clone(),
                halt_reason: example.halt_reason.clone(),
            },
        )
        .collect()
}

fn validate_suite(
    suite: &TassadarArticleTransformerWeightProductionSuite,
    config: &EncoderDecoderTransformerConfig,
) -> Result<(), TassadarArticleTransformerWeightProductionError> {
    if suite.training_examples.is_empty() {
        return Err(TassadarArticleTransformerWeightProductionError::EmptyTrainingExamples);
    }
    if suite.held_out_examples.is_empty() {
        return Err(TassadarArticleTransformerWeightProductionError::EmptyHeldOutExamples);
    }
    for example in suite
        .training_examples
        .iter()
        .chain(suite.held_out_examples.iter())
    {
        if example.source_tokens.is_empty() {
            return Err(
                TassadarArticleTransformerWeightProductionError::EmptySourceSequence {
                    case_id: example.case_id.clone(),
                },
            );
        }
        if example.target_tokens.is_empty() {
            return Err(
                TassadarArticleTransformerWeightProductionError::EmptyTargetSequence {
                    case_id: example.case_id.clone(),
                },
            );
        }
        for &token_id in example
            .source_tokens
            .iter()
            .chain(example.target_tokens.iter())
            .chain(std::iter::once(&suite.bos_token_id))
        {
            if token_id >= config.source_vocab_size || token_id >= config.target_vocab_size {
                return Err(
                    TassadarArticleTransformerWeightProductionError::TokenOutOfRange {
                        case_id: example.case_id.clone(),
                        token_id,
                        vocab_size: config.source_vocab_size.min(config.target_vocab_size),
                    },
                );
            }
        }
    }
    Ok(())
}

fn selected_trainable_parameters(
    model: &TassadarArticleTransformer,
    selected_ids: &[String],
) -> Vec<TassadarArticleTransformerParameterVector> {
    let selected = selected_ids.iter().cloned().collect::<BTreeSet<_>>();
    model
        .trainable_parameter_vectors()
        .into_iter()
        .filter(|parameter| selected.contains(&parameter.parameter_id))
        .collect()
}

fn build_training_groups(
    parameters: &[TassadarArticleTransformerParameterVector],
    config: &TassadarArticleTransformerWeightProductionConfig,
) -> Result<Vec<TrainingParameterGroupState>, TassadarArticleTransformerWeightProductionError> {
    parameters
        .iter()
        .map(|parameter| {
            let class = if parameter.parameter_id.ends_with("bias") {
                TrainingParameterClass::Bias
            } else {
                TrainingParameterClass::Embedding
            };
            Ok(TrainingParameterGroupState::new(
                parameter.parameter_id.clone(),
                class,
                TrainingTensorBuffer::from_f32(
                    parameter.parameter_id.clone(),
                    TensorSpec::new(
                        Shape::new(parameter.shape.clone()),
                        DType::F32,
                        Device::cpu(),
                    ),
                    parameter.values.clone(),
                )?,
                config.optimizer.clone(),
                TrainingOptimizerResidencyPolicy::host_only(),
            )?
            .with_scheduler(config.scheduler.clone()))
        })
        .collect()
}

fn build_gradient_checks(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    label_smoothing: f32,
    epsilon: f32,
    selected_ids: &[String],
) -> Result<
    Vec<TassadarArticleTransformerGradientCheckEvidence>,
    TassadarArticleTransformerWeightProductionError,
> {
    let gradients = parameter_gradients(model, suite, label_smoothing, epsilon, selected_ids)?;
    Ok(gradients
        .into_iter()
        .map(|(parameter_id, values)| {
            let max_abs_gradient = values
                .iter()
                .copied()
                .map(f32::abs)
                .fold(0.0f32, f32::max);
            let all_finite = values.iter().all(|value| value.is_finite());
            TassadarArticleTransformerGradientCheckEvidence {
                gradient_len: values.len(),
                parameter_id,
                max_abs_gradient,
                all_finite,
                detail: String::from(
                    "selected trace-bound article-Transformer gradients were computed directly on the bounded trainable surface, using exact bias gradients where available and finite differences otherwise",
                ),
            }
        })
        .collect())
}

fn build_gradient_batch(
    base_model: &TassadarArticleTransformer,
    current_model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    label_smoothing: f32,
    epsilon: f32,
    selected_ids: &[String],
) -> Result<TrainingGradientBatch, TassadarArticleTransformerWeightProductionError> {
    let batch_loss = mean_dataset_loss(
        current_model,
        suite,
        &suite.training_examples,
        label_smoothing,
    )?;
    let gradients =
        parameter_gradients(current_model, suite, label_smoothing, epsilon, selected_ids)?;
    let mut batch_gradients = BTreeMap::new();
    for parameter in selected_trainable_parameters(base_model, selected_ids) {
        let values = gradients
            .get(parameter.parameter_id.as_str())
            .cloned()
            .unwrap_or_else(|| vec![0.0; parameter.values.len()]);
        batch_gradients.insert(
            parameter.parameter_id.clone(),
            TrainingTensorBuffer::from_f32(
                parameter.parameter_id.clone(),
                TensorSpec::new(
                    Shape::new(parameter.shape.clone()),
                    DType::F32,
                    Device::cpu(),
                ),
                values,
            )?,
        );
    }
    Ok(TrainingGradientBatch::new(
        format!("{}-gradient-batch", suite.suite_id),
        batch_loss,
        suite.training_examples.len() as u32,
        batch_gradients,
    ))
}

fn parameter_gradients(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    label_smoothing: f32,
    epsilon: f32,
    selected_ids: &[String],
) -> Result<BTreeMap<String, Vec<f32>>, TassadarArticleTransformerWeightProductionError> {
    let mut gradients = BTreeMap::new();
    for parameter in selected_trainable_parameters(model, selected_ids) {
        if parameter.parameter_id == TassadarArticleTransformer::LOGITS_PROJECTION_BIAS_PARAMETER_ID
        {
            gradients.insert(
                parameter.parameter_id.clone(),
                analytic_logits_projection_bias_gradient(model, suite, label_smoothing)?,
            );
            continue;
        }
        let mut gradient = vec![0.0; parameter.values.len()];
        for index in 0..gradient.len() {
            let mut plus = BTreeMap::new();
            let mut minus = BTreeMap::new();
            let mut plus_values = parameter.values.clone();
            let mut minus_values = parameter.values.clone();
            plus_values[index] += epsilon;
            minus_values[index] -= epsilon;
            plus.insert(parameter.parameter_id.clone(), plus_values);
            minus.insert(parameter.parameter_id.clone(), minus_values);
            let plus_model = model.with_parameter_overrides(&plus)?;
            let minus_model = model.with_parameter_overrides(&minus)?;
            let plus_loss = mean_dataset_loss(
                &plus_model,
                suite,
                &suite.training_examples,
                label_smoothing,
            )?;
            let minus_loss = mean_dataset_loss(
                &minus_model,
                suite,
                &suite.training_examples,
                label_smoothing,
            )?;
            gradient[index] = (plus_loss - minus_loss) / (2.0 * epsilon);
        }
        gradients.insert(parameter.parameter_id.clone(), gradient);
    }
    Ok(gradients)
}

fn analytic_logits_projection_bias_gradient(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    label_smoothing: f32,
) -> Result<Vec<f32>, TassadarArticleTransformerWeightProductionError> {
    let vocab_size = model.descriptor().config.target_vocab_size;
    let mut gradient = vec![0.0f32; vocab_size];
    for example in &suite.training_examples {
        let target_input = decoder_input_tokens(suite.bos_token_id, &example.target_tokens);
        let output = model.forward(
            example.source_tokens.len().into_batch_shape(),
            example.source_tokens.as_slice(),
            target_input.len().into_batch_shape(),
            target_input.as_slice(),
            TransformerExecutionMode::Eval,
        )?;
        let logits = dense_f32_values(
            &output.logits.data,
            "article_transformer_weight_production.bias_gradient",
        )?;
        for (position, &target_token) in example.target_tokens.iter().enumerate() {
            let start = position * vocab_size;
            let row = &logits[start..start + vocab_size];
            let probabilities = softmax_probabilities(row);
            let class_count = vocab_size as f32;
            let off_target = label_smoothing / class_count;
            let on_target = (1.0 - label_smoothing) + off_target;
            for (index, probability) in probabilities.into_iter().enumerate() {
                let target = if index == target_token {
                    on_target
                } else {
                    off_target
                };
                gradient[index] += (probability - target)
                    / example.target_tokens.len() as f32
                    / suite.training_examples.len() as f32;
            }
        }
    }
    Ok(gradient)
}

fn materialize_model(
    base_model: &TassadarArticleTransformer,
    run: &FixedBudgetTrainingRun,
    selected_ids: &[String],
) -> Result<TassadarArticleTransformer, TassadarArticleTransformerWeightProductionError> {
    let mut overrides = BTreeMap::new();
    for parameter in selected_trainable_parameters(base_model, selected_ids) {
        let group = required_group(run, parameter.parameter_id.as_str())?;
        overrides.insert(
            parameter.parameter_id.clone(),
            dense_values(group, parameter.parameter_id.as_str())?.to_vec(),
        );
    }
    base_model
        .with_parameter_overrides(&overrides)
        .map_err(Into::into)
}

fn export_checkpoint(
    base_model: &TassadarArticleTransformer,
    current_model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    config: &TassadarArticleTransformerWeightProductionConfig,
    step: u64,
    parent: Option<&TassadarArticleTransformerCheckpointArtifact>,
) -> Result<
    TassadarArticleTransformerCheckpointArtifact,
    TassadarArticleTransformerWeightProductionError,
> {
    let parameter_vectors =
        selected_trainable_parameters(current_model, config.trainable_parameter_ids.as_slice());
    let parameter_ids = parameter_vectors
        .iter()
        .map(|parameter| parameter.parameter_id.clone())
        .collect::<Vec<_>>();
    let weights_bytes = export_checkpoint_weights(parameter_vectors.as_slice())?;
    let checkpoint_ref = format!("{}:step:{}", config.run_id, step);
    let manifest = crate::TassadarArticleTransformerCheckpointManifest {
        schema_version: 1,
        checkpoint_ref: checkpoint_ref.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        run_id: config.run_id.clone(),
        step,
        config: config.model_config.clone(),
        embedding_strategy: config.embedding_strategy,
        base_descriptor_digest: base_model.descriptor().stable_digest(),
        base_trainable_parameter_digest: base_model.trainable_parameter_digest(),
        current_trainable_parameter_digest: current_model.trainable_parameter_digest(),
        training_suite_digest: stable_digest(
            b"psionic_tassadar_article_transformer_weight_production_training_examples|",
            &suite.training_examples,
        ),
        held_out_suite_digest: stable_digest(
            b"psionic_tassadar_article_transformer_weight_production_held_out_examples|",
            &suite.held_out_examples,
        ),
        parameter_ids,
        parent_checkpoint_ref: parent.map(|checkpoint| checkpoint.manifest.checkpoint_ref.clone()),
        parent_manifest_digest: parent.map(|checkpoint| checkpoint.manifest.stable_digest()),
    };
    let checkpoint = psionic_runtime::TrainingCheckpointReference::new(
        config.checkpoint_family.clone(),
        format!(
            "tassadar.article_transformer.weight_production.{}.{}",
            config.run_id, step
        ),
        manifest.stable_digest(),
        stable_bytes_digest(
            b"psionic_tassadar_article_transformer_weight_production_checkpoint_weights|",
            &weights_bytes,
        ),
        "psionic.local.cpu_reference",
        0,
        "cluster.local.cpu_reference",
        "topology.cpu_reference",
        config.started_at_ms + step.saturating_mul(config.step_duration_ms),
    )
    .with_checkpoint_ref(checkpoint_ref)
    .with_step(step)
    .with_durable_at_ms(config.started_at_ms + step.saturating_mul(config.step_duration_ms));
    Ok(TassadarArticleTransformerCheckpointArtifact {
        manifest,
        weights_bytes,
        checkpoint,
    })
}

fn export_checkpoint_weights(
    parameters: &[TassadarArticleTransformerParameterVector],
) -> Result<Vec<u8>, TassadarArticleTransformerWeightProductionError> {
    let mut raw_buffers = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        raw_buffers.push((
            parameter.parameter_id.clone(),
            encode_f32_bytes(parameter.values.as_slice()),
            parameter.shape.clone(),
        ));
    }

    let mut views = Vec::with_capacity(raw_buffers.len());
    for (parameter_id, bytes, shape) in &raw_buffers {
        let view = safetensors::tensor::TensorView::new(
            safetensors::Dtype::F32,
            shape.clone(),
            bytes.as_slice(),
        )
        .map_err(|error| {
            TassadarArticleTransformerWeightProductionError::Serialization {
                context: "article transformer weight production checkpoint export",
                message: error.to_string(),
            }
        })?;
        views.push((parameter_id.clone(), view));
    }
    safetensors::serialize(
        views
            .iter()
            .map(|(parameter_id, view)| (parameter_id.as_str(), view.clone())),
        None,
    )
    .map_err(
        |error| TassadarArticleTransformerWeightProductionError::Serialization {
            context: "article transformer weight production checkpoint export",
            message: error.to_string(),
        },
    )
}

fn case_metrics(
    trained_model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    label_smoothing: f32,
) -> Result<
    Vec<TassadarArticleTransformerWeightProductionCaseMetric>,
    TassadarArticleTransformerWeightProductionError,
> {
    let base_model = TassadarArticleTransformer::article_trace_domain_reference()?;
    suite
        .training_examples
        .iter()
        .chain(suite.held_out_examples.iter())
        .map(|example| {
            let (initial_loss, initial_prediction) =
                example_loss_and_prediction(&base_model, suite, example, label_smoothing)?;
            let (final_loss, final_prediction) =
                example_loss_and_prediction(trained_model, suite, example, label_smoothing)?;
            let initial_exact_tokens = initial_prediction
                .iter()
                .zip(example.target_tokens.iter())
                .filter(|(left, right)| left == right)
                .count();
            let final_exact_tokens = final_prediction
                .iter()
                .zip(example.target_tokens.iter())
                .filter(|(left, right)| left == right)
                .count();
            Ok(TassadarArticleTransformerWeightProductionCaseMetric {
                case_id: example.case_id.clone(),
                split: example.split,
                initial_mean_loss: initial_loss,
                final_mean_loss: final_loss,
                initial_token_exactness_bps: ((initial_exact_tokens * 10_000)
                    / example.target_tokens.len())
                    as u32,
                final_token_exactness_bps: ((final_exact_tokens * 10_000)
                    / example.target_tokens.len())
                    as u32,
                initial_exact_target_match: initial_prediction == example.target_tokens,
                final_exact_target_match: final_prediction == example.target_tokens,
                improved: final_loss < initial_loss || final_exact_tokens > initial_exact_tokens,
                initial_prediction_digest: stable_digest(
                    b"psionic_tassadar_article_transformer_weight_production_initial_prediction|",
                    &initial_prediction,
                ),
                final_prediction_digest: stable_digest(
                    b"psionic_tassadar_article_transformer_weight_production_final_prediction|",
                    &final_prediction,
                ),
            })
        })
        .collect()
}

fn dataset_metrics(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    examples: &[TassadarArticleTransformerWeightProductionExample],
    label_smoothing: f32,
) -> Result<DatasetMetrics, TassadarArticleTransformerWeightProductionError> {
    let mut total_loss = 0.0f32;
    let mut exact_match_count = 0usize;
    let mut token_exact_count = 0usize;
    let mut token_count = 0usize;
    for example in examples {
        let (loss, predicted) =
            example_loss_and_prediction(model, suite, example, label_smoothing)?;
        total_loss += loss;
        token_exact_count += predicted
            .iter()
            .zip(example.target_tokens.iter())
            .filter(|(left, right)| left == right)
            .count();
        token_count += example.target_tokens.len();
        if predicted == example.target_tokens {
            exact_match_count += 1;
        }
    }
    Ok(DatasetMetrics {
        mean_loss: total_loss / examples.len() as f32,
        exact_match_count,
        token_exact_count,
        token_count,
    })
}

fn mean_dataset_loss(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    examples: &[TassadarArticleTransformerWeightProductionExample],
    label_smoothing: f32,
) -> Result<f32, TassadarArticleTransformerWeightProductionError> {
    let mut total_loss = 0.0f32;
    for example in examples {
        total_loss += example_loss_and_prediction(model, suite, example, label_smoothing)?.0;
    }
    Ok(total_loss / examples.len() as f32)
}

fn example_loss_and_prediction(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerWeightProductionSuite,
    example: &TassadarArticleTransformerWeightProductionExample,
    label_smoothing: f32,
) -> Result<(f32, Vec<usize>), TassadarArticleTransformerWeightProductionError> {
    let target_input = decoder_input_tokens(suite.bos_token_id, &example.target_tokens);
    let output = model.forward(
        example.source_tokens.len().into_batch_shape(),
        example.source_tokens.as_slice(),
        target_input.len().into_batch_shape(),
        target_input.as_slice(),
        TransformerExecutionMode::Eval,
    )?;
    let logits = dense_f32_values(
        &output.logits.data,
        "article_transformer_weight_production.logits",
    )?;
    let vocab_size = model.descriptor().config.target_vocab_size;
    let mut total_loss = 0.0f32;
    let mut predicted = Vec::with_capacity(example.target_tokens.len());
    for (position, &target_token) in example.target_tokens.iter().enumerate() {
        let start = position * vocab_size;
        let row = &logits[start..start + vocab_size];
        total_loss += label_smoothed_cross_entropy(row, target_token, label_smoothing);
        predicted.push(argmax(row));
    }
    Ok((total_loss / example.target_tokens.len() as f32, predicted))
}

fn decoder_input_tokens(bos_token_id: usize, target_tokens: &[usize]) -> Vec<usize> {
    let mut input = Vec::with_capacity(target_tokens.len());
    input.push(bos_token_id);
    input.extend(
        target_tokens
            .iter()
            .copied()
            .take(target_tokens.len().saturating_sub(1)),
    );
    input
}

fn label_smoothed_cross_entropy(logits: &[f32], target_token: usize, label_smoothing: f32) -> f32 {
    let log_probs = softmax_probabilities(logits)
        .into_iter()
        .map(|value| value.max(f32::EPSILON).ln())
        .collect::<Vec<_>>();
    let class_count = logits.len() as f32;
    let off_target = label_smoothing / class_count;
    let on_target = (1.0 - label_smoothing) + off_target;
    let mut loss = 0.0f32;
    for (index, log_prob) in log_probs.iter().enumerate() {
        let target = if index == target_token {
            on_target
        } else {
            off_target
        };
        loss -= target * log_prob;
    }
    loss
}

fn softmax_probabilities(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = logits
        .iter()
        .map(|value| (*value - max_logit).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(f32::EPSILON);
    exp.into_iter().map(|value| value / sum).collect()
}

fn argmax(values: &[f32]) -> usize {
    values
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn required_group<'a>(
    run: &'a FixedBudgetTrainingRun,
    group_id: &str,
) -> Result<&'a TrainingParameterGroupState, TassadarArticleTransformerWeightProductionError> {
    run.parameter_group(group_id).ok_or_else(|| {
        TassadarArticleTransformerWeightProductionError::MissingParameterGroup {
            group_id: String::from(group_id),
        }
    })
}

fn dense_values<'a>(
    group: &'a TrainingParameterGroupState,
    group_id: &str,
) -> Result<&'a [f32], TassadarArticleTransformerWeightProductionError> {
    match &group.parameter.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        TensorData::I32(_) => Err(
            TassadarArticleTransformerWeightProductionError::NonDenseGroup {
                group_id: String::from(group_id),
            },
        ),
        TensorData::QuantizedBlocks(_) => Err(
            TassadarArticleTransformerWeightProductionError::NonDenseGroup {
                group_id: String::from(group_id),
            },
        ),
    }
}

fn dense_f32_values<'a>(
    data: &'a TensorData,
    context: &'static str,
) -> Result<&'a [f32], TassadarArticleTransformerWeightProductionError> {
    match data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        TensorData::I32(_) => Err(
            TassadarArticleTransformerWeightProductionError::Serialization {
                context,
                message: String::from("expected dense f32 tensor data"),
            },
        ),
        TensorData::QuantizedBlocks(_) => Err(
            TassadarArticleTransformerWeightProductionError::Serialization {
                context,
                message: String::from("expected dense f32 tensor data"),
            },
        ),
    }
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn stable_bytes_digest(prefix: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn scheduler_kind_label(kind: crate::TrainingSchedulerKind) -> String {
    match kind {
        crate::TrainingSchedulerKind::Constant => String::from("constant"),
        crate::TrainingSchedulerKind::StepLr => String::from("step_lr"),
        crate::TrainingSchedulerKind::LinearWarmup => String::from("linear_warmup"),
        crate::TrainingSchedulerKind::InverseSquareRootWarmup => {
            String::from("inverse_square_root_warmup")
        }
        crate::TrainingSchedulerKind::CosineAnnealing => String::from("cosine_annealing"),
    }
}

fn resolve_output_path(relative_or_absolute: &str) -> PathBuf {
    let path = Path::new(relative_or_absolute);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root().join(path)
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-train should live under <repo>/crates/psionic-train")
        .to_path_buf()
}

trait IntoBatchShape {
    fn into_batch_shape(self) -> Shape;
}

impl IntoBatchShape for usize {
    fn into_batch_shape(self) -> Shape {
        Shape::new(vec![1, self])
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_weight_production_evidence_bundle,
        run_tassadar_article_transformer_weight_production,
        TassadarArticleTransformerWeightProductionConfig,
        TassadarArticleTransformerWeightProductionSuite,
    };

    #[test]
    fn reference_weight_production_suite_uses_expected_article_cases() {
        let suite = TassadarArticleTransformerWeightProductionSuite::reference().expect("suite");

        assert_eq!(suite.training_examples.len(), 1);
        assert_eq!(suite.held_out_examples.len(), 3);
        assert_eq!(suite.training_examples[0].case_id, "hungarian_matching");
        assert_eq!(suite.held_out_examples[0].case_id, "micro_wasm_kernel");
        assert_eq!(suite.held_out_examples[1].case_id, "branch_heavy_kernel");
        assert_eq!(suite.held_out_examples[2].case_id, "memory_heavy_kernel");
    }

    #[test]
    fn weight_production_run_emits_checkpoint_and_reload_parity() {
        let suite = TassadarArticleTransformerWeightProductionSuite::reference().expect("suite");
        let mut config =
            TassadarArticleTransformerWeightProductionConfig::reference().expect("config");
        let directory = tempfile::tempdir().expect("tempdir");
        config.base_descriptor_ref = directory
            .path()
            .join("article_transformer_base_descriptor.json")
            .display()
            .to_string();
        config.base_artifact_ref = directory
            .path()
            .join("article_transformer_base_weights.safetensors")
            .display()
            .to_string();
        config.produced_descriptor_ref = directory
            .path()
            .join("article_transformer_descriptor.json")
            .display()
            .to_string();
        config.produced_artifact_ref = directory
            .path()
            .join("article_transformer_weights.safetensors")
            .display()
            .to_string();

        let outcome =
            run_tassadar_article_transformer_weight_production(&suite, &config).expect("run");
        let bundle = build_tassadar_article_transformer_weight_production_evidence_bundle(
            &suite, &config, &outcome,
        );

        assert!(bundle.produced_artifact_differs_from_base);
        assert!(bundle.checkpoint.restore_matches_trained_state);
        assert!(bundle.artifact_reload_matches_trained_state);
        assert!(bundle.final_training_mean_loss <= bundle.initial_training_mean_loss);
    }
}
