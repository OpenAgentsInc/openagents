use std::collections::{BTreeMap, HashMap};

use psionic_core::{DType, Device, TensorData, TensorSpec};
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifestRef, DatastreamSubjectKind,
};
use psionic_eval::{
    evaluate_attnres_training_shift, AttnResTrainingEvalError, AttnResTrainingEvalReport,
};
use psionic_models::{
    AttnResConfig, AttnResCpuReferenceModel, AttnResDiagnosticsSnapshot, AttnResExecutionError,
    AttnResModelError, AttnResNextTokenSample, AttnResParameterVector, TokenSequence,
};
use psionic_runtime::TrainingCheckpointReference;
use safetensors::{serialize, tensor::TensorView, Dtype as SafeTensorsDType, SafeTensors};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, TrainingCoreError, TrainingGradientBatch, TrainingLoopBudget,
    TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy, TrainingParameterClass,
    TrainingParameterGroupState, TrainingRunSummary, TrainingStepInput, TrainingStepReceipt,
    TrainingTensorBuffer,
};

const ATTNRES_CHECKPOINT_MANIFEST_KEY: &str = "psionic.attnres.checkpoint_manifest";

/// Repo-owned tiny corpus contract for the bounded AttnRes training lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingCorpus {
    /// Human-readable corpus description.
    pub description: String,
    /// Bound AttnRes model config.
    pub config: AttnResConfig,
    /// Training split.
    pub training_samples: Vec<AttnResNextTokenSample>,
    /// Held-out split.
    pub held_out_samples: Vec<AttnResNextTokenSample>,
}

impl AttnResTinyTrainingCorpus {
    /// Returns the canonical fixture-backed tiny-training corpus.
    pub fn reference() -> Result<Self, AttnResTinyTrainingError> {
        let corpus = serde_json::from_str(include_str!(
            "../../../fixtures/attnres/tiny_training_cases.json"
        ))
        .map_err(|error| AttnResTinyTrainingError::Serialization {
            context: "attnres reference corpus load",
            message: error.to_string(),
        })?;
        validate_corpus(&corpus)?;
        Ok(corpus)
    }

    /// Returns a stable digest over the training split.
    #[must_use]
    pub fn training_digest(&self) -> String {
        stable_digest(b"psionic_attnres_training_split|", &self.training_samples)
    }

    /// Returns a stable digest over the held-out split.
    #[must_use]
    pub fn held_out_digest(&self) -> String {
        stable_digest(b"psionic_attnres_held_out_split|", &self.held_out_samples)
    }
}

/// Public alias for the full local AttnRes reference corpus contract.
pub type AttnResLocalReferenceTrainingCorpus = AttnResTinyTrainingCorpus;

/// Returns the canonical local-reference AttnRes corpus.
pub fn attnres_local_reference_training_corpus(
) -> Result<AttnResLocalReferenceTrainingCorpus, AttnResTinyTrainingError> {
    AttnResTinyTrainingCorpus::reference()
}

/// Configuration for the bounded AttnRes tiny-training lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingConfig {
    /// Stable model identifier for the seeded reference model.
    pub model_id: String,
    /// Stable model revision for the seeded reference model.
    pub model_revision: String,
    /// Stable training run identifier.
    pub run_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Logical training start time.
    pub started_at_ms: u64,
    /// Per-step logical duration.
    pub step_duration_ms: u64,
    /// Fixed-budget schedule.
    pub budget: TrainingLoopBudget,
    /// Optimizer used for routing pseudo-query groups.
    pub routing_optimizer: TrainingOptimizerConfig,
    /// Optimizer used for LM-head groups.
    pub head_optimizer: TrainingOptimizerConfig,
    /// Finite-difference epsilon for routing gradients.
    pub finite_difference_epsilon: f32,
}

impl AttnResTinyTrainingConfig {
    /// Returns the bounded reference config used by repo-owned tests and fixtures.
    pub fn reference() -> Result<Self, AttnResTinyTrainingError> {
        Ok(Self {
            model_id: String::from("attnres-tiny-train"),
            model_revision: String::from("v0"),
            run_id: String::from("attnres-tiny-training-run"),
            checkpoint_family: String::from("train.attnres.tiny"),
            started_at_ms: 1_761_000_000_000,
            step_duration_ms: 25,
            budget: TrainingLoopBudget::new(6, 1, 1)?,
            routing_optimizer: TrainingOptimizerConfig::adam(0.05, 0.9, 0.99, 1e-8)
                .with_gradient_clip_norm(1.0),
            head_optimizer: TrainingOptimizerConfig::adamw(0.08, 0.9, 0.99, 1e-8)
                .with_weight_decay(0.01)
                .with_gradient_clip_norm(1.0),
            finite_difference_epsilon: 0.01,
        })
    }

    fn validate(&self) -> Result<(), AttnResTinyTrainingError> {
        if self.model_id.trim().is_empty() {
            return Err(AttnResTinyTrainingError::MissingModelId);
        }
        if self.model_revision.trim().is_empty() {
            return Err(AttnResTinyTrainingError::MissingModelRevision);
        }
        if self.run_id.trim().is_empty() {
            return Err(AttnResTinyTrainingError::MissingRunId);
        }
        if self.checkpoint_family.trim().is_empty() {
            return Err(AttnResTinyTrainingError::MissingCheckpointFamily);
        }
        if self.step_duration_ms == 0 {
            return Err(AttnResTinyTrainingError::InvalidStepDuration);
        }
        if !self.finite_difference_epsilon.is_finite() || self.finite_difference_epsilon <= 0.0 {
            return Err(AttnResTinyTrainingError::InvalidFiniteDifferenceEpsilon {
                epsilon: self.finite_difference_epsilon,
            });
        }
        Ok(())
    }
}

/// Public alias for the full local AttnRes reference config contract.
pub type AttnResLocalReferenceTrainingConfig = AttnResTinyTrainingConfig;

/// Returns the canonical local-reference config used for the full AttnRes demo run.
pub fn attnres_local_reference_training_config(
) -> Result<AttnResLocalReferenceTrainingConfig, AttnResTinyTrainingError> {
    Ok(AttnResTinyTrainingConfig {
        model_id: String::from("attnres-local-reference"),
        model_revision: String::from("v1"),
        run_id: String::from("attnres-local-reference-run"),
        checkpoint_family: String::from("train.attnres.local_reference"),
        started_at_ms: 1_761_000_000_000,
        step_duration_ms: 85,
        budget: TrainingLoopBudget::new(320, 1, 1)?,
        routing_optimizer: TrainingOptimizerConfig::adam(0.005, 0.9, 0.99, 1e-8)
            .with_gradient_clip_norm(1.0),
        head_optimizer: TrainingOptimizerConfig::adamw(0.008, 0.9, 0.99, 1e-8)
            .with_weight_decay(0.01)
            .with_gradient_clip_norm(1.0),
        finite_difference_epsilon: 0.01,
    })
}

/// One machine-readable checkpoint artifact for the AttnRes tiny-training lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingArtifact {
    /// Stable artifact kind.
    pub artifact_kind: String,
    /// Stable artifact reference.
    pub artifact_ref: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Serialized artifact bytes.
    pub bytes: Vec<u8>,
}

impl AttnResTinyTrainingArtifact {
    fn new(
        artifact_kind: impl Into<String>,
        artifact_ref: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        let artifact_kind = artifact_kind.into();
        let artifact_ref = artifact_ref.into();
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_attnres_training_artifact|");
        hasher.update(artifact_kind.as_bytes());
        hasher.update(b"|");
        hasher.update(artifact_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(&bytes);
        Self {
            artifact_kind,
            artifact_ref,
            artifact_digest: hex::encode(hasher.finalize()),
            bytes,
        }
    }
}

/// JSON manifest paired with one safetensors checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingCheckpointManifest {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable checkpoint reference.
    pub checkpoint_ref: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Logical checkpoint step.
    pub step: u64,
    /// Stable model identifier.
    pub model_id: String,
    /// Stable model revision.
    pub model_revision: String,
    /// Bound AttnRes config.
    pub config: AttnResConfig,
    /// Stable base descriptor digest.
    pub base_descriptor_digest: String,
    /// Stable base weight digest.
    pub base_weight_digest: String,
    /// Stable digest over the serialized checkpoint payload.
    pub parameter_state_digest: String,
    /// Stable training-split digest.
    pub training_dataset_digest: String,
    /// Stable held-out-split digest.
    pub held_out_dataset_digest: String,
    /// Parameter ids included in the checkpoint.
    pub parameter_ids: Vec<String>,
    /// Optional parent checkpoint reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_checkpoint_ref: Option<String>,
    /// Optional parent manifest digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_manifest_digest: Option<String>,
    /// Optional final receipt identifier for the step that produced this checkpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_receipt_id: Option<String>,
}

impl AttnResTinyTrainingCheckpointManifest {
    /// Returns a stable digest over the manifest payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_attnres_checkpoint_manifest|", self)
    }
}

/// One persisted checkpoint plus explicit lineage refs for the AttnRes tiny-training lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingCheckpointArtifact {
    /// Safetensors checkpoint artifact.
    pub weights_artifact: AttnResTinyTrainingArtifact,
    /// JSON manifest artifact.
    pub manifest_artifact: AttnResTinyTrainingArtifact,
    /// Structured manifest.
    pub manifest: AttnResTinyTrainingCheckpointManifest,
    /// Runtime-owned checkpoint reference.
    pub checkpoint: TrainingCheckpointReference,
    /// Datastream-style manifest ref for the checkpoint bytes.
    pub manifest_ref: DatastreamManifestRef,
}

/// One per-step summary above the fixed-budget core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingStepMetrics {
    /// Stable step receipt identifier.
    pub receipt_id: String,
    /// One-based global step.
    pub global_step: u64,
    /// Mean training loss across the training split after the step.
    pub training_mean_loss: f32,
    /// Mean held-out loss across the held-out split after the step.
    pub held_out_mean_loss: f32,
    /// Mean held-out routing delta from the baseline model.
    pub held_out_mean_routing_l2_delta: f32,
    /// Number of held-out cases whose loss improved.
    pub held_out_improved_case_count: u32,
    /// Mean selectivity across the current diagnostics snapshot.
    pub mean_selectivity: f32,
}

/// Final machine-readable summary for the AttnRes tiny-training lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingSummary {
    /// Fixed-budget run summary from the shared train core.
    pub run_summary: TrainingRunSummary,
    /// Mean baseline loss across the training split.
    pub initial_training_mean_loss: f32,
    /// Mean trained loss across the training split.
    pub final_training_mean_loss: f32,
    /// Training loss delta (`final - initial`).
    pub training_loss_delta: f32,
    /// Final held-out comparison report.
    pub held_out_eval: AttnResTrainingEvalReport,
    /// Stable digest of the initial checkpoint manifest.
    pub initial_checkpoint_manifest_digest: String,
    /// Stable digest of the final checkpoint manifest.
    pub final_checkpoint_manifest_digest: String,
}

/// Full outcome for the AttnRes tiny-training lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingOutcome {
    /// Seeded baseline model before any step.
    pub initial_model: AttnResCpuReferenceModel,
    /// Final trained model after the fixed budget.
    pub trained_model: AttnResCpuReferenceModel,
    /// Shared train-core receipts.
    pub step_receipts: Vec<TrainingStepReceipt>,
    /// Higher-level per-step metrics.
    pub step_metrics: Vec<AttnResTinyTrainingStepMetrics>,
    /// Initial checkpoint artifact.
    pub initial_checkpoint: AttnResTinyTrainingCheckpointArtifact,
    /// Final checkpoint artifact.
    pub final_checkpoint: AttnResTinyTrainingCheckpointArtifact,
    /// Final summary.
    pub summary: AttnResTinyTrainingSummary,
}

/// Lifecycle status for the stepwise AttnRes tiny-training runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttnResTinyTrainingLifecycleStatus {
    /// Runner seeded a baseline model and is ready for the first step.
    Starting,
    /// Runner applied at least one step and can continue.
    Running,
    /// Runner exhausted its fixed budget and produced the final stepped state.
    Completed,
}

/// One renderer-neutral live update from the AttnRes tiny-training runner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTinyTrainingUpdate {
    /// Stable run identifier.
    pub run_id: String,
    /// Current lifecycle status.
    pub lifecycle: AttnResTinyTrainingLifecycleStatus,
    /// Current completed step count.
    pub current_global_step: u64,
    /// Fixed-budget upper bound for the run.
    pub max_steps: u64,
    /// Current mean training loss across the training split.
    pub current_training_mean_loss: f32,
    /// Current mean held-out loss across the held-out split.
    pub current_held_out_mean_loss: f32,
    /// Current mean held-out routing delta from the baseline model.
    pub current_held_out_mean_routing_l2_delta: f32,
    /// Current number of held-out cases whose loss improved.
    pub current_held_out_improved_case_count: u32,
    /// Logical per-step duration for this run.
    pub logical_step_duration_ms: u64,
    /// Logical elapsed duration through the current completed step count.
    pub logical_elapsed_ms: u64,
    /// Logical remaining duration through the configured fixed budget.
    pub logical_remaining_ms: u64,
    /// Stable sample identifier used for live diagnostics.
    pub inspection_sample_id: String,
    /// Input tokens used for live diagnostics.
    pub inspection_tokens: TokenSequence,
    /// Current routing diagnostics from the stepped model.
    pub diagnostics: AttnResDiagnosticsSnapshot,
    /// Step metrics for the most recently applied step when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_metrics: Option<AttnResTinyTrainingStepMetrics>,
    /// Receipt for the most recently applied step when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_receipt: Option<TrainingStepReceipt>,
    /// Current checkpoint reference for the stepped model state.
    pub checkpoint: TrainingCheckpointReference,
    /// Optional operator-facing note for this update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Stepwise in-memory runner for the bounded AttnRes tiny-training lane.
#[derive(Debug)]
pub struct AttnResTinyTrainingRunner {
    corpus: AttnResTinyTrainingCorpus,
    config: AttnResTinyTrainingConfig,
    initial_model: AttnResCpuReferenceModel,
    current_model: AttnResCpuReferenceModel,
    inspection_sample: AttnResNextTokenSample,
    run: FixedBudgetTrainingRun,
    trainable_parameter_ids: Vec<String>,
    step_receipts: Vec<TrainingStepReceipt>,
    step_metrics: Vec<AttnResTinyTrainingStepMetrics>,
    initial_checkpoint: AttnResTinyTrainingCheckpointArtifact,
    latest_checkpoint: AttnResTinyTrainingCheckpointArtifact,
    initial_training_mean_loss: f32,
    current_training_mean_loss: f32,
    current_held_out_eval: AttnResTrainingEvalReport,
    current_diagnostics: AttnResDiagnosticsSnapshot,
    latest_note: Option<String>,
}

/// Public alias for the full local AttnRes reference lifecycle status.
pub type AttnResLocalReferenceTrainingLifecycleStatus = AttnResTinyTrainingLifecycleStatus;

/// Public alias for the full local AttnRes reference step metrics.
pub type AttnResLocalReferenceTrainingStepMetrics = AttnResTinyTrainingStepMetrics;

/// Public alias for the full local AttnRes reference update contract.
pub type AttnResLocalReferenceTrainingUpdate = AttnResTinyTrainingUpdate;

/// Public alias for the full local AttnRes reference runner contract.
pub type AttnResLocalReferenceTrainingRunner = AttnResTinyTrainingRunner;

/// Public alias for the full local AttnRes reference outcome contract.
pub type AttnResLocalReferenceTrainingOutcome = AttnResTinyTrainingOutcome;

/// Public alias for the full local AttnRes reference error contract.
pub type AttnResLocalReferenceTrainingError = AttnResTinyTrainingError;

/// Bounded AttnRes tiny-training failure.
#[derive(Debug, Error, PartialEq)]
pub enum AttnResTinyTrainingError {
    #[error("attnres tiny training requires a non-empty model id")]
    MissingModelId,
    #[error("attnres tiny training requires a non-empty model revision")]
    MissingModelRevision,
    #[error("attnres tiny training requires a non-empty run id")]
    MissingRunId,
    #[error("attnres tiny training requires a non-empty checkpoint family")]
    MissingCheckpointFamily,
    #[error("attnres tiny training requires a non-zero step duration")]
    InvalidStepDuration,
    #[error("attnres tiny training requires a positive finite-difference epsilon, got {epsilon}")]
    InvalidFiniteDifferenceEpsilon { epsilon: f32 },
    #[error("attnres tiny training requires at least one training sample")]
    EmptyTrainingSamples,
    #[error("attnres tiny training requires at least one held-out sample")]
    EmptyHeldOutSamples,
    #[error("attnres tiny training sample `{sample_id}` has an empty prefix")]
    EmptySamplePrefix { sample_id: String },
    #[error(
        "attnres tiny training sample `{sample_id}` target token {target_token} exceeds vocab size {vocab_size}"
    )]
    TargetOutOfRange {
        sample_id: String,
        target_token: u32,
        vocab_size: usize,
    },
    #[error("attnres tiny training run is missing parameter group `{group_id}`")]
    MissingParameterGroup { group_id: String },
    #[error("attnres tiny training group `{group_id}` is not dense f32")]
    NonDenseGroup { group_id: String },
    #[error("attnres tiny training runner `{run_id}` already completed")]
    AlreadyCompleted { run_id: String },
    #[error(
        "attnres tiny training runner ended early: completed {completed_steps} of {max_steps} steps"
    )]
    IncompleteRun {
        completed_steps: u64,
        max_steps: u64,
    },
    #[error("{context}: {message}")]
    Serialization {
        context: &'static str,
        message: String,
    },
    #[error(transparent)]
    TrainCore(#[from] TrainingCoreError),
    #[error(transparent)]
    Model(#[from] AttnResModelError),
    #[error(transparent)]
    Execution(#[from] AttnResExecutionError),
    #[error(transparent)]
    Eval(#[from] AttnResTrainingEvalError),
}

impl AttnResTinyTrainingRunner {
    /// Seeds one stepwise runner over the bounded AttnRes tiny-training lane.
    pub fn new(
        corpus: &AttnResTinyTrainingCorpus,
        config: &AttnResTinyTrainingConfig,
    ) -> Result<Self, AttnResTinyTrainingError> {
        config.validate()?;
        validate_corpus(corpus)?;

        let initial_model = AttnResCpuReferenceModel::seeded(
            config.model_id.clone(),
            config.model_revision.clone(),
            corpus.config.clone(),
        )?;
        let trainable_parameters = initial_model
            .weights()
            .parameter_vectors()
            .into_iter()
            .filter(|parameter| {
                parameter.parameter_id.ends_with(".pseudo_query")
                    || parameter.parameter_id == "lm_head.weight"
                    || parameter.parameter_id == "lm_head.bias"
            })
            .collect::<Vec<_>>();
        let trainable_parameter_ids = trainable_parameters
            .iter()
            .map(|parameter| parameter.parameter_id.clone())
            .collect::<Vec<_>>();
        let run = FixedBudgetTrainingRun::new(
            config.run_id.clone(),
            config.checkpoint_family.clone(),
            config.budget,
            build_training_groups(
                trainable_parameters.as_slice(),
                &config.routing_optimizer,
                &config.head_optimizer,
            )?,
        )?;
        let initial_checkpoint = export_checkpoint(
            &initial_model,
            &run,
            trainable_parameter_ids.as_slice(),
            corpus,
            config,
            0,
            None,
            None,
        )?;
        let initial_training_mean_loss = mean_loss(&initial_model, &corpus.training_samples)?;
        let inspection_sample = inspection_sample(corpus)?.clone();
        let current_held_out_eval = evaluate_attnres_training_shift(
            &initial_model,
            &initial_model,
            &corpus.held_out_samples,
        )?;
        let current_diagnostics = inspection_diagnostics(&initial_model, &inspection_sample)?;
        let latest_note = Some(format!(
            "seeded {} with inspection sample {}",
            config.run_id, inspection_sample.sample_id
        ));
        Ok(Self {
            corpus: corpus.clone(),
            config: config.clone(),
            initial_model: initial_model.clone(),
            current_model: initial_model,
            inspection_sample,
            run,
            trainable_parameter_ids,
            step_receipts: Vec::new(),
            step_metrics: Vec::new(),
            initial_checkpoint: initial_checkpoint.clone(),
            latest_checkpoint: initial_checkpoint,
            initial_training_mean_loss,
            current_training_mean_loss: initial_training_mean_loss,
            current_held_out_eval,
            current_diagnostics,
            latest_note,
        })
    }

    /// Returns whether the fixed-budget run already reached completion.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.run.completed_steps() >= self.config.budget.max_steps
    }

    /// Returns the current renderer-neutral live update.
    #[must_use]
    pub fn current_update(&self) -> AttnResTinyTrainingUpdate {
        let completed_steps = self.run.completed_steps();
        let logical_elapsed_ms = completed_steps.saturating_mul(self.config.step_duration_ms);
        let logical_remaining_ms = self
            .config
            .budget
            .max_steps
            .saturating_sub(completed_steps)
            .saturating_mul(self.config.step_duration_ms);
        AttnResTinyTrainingUpdate {
            run_id: self.config.run_id.clone(),
            lifecycle: if self.is_complete() {
                AttnResTinyTrainingLifecycleStatus::Completed
            } else if completed_steps == 0 {
                AttnResTinyTrainingLifecycleStatus::Starting
            } else {
                AttnResTinyTrainingLifecycleStatus::Running
            },
            current_global_step: completed_steps,
            max_steps: self.config.budget.max_steps,
            current_training_mean_loss: self.current_training_mean_loss,
            current_held_out_mean_loss: self.current_held_out_eval.trained_mean_loss,
            current_held_out_mean_routing_l2_delta: self
                .current_held_out_eval
                .mean_routing_l2_delta,
            current_held_out_improved_case_count: self.current_held_out_eval.improved_case_count,
            logical_step_duration_ms: self.config.step_duration_ms,
            logical_elapsed_ms,
            logical_remaining_ms,
            inspection_sample_id: self.inspection_sample.sample_id.clone(),
            inspection_tokens: self.inspection_sample.input_tokens.clone(),
            diagnostics: self.current_diagnostics.clone(),
            step_metrics: self.step_metrics.last().cloned(),
            step_receipt: self.step_receipts.last().cloned(),
            checkpoint: self.latest_checkpoint.checkpoint.clone(),
            note: self.latest_note.clone(),
        }
    }

    /// Advances the run by exactly one trainer step and returns the new update.
    pub fn step(&mut self) -> Result<AttnResTinyTrainingUpdate, AttnResTinyTrainingError> {
        if self.is_complete() {
            return Err(AttnResTinyTrainingError::AlreadyCompleted {
                run_id: self.config.run_id.clone(),
            });
        }
        let step_index = self.run.completed_steps();
        let sample =
            &self.corpus.training_samples[step_index as usize % self.corpus.training_samples.len()];
        let batch = build_gradient_batch(
            &self.initial_model,
            &self.run,
            &self.current_model,
            sample,
            self.config.finite_difference_epsilon,
        )?;
        let started_at_ms = self
            .config
            .started_at_ms
            .saturating_add(step_index.saturating_mul(self.config.step_duration_ms));
        let finished_at_ms = started_at_ms.saturating_add(self.config.step_duration_ms);
        let receipt =
            self.run
                .apply_step(TrainingStepInput::new(batch, started_at_ms, finished_at_ms))?;
        self.current_model = materialize_model(&self.initial_model, &self.run)?;
        self.current_training_mean_loss =
            mean_loss(&self.current_model, &self.corpus.training_samples)?;
        self.current_held_out_eval = evaluate_attnres_training_shift(
            &self.initial_model,
            &self.current_model,
            &self.corpus.held_out_samples,
        )?;
        self.current_diagnostics =
            inspection_diagnostics(&self.current_model, &self.inspection_sample)?;
        let step_metrics = AttnResTinyTrainingStepMetrics {
            receipt_id: receipt.receipt_id.clone(),
            global_step: receipt.schedule.global_step,
            training_mean_loss: self.current_training_mean_loss,
            held_out_mean_loss: self.current_held_out_eval.trained_mean_loss,
            held_out_mean_routing_l2_delta: self.current_held_out_eval.mean_routing_l2_delta,
            held_out_improved_case_count: self.current_held_out_eval.improved_case_count,
            mean_selectivity: mean_selectivity_from_diagnostics(&self.current_diagnostics),
        };
        let checkpoint = export_checkpoint(
            &self.initial_model,
            &self.run,
            self.trainable_parameter_ids.as_slice(),
            &self.corpus,
            &self.config,
            self.run.completed_steps(),
            Some(&self.latest_checkpoint),
            Some(&receipt),
        )?;
        self.step_receipts.push(receipt);
        self.step_metrics.push(step_metrics);
        self.latest_checkpoint = checkpoint;
        self.latest_note = Some(format!(
            "applied step {} of {}",
            self.run.completed_steps(),
            self.config.budget.max_steps
        ));
        Ok(self.current_update())
    }

    /// Returns the bound corpus for the active run.
    #[must_use]
    pub fn corpus(&self) -> &AttnResTinyTrainingCorpus {
        &self.corpus
    }

    /// Returns the bound config for the active run.
    #[must_use]
    pub fn config(&self) -> &AttnResTinyTrainingConfig {
        &self.config
    }

    /// Returns the seeded baseline model.
    #[must_use]
    pub fn initial_model(&self) -> &AttnResCpuReferenceModel {
        &self.initial_model
    }

    /// Returns the current stepped model.
    #[must_use]
    pub fn current_model(&self) -> &AttnResCpuReferenceModel {
        &self.current_model
    }

    /// Returns the accumulated per-step metrics.
    #[must_use]
    pub fn step_metrics(&self) -> &[AttnResTinyTrainingStepMetrics] {
        self.step_metrics.as_slice()
    }

    /// Consumes a completed runner and emits the stable whole-run outcome.
    pub fn into_outcome(self) -> Result<AttnResTinyTrainingOutcome, AttnResTinyTrainingError> {
        if !self.is_complete() {
            return Err(AttnResTinyTrainingError::IncompleteRun {
                completed_steps: self.run.completed_steps(),
                max_steps: self.config.budget.max_steps,
            });
        }
        let summary = AttnResTinyTrainingSummary {
            run_summary: self.run.summary(),
            initial_training_mean_loss: self.initial_training_mean_loss,
            final_training_mean_loss: self.current_training_mean_loss,
            training_loss_delta: self.current_training_mean_loss - self.initial_training_mean_loss,
            held_out_eval: self.current_held_out_eval,
            initial_checkpoint_manifest_digest: self.initial_checkpoint.manifest.stable_digest(),
            final_checkpoint_manifest_digest: self.latest_checkpoint.manifest.stable_digest(),
        };
        Ok(AttnResTinyTrainingOutcome {
            initial_model: self.initial_model,
            trained_model: self.current_model,
            step_receipts: self.step_receipts,
            step_metrics: self.step_metrics,
            initial_checkpoint: self.initial_checkpoint,
            final_checkpoint: self.latest_checkpoint,
            summary,
        })
    }
}

/// Runs the bounded AttnRes tiny-training lane end to end.
pub fn train_attnres_tiny_next_token(
    corpus: &AttnResTinyTrainingCorpus,
    config: &AttnResTinyTrainingConfig,
) -> Result<AttnResTinyTrainingOutcome, AttnResTinyTrainingError> {
    let mut runner = AttnResTinyTrainingRunner::new(corpus, config)?;
    while !runner.is_complete() {
        runner.step()?;
    }
    runner.into_outcome()
}

/// Runs the full local AttnRes reference training loop to completion.
pub fn train_attnres_local_reference_next_token(
    corpus: &AttnResLocalReferenceTrainingCorpus,
    config: &AttnResLocalReferenceTrainingConfig,
) -> Result<AttnResLocalReferenceTrainingOutcome, AttnResLocalReferenceTrainingError> {
    train_attnres_tiny_next_token(corpus, config)
}

/// Restores one AttnRes model from a persisted tiny-training checkpoint.
pub fn restore_attnres_tiny_checkpoint(
    manifest: &AttnResTinyTrainingCheckpointManifest,
    weights_bytes: &[u8],
) -> Result<AttnResCpuReferenceModel, AttnResTinyTrainingError> {
    let base_model = AttnResCpuReferenceModel::seeded(
        manifest.model_id.clone(),
        manifest.model_revision.clone(),
        manifest.config.clone(),
    )?;
    let safetensors = SafeTensors::deserialize(weights_bytes)
        .map_err(|error| serialization_error("checkpoint restore", error))?;
    let mut overrides = BTreeMap::new();
    for parameter_id in &manifest.parameter_ids {
        let tensor = safetensors
            .tensor(parameter_id)
            .map_err(|error| serialization_error("checkpoint restore", error))?;
        let values = decode_f32_bytes(parameter_id.as_str(), tensor.data())?;
        overrides.insert(parameter_id.clone(), values);
    }
    let weights = base_model
        .weights()
        .with_parameter_overrides(&manifest.config, &overrides)?;
    AttnResCpuReferenceModel::with_weights(
        base_model.descriptor().model.clone(),
        manifest.config.clone(),
        weights,
    )
    .map_err(Into::into)
}

/// Restores one local-reference checkpoint exported from the full AttnRes lane.
pub fn restore_attnres_local_reference_checkpoint(
    manifest: &AttnResTinyTrainingCheckpointManifest,
    weights_bytes: &[u8],
) -> Result<AttnResCpuReferenceModel, AttnResTinyTrainingError> {
    restore_attnres_tiny_checkpoint(manifest, weights_bytes)
}

fn validate_corpus(corpus: &AttnResTinyTrainingCorpus) -> Result<(), AttnResTinyTrainingError> {
    if corpus.training_samples.is_empty() {
        return Err(AttnResTinyTrainingError::EmptyTrainingSamples);
    }
    if corpus.held_out_samples.is_empty() {
        return Err(AttnResTinyTrainingError::EmptyHeldOutSamples);
    }
    for sample in corpus
        .training_samples
        .iter()
        .chain(corpus.held_out_samples.iter())
    {
        if sample.input_tokens.is_empty() {
            return Err(AttnResTinyTrainingError::EmptySamplePrefix {
                sample_id: sample.sample_id.clone(),
            });
        }
        if sample.target_token.as_u32() as usize >= corpus.config.vocab_size {
            return Err(AttnResTinyTrainingError::TargetOutOfRange {
                sample_id: sample.sample_id.clone(),
                target_token: sample.target_token.as_u32(),
                vocab_size: corpus.config.vocab_size,
            });
        }
    }
    Ok(())
}

fn mean_selectivity_from_diagnostics(diagnostics: &AttnResDiagnosticsSnapshot) -> f32 {
    let values = diagnostics
        .sublayers
        .iter()
        .map(|sublayer| {
            selectivity_from_attnres_weights(
                aggregate_attnres_source_values(&sublayer.routing_weights, sublayer.source_shape)
                    .as_slice(),
            )
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

fn aggregate_attnres_source_values(values: &[f32], source_shape: [usize; 3]) -> Vec<f32> {
    let [sources, batch, sequence] = source_shape;
    if sources == 0 {
        return Vec::new();
    }
    let stride = batch.saturating_mul(sequence).max(1);
    (0..sources)
        .map(|source_index| {
            let start = source_index.saturating_mul(stride);
            let end = (start + stride).min(values.len());
            let slice = &values[start..end];
            if slice.is_empty() {
                0.0
            } else {
                slice.iter().sum::<f32>() / slice.len() as f32
            }
        })
        .collect()
}

fn selectivity_from_attnres_weights(weights: &[f32]) -> f32 {
    if weights.len() <= 1 {
        return 0.0;
    }
    let max_weight = weights
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max)
        .max(0.0);
    let uniform = 1.0 / weights.len() as f32;
    ((max_weight - uniform) / (1.0 - uniform)).clamp(0.0, 1.0)
}

fn inspection_sample(
    corpus: &AttnResTinyTrainingCorpus,
) -> Result<&AttnResNextTokenSample, AttnResTinyTrainingError> {
    corpus
        .held_out_samples
        .first()
        .or_else(|| corpus.training_samples.first())
        .ok_or(AttnResTinyTrainingError::EmptyTrainingSamples)
}

fn inspection_diagnostics(
    model: &AttnResCpuReferenceModel,
    sample: &AttnResNextTokenSample,
) -> Result<AttnResDiagnosticsSnapshot, AttnResTinyTrainingError> {
    model
        .forward_hidden_with_diagnostics(std::slice::from_ref(&sample.input_tokens))
        .map(|(_, diagnostics)| diagnostics)
        .map_err(Into::into)
}

fn build_training_groups(
    parameters: &[AttnResParameterVector],
    routing_optimizer: &TrainingOptimizerConfig,
    head_optimizer: &TrainingOptimizerConfig,
) -> Result<Vec<TrainingParameterGroupState>, AttnResTinyTrainingError> {
    let mut groups = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let class = if parameter.parameter_id.ends_with(".pseudo_query") {
            TrainingParameterClass::Scalar
        } else if parameter.parameter_id.ends_with(".bias") {
            TrainingParameterClass::Bias
        } else {
            TrainingParameterClass::Head
        };
        let optimizer = if parameter.parameter_id.ends_with(".pseudo_query") {
            routing_optimizer.clone()
        } else {
            head_optimizer.clone()
        };
        groups.push(TrainingParameterGroupState::new(
            parameter.parameter_id.clone(),
            class,
            TrainingTensorBuffer::from_f32(
                parameter.parameter_id.clone(),
                TensorSpec::new(parameter.shape.clone(), DType::F32, Device::cpu()),
                parameter.values.clone(),
            )?,
            optimizer,
            TrainingOptimizerResidencyPolicy::host_only(),
        )?);
    }
    Ok(groups)
}

fn build_gradient_batch(
    initial_model: &AttnResCpuReferenceModel,
    run: &FixedBudgetTrainingRun,
    current_model: &AttnResCpuReferenceModel,
    sample: &AttnResNextTokenSample,
    epsilon: f32,
) -> Result<TrainingGradientBatch, AttnResTinyTrainingError> {
    let (last_hidden, last_logits, loss) = sample_forward(current_model, sample)?;
    let probs = softmax(last_logits.as_slice());
    let target_index = sample.target_token.as_u32() as usize;
    let mut logits_gradient = probs.clone();
    logits_gradient[target_index] -= 1.0;

    let weight_group = required_group(run, "lm_head.weight")?;
    let bias_group = required_group(run, "lm_head.bias")?;
    let mut gradients = BTreeMap::new();
    gradients.insert(
        String::from("lm_head.weight"),
        TrainingTensorBuffer::from_f32(
            String::from("lm_head.weight"),
            weight_group.parameter.spec.clone(),
            lm_head_weight_gradient(last_hidden.as_slice(), logits_gradient.as_slice()),
        )?,
    );
    gradients.insert(
        String::from("lm_head.bias"),
        TrainingTensorBuffer::from_f32(
            String::from("lm_head.bias"),
            bias_group.parameter.spec.clone(),
            logits_gradient.clone(),
        )?,
    );

    let base_overrides = collect_overrides(run)?;
    for group_id in run
        .summary()
        .final_parameter_norms_l2
        .keys()
        .filter(|group_id| group_id.ends_with(".pseudo_query"))
    {
        let group = required_group(run, group_id)?;
        let mut gradient = vec![0.0f32; dense_values(group, group_id.as_str())?.len()];
        for index in 0..gradient.len() {
            let mut plus = base_overrides.clone();
            let mut minus = base_overrides.clone();
            plus.get_mut(group_id).expect("routing group override")[index] += epsilon;
            minus.get_mut(group_id).expect("routing group override")[index] -= epsilon;
            let plus_model = materialize_model_with_overrides(initial_model, &plus)?;
            let minus_model = materialize_model_with_overrides(initial_model, &minus)?;
            let plus_loss = sample_loss(&plus_model, sample)?;
            let minus_loss = sample_loss(&minus_model, sample)?;
            gradient[index] = (plus_loss - minus_loss) / (2.0 * epsilon);
        }
        gradients.insert(
            group_id.clone(),
            TrainingTensorBuffer::from_f32(
                group_id.clone(),
                group.parameter.spec.clone(),
                gradient,
            )?,
        );
    }

    Ok(TrainingGradientBatch::new(
        format!("{}-gradient", sample.sample_id),
        loss,
        1,
        gradients,
    ))
}

fn sample_forward(
    model: &AttnResCpuReferenceModel,
    sample: &AttnResNextTokenSample,
) -> Result<(Vec<f32>, Vec<f32>, f32), AttnResTinyTrainingError> {
    let batch = [sample.input_tokens.clone()];
    let hidden = model.forward_hidden(&batch)?;
    let logits = model.forward(&batch)?;
    let last_hidden = last_position_slice(&hidden);
    let last_logits = last_position_slice(&logits);
    let probabilities = softmax(last_logits.as_slice());
    let target_probability = probabilities[sample.target_token.as_u32() as usize].max(f32::EPSILON);
    Ok((last_hidden, last_logits, -target_probability.ln()))
}

fn sample_loss(
    model: &AttnResCpuReferenceModel,
    sample: &AttnResNextTokenSample,
) -> Result<f32, AttnResTinyTrainingError> {
    sample_forward(model, sample).map(|(_, _, loss)| loss)
}

fn mean_loss(
    model: &AttnResCpuReferenceModel,
    samples: &[AttnResNextTokenSample],
) -> Result<f32, AttnResTinyTrainingError> {
    let mut total = 0.0f32;
    for sample in samples {
        total += sample_loss(model, sample)?;
    }
    Ok(total / samples.len() as f32)
}

fn lm_head_weight_gradient(hidden: &[f32], logits_gradient: &[f32]) -> Vec<f32> {
    let mut gradient = vec![0.0f32; hidden.len() * logits_gradient.len()];
    for (input_index, hidden_value) in hidden.iter().enumerate() {
        for (output_index, logit_grad) in logits_gradient.iter().enumerate() {
            gradient[input_index * logits_gradient.len() + output_index] =
                hidden_value * logit_grad;
        }
    }
    gradient
}

fn collect_overrides(
    run: &FixedBudgetTrainingRun,
) -> Result<BTreeMap<String, Vec<f32>>, AttnResTinyTrainingError> {
    let mut overrides = BTreeMap::new();
    for group_id in run.summary().final_parameter_norms_l2.keys() {
        let group = required_group(run, group_id.as_str())?;
        overrides.insert(
            group_id.clone(),
            dense_values(group, group_id.as_str())?.to_vec(),
        );
    }
    Ok(overrides)
}

fn materialize_model(
    initial_model: &AttnResCpuReferenceModel,
    run: &FixedBudgetTrainingRun,
) -> Result<AttnResCpuReferenceModel, AttnResTinyTrainingError> {
    materialize_model_with_overrides(initial_model, &collect_overrides(run)?)
}

fn materialize_model_with_overrides(
    initial_model: &AttnResCpuReferenceModel,
    overrides: &BTreeMap<String, Vec<f32>>,
) -> Result<AttnResCpuReferenceModel, AttnResTinyTrainingError> {
    let weights = initial_model
        .weights()
        .with_parameter_overrides(initial_model.config(), overrides)?;
    AttnResCpuReferenceModel::with_weights(
        initial_model.descriptor().model.clone(),
        initial_model.config().clone(),
        weights,
    )
    .map_err(Into::into)
}

fn required_group<'a>(
    run: &'a FixedBudgetTrainingRun,
    group_id: &str,
) -> Result<&'a TrainingParameterGroupState, AttnResTinyTrainingError> {
    run.parameter_group(group_id)
        .ok_or_else(|| AttnResTinyTrainingError::MissingParameterGroup {
            group_id: String::from(group_id),
        })
}

fn dense_values<'a>(
    group: &'a TrainingParameterGroupState,
    group_id: &str,
) -> Result<&'a [f32], AttnResTinyTrainingError> {
    match &group.parameter.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        TensorData::I32(_) => Err(AttnResTinyTrainingError::NonDenseGroup {
            group_id: String::from(group_id),
        }),
        TensorData::QuantizedBlocks(_) => Err(AttnResTinyTrainingError::NonDenseGroup {
            group_id: String::from(group_id),
        }),
    }
}

fn last_position_slice(tensor: &psionic_models::AttnResTensor3) -> Vec<f32> {
    let width = tensor.width();
    let last_position = tensor.sequence_length() - 1;
    let offset = last_position * width;
    tensor.values()[offset..offset + width].to_vec()
}

fn export_checkpoint(
    initial_model: &AttnResCpuReferenceModel,
    run: &FixedBudgetTrainingRun,
    parameter_ids: &[String],
    corpus: &AttnResTinyTrainingCorpus,
    config: &AttnResTinyTrainingConfig,
    step: u64,
    parent: Option<&AttnResTinyTrainingCheckpointArtifact>,
    receipt: Option<&TrainingStepReceipt>,
) -> Result<AttnResTinyTrainingCheckpointArtifact, AttnResTinyTrainingError> {
    let checkpoint_ref = format!("{}:step:{}", config.run_id, step);
    let weights_bytes = export_checkpoint_weights(run, parameter_ids)?;
    let manifest = AttnResTinyTrainingCheckpointManifest {
        schema_version: 1,
        checkpoint_ref: checkpoint_ref.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        run_id: config.run_id.clone(),
        step,
        model_id: config.model_id.clone(),
        model_revision: config.model_revision.clone(),
        config: corpus.config.clone(),
        base_descriptor_digest: initial_model.descriptor().stable_digest(),
        base_weight_digest: initial_model.descriptor().weights.digest.clone(),
        parameter_state_digest: stable_bytes_digest(
            b"psionic_attnres_checkpoint_weights|",
            &weights_bytes,
        ),
        training_dataset_digest: corpus.training_digest(),
        held_out_dataset_digest: corpus.held_out_digest(),
        parameter_ids: parameter_ids.to_vec(),
        parent_checkpoint_ref: parent.map(|checkpoint| checkpoint.manifest.checkpoint_ref.clone()),
        parent_manifest_digest: parent.map(|checkpoint| checkpoint.manifest.stable_digest()),
        step_receipt_id: receipt.map(|receipt| receipt.receipt_id.clone()),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        AttnResTinyTrainingError::Serialization {
            context: "attnres checkpoint manifest export",
            message: error.to_string(),
        }
    })?;
    let weights_artifact = AttnResTinyTrainingArtifact::new(
        "attnres_checkpoint_weights",
        format!(
            "artifact://attnres/{}/checkpoint/{step}/weights.safetensors",
            config.run_id
        ),
        weights_bytes.clone(),
    );
    let manifest_artifact = AttnResTinyTrainingArtifact::new(
        "attnres_checkpoint_manifest",
        format!(
            "artifact://attnres/{}/checkpoint/{step}/manifest.json",
            config.run_id
        ),
        manifest_bytes,
    );
    let stream_id = format!("attnres.checkpoint.{}.{}", config.run_id, step);
    let manifest_ref = DatastreamManifestRef {
        stream_id: stream_id.clone(),
        manifest_digest: manifest_artifact.artifact_digest.clone(),
        subject: DatastreamSubjectKind::Checkpoint,
        object_digest: weights_artifact.artifact_digest.clone(),
        total_bytes: weights_artifact.bytes.len() as u64,
        chunk_count: 1,
        chunk_bytes: weights_artifact.bytes.len(),
        encoding: DatastreamEncoding::Safetensors,
        compression: None,
        provenance_digest: Some(initial_model.descriptor().stable_digest()),
        dataset_binding: None,
        checkpoint_binding: Some(
            DatastreamCheckpointBinding::new(config.checkpoint_family.clone())
                .with_checkpoint_ref(checkpoint_ref.clone())
                .with_step(step),
        ),
        policy_weight_binding: None,
        mirrors: Vec::new(),
    };
    let checkpoint = TrainingCheckpointReference::new(
        config.checkpoint_family.clone(),
        stream_id,
        manifest_ref.manifest_digest.clone(),
        manifest_ref.object_digest.clone(),
        "psionic.local.cpu_reference",
        0,
        "cluster.local.cpu_reference",
        "topology.cpu_reference",
        config.started_at_ms + step.saturating_mul(config.step_duration_ms),
    )
    .with_checkpoint_ref(checkpoint_ref)
    .with_step(step)
    .with_durable_at_ms(config.started_at_ms + step.saturating_mul(config.step_duration_ms));
    Ok(AttnResTinyTrainingCheckpointArtifact {
        weights_artifact,
        manifest_artifact,
        manifest,
        checkpoint,
        manifest_ref,
    })
}

fn export_checkpoint_weights(
    run: &FixedBudgetTrainingRun,
    parameter_ids: &[String],
) -> Result<Vec<u8>, AttnResTinyTrainingError> {
    let manifest_json = serde_json::to_string(parameter_ids).map_err(|error| {
        AttnResTinyTrainingError::Serialization {
            context: "attnres checkpoint metadata export",
            message: error.to_string(),
        }
    })?;
    let mut metadata = HashMap::new();
    metadata.insert(String::from(ATTNRES_CHECKPOINT_MANIFEST_KEY), manifest_json);

    let mut raw_buffers = Vec::with_capacity(parameter_ids.len());
    for parameter_id in parameter_ids {
        let group = required_group(run, parameter_id.as_str())?;
        raw_buffers.push((
            parameter_id.clone(),
            encode_f32_bytes(dense_values(group, parameter_id.as_str())?),
            group.parameter.spec.shape().dims().to_vec(),
        ));
    }

    let mut views = Vec::with_capacity(raw_buffers.len());
    for (parameter_id, raw_bytes, shape) in &raw_buffers {
        let view = TensorView::new(SafeTensorsDType::F32, shape.clone(), raw_bytes.as_slice())
            .map_err(|error| serialization_error("attnres checkpoint safetensors export", error))?;
        views.push((parameter_id.clone(), view));
    }
    serialize(
        views
            .iter()
            .map(|(parameter_id, view)| (parameter_id.as_str(), view.clone())),
        Some(metadata),
    )
    .map_err(|error| serialization_error("attnres checkpoint safetensors export", error))
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
) -> Result<Vec<f32>, AttnResTinyTrainingError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(AttnResTinyTrainingError::Serialization {
            context: "checkpoint restore",
            message: format!(
                "tensor `{parameter_id}` byte length {} is not divisible by 4",
                bytes.len()
            ),
        });
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn softmax(values: &[f32]) -> Vec<f32> {
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = values
        .iter()
        .map(|value| (*value - max).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(f32::EPSILON);
    exp.into_iter().map(|value| value / sum).collect()
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    stable_bytes_digest(prefix, &encoded)
}

fn stable_bytes_digest(prefix: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn serialization_error(context: &'static str, error: impl ToString) -> AttnResTinyTrainingError {
    AttnResTinyTrainingError::Serialization {
        context,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        attnres_local_reference_training_config, attnres_local_reference_training_corpus,
        restore_attnres_tiny_checkpoint, train_attnres_tiny_next_token, AttnResTinyTrainingConfig,
        AttnResTinyTrainingCorpus, AttnResTinyTrainingLifecycleStatus, AttnResTinyTrainingRunner,
    };

    #[test]
    fn reference_corpus_loads_fixture() -> Result<(), Box<dyn Error>> {
        let corpus = AttnResTinyTrainingCorpus::reference()?;
        assert!(!corpus.training_samples.is_empty());
        assert!(!corpus.held_out_samples.is_empty());
        Ok(())
    }

    #[test]
    fn attnres_runner_exposes_starting_update_and_steps_to_completion() -> Result<(), Box<dyn Error>>
    {
        let corpus = AttnResTinyTrainingCorpus::reference()?;
        let config = AttnResTinyTrainingConfig::reference()?;
        let mut runner = AttnResTinyTrainingRunner::new(&corpus, &config)?;
        let initial = runner.current_update();
        assert_eq!(
            initial.lifecycle,
            AttnResTinyTrainingLifecycleStatus::Starting
        );
        assert_eq!(initial.current_global_step, 0);
        assert!(initial.step_metrics.is_none());
        assert_eq!(initial.checkpoint.step, Some(0));

        let mut last = initial;
        while !runner.is_complete() {
            last = runner.step()?;
        }

        assert_eq!(
            last.lifecycle,
            AttnResTinyTrainingLifecycleStatus::Completed
        );
        assert_eq!(last.current_global_step, config.budget.max_steps);
        assert_eq!(
            last.step_metrics
                .as_ref()
                .map(|metrics| metrics.global_step),
            Some(config.budget.max_steps)
        );
        assert_eq!(last.checkpoint.step, Some(config.budget.max_steps));

        let outcome = runner.into_outcome()?;
        assert_eq!(outcome.step_metrics.len() as u64, config.budget.max_steps);
        assert!(
            outcome.summary.final_training_mean_loss < outcome.summary.initial_training_mean_loss
        );
        assert!(outcome.summary.held_out_eval.mean_routing_l2_delta > 0.0);
        Ok(())
    }

    #[test]
    fn whole_run_api_stays_layered_on_runner() -> Result<(), Box<dyn Error>> {
        let corpus = AttnResTinyTrainingCorpus::reference()?;
        let config = AttnResTinyTrainingConfig::reference()?;
        let whole_run = train_attnres_tiny_next_token(&corpus, &config)?;
        let mut runner = AttnResTinyTrainingRunner::new(&corpus, &config)?;
        while !runner.is_complete() {
            runner.step()?;
        }
        let stepped = runner.into_outcome()?;
        assert_eq!(stepped.step_metrics, whole_run.step_metrics);
        assert_eq!(stepped.summary, whole_run.summary);
        assert_eq!(
            stepped.final_checkpoint.manifest,
            whole_run.final_checkpoint.manifest
        );
        Ok(())
    }

    #[test]
    fn attnres_tiny_training_runs_end_to_end_and_restores_checkpoint() -> Result<(), Box<dyn Error>>
    {
        let corpus = AttnResTinyTrainingCorpus::reference()?;
        let config = AttnResTinyTrainingConfig::reference()?;
        let outcome = train_attnres_tiny_next_token(&corpus, &config)?;
        let restored = restore_attnres_tiny_checkpoint(
            &outcome.final_checkpoint.manifest,
            &outcome.final_checkpoint.weights_artifact.bytes,
        )?;
        assert_eq!(restored.descriptor(), outcome.trained_model.descriptor());
        let restored_eval = psionic_eval::evaluate_attnres_training_shift(
            &outcome.initial_model,
            &restored,
            &corpus.held_out_samples,
        )?;
        assert_eq!(restored_eval, outcome.summary.held_out_eval);
        Ok(())
    }

    #[test]
    fn local_reference_config_exposes_full_run_budget() -> Result<(), Box<dyn Error>> {
        let config = attnres_local_reference_training_config()?;
        assert_eq!(config.budget.max_steps, 320);
        assert_eq!(config.run_id, "attnres-local-reference-run");
        assert_eq!(config.step_duration_ms, 85);
        Ok(())
    }

    #[test]
    fn local_reference_runner_steps_to_completion() -> Result<(), Box<dyn Error>> {
        let corpus = attnres_local_reference_training_corpus()?;
        let config = attnres_local_reference_training_config()?;
        let mut runner = AttnResTinyTrainingRunner::new(&corpus, &config)?;

        let initial_loss = runner.current_update().current_training_mean_loss;
        while !runner.is_complete() {
            runner.step()?;
        }
        let final_update = runner.current_update();
        assert_eq!(
            final_update.lifecycle,
            AttnResTinyTrainingLifecycleStatus::Completed
        );
        assert_eq!(final_update.current_global_step, config.budget.max_steps);
        assert!(final_update.current_training_mean_loss < initial_loss);
        Ok(())
    }
}
