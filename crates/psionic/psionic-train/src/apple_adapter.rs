use std::collections::BTreeMap;

use psionic_core::{DType, Device, Shape, TensorSpec};
use psionic_data::{
    AppleAdapterDatasetContract, AppleAdapterMessage, AppleAdapterMessageRole,
    AppleAdapterSampleKind, AppleAdapterSampleTokenCapture, DatasetPackingPlan,
    DatasetPackingPolicy,
};
use psionic_environments::{
    AppleAdapterEnvironmentBundle, AppleAdapterEnvironmentError, EnvironmentWorkloadClass,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, TrainingCoreError, TrainingGradientBatch, TrainingLoopBudget,
    TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy, TrainingParameterClass,
    TrainingParameterGroupState, TrainingStepInput, TrainingTensorBuffer,
};

/// Precision posture for the first repo-owned Apple reference backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterPrecisionPolicy {
    /// Dense `f32` parameters, gradients, and activations.
    F32Reference,
    /// Reserved for later mixed-precision support.
    Bf16Mixed,
}

/// Activation-checkpoint posture for the first repo-owned Apple reference backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterActivationCheckpointPolicy {
    /// Keep the whole reference path explicit and uncheckpointed.
    Disabled,
    /// Reserved for later activation rematerialization.
    PromptPrefixRecompute,
}

/// One trainable low-rank target kept separate from the frozen base model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterTrainableTarget {
    /// Stable target identifier.
    pub target_id: String,
    /// Low-rank dimension used by the adapter.
    pub lora_rank: usize,
    /// LoRA scaling factor.
    pub lora_alpha: f32,
    /// Optimizer config reused by the fixed-budget core.
    pub optimizer: TrainingOptimizerConfig,
    /// Residency policy reused by the fixed-budget core.
    pub optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
}

impl AppleAdapterTrainableTarget {
    /// Returns the stable `A` matrix group identifier.
    #[must_use]
    pub fn lora_a_group_id(&self) -> String {
        format!("{}.lora_a", self.target_id)
    }

    /// Returns the stable `B` matrix group identifier.
    #[must_use]
    pub fn lora_b_group_id(&self) -> String {
        format!("{}.lora_b", self.target_id)
    }

    fn validate(&self) -> Result<(), AppleAdapterTrainingExecutionError> {
        if self.target_id.trim().is_empty() {
            return Err(AppleAdapterTrainingExecutionError::MissingTargetId);
        }
        if self.lora_rank == 0 {
            return Err(AppleAdapterTrainingExecutionError::InvalidLoraRank {
                target_id: self.target_id.clone(),
                rank: self.lora_rank,
            });
        }
        if !self.lora_alpha.is_finite() || self.lora_alpha <= 0.0 {
            return Err(AppleAdapterTrainingExecutionError::InvalidLoraAlpha {
                target_id: self.target_id.clone(),
                alpha: self.lora_alpha,
            });
        }
        Ok(())
    }

    fn scale(&self) -> f32 {
        self.lora_alpha / self.lora_rank as f32
    }
}

/// Minimal frozen-base and trainable-adapter representation used by the repo-owned backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterReferenceModel {
    /// Stable Apple base-model signature the adapter targets.
    pub base_model_signature: String,
    /// Tokenizer digest that must match the dataset contract.
    pub tokenizer_digest: String,
    /// Prompt-shaping digest that must match the dataset contract.
    pub prompt_shaping_digest: String,
    /// Feature width used for prompt-side projection.
    pub input_width: usize,
    /// Output width used for completion supervision.
    pub output_width: usize,
    /// Adapter targets kept trainable while the frozen base stays fixed.
    pub targets: Vec<AppleAdapterTrainableTarget>,
}

impl AppleAdapterReferenceModel {
    fn validate(&self) -> Result<(), AppleAdapterTrainingExecutionError> {
        if self.base_model_signature.trim().is_empty() {
            return Err(AppleAdapterTrainingExecutionError::MissingBaseModelSignature);
        }
        if self.tokenizer_digest.trim().is_empty() {
            return Err(AppleAdapterTrainingExecutionError::MissingTokenizerDigest);
        }
        if self.prompt_shaping_digest.trim().is_empty() {
            return Err(AppleAdapterTrainingExecutionError::MissingPromptShapingDigest);
        }
        if self.input_width == 0 || self.output_width == 0 {
            return Err(AppleAdapterTrainingExecutionError::InvalidFeatureWidth {
                input_width: self.input_width,
                output_width: self.output_width,
            });
        }
        if self.targets.is_empty() {
            return Err(AppleAdapterTrainingExecutionError::MissingTrainableTargets);
        }
        for target in &self.targets {
            target.validate()?;
        }
        Ok(())
    }
}

/// Full execution config for the first repo-owned Apple backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterExecutionConfig {
    /// Stable run identifier owned by the fixed-budget core.
    pub run_id: String,
    /// Stable checkpoint family label owned by the run.
    pub checkpoint_family: String,
    /// Fixed-budget policy reused by the training core.
    pub budget: TrainingLoopBudget,
    /// Packing policy used to turn dataset samples into model-ready batches.
    pub packing_policy: DatasetPackingPolicy,
    /// Explicit precision posture.
    pub precision_policy: AppleAdapterPrecisionPolicy,
    /// Explicit activation-checkpoint posture.
    pub activation_checkpoint_policy: AppleAdapterActivationCheckpointPolicy,
    /// Frozen-base and adapter-target layout.
    pub model: AppleAdapterReferenceModel,
}

impl AppleAdapterExecutionConfig {
    fn validate(&self) -> Result<(), AppleAdapterTrainingExecutionError> {
        if self.run_id.trim().is_empty() {
            return Err(AppleAdapterTrainingExecutionError::MissingRunId);
        }
        if self.checkpoint_family.trim().is_empty() {
            return Err(AppleAdapterTrainingExecutionError::MissingCheckpointFamily);
        }
        if self.budget.max_steps == 0 {
            return Err(AppleAdapterTrainingExecutionError::InvalidBudget);
        }
        self.model.validate()?;
        match self.precision_policy {
            AppleAdapterPrecisionPolicy::F32Reference => {}
            AppleAdapterPrecisionPolicy::Bf16Mixed => {
                return Err(
                    AppleAdapterTrainingExecutionError::UnsupportedPrecisionPolicy(
                        self.precision_policy,
                    ),
                );
            }
        }
        match self.activation_checkpoint_policy {
            AppleAdapterActivationCheckpointPolicy::Disabled => {}
            AppleAdapterActivationCheckpointPolicy::PromptPrefixRecompute => {
                return Err(
                    AppleAdapterTrainingExecutionError::UnsupportedActivationCheckpointPolicy(
                        self.activation_checkpoint_policy,
                    ),
                );
            }
        }
        Ok(())
    }
}

/// One model-ready sample record produced from the Apple dataset contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterBatchFeatureRecord {
    /// Stable sample id from the dataset contract.
    pub sample_id: String,
    /// Sample family carried by the record.
    pub sample_kind: AppleAdapterSampleKind,
    /// Prompt-side features fed into the reference forward path.
    pub prompt_features: Vec<f32>,
    /// Completion-side supervision target.
    pub target_features: Vec<f32>,
    /// Prompt token count preserved from the explicit capture.
    pub prompt_tokens: u32,
    /// Completion token count preserved from the explicit capture.
    pub completion_tokens: u32,
    /// Stable digest over the model-ready record.
    pub feature_digest: String,
}

/// One packed training batch backed by deterministic Apple feature records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterPackedTrainingBatch {
    /// Stable batch identifier inherited from the packing plan.
    pub batch_id: String,
    /// Sample records assigned to the batch.
    pub records: Vec<AppleAdapterBatchFeatureRecord>,
    /// Total prompt tokens in the batch.
    pub total_prompt_tokens: u32,
    /// Total completion tokens in the batch.
    pub total_completion_tokens: u32,
    /// Stable digest over the batch contents.
    pub batch_digest: String,
}

/// Machine-legible provenance frozen by the repo-owned Apple execution backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterExecutionProvenance {
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Stable core environment ref.
    pub environment_ref: String,
    /// Stable environment group ref.
    pub environment_group_ref: String,
    /// Stable packing policy digest.
    pub packing_policy_digest: String,
    /// Explicit precision posture.
    pub precision_policy: AppleAdapterPrecisionPolicy,
    /// Explicit activation-checkpoint posture.
    pub activation_checkpoint_policy: AppleAdapterActivationCheckpointPolicy,
}

/// Gradient-production artifact emitted by the repo-owned Apple backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterGradientBatchRecord {
    /// Stable batch identifier.
    pub batch_id: String,
    /// Stable digest of the packed batch that sourced the computation.
    pub batch_digest: String,
    /// Machine-legible gradient batch for the fixed-budget trainer core.
    pub training_batch: TrainingGradientBatch,
    /// Mean loss over the packed batch.
    pub mean_loss: f32,
    /// Gradient L2 norms keyed by parameter-group identifier.
    pub gradient_norms_l2: BTreeMap<String, f32>,
    /// Stable digest over the gradient-production record.
    pub execution_digest: String,
}

/// Repo-owned Apple dataset -> batch -> gradient backend.
#[derive(Clone, Debug)]
pub struct AppleAdapterTrainingExecutionBackend {
    config: AppleAdapterExecutionConfig,
    provenance: AppleAdapterExecutionProvenance,
    batches: Vec<AppleAdapterPackedTrainingBatch>,
    frozen_base_projection: Vec<f32>,
}

impl AppleAdapterTrainingExecutionBackend {
    /// Builds the repo-owned Apple execution backend from real dataset and environment contracts.
    pub fn new(
        config: AppleAdapterExecutionConfig,
        dataset: &AppleAdapterDatasetContract,
        captures: &[AppleAdapterSampleTokenCapture],
        environment: &AppleAdapterEnvironmentBundle,
    ) -> Result<Self, AppleAdapterTrainingExecutionError> {
        config.validate()?;
        dataset.validate()?;
        environment
            .core_package
            .supported_workloads
            .contains(&EnvironmentWorkloadClass::Sft)
            .then_some(())
            .ok_or(AppleAdapterTrainingExecutionError::EnvironmentMissingSftWorkload)?;
        if config.model.tokenizer_digest != dataset.metadata.tokenizer.tokenizer_digest {
            return Err(
                AppleAdapterTrainingExecutionError::TokenizerDigestMismatch {
                    expected: config.model.tokenizer_digest.clone(),
                    actual: dataset.metadata.tokenizer.tokenizer_digest.clone(),
                },
            );
        }
        if config.model.prompt_shaping_digest != dataset.metadata.prompt_shaping_digest {
            return Err(
                AppleAdapterTrainingExecutionError::PromptShapingDigestMismatch {
                    expected: config.model.prompt_shaping_digest.clone(),
                    actual: dataset.metadata.prompt_shaping_digest.clone(),
                },
            );
        }

        let packing_plan = dataset.plan_packing(captures, &config.packing_policy)?;
        let batches = build_packed_batches(
            dataset,
            captures,
            &packing_plan,
            config.model.input_width,
            config.model.output_width,
        )?;
        let frozen_base_projection = seeded_matrix(
            format!(
                "{}|base_projection|{}x{}",
                config.model.base_model_signature,
                config.model.output_width,
                config.model.input_width
            )
            .as_str(),
            config.model.output_width,
            config.model.input_width,
            0.05,
        );
        Ok(Self {
            provenance: AppleAdapterExecutionProvenance {
                dataset_digest: dataset.stable_digest(),
                environment_ref: environment.core_package.key.environment_ref.clone(),
                environment_group_ref: environment.group.group_ref.clone(),
                packing_policy_digest: packing_plan.policy_digest.clone(),
                precision_policy: config.precision_policy,
                activation_checkpoint_policy: config.activation_checkpoint_policy,
            },
            config,
            batches,
            frozen_base_projection,
        })
    }

    /// Returns the immutable execution config.
    #[must_use]
    pub fn config(&self) -> &AppleAdapterExecutionConfig {
        &self.config
    }

    /// Returns machine-legible provenance for later summary/export layers.
    #[must_use]
    pub fn provenance(&self) -> &AppleAdapterExecutionProvenance {
        &self.provenance
    }

    /// Returns the packed Apple training batches.
    #[must_use]
    pub fn batches(&self) -> &[AppleAdapterPackedTrainingBatch] {
        self.batches.as_slice()
    }

    /// Builds the initial trainable adapter groups with frozen-base semantics.
    pub fn initial_training_groups(
        &self,
    ) -> Result<Vec<TrainingParameterGroupState>, AppleAdapterTrainingExecutionError> {
        let mut groups = Vec::with_capacity(self.config.model.targets.len() * 2);
        for target in &self.config.model.targets {
            let a_spec = lora_a_spec(self.config.model.output_width, target.lora_rank);
            groups.push(TrainingParameterGroupState::new(
                target.lora_a_group_id(),
                TrainingParameterClass::Matrix,
                TrainingTensorBuffer::from_f32(
                    target.lora_a_group_id(),
                    a_spec,
                    seeded_matrix(
                        format!(
                            "{}|{}|lora_a",
                            self.config.model.base_model_signature, target.target_id
                        )
                        .as_str(),
                        self.config.model.output_width,
                        target.lora_rank,
                        0.01,
                    ),
                )?,
                target.optimizer.clone(),
                target.optimizer_residency_policy,
            )?);

            let b_spec = lora_b_spec(target.lora_rank, self.config.model.input_width);
            groups.push(TrainingParameterGroupState::new(
                target.lora_b_group_id(),
                TrainingParameterClass::Matrix,
                TrainingTensorBuffer::from_f32(
                    target.lora_b_group_id(),
                    b_spec,
                    seeded_matrix(
                        format!(
                            "{}|{}|lora_b",
                            self.config.model.base_model_signature, target.target_id
                        )
                        .as_str(),
                        target.lora_rank,
                        self.config.model.input_width,
                        0.02,
                    ),
                )?,
                target.optimizer.clone(),
                target.optimizer_residency_policy,
            )?);
        }
        Ok(groups)
    }

    /// Creates a fresh fixed-budget run seeded with trainable adapter-only groups.
    pub fn initialize_run(
        &self,
    ) -> Result<FixedBudgetTrainingRun, AppleAdapterTrainingExecutionError> {
        Ok(FixedBudgetTrainingRun::new(
            self.config.run_id.clone(),
            self.config.checkpoint_family.clone(),
            self.config.budget,
            self.initial_training_groups()?,
        )?)
    }

    /// Snapshots the current trainable groups from one fixed-budget run.
    pub fn snapshot_training_groups(
        &self,
        run: &FixedBudgetTrainingRun,
    ) -> Result<Vec<TrainingParameterGroupState>, AppleAdapterTrainingExecutionError> {
        let mut groups = Vec::with_capacity(self.config.model.targets.len() * 2);
        for target in &self.config.model.targets {
            groups.push(
                self.training_group(run, target.lora_a_group_id().as_str())?
                    .clone(),
            );
            groups.push(
                self.training_group(run, target.lora_b_group_id().as_str())?
                    .clone(),
            );
        }
        Ok(groups)
    }

    /// Produces one gradient batch for the requested packed Apple batch.
    pub fn produce_gradient_batch(
        &self,
        run: &FixedBudgetTrainingRun,
        batch_index: usize,
    ) -> Result<AppleAdapterGradientBatchRecord, AppleAdapterTrainingExecutionError> {
        let batch = self
            .batches
            .get(batch_index)
            .ok_or(AppleAdapterTrainingExecutionError::UnknownBatchIndex { batch_index })?;
        let mut gradients = BTreeMap::new();
        let mut gradient_norms_l2 = BTreeMap::new();
        let mut mean_loss = 0.0_f32;

        for target in &self.config.model.targets {
            let group_a_id = target.lora_a_group_id();
            let group_b_id = target.lora_b_group_id();
            let group_a = self.training_group(run, group_a_id.as_str())?;
            let group_b = self.training_group(run, group_b_id.as_str())?;
            let a_values = dense_values(group_a, group_a_id.as_str())?;
            let b_values = dense_values(group_b, group_b_id.as_str())?;
            let mut grad_a = vec![0.0_f32; a_values.len()];
            let mut grad_b = vec![0.0_f32; b_values.len()];

            for record in &batch.records {
                let forward = self.forward_sample(record, run)?;
                mean_loss += forward.loss;
                accumulate_target_gradients(
                    &mut grad_a,
                    &mut grad_b,
                    a_values,
                    b_values,
                    target,
                    &record.prompt_features,
                    forward.residual.as_slice(),
                );
            }

            let scale = 1.0_f32 / batch.records.len() as f32;
            for value in &mut grad_a {
                *value *= scale;
            }
            for value in &mut grad_b {
                *value *= scale;
            }
            gradient_norms_l2.insert(group_a_id.clone(), l2_norm(grad_a.as_slice()));
            gradient_norms_l2.insert(group_b_id.clone(), l2_norm(grad_b.as_slice()));
            gradients.insert(
                group_a_id.clone(),
                TrainingTensorBuffer::from_f32(
                    group_a_id.clone(),
                    group_a.parameter.spec.clone(),
                    grad_a,
                )?,
            );
            gradients.insert(
                group_b_id.clone(),
                TrainingTensorBuffer::from_f32(
                    group_b_id.clone(),
                    group_b.parameter.spec.clone(),
                    grad_b,
                )?,
            );
        }

        mean_loss /= batch.records.len() as f32;
        let training_batch = TrainingGradientBatch::new(
            format!("{}-gradient", batch.batch_id),
            mean_loss,
            batch.records.len() as u32,
            gradients,
        );
        let execution_digest = stable_gradient_execution_digest(
            batch.batch_id.as_str(),
            batch.batch_digest.as_str(),
            &gradient_norms_l2,
            mean_loss,
        );
        Ok(AppleAdapterGradientBatchRecord {
            batch_id: batch.batch_id.clone(),
            batch_digest: batch.batch_digest.clone(),
            training_batch,
            mean_loss,
            gradient_norms_l2,
            execution_digest,
        })
    }

    /// Produces one typed step input for the fixed-budget core.
    pub fn produce_step_input(
        &self,
        run: &FixedBudgetTrainingRun,
        batch_index: usize,
        started_at_ms: u64,
        finished_at_ms: u64,
    ) -> Result<
        (TrainingStepInput, AppleAdapterGradientBatchRecord),
        AppleAdapterTrainingExecutionError,
    > {
        let gradient_record = self.produce_gradient_batch(run, batch_index)?;
        Ok((
            TrainingStepInput::new(
                gradient_record.training_batch.clone(),
                started_at_ms,
                finished_at_ms,
            ),
            gradient_record,
        ))
    }

    fn training_group<'a>(
        &self,
        run: &'a FixedBudgetTrainingRun,
        group_id: &str,
    ) -> Result<&'a TrainingParameterGroupState, AppleAdapterTrainingExecutionError> {
        run.parameter_group(group_id).ok_or_else(|| {
            AppleAdapterTrainingExecutionError::MissingParameterGroup {
                group_id: String::from(group_id),
            }
        })
    }

    fn forward_sample(
        &self,
        record: &AppleAdapterBatchFeatureRecord,
        run: &FixedBudgetTrainingRun,
    ) -> Result<ForwardRecord, AppleAdapterTrainingExecutionError> {
        let mut prediction = mat_vec(
            self.frozen_base_projection.as_slice(),
            self.config.model.output_width,
            self.config.model.input_width,
            record.prompt_features.as_slice(),
        );
        for target in &self.config.model.targets {
            let group_a = self.training_group(run, target.lora_a_group_id().as_str())?;
            let group_b = self.training_group(run, target.lora_b_group_id().as_str())?;
            let a_values = dense_values(group_a, target.lora_a_group_id().as_str())?;
            let b_values = dense_values(group_b, target.lora_b_group_id().as_str())?;
            let low_rank = mat_vec(
                b_values,
                target.lora_rank,
                self.config.model.input_width,
                record.prompt_features.as_slice(),
            );
            let delta = mat_vec(
                a_values,
                self.config.model.output_width,
                target.lora_rank,
                low_rank.as_slice(),
            );
            add_scaled(prediction.as_mut_slice(), delta.as_slice(), target.scale());
        }
        let residual = prediction
            .iter()
            .zip(record.target_features.as_slice())
            .map(|(prediction_value, target_value)| prediction_value - target_value)
            .collect::<Vec<_>>();
        let loss = 0.5 * residual.iter().map(|value| value * value).sum::<f32>();
        Ok(ForwardRecord { residual, loss })
    }
}

#[derive(Clone, Debug)]
struct ForwardRecord {
    residual: Vec<f32>,
    loss: f32,
}

/// Error surfaced by the repo-owned Apple backend.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AppleAdapterTrainingExecutionError {
    #[error("Apple adapter execution config is missing `run_id`")]
    MissingRunId,
    #[error("Apple adapter execution config is missing `checkpoint_family`")]
    MissingCheckpointFamily,
    #[error("Apple adapter execution config must use a non-zero fixed budget")]
    InvalidBudget,
    #[error("Apple adapter reference model is missing `base_model_signature`")]
    MissingBaseModelSignature,
    #[error("Apple adapter reference model is missing `tokenizer_digest`")]
    MissingTokenizerDigest,
    #[error("Apple adapter reference model is missing `prompt_shaping_digest`")]
    MissingPromptShapingDigest,
    #[error(
        "Apple adapter reference model requires positive input/output widths; found {input_width}x{output_width}"
    )]
    InvalidFeatureWidth {
        input_width: usize,
        output_width: usize,
    },
    #[error("Apple adapter reference model requires at least one trainable target")]
    MissingTrainableTargets,
    #[error("Apple adapter trainable target is missing `target_id`")]
    MissingTargetId,
    #[error("Apple adapter trainable target `{target_id}` uses invalid LoRA rank `{rank}`")]
    InvalidLoraRank { target_id: String, rank: usize },
    #[error("Apple adapter trainable target `{target_id}` uses invalid LoRA alpha `{alpha}`")]
    InvalidLoraAlpha { target_id: String, alpha: f32 },
    #[error("Apple adapter backend does not yet support precision policy `{0:?}`")]
    UnsupportedPrecisionPolicy(AppleAdapterPrecisionPolicy),
    #[error("Apple adapter backend does not yet support activation-checkpoint policy `{0:?}`")]
    UnsupportedActivationCheckpointPolicy(AppleAdapterActivationCheckpointPolicy),
    #[error("Apple adapter environment bundle does not expose an SFT-capable core package")]
    EnvironmentMissingSftWorkload,
    #[error("Apple adapter tokenizer digest mismatch: expected `{expected}`, found `{actual}`")]
    TokenizerDigestMismatch { expected: String, actual: String },
    #[error(
        "Apple adapter prompt-shaping digest mismatch: expected `{expected}`, found `{actual}`"
    )]
    PromptShapingDigestMismatch { expected: String, actual: String },
    #[error("Apple adapter execution requested unknown batch index `{batch_index}`")]
    UnknownBatchIndex { batch_index: usize },
    #[error("Apple adapter execution is missing parameter group `{group_id}`")]
    MissingParameterGroup { group_id: String },
    #[error("Apple adapter parameter group `{group_id}` must use dense `f32` values")]
    NonDenseGroup { group_id: String },
    #[error(transparent)]
    Dataset(#[from] psionic_data::AppleAdapterDatasetError),
    #[error(transparent)]
    Environment(#[from] AppleAdapterEnvironmentError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
}

fn build_packed_batches(
    dataset: &AppleAdapterDatasetContract,
    captures: &[AppleAdapterSampleTokenCapture],
    packing_plan: &DatasetPackingPlan,
    input_width: usize,
    output_width: usize,
) -> Result<Vec<AppleAdapterPackedTrainingBatch>, AppleAdapterTrainingExecutionError> {
    let sample_by_id = dataset
        .samples
        .iter()
        .map(|sample| (sample.sample_id.clone(), sample))
        .collect::<BTreeMap<_, _>>();
    let capture_by_id = captures
        .iter()
        .map(|capture| (capture.sample_id.clone(), capture))
        .collect::<BTreeMap<_, _>>();
    let mut batches = Vec::with_capacity(packing_plan.batches.len());
    for packed_batch in &packing_plan.batches {
        let mut records = Vec::new();
        let mut total_prompt_tokens = 0_u32;
        let mut total_completion_tokens = 0_u32;
        for row in &packed_batch.rows {
            for sequence in &row.source_sequences {
                let sample = sample_by_id
                    .get(sequence.sequence_id.as_str())
                    .expect("packing plan only references known samples");
                let capture = capture_by_id
                    .get(sequence.sequence_id.as_str())
                    .expect("packing plan only references known captures");
                total_prompt_tokens = total_prompt_tokens.saturating_add(
                    capture.prompt_tokens + capture.tool_tokens + capture.response_schema_tokens,
                );
                total_completion_tokens =
                    total_completion_tokens.saturating_add(capture.completion_tokens);
                records.push(AppleAdapterBatchFeatureRecord {
                    sample_id: sample.sample_id.clone(),
                    sample_kind: sample.sample_kind,
                    prompt_features: prompt_feature_vector(sample, capture, input_width),
                    target_features: target_feature_vector(sample, capture, output_width),
                    prompt_tokens: capture.prompt_tokens,
                    completion_tokens: capture.completion_tokens,
                    feature_digest: stable_feature_digest(
                        sample,
                        capture,
                        input_width,
                        output_width,
                    ),
                });
            }
        }
        let batch_digest = stable_batch_digest(
            packed_batch.batch_id.as_str(),
            records
                .iter()
                .map(|record| record.feature_digest.as_str())
                .collect::<Vec<_>>()
                .as_slice(),
        );
        batches.push(AppleAdapterPackedTrainingBatch {
            batch_id: packed_batch.batch_id.clone(),
            records,
            total_prompt_tokens,
            total_completion_tokens,
            batch_digest,
        });
    }
    Ok(batches)
}

fn prompt_feature_vector(
    sample: &psionic_data::AppleAdapterTrainingSample,
    capture: &AppleAdapterSampleTokenCapture,
    width: usize,
) -> Vec<f32> {
    let mut features = vec![0.0_f32; width];
    let prompt_scale = 1.0_f32 / capture.prompt_tokens.max(1) as f32;
    for message in prompt_messages(sample).iter() {
        accumulate_bucketed_text(
            features.as_mut_slice(),
            format!("{}:{}", role_label(message.role), message.content).as_str(),
            prompt_scale,
        );
        if let Some(response_format) = &message.response_format {
            accumulate_bucketed_text(
                features.as_mut_slice(),
                canonical_json(response_format).as_str(),
                capture.response_schema_tokens.max(1) as f32 / capture.total_tokens().max(1) as f32,
            );
        }
        if !message.tools.is_empty() {
            accumulate_bucketed_text(
                features.as_mut_slice(),
                canonical_json(&message.tools).as_str(),
                capture.tool_tokens.max(1) as f32 / capture.total_tokens().max(1) as f32,
            );
        }
    }
    features[sample_kind_bucket(sample.sample_kind, width)] += 0.5;
    normalize_unit(features.as_mut_slice());
    features
}

fn target_feature_vector(
    sample: &psionic_data::AppleAdapterTrainingSample,
    capture: &AppleAdapterSampleTokenCapture,
    width: usize,
) -> Vec<f32> {
    let mut features = vec![0.0_f32; width];
    let assistant = sample
        .messages
        .last()
        .expect("validated Apple samples always end with assistant");
    let scale = 1.0_f32 / capture.completion_tokens.max(1) as f32;
    accumulate_bucketed_text(features.as_mut_slice(), assistant.content.as_str(), scale);
    if let Some(structured) = &sample.structured_assistant_output {
        accumulate_bucketed_text(
            features.as_mut_slice(),
            canonical_json(structured).as_str(),
            scale,
        );
    }
    features[sample_kind_bucket(sample.sample_kind, width)] += 0.5;
    normalize_unit(features.as_mut_slice());
    features
}

fn prompt_messages(sample: &psionic_data::AppleAdapterTrainingSample) -> Vec<&AppleAdapterMessage> {
    sample
        .messages
        .iter()
        .take(sample.messages.len().saturating_sub(1))
        .collect()
}

fn sample_kind_bucket(sample_kind: AppleAdapterSampleKind, width: usize) -> usize {
    let index = match sample_kind {
        AppleAdapterSampleKind::SupervisedFineTune => 0,
        AppleAdapterSampleKind::SchemaFreeGuidedGeneration => 1,
        AppleAdapterSampleKind::GuidedGenerationWithSchema => 2,
        AppleAdapterSampleKind::ToolCalling => 3,
    };
    index % width.max(1)
}

fn role_label(role: AppleAdapterMessageRole) -> &'static str {
    match role {
        AppleAdapterMessageRole::System => "system",
        AppleAdapterMessageRole::User => "user",
        AppleAdapterMessageRole::Assistant => "assistant",
    }
}

fn accumulate_target_gradients(
    grad_a: &mut [f32],
    grad_b: &mut [f32],
    a_values: &[f32],
    _b_values: &[f32],
    target: &AppleAdapterTrainableTarget,
    prompt_features: &[f32],
    residual: &[f32],
) {
    let rank = target.lora_rank;
    let scale = target.scale();
    let low_rank = mat_vec(&_b_values, rank, prompt_features.len(), prompt_features);
    for output_index in 0..residual.len() {
        for rank_index in 0..rank {
            grad_a[output_index * rank + rank_index] +=
                residual[output_index] * low_rank[rank_index] * scale;
        }
    }
    let mut propagated = vec![0.0_f32; rank];
    for rank_index in 0..rank {
        for output_index in 0..residual.len() {
            propagated[rank_index] +=
                a_values[output_index * rank + rank_index] * residual[output_index] * scale;
        }
    }
    for rank_index in 0..rank {
        for input_index in 0..prompt_features.len() {
            grad_b[rank_index * prompt_features.len() + input_index] +=
                propagated[rank_index] * prompt_features[input_index];
        }
    }
}

fn dense_values<'a>(
    group: &'a TrainingParameterGroupState,
    group_id: &str,
) -> Result<&'a [f32], AppleAdapterTrainingExecutionError> {
    match &group.parameter.data {
        psionic_core::TensorData::F32(values) => Ok(values.as_slice()),
        psionic_core::TensorData::QuantizedBlocks(_) => {
            Err(AppleAdapterTrainingExecutionError::NonDenseGroup {
                group_id: String::from(group_id),
            })
        }
    }
}

fn lora_a_spec(output_width: usize, rank: usize) -> TensorSpec {
    TensorSpec::new(
        Shape::new(vec![output_width, rank]),
        DType::F32,
        Device::cpu(),
    )
}

fn lora_b_spec(rank: usize, input_width: usize) -> TensorSpec {
    TensorSpec::new(
        Shape::new(vec![rank, input_width]),
        DType::F32,
        Device::cpu(),
    )
}

fn mat_vec(matrix: &[f32], rows: usize, cols: usize, vector: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0_f32; rows];
    for row in 0..rows {
        let mut total = 0.0_f32;
        for col in 0..cols {
            total += matrix[row * cols + col] * vector[col];
        }
        out[row] = total;
    }
    out
}

fn add_scaled(dst: &mut [f32], src: &[f32], scale: f32) {
    for (left, right) in dst.iter_mut().zip(src) {
        *left += right * scale;
    }
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn normalize_unit(values: &mut [f32]) {
    let norm = l2_norm(values);
    if norm == 0.0 {
        if let Some(first) = values.first_mut() {
            *first = 1.0;
        }
        return;
    }
    for value in values {
        *value /= norm;
    }
}

fn accumulate_bucketed_text(out: &mut [f32], text: &str, scale: f32) {
    let tokens = if text.split_whitespace().next().is_some() {
        text.split_whitespace()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    } else {
        vec![String::from(text)]
    };
    for token in tokens {
        let digest = Sha256::digest(token.as_bytes());
        let index = (((digest[0] as usize) << 8) | digest[1] as usize) % out.len().max(1);
        let sign = if digest[2] & 1 == 0 { 1.0 } else { -1.0 };
        out[index] += sign * scale;
    }
}

fn seeded_matrix(seed: &str, rows: usize, cols: usize, scale: f32) -> Vec<f32> {
    (0..rows * cols)
        .map(|index| {
            let digest = Sha256::digest(format!("{seed}|{index}").as_bytes());
            let raw = u16::from_le_bytes([digest[0], digest[1]]) as f32 / u16::MAX as f32;
            ((raw * 2.0) - 1.0) * scale
        })
        .collect()
}

fn stable_feature_digest(
    sample: &psionic_data::AppleAdapterTrainingSample,
    capture: &AppleAdapterSampleTokenCapture,
    input_width: usize,
    output_width: usize,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_apple_adapter_feature_record|");
    hasher.update(sample.stable_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(capture.sample_id.as_bytes());
    hasher.update(b"|");
    hasher.update(capture.total_tokens().to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(input_width.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(output_width.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_batch_digest(batch_id: &str, feature_digests: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_apple_adapter_packed_batch|");
    hasher.update(batch_id.as_bytes());
    for digest in feature_digests {
        hasher.update(b"|");
        hasher.update(digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_gradient_execution_digest(
    batch_id: &str,
    batch_digest: &str,
    gradient_norms_l2: &BTreeMap<String, f32>,
    mean_loss: f32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_apple_adapter_gradient_execution|");
    hasher.update(batch_id.as_bytes());
    hasher.update(b"|");
    hasher.update(batch_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(mean_loss.to_bits().to_le_bytes());
    for (group_id, norm) in gradient_norms_l2 {
        hasher.update(b"|group|");
        hasher.update(group_id.as_bytes());
        hasher.update(b"|");
        hasher.update(norm.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn canonical_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| String::from("{}"))
}

#[cfg(test)]
mod tests {
    use psionic_data::{
        AppleAdapterDatasetMetadata, AppleAdapterSampleTokenCapture, DatasetKey,
        DatasetPackingMode, OverlongSequencePosture, TokenizerDigest, TokenizerFamily,
    };
    use psionic_environments::{
        AppleAdapterEnvironmentPackageRefs, AppleAdapterEnvironmentRuntimeRequirements,
        AppleAdapterEnvironmentSpec, EnvironmentArtifactExpectation, EnvironmentDatasetBinding,
        EnvironmentDifficultyMetadata, EnvironmentPolicyKind, EnvironmentPolicyReference,
        EnvironmentRubricHook, EnvironmentRubricScoreKind, EnvironmentToolContract,
        EnvironmentToolInterface,
    };

    use super::*;

    fn dataset_metadata() -> AppleAdapterDatasetMetadata {
        AppleAdapterDatasetMetadata::new(
            TokenizerDigest::new(
                TokenizerFamily::SentencePiece,
                "apple-tokenizer-digest-v1",
                32_768,
            )
            .with_special_tokens_digest("apple-special-tokens-v1")
            .with_template_digest("apple-template-v1"),
            "apple-prompt-shaping-v1",
        )
        .with_default_instruction("A conversation between a user and a helpful assistant.")
        .with_locale("en-US")
    }

    fn dataset() -> AppleAdapterDatasetContract {
        let input = format!(
            "{}\n{}\n{}",
            include_str!("../../fixtures/apple_adapter/datasets/minimal_sft_train.jsonl").trim(),
            include_str!(
                "../../fixtures/apple_adapter/datasets/guided_generation_with_schema_train.jsonl"
            )
            .trim(),
            include_str!("../../fixtures/apple_adapter/datasets/tool_calling_train.jsonl").trim()
        );
        AppleAdapterDatasetContract::from_jsonl_str(input.as_str(), dataset_metadata())
            .expect("dataset should import")
    }

    fn captures(dataset: &AppleAdapterDatasetContract) -> Vec<AppleAdapterSampleTokenCapture> {
        vec![
            AppleAdapterSampleTokenCapture::new(
                dataset.samples[0].sample_id.clone(),
                dataset.metadata.tokenizer.tokenizer_digest.clone(),
                dataset.metadata.prompt_shaping_digest.clone(),
                28,
                18,
            ),
            AppleAdapterSampleTokenCapture::new(
                dataset.samples[1].sample_id.clone(),
                dataset.metadata.tokenizer.tokenizer_digest.clone(),
                dataset.metadata.prompt_shaping_digest.clone(),
                30,
                14,
            )
            .with_response_schema_tokens(22),
            AppleAdapterSampleTokenCapture::new(
                dataset.samples[2].sample_id.clone(),
                dataset.metadata.tokenizer.tokenizer_digest.clone(),
                dataset.metadata.prompt_shaping_digest.clone(),
                26,
                16,
            )
            .with_tool_tokens(24),
        ]
    }

    fn environment_bundle() -> AppleAdapterEnvironmentBundle {
        AppleAdapterEnvironmentSpec {
            version: String::from("2026.03.15"),
            display_name: String::from("Apple Adapter Train"),
            core_environment_ref: String::from("env.openagents.apple_adapter.helpdesk.core"),
            benchmark_environment_ref: String::from(
                "env.openagents.apple_adapter.helpdesk.benchmark",
            ),
            train_dataset: EnvironmentDatasetBinding {
                dataset: DatasetKey::new("dataset://openagents/apple-train", "2026.03.15"),
                split: Some(String::from("train")),
                mount_path: String::from("/datasets/apple/train"),
                required: true,
            },
            held_out_eval_dataset: EnvironmentDatasetBinding {
                dataset: DatasetKey::new("dataset://openagents/apple-train", "2026.03.15"),
                split: Some(String::from("held_out")),
                mount_path: String::from("/datasets/apple/held_out"),
                required: true,
            },
            benchmark_dataset: None,
            package_refs: AppleAdapterEnvironmentPackageRefs {
                group_ref: String::from("group.apple.train"),
                core_pin_alias: String::from("apple_train_core"),
                benchmark_pin_alias: String::from("apple_train_benchmark"),
                core_member_ref: String::from("apple_train_core_member"),
                benchmark_member_ref: String::from("apple_train_benchmark_member"),
                session_profile_ref: String::from("session://apple/train"),
                runtime_profile_ref: String::from("runtime://apple/fm"),
                tool_bundle_ref: String::from("tools://apple/train"),
                rubric_binding_ref: String::from("rubric://apple/train"),
                structured_output_profile_ref: Some(String::from("structured://apple/train")),
                benchmark_profile_ref: String::from("benchmark://apple/train/default"),
                benchmark_runtime_profile_ref: String::from("runtime://apple/train/benchmark"),
            },
            runtime_requirements: AppleAdapterEnvironmentRuntimeRequirements {
                foundation_bridge_ref: String::from("bridge://apple-foundation-models"),
                model_id: String::from("apple-foundation-model"),
                platform_requirement: String::from("macos26_apple_silicon"),
                adapter_inventory_required: true,
                session_attach_required: true,
                structured_output_supported: true,
                tool_calling_supported: true,
                max_context_tokens: 4096,
                max_session_turns: 4,
                time_budget_ms: 30_000,
            },
            tools: vec![EnvironmentToolContract {
                tool_name: String::from("lookup_order"),
                interface: EnvironmentToolInterface::NativeFunction,
                description: String::from("Lookup one order"),
                args_schema: serde_json::json!({"type": "object"}),
                result_schema: None,
            }],
            rubric_hooks: vec![EnvironmentRubricHook {
                rubric_ref: String::from("rubric://apple/train/answer"),
                hook_name: String::from("score_answer"),
                score_kind: EnvironmentRubricScoreKind::Scalar,
                pass_threshold: Some(8000),
            }],
            expected_artifacts: vec![EnvironmentArtifactExpectation {
                artifact_kind: String::from("train_trace.json"),
                required: true,
                verification_policy_ref: Some(String::from("verify://apple/train/trace")),
            }],
            core_policy_references: vec![EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Training,
                policy_ref: String::from("policy://apple/train"),
                required: true,
            }],
            benchmark_policy_references: vec![EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://apple/train/benchmark"),
                required: true,
            }],
            difficulty: Some(EnvironmentDifficultyMetadata {
                difficulty_tier: String::from("narrow"),
                min_agent_level: Some(1),
                tags: vec![String::from("apple_adapter")],
            }),
        }
        .build_bundle()
        .expect("environment bundle should build")
    }

    fn config() -> AppleAdapterExecutionConfig {
        AppleAdapterExecutionConfig {
            run_id: String::from("apple-train-run"),
            checkpoint_family: String::from("apple.adapter.reference"),
            budget: TrainingLoopBudget::new(2, 1, 1).expect("budget"),
            packing_policy: DatasetPackingPolicy::new(
                DatasetPackingMode::PackIntoContextWindow,
                96,
                192,
                2,
            )
            .with_pad_to_multiple_of(8)
            .with_overlong_sequence_posture(OverlongSequencePosture::Refuse),
            precision_policy: AppleAdapterPrecisionPolicy::F32Reference,
            activation_checkpoint_policy: AppleAdapterActivationCheckpointPolicy::Disabled,
            model: AppleAdapterReferenceModel {
                base_model_signature: String::from("9799725ff8e851184037110b422d891ad3b92ec1"),
                tokenizer_digest: String::from("apple-tokenizer-digest-v1"),
                prompt_shaping_digest: String::from("apple-prompt-shaping-v1"),
                input_width: 12,
                output_width: 8,
                targets: vec![
                    AppleAdapterTrainableTarget {
                        target_id: String::from("decoder.attn.q_proj"),
                        lora_rank: 4,
                        lora_alpha: 8.0,
                        optimizer: TrainingOptimizerConfig::adamw(0.05, 0.9, 0.99, 1e-8)
                            .with_gradient_clip_norm(1.0),
                        optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                    },
                    AppleAdapterTrainableTarget {
                        target_id: String::from("decoder.ffn.up_proj"),
                        lora_rank: 4,
                        lora_alpha: 8.0,
                        optimizer: TrainingOptimizerConfig::adamw(0.05, 0.9, 0.99, 1e-8)
                            .with_gradient_clip_norm(1.0),
                        optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                    },
                ],
            },
        }
    }

    #[test]
    fn apple_adapter_backend_produces_repo_owned_gradients_and_fixed_budget_steps()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset = dataset();
        let backend = AppleAdapterTrainingExecutionBackend::new(
            config(),
            &dataset,
            captures(&dataset).as_slice(),
            &environment_bundle(),
        )?;
        assert_eq!(backend.batches().len(), 2);
        assert_eq!(
            backend.provenance().environment_ref,
            "env.openagents.apple_adapter.helpdesk.core"
        );

        let mut run = backend.initialize_run()?;
        let (step_input, gradient_record) = backend.produce_step_input(&run, 0, 1_000, 1_040)?;
        assert!(!gradient_record.training_batch.gradients.is_empty());
        assert!(gradient_record.mean_loss > 0.0);
        assert!(
            gradient_record
                .gradient_norms_l2
                .values()
                .all(|norm| *norm > 0.0)
        );

        let receipt = run.apply_step(step_input)?;
        assert_eq!(
            receipt.execution_mode,
            crate::TrainingStepExecutionMode::ExplicitGradientBatch
        );
        assert_eq!(receipt.sample_count, 2);
        assert_eq!(run.completed_steps(), 1);

        let snapshot = backend.snapshot_training_groups(&run)?;
        let non_zero_groups = snapshot
            .iter()
            .filter(|group| match &group.parameter.data {
                psionic_core::TensorData::F32(values) => {
                    values.iter().any(|value| value.abs() > 0.0)
                }
                psionic_core::TensorData::QuantizedBlocks(_) => false,
            })
            .count();
        assert_eq!(non_zero_groups, 4);
        Ok(())
    }

    #[test]
    fn apple_adapter_backend_rejects_tokenizer_drift() {
        let dataset = dataset();
        let mut drifted = config();
        drifted.model.tokenizer_digest = String::from("other-tokenizer");
        let err = AppleAdapterTrainingExecutionBackend::new(
            drifted,
            &dataset,
            captures(&dataset).as_slice(),
            &environment_bundle(),
        )
        .expect_err("tokenizer drift should fail");
        assert!(matches!(
            err,
            AppleAdapterTrainingExecutionError::TokenizerDigestMismatch { .. }
        ));
    }
}
