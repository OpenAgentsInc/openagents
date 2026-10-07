use std::collections::BTreeMap;

use psionic_core::{DType, Device, Shape, TensorData, TensorSpec};
use psionic_eval::{
    TassadarArticleTransformerCheckpointEvidence, TassadarArticleTransformerGradientCheckEvidence,
    TassadarArticleTransformerToyTaskExample, TassadarArticleTransformerToyTaskKind,
    TassadarArticleTransformerTrainingEvidenceBundle,
    TassadarArticleTransformerTrainingStepEvidence,
};
use psionic_models::{
    TassadarArticleTransformer, TassadarArticleTransformerEmbeddingStrategy,
    TassadarArticleTransformerError, TassadarArticleTransformerParameterVector,
};
use psionic_runtime::TrainingCheckpointReference;
use psionic_transformer::{EncoderDecoderTransformerConfig, TransformerExecutionMode};
use safetensors::{serialize, tensor::TensorView, Dtype as SafeTensorsDType, SafeTensors};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, TrainingCoreError, TrainingGradientBatch, TrainingLoopBudget,
    TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy, TrainingParameterClass,
    TrainingParameterGroupState, TrainingSchedulerConfig, TrainingStepInput, TrainingStepReceipt,
    TrainingTensorBuffer,
};

const MODEL_MODULE_REF: &str = "crates/psionic-models/src/tassadar_article_transformer.rs";
const TRANSFORMER_MODULE_REF: &str = "crates/psionic-transformer/src/encoder_decoder.rs";
const TRAIN_MODULE_REF: &str = "crates/psionic-train/src/tassadar_article_transformer_training.rs";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerToyTaskSuite {
    pub suite_id: String,
    pub description: String,
    pub bos_token_id: usize,
    pub training_examples: Vec<TassadarArticleTransformerToyTaskExample>,
    pub held_out_examples: Vec<TassadarArticleTransformerToyTaskExample>,
}

impl TassadarArticleTransformerToyTaskSuite {
    #[must_use]
    pub fn reference() -> Self {
        Self {
            suite_id: String::from("tassadar.article_transformer.toy_tasks.v1"),
            description: String::from(
                "two bounded selector tasks over a shared vocabulary: select-first and select-second",
            ),
            bos_token_id: 0,
            training_examples: vec![
                TassadarArticleTransformerToyTaskExample {
                    example_id: String::from("select_first_ab"),
                    task_kind: TassadarArticleTransformerToyTaskKind::SelectFirst,
                    source_tokens: vec![6, 2, 3],
                    target_tokens: vec![2],
                },
                TassadarArticleTransformerToyTaskExample {
                    example_id: String::from("select_second_cd"),
                    task_kind: TassadarArticleTransformerToyTaskKind::SelectSecond,
                    source_tokens: vec![7, 4, 5],
                    target_tokens: vec![5],
                },
            ],
            held_out_examples: vec![
                TassadarArticleTransformerToyTaskExample {
                    example_id: String::from("select_first_cd"),
                    task_kind: TassadarArticleTransformerToyTaskKind::SelectFirst,
                    source_tokens: vec![6, 4, 5],
                    target_tokens: vec![4],
                },
                TassadarArticleTransformerToyTaskExample {
                    example_id: String::from("select_second_ab"),
                    task_kind: TassadarArticleTransformerToyTaskKind::SelectSecond,
                    source_tokens: vec![7, 2, 3],
                    target_tokens: vec![3],
                },
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTrainingConfig {
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
}

impl TassadarArticleTransformerTrainingConfig {
    pub fn reference() -> Result<Self, TassadarArticleTransformerTrainingError> {
        Ok(Self {
            run_id: String::from("tassadar-article-transformer-training-run"),
            checkpoint_family: String::from("train.tassadar.article_transformer"),
            started_at_ms: 1_774_320_000_000,
            step_duration_ms: 40,
            budget: TrainingLoopBudget::new(64, 8, 2)?,
            model_config: TassadarArticleTransformer::tiny_reference_config(),
            embedding_strategy: TassadarArticleTransformerEmbeddingStrategy::Unshared,
            optimizer: TrainingOptimizerConfig::adam(0.15, 0.9, 0.98, 1e-8)
                .with_gradient_clip_norm(1.0),
            scheduler: TrainingSchedulerConfig::inverse_square_root_warmup(8, 0.25),
            label_smoothing: 0.01,
            finite_difference_epsilon: 0.01,
        })
    }

    fn validate(&self) -> Result<(), TassadarArticleTransformerTrainingError> {
        if self.run_id.trim().is_empty() {
            return Err(TassadarArticleTransformerTrainingError::MissingRunId);
        }
        if self.checkpoint_family.trim().is_empty() {
            return Err(TassadarArticleTransformerTrainingError::MissingCheckpointFamily);
        }
        if self.step_duration_ms == 0 {
            return Err(TassadarArticleTransformerTrainingError::InvalidStepDuration);
        }
        if !self.label_smoothing.is_finite()
            || self.label_smoothing <= 0.0
            || self.label_smoothing >= 1.0
        {
            return Err(
                TassadarArticleTransformerTrainingError::InvalidLabelSmoothing {
                    value: self.label_smoothing,
                },
            );
        }
        if !self.finite_difference_epsilon.is_finite() || self.finite_difference_epsilon <= 0.0 {
            return Err(
                TassadarArticleTransformerTrainingError::InvalidFiniteDifferenceEpsilon {
                    epsilon: self.finite_difference_epsilon,
                },
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerCheckpointManifest {
    pub schema_version: u16,
    pub checkpoint_ref: String,
    pub checkpoint_family: String,
    pub run_id: String,
    pub step: u64,
    pub config: EncoderDecoderTransformerConfig,
    pub embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
    pub base_descriptor_digest: String,
    pub base_trainable_parameter_digest: String,
    pub current_trainable_parameter_digest: String,
    pub training_suite_digest: String,
    pub held_out_suite_digest: String,
    pub parameter_ids: Vec<String>,
    pub parent_checkpoint_ref: Option<String>,
    pub parent_manifest_digest: Option<String>,
}

impl TassadarArticleTransformerCheckpointManifest {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(
            b"psionic_tassadar_article_transformer_checkpoint_manifest|",
            self,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerCheckpointArtifact {
    pub manifest: TassadarArticleTransformerCheckpointManifest,
    pub weights_bytes: Vec<u8>,
    pub checkpoint: TrainingCheckpointReference,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTrainingOutcome {
    pub initial_model: TassadarArticleTransformer,
    pub trained_model: TassadarArticleTransformer,
    pub restored_model: TassadarArticleTransformer,
    pub gradient_checks: Vec<TassadarArticleTransformerGradientCheckEvidence>,
    pub step_receipts: Vec<TrainingStepReceipt>,
    pub step_evidence: Vec<TassadarArticleTransformerTrainingStepEvidence>,
    pub initial_training_mean_loss: f32,
    pub final_training_mean_loss: f32,
    pub initial_training_exact_match_count: usize,
    pub final_training_exact_match_count: usize,
    pub final_held_out_mean_loss: f32,
    pub final_held_out_exact_match_count: usize,
    pub overfit_training_exact_match: bool,
    pub initial_checkpoint: TassadarArticleTransformerCheckpointArtifact,
    pub final_checkpoint: TassadarArticleTransformerCheckpointArtifact,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerTrainingError {
    #[error("article Transformer training requires a non-empty run id")]
    MissingRunId,
    #[error("article Transformer training requires a non-empty checkpoint family")]
    MissingCheckpointFamily,
    #[error("article Transformer training requires a non-zero step duration")]
    InvalidStepDuration,
    #[error(
        "article Transformer training requires label_smoothing in the open interval (0, 1), got {value}"
    )]
    InvalidLabelSmoothing { value: f32 },
    #[error(
        "article Transformer training requires a positive finite-difference epsilon, got {epsilon}"
    )]
    InvalidFiniteDifferenceEpsilon { epsilon: f32 },
    #[error("article Transformer toy suite must contain at least one training example")]
    EmptyTrainingExamples,
    #[error("article Transformer toy suite must contain at least one held-out example")]
    EmptyHeldOutExamples,
    #[error("article Transformer toy example `{example_id}` has an empty source sequence")]
    EmptySourceSequence { example_id: String },
    #[error("article Transformer toy example `{example_id}` has an empty target sequence")]
    EmptyTargetSequence { example_id: String },
    #[error(
        "article Transformer token {token_id} in `{example_id}` exceeds vocabulary size {vocab_size}"
    )]
    TokenOutOfRange {
        example_id: String,
        token_id: usize,
        vocab_size: usize,
    },
    #[error("article Transformer training run is missing parameter group `{group_id}`")]
    MissingParameterGroup { group_id: String },
    #[error("article Transformer training group `{group_id}` is not dense f32")]
    NonDenseGroup { group_id: String },
    #[error("{context}: {message}")]
    Serialization {
        context: &'static str,
        message: String,
    },
    #[error(transparent)]
    TrainCore(#[from] TrainingCoreError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
}

pub fn train_tassadar_article_transformer_toy_suite(
    suite: &TassadarArticleTransformerToyTaskSuite,
    config: &TassadarArticleTransformerTrainingConfig,
) -> Result<TassadarArticleTransformerTrainingOutcome, TassadarArticleTransformerTrainingError> {
    config.validate()?;
    validate_suite(suite, &config.model_config)?;

    let initial_model = TassadarArticleTransformer::paper_faithful_reference(
        config.model_config.clone(),
        config.embedding_strategy,
    )?;
    let trainable_vectors = initial_model.trainable_parameter_vectors();
    let gradient_checks = build_gradient_checks(
        &initial_model,
        suite,
        config.label_smoothing,
        config.finite_difference_epsilon,
    )?;
    let initial_metrics = dataset_metrics(
        &initial_model,
        suite,
        &suite.training_examples,
        config.label_smoothing,
    )?;
    let mut run = FixedBudgetTrainingRun::new(
        config.run_id.clone(),
        config.checkpoint_family.clone(),
        config.budget,
        build_training_groups(trainable_vectors.as_slice(), config)?,
    )?;
    let initial_checkpoint =
        export_checkpoint(&initial_model, &initial_model, suite, config, 0, None)?;

    let mut current_model = initial_model.clone();
    let mut step_receipts = Vec::new();
    let mut step_evidence = Vec::new();
    for step_index in 0..config.budget.max_steps {
        let batch = build_gradient_batch(
            &initial_model,
            &current_model,
            suite,
            config.label_smoothing,
            config.finite_difference_epsilon,
        )?;
        let started_at_ms = config
            .started_at_ms
            .saturating_add(step_index.saturating_mul(config.step_duration_ms));
        let finished_at_ms = started_at_ms.saturating_add(config.step_duration_ms);
        let receipt =
            run.apply_step(TrainingStepInput::new(batch, started_at_ms, finished_at_ms))?;
        current_model = materialize_model(&initial_model, &run)?;
        let train_metrics = dataset_metrics(
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
        step_evidence.push(TassadarArticleTransformerTrainingStepEvidence {
            global_step: receipt.schedule.global_step,
            batch_id: receipt.batch_id.clone(),
            training_mean_loss: train_metrics.mean_loss,
            training_exact_match_count: train_metrics.exact_match_count,
            held_out_mean_loss: held_out_metrics.mean_loss,
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
        &initial_model,
        &current_model,
        suite,
        config,
        config.budget.max_steps,
        Some(&initial_checkpoint),
    )?;
    let restored_model = restore_tassadar_article_transformer_checkpoint(
        &final_checkpoint.manifest,
        &final_checkpoint.weights_bytes,
    )?;
    let final_train_metrics = dataset_metrics(
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

    Ok(TassadarArticleTransformerTrainingOutcome {
        initial_model,
        trained_model: current_model,
        restored_model,
        gradient_checks,
        step_receipts,
        step_evidence,
        initial_training_mean_loss: initial_metrics.mean_loss,
        final_training_mean_loss: final_train_metrics.mean_loss,
        initial_training_exact_match_count: initial_metrics.exact_match_count,
        final_training_exact_match_count: final_train_metrics.exact_match_count,
        final_held_out_mean_loss: final_held_out_metrics.mean_loss,
        final_held_out_exact_match_count: final_held_out_metrics.exact_match_count,
        overfit_training_exact_match: final_train_metrics.exact_match_count
            == suite.training_examples.len(),
        initial_checkpoint,
        final_checkpoint,
    })
}

pub fn build_tassadar_article_transformer_training_evidence_bundle(
    suite: &TassadarArticleTransformerToyTaskSuite,
    config: &TassadarArticleTransformerTrainingConfig,
    outcome: &TassadarArticleTransformerTrainingOutcome,
) -> TassadarArticleTransformerTrainingEvidenceBundle {
    let final_checkpoint = &outcome.final_checkpoint;
    TassadarArticleTransformerTrainingEvidenceBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.article_transformer.training_evidence_bundle.v1"),
        tied_requirement_id: String::from("TAS-164"),
        model_module_ref: String::from(MODEL_MODULE_REF),
        transformer_module_ref: String::from(TRANSFORMER_MODULE_REF),
        train_module_ref: String::from(TRAIN_MODULE_REF),
        run_id: config.run_id.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        architecture_variant: outcome.trained_model.descriptor().architecture_variant,
        config: config.model_config.clone(),
        embedding_strategy: config.embedding_strategy,
        trainable_parameter_ids: outcome
            .trained_model
            .trainable_parameter_vectors()
            .into_iter()
            .map(|parameter| parameter.parameter_id)
            .collect(),
        trainable_parameter_scalar_count: outcome
            .trained_model
            .trainable_parameter_vectors()
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
        training_examples: suite.training_examples.clone(),
        held_out_examples: suite.held_out_examples.clone(),
        gradient_checks: outcome.gradient_checks.clone(),
        step_evidence: outcome.step_evidence.clone(),
        initial_training_mean_loss: outcome.initial_training_mean_loss,
        final_training_mean_loss: outcome.final_training_mean_loss,
        initial_training_exact_match_count: outcome.initial_training_exact_match_count,
        final_training_exact_match_count: outcome.final_training_exact_match_count,
        final_held_out_mean_loss: outcome.final_held_out_mean_loss,
        final_held_out_exact_match_count: outcome.final_held_out_exact_match_count,
        overfit_training_exact_match: outcome.overfit_training_exact_match,
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
            trained_trainable_parameter_digest: outcome
                .trained_model
                .trainable_parameter_digest(),
            restored_trainable_parameter_digest: outcome
                .restored_model
                .trainable_parameter_digest(),
            restore_matches_trained_state: outcome.trained_model.trainable_parameter_digest()
                == outcome.restored_model.trainable_parameter_digest(),
        },
        claim_boundary: String::from(
            "this evidence bundle covers one bounded article-Transformer training lane over two toy sequence tasks, a finite-difference optimizer loop, and deterministic checkpoint restore on the canonical owned stack. It does not claim full-parameter article training, benchmark parity, or final article-equivalence green status.",
        ),
        bundle_digest: String::new(),
    }
    .with_bundle_digest()
}

pub fn restore_tassadar_article_transformer_checkpoint(
    manifest: &TassadarArticleTransformerCheckpointManifest,
    weights_bytes: &[u8],
) -> Result<TassadarArticleTransformer, TassadarArticleTransformerTrainingError> {
    let base_model = TassadarArticleTransformer::paper_faithful_reference(
        manifest.config.clone(),
        manifest.embedding_strategy,
    )?;
    restore_tassadar_article_transformer_checkpoint_with_base_model(
        &base_model,
        manifest,
        weights_bytes,
    )
}

pub fn restore_tassadar_article_transformer_checkpoint_with_base_model(
    base_model: &TassadarArticleTransformer,
    manifest: &TassadarArticleTransformerCheckpointManifest,
    weights_bytes: &[u8],
) -> Result<TassadarArticleTransformer, TassadarArticleTransformerTrainingError> {
    let safetensors = SafeTensors::deserialize(weights_bytes).map_err(|error| {
        TassadarArticleTransformerTrainingError::Serialization {
            context: "article transformer checkpoint restore",
            message: error.to_string(),
        }
    })?;
    let mut overrides = BTreeMap::new();
    for parameter_id in &manifest.parameter_ids {
        let tensor = safetensors.tensor(parameter_id).map_err(|error| {
            TassadarArticleTransformerTrainingError::Serialization {
                context: "article transformer checkpoint restore",
                message: error.to_string(),
            }
        })?;
        overrides.insert(
            parameter_id.clone(),
            decode_f32_bytes(parameter_id.as_str(), tensor.data())?,
        );
    }
    Ok(base_model.with_parameter_overrides(&overrides)?)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DatasetMetrics {
    mean_loss: f32,
    exact_match_count: usize,
}

fn validate_suite(
    suite: &TassadarArticleTransformerToyTaskSuite,
    config: &EncoderDecoderTransformerConfig,
) -> Result<(), TassadarArticleTransformerTrainingError> {
    if suite.training_examples.is_empty() {
        return Err(TassadarArticleTransformerTrainingError::EmptyTrainingExamples);
    }
    if suite.held_out_examples.is_empty() {
        return Err(TassadarArticleTransformerTrainingError::EmptyHeldOutExamples);
    }
    for example in suite
        .training_examples
        .iter()
        .chain(suite.held_out_examples.iter())
    {
        if example.source_tokens.is_empty() {
            return Err(
                TassadarArticleTransformerTrainingError::EmptySourceSequence {
                    example_id: example.example_id.clone(),
                },
            );
        }
        if example.target_tokens.is_empty() {
            return Err(
                TassadarArticleTransformerTrainingError::EmptyTargetSequence {
                    example_id: example.example_id.clone(),
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
                return Err(TassadarArticleTransformerTrainingError::TokenOutOfRange {
                    example_id: example.example_id.clone(),
                    token_id,
                    vocab_size: config.source_vocab_size.min(config.target_vocab_size),
                });
            }
        }
    }
    Ok(())
}

fn build_training_groups(
    parameters: &[TassadarArticleTransformerParameterVector],
    config: &TassadarArticleTransformerTrainingConfig,
) -> Result<Vec<TrainingParameterGroupState>, TassadarArticleTransformerTrainingError> {
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
    suite: &TassadarArticleTransformerToyTaskSuite,
    label_smoothing: f32,
    epsilon: f32,
) -> Result<
    Vec<TassadarArticleTransformerGradientCheckEvidence>,
    TassadarArticleTransformerTrainingError,
> {
    let gradients = finite_difference_gradients(model, suite, label_smoothing, epsilon)?;
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
                    "finite-difference gradients were computed directly on the bounded trainable article-Transformer surface",
                ),
            }
        })
        .collect())
}

fn build_gradient_batch(
    initial_model: &TassadarArticleTransformer,
    current_model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerToyTaskSuite,
    label_smoothing: f32,
    epsilon: f32,
) -> Result<TrainingGradientBatch, TassadarArticleTransformerTrainingError> {
    let batch_loss = mean_dataset_loss(
        current_model,
        suite,
        &suite.training_examples,
        label_smoothing,
    )?;
    let gradients = finite_difference_gradients(current_model, suite, label_smoothing, epsilon)?;
    let mut batch_gradients = BTreeMap::new();
    for parameter in initial_model.trainable_parameter_vectors() {
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

fn finite_difference_gradients(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerToyTaskSuite,
    label_smoothing: f32,
    epsilon: f32,
) -> Result<BTreeMap<String, Vec<f32>>, TassadarArticleTransformerTrainingError> {
    let mut gradients = BTreeMap::new();
    let base_parameters = model.trainable_parameter_vectors();
    for parameter in &base_parameters {
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

fn dataset_metrics(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerToyTaskSuite,
    examples: &[TassadarArticleTransformerToyTaskExample],
    label_smoothing: f32,
) -> Result<DatasetMetrics, TassadarArticleTransformerTrainingError> {
    let mut total_loss = 0.0f32;
    let mut exact_match_count = 0usize;
    for example in examples {
        let (loss, predicted) =
            example_loss_and_prediction(model, suite, example, label_smoothing)?;
        total_loss += loss;
        if predicted == example.target_tokens {
            exact_match_count += 1;
        }
    }
    Ok(DatasetMetrics {
        mean_loss: total_loss / examples.len() as f32,
        exact_match_count,
    })
}

fn mean_dataset_loss(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerToyTaskSuite,
    examples: &[TassadarArticleTransformerToyTaskExample],
    label_smoothing: f32,
) -> Result<f32, TassadarArticleTransformerTrainingError> {
    let mut total_loss = 0.0f32;
    for example in examples {
        total_loss += example_loss_and_prediction(model, suite, example, label_smoothing)?.0;
    }
    Ok(total_loss / examples.len() as f32)
}

fn example_loss_and_prediction(
    model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerToyTaskSuite,
    example: &TassadarArticleTransformerToyTaskExample,
    label_smoothing: f32,
) -> Result<(f32, Vec<usize>), TassadarArticleTransformerTrainingError> {
    let target_input = decoder_input_tokens(suite.bos_token_id, &example.target_tokens);
    let output = model.forward(
        example.source_tokens.len().into_batch_shape(),
        example.source_tokens.as_slice(),
        target_input.len().into_batch_shape(),
        target_input.as_slice(),
        TransformerExecutionMode::Eval,
    )?;
    let logits = dense_f32_values(&output.logits.data, "article_transformer.logits")?;
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
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = logits
        .iter()
        .map(|value| (*value - max_logit).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(f32::EPSILON);
    let log_probs = exp
        .iter()
        .map(|value| (value / sum).max(f32::EPSILON).ln())
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

fn argmax(values: &[f32]) -> usize {
    values
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn materialize_model(
    initial_model: &TassadarArticleTransformer,
    run: &FixedBudgetTrainingRun,
) -> Result<TassadarArticleTransformer, TassadarArticleTransformerTrainingError> {
    let mut overrides = BTreeMap::new();
    for parameter in initial_model.trainable_parameter_vectors() {
        let group = required_group(run, parameter.parameter_id.as_str())?;
        overrides.insert(
            parameter.parameter_id.clone(),
            dense_values(group, parameter.parameter_id.as_str())?.to_vec(),
        );
    }
    initial_model
        .with_parameter_overrides(&overrides)
        .map_err(Into::into)
}

fn export_checkpoint(
    initial_model: &TassadarArticleTransformer,
    current_model: &TassadarArticleTransformer,
    suite: &TassadarArticleTransformerToyTaskSuite,
    config: &TassadarArticleTransformerTrainingConfig,
    step: u64,
    parent: Option<&TassadarArticleTransformerCheckpointArtifact>,
) -> Result<TassadarArticleTransformerCheckpointArtifact, TassadarArticleTransformerTrainingError> {
    let parameter_vectors = current_model.trainable_parameter_vectors();
    let parameter_ids = parameter_vectors
        .iter()
        .map(|parameter| parameter.parameter_id.clone())
        .collect::<Vec<_>>();
    let weights_bytes = export_checkpoint_weights(parameter_vectors.as_slice())?;
    let checkpoint_ref = format!("{}:step:{}", config.run_id, step);
    let manifest = TassadarArticleTransformerCheckpointManifest {
        schema_version: 1,
        checkpoint_ref: checkpoint_ref.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        run_id: config.run_id.clone(),
        step,
        config: config.model_config.clone(),
        embedding_strategy: config.embedding_strategy,
        base_descriptor_digest: initial_model.descriptor().stable_digest(),
        base_trainable_parameter_digest: initial_model.trainable_parameter_digest(),
        current_trainable_parameter_digest: current_model.trainable_parameter_digest(),
        training_suite_digest: stable_digest(
            b"psionic_tassadar_article_transformer_training_examples|",
            &suite.training_examples,
        ),
        held_out_suite_digest: stable_digest(
            b"psionic_tassadar_article_transformer_held_out_examples|",
            &suite.held_out_examples,
        ),
        parameter_ids,
        parent_checkpoint_ref: parent.map(|checkpoint| checkpoint.manifest.checkpoint_ref.clone()),
        parent_manifest_digest: parent.map(|checkpoint| checkpoint.manifest.stable_digest()),
    };
    let checkpoint = TrainingCheckpointReference::new(
        config.checkpoint_family.clone(),
        format!(
            "tassadar.article_transformer.checkpoint.{}.{}",
            config.run_id, step
        ),
        manifest.stable_digest(),
        stable_bytes_digest(
            b"psionic_tassadar_article_transformer_checkpoint_weights|",
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
) -> Result<Vec<u8>, TassadarArticleTransformerTrainingError> {
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
        let view = TensorView::new(SafeTensorsDType::F32, shape.clone(), bytes.as_slice())
            .map_err(
                |error| TassadarArticleTransformerTrainingError::Serialization {
                    context: "article transformer checkpoint export",
                    message: error.to_string(),
                },
            )?;
        views.push((parameter_id.clone(), view));
    }
    serialize(
        views
            .iter()
            .map(|(parameter_id, view)| (parameter_id.as_str(), view.clone())),
        None,
    )
    .map_err(
        |error| TassadarArticleTransformerTrainingError::Serialization {
            context: "article transformer checkpoint export",
            message: error.to_string(),
        },
    )
}

fn required_group<'a>(
    run: &'a FixedBudgetTrainingRun,
    group_id: &str,
) -> Result<&'a TrainingParameterGroupState, TassadarArticleTransformerTrainingError> {
    run.parameter_group(group_id).ok_or_else(|| {
        TassadarArticleTransformerTrainingError::MissingParameterGroup {
            group_id: String::from(group_id),
        }
    })
}

fn dense_values<'a>(
    group: &'a TrainingParameterGroupState,
    group_id: &str,
) -> Result<&'a [f32], TassadarArticleTransformerTrainingError> {
    match &group.parameter.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        TensorData::I32(_) => Err(TassadarArticleTransformerTrainingError::NonDenseGroup {
            group_id: String::from(group_id),
        }),
        TensorData::QuantizedBlocks(_) => {
            Err(TassadarArticleTransformerTrainingError::NonDenseGroup {
                group_id: String::from(group_id),
            })
        }
    }
}

fn dense_f32_values<'a>(
    data: &'a TensorData,
    context: &'static str,
) -> Result<&'a [f32], TassadarArticleTransformerTrainingError> {
    match data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        TensorData::I32(_) => Err(TassadarArticleTransformerTrainingError::Serialization {
            context,
            message: String::from("expected dense f32 tensor data"),
        }),
        TensorData::QuantizedBlocks(_) => {
            Err(TassadarArticleTransformerTrainingError::Serialization {
                context,
                message: String::from("expected dense f32 tensor data"),
            })
        }
    }
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_f32_bytes(
    parameter_id: &str,
    bytes: &[u8],
) -> Result<Vec<f32>, TassadarArticleTransformerTrainingError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(TassadarArticleTransformerTrainingError::Serialization {
            context: "article transformer checkpoint restore",
            message: format!(
                "parameter `{parameter_id}` byte length {} is not divisible by 4",
                bytes.len()
            ),
        });
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
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
        build_tassadar_article_transformer_training_evidence_bundle,
        restore_tassadar_article_transformer_checkpoint,
        train_tassadar_article_transformer_toy_suite, TassadarArticleTransformerToyTaskSuite,
        TassadarArticleTransformerTrainingConfig,
    };
    use psionic_eval::{
        read_tassadar_article_transformer_training_evidence_bundle,
        write_tassadar_article_transformer_training_evidence_bundle,
        TassadarArticleTransformerTrainingEvidenceBundle,
        TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
    };

    #[test]
    fn reference_article_transformer_training_bundle_overfits_and_restores() {
        let suite = TassadarArticleTransformerToyTaskSuite::reference();
        let config = TassadarArticleTransformerTrainingConfig::reference().expect("config");
        let outcome =
            train_tassadar_article_transformer_toy_suite(&suite, &config).expect("outcome");

        assert!(outcome.final_training_mean_loss < outcome.initial_training_mean_loss);
        assert!(outcome.overfit_training_exact_match);
        assert_eq!(
            outcome.trained_model.trainable_parameter_digest(),
            outcome.restored_model.trainable_parameter_digest()
        );

        let restored = restore_tassadar_article_transformer_checkpoint(
            &outcome.final_checkpoint.manifest,
            &outcome.final_checkpoint.weights_bytes,
        )
        .expect("restore");
        assert_eq!(
            restored.trainable_parameter_digest(),
            outcome.trained_model.trainable_parameter_digest()
        );
    }

    #[test]
    fn article_transformer_training_evidence_bundle_tracks_recipe_and_restore() {
        let suite = TassadarArticleTransformerToyTaskSuite::reference();
        let config = TassadarArticleTransformerTrainingConfig::reference().expect("config");
        let outcome =
            train_tassadar_article_transformer_toy_suite(&suite, &config).expect("outcome");
        let bundle =
            build_tassadar_article_transformer_training_evidence_bundle(&suite, &config, &outcome);

        assert_eq!(bundle.tied_requirement_id, "TAS-164");
        assert_eq!(bundle.loss_kind, "label_smoothed_cross_entropy");
        assert_eq!(bundle.optimizer_kind, "adam");
        assert_eq!(bundle.scheduler_kind, "inverse_square_root_warmup");
        assert!(bundle.label_smoothing > 0.0);
        assert!(bundle.overfit_training_exact_match);
        assert!(bundle.checkpoint.restore_matches_trained_state);
        assert!(bundle.gradient_checks.iter().all(|row| row.all_finite));
        assert_eq!(bundle.training_examples.len(), 2);
        assert_eq!(bundle.held_out_examples.len(), 2);
    }

    #[test]
    fn article_transformer_training_evidence_bundle_matches_committed_truth() {
        let suite = TassadarArticleTransformerToyTaskSuite::reference();
        let config = TassadarArticleTransformerTrainingConfig::reference().expect("config");
        let outcome =
            train_tassadar_article_transformer_toy_suite(&suite, &config).expect("outcome");
        let generated =
            build_tassadar_article_transformer_training_evidence_bundle(&suite, &config, &outcome);
        let committed: TassadarArticleTransformerTrainingEvidenceBundle =
            read_tassadar_article_transformer_training_evidence_bundle(
                TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
            )
            .expect("committed evidence bundle");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_training_evidence_bundle_persists_current_truth() {
        let suite = TassadarArticleTransformerToyTaskSuite::reference();
        let config = TassadarArticleTransformerTrainingConfig::reference().expect("config");
        let outcome =
            train_tassadar_article_transformer_toy_suite(&suite, &config).expect("outcome");
        let bundle =
            build_tassadar_article_transformer_training_evidence_bundle(&suite, &config, &outcome);
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("article_transformer_training_evidence_bundle.json");
        let written =
            write_tassadar_article_transformer_training_evidence_bundle(&output_path, &bundle)
                .expect("write bundle");
        let persisted: TassadarArticleTransformerTrainingEvidenceBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
