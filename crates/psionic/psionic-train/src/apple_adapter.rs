use std::{collections::BTreeMap, path::Path};

use half::f16;
use psionic_adapters::{
    AppleFmAdapterPackage, AppleFmAdapterPackageError, AppleFmAdapterPackageMetadata,
};
use psionic_core::{DType, Device, Shape, TensorSpec};
use psionic_data::{
    AppleAdapterDatasetContract, AppleAdapterMessage, AppleAdapterMessageRole,
    AppleAdapterSampleKind, AppleAdapterSampleTokenCapture, DatasetPackingPlan,
    DatasetPackingPolicy, TokenizerDigest,
};
use psionic_environments::{
    AppleAdapterEnvironmentBundle, AppleAdapterEnvironmentError, EnvironmentWorkloadClass,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, ModelAdapterDelta, ModelIoArtifactReceipt, PortableModelBundle,
    PortableTokenizerAssetFormat, PortableTokenizerBinding, TrainingCoreError,
    TrainingGradientBatch, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, TrainingParameterClass, TrainingParameterGroupState,
    TrainingRunSummary, TrainingStepInput, TrainingStepReceipt, TrainingTensorBuffer,
};

const OPENAGENTS_APPLE_FMADAPTER_PACKAGE_FORMAT_VERSION: &str = "openagents.apple-fmadapter.v1";
const APPLE_ADAPTER_FIDELITY_PLAN_ID: &str = "openagents.apple.token_sequence_reference.v1";
/// Feature width used by the current live Rust-native Apple reference lane.
pub const APPLE_LIVE_REFERENCE_FEATURE_WIDTH: usize = 2048;
/// LoRA rank used by the current live Rust-native Apple reference lane.
pub const APPLE_LIVE_REFERENCE_LORA_RANK: usize = 32;
const APPLE_RUNTIME_KV_FEATURE_WIDTH: usize = 256;
const APPLE_RUNTIME_FEED_FORWARD_WIDTH: usize = 6656;
const APPLE_RUNTIME_SEGMENT0_LAYER_COUNT: usize = 35;
const APPLE_RUNTIME_SEGMENT1_LAYER_COUNT: usize = 21;
const APPLE_LIVE_REFERENCE_SEGMENT0_START_LAYER: usize = 30;
const APPLE_LIVE_REFERENCE_SEGMENT1_START_LAYER: usize = 16;

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

/// Returns the current live Rust-native Apple target family used by the desktop operator.
#[must_use]
pub fn apple_live_reference_trainable_targets(
    optimizer: TrainingOptimizerConfig,
    optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
) -> Vec<AppleAdapterTrainableTarget> {
    let mut targets = Vec::new();
    for layer in APPLE_LIVE_REFERENCE_SEGMENT0_START_LAYER..APPLE_RUNTIME_SEGMENT0_LAYER_COUNT {
        for stem in [
            format!(
                "layers.segment_0.layer_{layer}.attention.qkv_transform.adapters.base_adapter.lora_0"
            ),
            format!(
                "layers.segment_0.layer_{layer}.attention.output_transform.adapters.base_adapter"
            ),
        ] {
            targets.push(AppleAdapterTrainableTarget {
                target_id: stem,
                lora_rank: APPLE_LIVE_REFERENCE_LORA_RANK,
                lora_alpha: APPLE_LIVE_REFERENCE_LORA_RANK as f32,
                optimizer: optimizer.clone(),
                optimizer_residency_policy,
            });
        }
    }
    for layer in APPLE_LIVE_REFERENCE_SEGMENT1_START_LAYER..APPLE_RUNTIME_SEGMENT1_LAYER_COUNT {
        for stem in [
            format!("layers.segment_1.layer_{layer}.attention.q_transform.adapters.base_adapter"),
            format!(
                "layers.segment_1.layer_{layer}.attention.output_transform.adapters.base_adapter"
            ),
        ] {
            targets.push(AppleAdapterTrainableTarget {
                target_id: stem,
                lora_rank: APPLE_LIVE_REFERENCE_LORA_RANK,
                lora_alpha: APPLE_LIVE_REFERENCE_LORA_RANK as f32,
                optimizer: optimizer.clone(),
                optimizer_residency_policy,
            });
        }
    }
    targets
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

/// Explicit fidelity statement for the current repo-owned Apple execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterExecutionFidelityPlan {
    /// Stable plan identifier.
    pub plan_id: String,
    /// Human-readable token encoder description.
    pub token_encoder: String,
    /// Human-readable prompt pooling description.
    pub prompt_pooling: String,
    /// Human-readable target supervision description.
    pub target_supervision: String,
    /// Faithful parts of the Apple-compatible path this backend now preserves.
    #[serde(default)]
    pub faithful_components: Vec<String>,
    /// Explicitly bounded or still-synthetic parts of the path.
    #[serde(default)]
    pub bounded_components: Vec<String>,
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
    /// Explicit first-real-run fidelity statement for this backend.
    pub fidelity_plan: AppleAdapterExecutionFidelityPlan,
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
                fidelity_plan: apple_adapter_execution_fidelity_plan(),
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
        Ok(ForwardRecord {
            prediction,
            residual,
            loss,
        })
    }
}

#[derive(Clone, Debug)]
struct ForwardRecord {
    prediction: Vec<f32>,
    residual: Vec<f32>,
    loss: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct AppleAdapterSftExecutionArtifacts {
    run: FixedBudgetTrainingRun,
    step_receipts: Vec<TrainingStepReceipt>,
    gradient_records: Vec<AppleAdapterGradientBatchRecord>,
    initial_bundle: PortableModelBundle,
    final_bundle: PortableModelBundle,
    initial_bundle_receipt: ModelIoArtifactReceipt,
    final_bundle_receipt: ModelIoArtifactReceipt,
    initial_bundle_bytes: Vec<u8>,
    final_bundle_bytes: Vec<u8>,
    runtime_asset_bytes: Vec<u8>,
    adapter_delta: ModelAdapterDelta,
    adapter_identifier: String,
}

/// Higher-level SFT/export request layered on top of the repo-owned Apple backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterSftRunRequest {
    /// Stable dataset ref carried into export lineage.
    pub dataset_ref: String,
    /// Stable benchmark refs carried into export lineage.
    #[serde(default)]
    pub benchmark_refs: Vec<String>,
    /// Stable validator policy ref carried into export lineage.
    pub validator_policy_ref: String,
    /// Stable package name or package stem for the final `.fmadapter`.
    pub package_name: String,
    /// Optional author label surfaced in package metadata.
    #[serde(default)]
    pub author: String,
    /// Optional description surfaced in package metadata.
    #[serde(default)]
    pub description: String,
    /// Optional license surfaced in package metadata.
    #[serde(default)]
    pub license: String,
    /// Logical training start timestamp for the first step.
    pub started_at_ms: u64,
    /// Duration applied to each produced step receipt.
    pub step_duration_ms: u64,
}

impl AppleAdapterSftRunRequest {
    fn validate(&self) -> Result<(), AppleAdapterSftError> {
        if self.dataset_ref.trim().is_empty() {
            return Err(AppleAdapterSftError::MissingDatasetRef);
        }
        if self.validator_policy_ref.trim().is_empty() {
            return Err(AppleAdapterSftError::MissingValidatorPolicyRef);
        }
        if self.package_name.trim().is_empty() {
            return Err(AppleAdapterSftError::MissingPackageName);
        }
        if self.step_duration_ms == 0 {
            return Err(AppleAdapterSftError::InvalidStepDuration);
        }
        for benchmark_ref in &self.benchmark_refs {
            if benchmark_ref.trim().is_empty() {
                return Err(AppleAdapterSftError::InvalidBenchmarkRef);
            }
        }
        Ok(())
    }
}

/// Typed training summary emitted by the higher-level Apple SFT lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterSftTrainingSummary {
    /// Fixed-budget training summary.
    pub run_summary: TrainingRunSummary,
    /// Repo-owned Apple execution provenance.
    pub execution_provenance: AppleAdapterExecutionProvenance,
    /// Stable dataset ref carried into export lineage.
    pub dataset_ref: String,
    /// Stable validator policy ref carried into export lineage.
    pub validator_policy_ref: String,
    /// Stable benchmark refs carried into export lineage.
    #[serde(default)]
    pub benchmark_refs: Vec<String>,
    /// Stable base-model signature the adapter targets.
    pub base_model_signature: String,
    /// Stable initial adapter state-dict digest.
    pub initial_state_dict_digest: String,
    /// Stable final adapter state-dict digest.
    pub final_state_dict_digest: String,
    /// Stable final package digest.
    pub package_digest: String,
    /// Stable final adapter identifier.
    pub adapter_identifier: String,
}

/// Full higher-level Apple SFT outcome including export artifacts.
#[derive(Clone, Debug, PartialEq)]
pub struct AppleAdapterSftRunOutcome {
    /// Step receipts emitted by the fixed-budget core.
    pub step_receipts: Vec<TrainingStepReceipt>,
    /// Gradient-production records emitted by the repo-owned Apple backend.
    pub gradient_records: Vec<AppleAdapterGradientBatchRecord>,
    /// Summary and reproducibility metadata.
    pub summary: AppleAdapterSftTrainingSummary,
    /// Initial adapter-only portable bundle snapshot.
    pub initial_bundle: PortableModelBundle,
    /// Final adapter-only portable bundle snapshot.
    pub final_bundle: PortableModelBundle,
    /// Initial adapter-only portable bundle receipt.
    pub initial_bundle_receipt: ModelIoArtifactReceipt,
    /// Final adapter-only portable bundle receipt.
    pub final_bundle_receipt: ModelIoArtifactReceipt,
    /// Typed adapter delta derived between the initial and final bundles.
    pub adapter_delta: ModelAdapterDelta,
    /// Final Apple package held in memory.
    pub adapter_package: AppleFmAdapterPackage,
}

impl AppleAdapterSftRunOutcome {
    /// Writes the final `.fmadapter` directory to disk.
    pub fn write_package_to_directory(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), AppleAdapterSftError> {
        self.adapter_package.write_to_directory(path)?;
        Ok(())
    }
}

/// Distillation request kept explicitly separate from the base SFT lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterDraftDistillationRequest {
    /// Stable draft-model identifier carried through export lineage.
    pub draft_model_id: String,
    /// Hidden width for the distilled draft network.
    pub hidden_width: usize,
    /// Optimizer config reused by the fixed-budget core.
    pub optimizer: TrainingOptimizerConfig,
    /// Residency policy reused by the fixed-budget core.
    pub optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
    /// Precision posture for the distilled draft path.
    pub draft_precision_policy: AppleAdapterPrecisionPolicy,
    /// Soft-target temperature applied to teacher logits.
    pub teacher_temperature: f32,
    /// Soft-target temperature applied to student logits.
    pub student_temperature: f32,
    /// Similarity threshold used for speculative acceptance accounting.
    pub acceptance_cosine_threshold: f32,
    /// Draft token count surfaced in the Apple package metadata.
    pub draft_token_count: u32,
    /// Logical training start timestamp for the first draft step.
    pub started_at_ms: u64,
    /// Duration applied to each produced draft step receipt.
    pub step_duration_ms: u64,
}

impl AppleAdapterDraftDistillationRequest {
    fn validate(&self) -> Result<(), AppleAdapterDraftDistillationError> {
        if self.draft_model_id.trim().is_empty() {
            return Err(AppleAdapterDraftDistillationError::MissingDraftModelId);
        }
        if self.hidden_width == 0 {
            return Err(AppleAdapterDraftDistillationError::InvalidDraftHiddenWidth);
        }
        if !self.teacher_temperature.is_finite() || self.teacher_temperature <= 0.0 {
            return Err(AppleAdapterDraftDistillationError::InvalidTeacherTemperature);
        }
        if !self.student_temperature.is_finite() || self.student_temperature <= 0.0 {
            return Err(AppleAdapterDraftDistillationError::InvalidStudentTemperature);
        }
        if !self.acceptance_cosine_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.acceptance_cosine_threshold)
        {
            return Err(AppleAdapterDraftDistillationError::InvalidAcceptanceCosineThreshold);
        }
        if self.draft_token_count == 0 {
            return Err(AppleAdapterDraftDistillationError::InvalidDraftTokenCount);
        }
        if self.step_duration_ms == 0 {
            return Err(AppleAdapterDraftDistillationError::InvalidStepDuration);
        }
        match self.draft_precision_policy {
            AppleAdapterPrecisionPolicy::F32Reference => Ok(()),
            unsupported => Err(
                AppleAdapterDraftDistillationError::UnsupportedDraftPrecisionPolicy(unsupported),
            ),
        }
    }
}

/// Explicit teacher/student runtime pairing for the draft lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterDraftRuntimePairing {
    /// Stable teacher base-model signature.
    pub teacher_base_model_signature: String,
    /// Stable teacher adapter identifier.
    pub teacher_adapter_identifier: String,
    /// Precision posture used by the teacher adapter lane.
    pub teacher_precision_policy: AppleAdapterPrecisionPolicy,
    /// Stable draft-model identifier.
    pub draft_model_id: String,
    /// Precision posture used by the draft lane.
    pub draft_precision_policy: AppleAdapterPrecisionPolicy,
}

/// One machine-legible batch record from the draft distillation lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterDraftDistillationBatchRecord {
    /// Stable batch identifier.
    pub batch_id: String,
    /// Stable digest of the packed batch that sourced the computation.
    pub batch_digest: String,
    /// Machine-legible gradient batch for the fixed-budget trainer core.
    pub training_batch: TrainingGradientBatch,
    /// Mean distillation loss over the packed batch.
    pub mean_distillation_loss: f32,
    /// Gradient norms keyed by parameter-group identifier.
    pub gradient_norms_l2: BTreeMap<String, f32>,
    /// Fraction of samples meeting the acceptance threshold.
    pub acceptance_ratio: f32,
    /// Mean estimated teacher latency over the packed batch.
    pub mean_teacher_latency_ms: u64,
    /// Mean estimated draft latency over the packed batch.
    pub mean_draft_latency_ms: u64,
    /// Stable digest over the batch distillation record.
    pub execution_digest: String,
}

/// Summary emitted by the optional draft distillation lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterDraftDistillationSummary {
    /// Teacher/draft runtime pairing used by the run.
    pub runtime_pairing: AppleAdapterDraftRuntimePairing,
    /// Fixed-budget training summary for the draft lane.
    pub run_summary: TrainingRunSummary,
    /// Mean distillation loss observed across batches.
    pub mean_distillation_loss: f32,
    /// Mean acceptance ratio observed across batches.
    pub acceptance_ratio: f32,
    /// Mean estimated teacher latency.
    pub mean_teacher_latency_ms: u64,
    /// Mean estimated draft latency.
    pub mean_draft_latency_ms: u64,
    /// Stable digest over the exported draft graph payload.
    pub draft_mil_digest: String,
    /// Stable artifact digest over the exported draft weights.
    pub draft_weights_artifact_digest: String,
    /// Stable state-dict digest over the exported draft weights.
    pub draft_weights_state_dict_digest: String,
    /// Stable final package digest that includes the draft payload.
    pub package_digest: String,
}

/// Full result of the optional draft distillation follow-on lane.
#[derive(Clone, Debug, PartialEq)]
pub struct AppleAdapterDraftDistillationOutcome {
    /// Base SFT outcome that produced the teacher adapter.
    pub sft_outcome: AppleAdapterSftRunOutcome,
    /// Step receipts emitted by the fixed-budget core for the draft lane.
    pub draft_step_receipts: Vec<TrainingStepReceipt>,
    /// Distillation batch records emitted by the repo-owned draft lane.
    pub draft_batch_records: Vec<AppleAdapterDraftDistillationBatchRecord>,
    /// Summary and reproducibility metadata for the draft lane.
    pub draft_summary: AppleAdapterDraftDistillationSummary,
    /// Exported portable draft-model bundle.
    pub draft_bundle: PortableModelBundle,
    /// Receipt for the exported draft weights.
    pub draft_bundle_receipt: ModelIoArtifactReceipt,
    /// Deterministic graph payload written to `draft.mil`.
    pub draft_mil_bytes: Vec<u8>,
    /// Final Apple package including the optional draft payload.
    pub adapter_package: AppleFmAdapterPackage,
}

impl AppleAdapterDraftDistillationOutcome {
    /// Writes the final `.fmadapter` directory to disk.
    pub fn write_package_to_directory(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), AppleAdapterDraftDistillationError> {
        self.adapter_package.write_to_directory(path)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppleRuntimeQkPermutation {
    n_heads: usize,
    dim1: usize,
    dim2: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppleRuntimeTensorTemplate {
    key_suffix: &'static str,
    rows: usize,
    cols: usize,
    qk_permutation: Option<AppleRuntimeQkPermutation>,
}

const APPLE_RUNTIME_SEGMENT0_TEMPLATES: [AppleRuntimeTensorTemplate; 14] = [
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.qkv_transform.adapters.base_adapter.lora_0.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.qkv_transform.adapters.base_adapter.lora_0.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        qk_permutation: Some(AppleRuntimeQkPermutation {
            n_heads: 16,
            dim1: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
            dim2: APPLE_LIVE_REFERENCE_LORA_RANK,
        }),
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.qkv_transform.adapters.base_adapter.lora_1.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.qkv_transform.adapters.base_adapter.lora_1.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_RUNTIME_KV_FEATURE_WIDTH,
        qk_permutation: Some(AppleRuntimeQkPermutation {
            n_heads: 2,
            dim1: APPLE_RUNTIME_KV_FEATURE_WIDTH,
            dim2: APPLE_LIVE_REFERENCE_LORA_RANK,
        }),
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.qkv_transform.adapters.base_adapter.lora_2.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.qkv_transform.adapters.base_adapter.lora_2.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_RUNTIME_KV_FEATURE_WIDTH,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.output_transform.adapters.base_adapter.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.output_transform.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.hidden_transform.linear_0.adapters.base_adapter.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.hidden_transform.linear_0.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_RUNTIME_FEED_FORWARD_WIDTH,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.hidden_transform.linear_1.adapters.base_adapter.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.hidden_transform.linear_1.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_RUNTIME_FEED_FORWARD_WIDTH,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.output_transform.adapters.base_adapter.a_transpose",
        rows: APPLE_RUNTIME_FEED_FORWARD_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.output_transform.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        qk_permutation: None,
    },
];

const APPLE_RUNTIME_SEGMENT1_TEMPLATES: [AppleRuntimeTensorTemplate; 10] = [
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.q_transform.adapters.base_adapter.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.q_transform.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        qk_permutation: Some(AppleRuntimeQkPermutation {
            n_heads: 16,
            dim1: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
            dim2: APPLE_LIVE_REFERENCE_LORA_RANK,
        }),
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.output_transform.adapters.base_adapter.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "attention.output_transform.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.hidden_transform.linear_0.adapters.base_adapter.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.hidden_transform.linear_0.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_RUNTIME_FEED_FORWARD_WIDTH,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.hidden_transform.linear_1.adapters.base_adapter.a_transpose",
        rows: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.hidden_transform.linear_1.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_RUNTIME_FEED_FORWARD_WIDTH,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.output_transform.adapters.base_adapter.a_transpose",
        rows: APPLE_RUNTIME_FEED_FORWARD_WIDTH,
        cols: APPLE_LIVE_REFERENCE_LORA_RANK,
        qk_permutation: None,
    },
    AppleRuntimeTensorTemplate {
        key_suffix: "feed_forward.output_transform.adapters.base_adapter.b_transpose",
        rows: APPLE_LIVE_REFERENCE_LORA_RANK,
        cols: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
        qk_permutation: None,
    },
];

/// Error surfaced while turning repo-owned LoRA groups into Apple runtime bytes.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AppleAdapterRuntimeAssetError {
    #[error("Apple runtime asset export does not support trained target `{target_id}`")]
    UnsupportedTargetId { target_id: String },
    #[error("Apple runtime asset export is missing training group `{group_id}`")]
    MissingTrainingGroup { group_id: String },
    #[error("Apple runtime asset export requires dense f32 values for `{group_id}`")]
    NonDenseTrainingGroup { group_id: String },
    #[error(
        "Apple runtime asset export shape mismatch for `{group_id}`: expected {expected_rows}x{expected_cols}, found {actual:?}"
    )]
    ShapeMismatch {
        group_id: String,
        expected_rows: usize,
        expected_cols: usize,
        actual: Vec<usize>,
    },
}

fn apple_runtime_tensor_templates() -> Vec<(String, AppleRuntimeTensorTemplate)> {
    let mut templates = Vec::with_capacity(
        APPLE_RUNTIME_SEGMENT0_LAYER_COUNT * APPLE_RUNTIME_SEGMENT0_TEMPLATES.len()
            + APPLE_RUNTIME_SEGMENT1_LAYER_COUNT * APPLE_RUNTIME_SEGMENT1_TEMPLATES.len(),
    );
    for layer in 0..APPLE_RUNTIME_SEGMENT0_LAYER_COUNT {
        for template in APPLE_RUNTIME_SEGMENT0_TEMPLATES {
            templates.push((
                format!("layers.segment_0.layer_{layer}.{}", template.key_suffix),
                template,
            ));
        }
    }
    for layer in 0..APPLE_RUNTIME_SEGMENT1_LAYER_COUNT {
        for template in APPLE_RUNTIME_SEGMENT1_TEMPLATES {
            templates.push((
                format!("layers.segment_1.layer_{layer}.{}", template.key_suffix),
                template,
            ));
        }
    }
    templates
}

fn export_native_apple_runtime_asset_bytes(
    backend: &AppleAdapterTrainingExecutionBackend,
    final_groups: &[TrainingParameterGroupState],
) -> Result<Vec<u8>, AppleAdapterRuntimeAssetError> {
    let templates = apple_runtime_tensor_templates();
    let template_by_key = templates
        .iter()
        .map(|(key, template)| (key.clone(), *template))
        .collect::<BTreeMap<_, _>>();
    for target in &backend.config().model.targets {
        for suffix in ["a_transpose", "b_transpose"] {
            let key = format!("{}.{}", target.target_id, suffix);
            if !template_by_key.contains_key(key.as_str()) {
                return Err(AppleAdapterRuntimeAssetError::UnsupportedTargetId {
                    target_id: target.target_id.clone(),
                });
            }
        }
    }

    let groups_by_id = final_groups
        .iter()
        .map(|group| (group.group_id.clone(), group))
        .collect::<BTreeMap<_, _>>();
    let mut bytes = Vec::new();
    for (checkpoint_key, template) in templates {
        let target_id = checkpoint_key
            .trim_end_matches(".a_transpose")
            .trim_end_matches(".b_transpose");
        let group_id = if checkpoint_key.ends_with(".a_transpose") {
            format!("{target_id}.lora_a")
        } else {
            format!("{target_id}.lora_b")
        };
        let values = if let Some(group) = groups_by_id.get(group_id.as_str()) {
            let actual = group.parameter.spec.shape().dims().to_vec();
            if actual != vec![template.rows, template.cols] {
                return Err(AppleAdapterRuntimeAssetError::ShapeMismatch {
                    group_id: group_id.clone(),
                    expected_rows: template.rows,
                    expected_cols: template.cols,
                    actual,
                });
            }
            dense_values(group, group_id.as_str())
                .map_err(|_| AppleAdapterRuntimeAssetError::NonDenseTrainingGroup {
                    group_id: group_id.clone(),
                })?
                .to_vec()
        } else {
            vec![0.0_f32; template.rows * template.cols]
        };
        let mut transformed = transpose_matrix(values.as_slice(), template.rows, template.cols);
        if let Some(permutation) = template.qk_permutation {
            transformed = permute_qk_matrix(
                transformed.as_slice(),
                permutation.n_heads,
                permutation.dim1,
                permutation.dim2,
            );
        }
        bytes.extend(encode_f16_bytes(transformed.as_slice()));
    }
    Ok(bytes)
}

fn transpose_matrix(values: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    let mut out = vec![0.0_f32; values.len()];
    for row in 0..rows {
        for col in 0..cols {
            out[col * rows + row] = values[row * cols + col];
        }
    }
    out
}

fn permute_qk_matrix(values: &[f32], n_heads: usize, dim1: usize, dim2: usize) -> Vec<f32> {
    let mut out = vec![0.0_f32; values.len()];
    let head_block = dim1 / n_heads / 2;
    for head in 0..n_heads {
        for block in 0..head_block {
            for pair in 0..2 {
                let src_row = head * head_block * 2 + block * 2 + pair;
                let dst_row = head * head_block * 2 + pair * head_block + block;
                let src_start = src_row * dim2;
                let dst_start = dst_row * dim2;
                out[dst_start..dst_start + dim2]
                    .copy_from_slice(&values[src_start..src_start + dim2]);
            }
        }
    }
    out
}

fn encode_f16_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * std::mem::size_of::<u16>());
    for value in values {
        bytes.extend_from_slice(&f16::from_f32(*value).to_bits().to_le_bytes());
    }
    bytes
}

/// Runs the first honest Rust-native Apple adapter SFT lane and returns a valid `.fmadapter`.
pub fn run_apple_adapter_sft_export(
    backend: &AppleAdapterTrainingExecutionBackend,
    dataset: &AppleAdapterDatasetContract,
    environment: &AppleAdapterEnvironmentBundle,
    request: &AppleAdapterSftRunRequest,
) -> Result<AppleAdapterSftRunOutcome, AppleAdapterSftError> {
    let artifacts = execute_apple_adapter_sft_artifacts(backend, dataset, environment, request)?;
    build_apple_adapter_sft_outcome(backend, dataset, environment, request, artifacts)
}

/// Runs the optional draft-model distillation follow-on lane and emits a package with draft payloads.
pub fn run_apple_adapter_draft_distillation_export(
    backend: &AppleAdapterTrainingExecutionBackend,
    dataset: &AppleAdapterDatasetContract,
    environment: &AppleAdapterEnvironmentBundle,
    sft_request: &AppleAdapterSftRunRequest,
    draft_request: &AppleAdapterDraftDistillationRequest,
) -> Result<AppleAdapterDraftDistillationOutcome, AppleAdapterDraftDistillationError> {
    draft_request.validate()?;
    let sft_artifacts =
        execute_apple_adapter_sft_artifacts(backend, dataset, environment, sft_request)?;
    let sft_outcome = build_apple_adapter_sft_outcome(
        backend,
        dataset,
        environment,
        sft_request,
        sft_artifacts.clone(),
    )?;
    let draft_artifacts = run_apple_adapter_draft_export_artifacts(
        backend,
        dataset,
        environment,
        draft_request,
        &sft_artifacts,
    )?;
    let adapter_package = build_apple_adapter_package(
        backend,
        dataset,
        environment,
        sft_request,
        &sft_artifacts,
        Some(&draft_artifacts),
    )?;
    let draft_summary = AppleAdapterDraftDistillationSummary {
        runtime_pairing: draft_artifacts.runtime_pairing.clone(),
        run_summary: draft_artifacts.run.summary(),
        mean_distillation_loss: mean_f32(
            draft_artifacts
                .batch_records
                .iter()
                .map(|record| record.mean_distillation_loss)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        acceptance_ratio: mean_f32(
            draft_artifacts
                .batch_records
                .iter()
                .map(|record| record.acceptance_ratio)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        mean_teacher_latency_ms: mean_u64(
            draft_artifacts
                .batch_records
                .iter()
                .map(|record| record.mean_teacher_latency_ms)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        mean_draft_latency_ms: mean_u64(
            draft_artifacts
                .batch_records
                .iter()
                .map(|record| record.mean_draft_latency_ms)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        draft_mil_digest: hex::encode(Sha256::digest(draft_artifacts.draft_mil_bytes.as_slice())),
        draft_weights_artifact_digest: draft_artifacts.draft_bundle_receipt.artifact_digest.clone(),
        draft_weights_state_dict_digest: draft_artifacts
            .draft_bundle_receipt
            .state_dict_digest
            .clone(),
        package_digest: adapter_package.package_digest.clone(),
    };
    Ok(AppleAdapterDraftDistillationOutcome {
        sft_outcome,
        draft_step_receipts: draft_artifacts.step_receipts,
        draft_batch_records: draft_artifacts.batch_records,
        draft_summary,
        draft_bundle: draft_artifacts.draft_bundle,
        draft_bundle_receipt: draft_artifacts.draft_bundle_receipt,
        draft_mil_bytes: draft_artifacts.draft_mil_bytes,
        adapter_package,
    })
}

fn tokenizer_binding(
    tokenizer_digest: TokenizerDigest,
    environment: &AppleAdapterEnvironmentBundle,
) -> PortableTokenizerBinding {
    PortableTokenizerBinding::new(
        tokenizer_digest,
        PortableTokenizerAssetFormat::PsionicDigest,
        environment.core_package.key.version.clone(),
    )
}

#[derive(Clone, Debug, PartialEq)]
struct AppleAdapterDraftExportArtifacts {
    run: FixedBudgetTrainingRun,
    step_receipts: Vec<TrainingStepReceipt>,
    batch_records: Vec<AppleAdapterDraftDistillationBatchRecord>,
    draft_bundle: PortableModelBundle,
    draft_bundle_receipt: ModelIoArtifactReceipt,
    draft_weights_bytes: Vec<u8>,
    draft_mil_bytes: Vec<u8>,
    runtime_pairing: AppleAdapterDraftRuntimePairing,
    draft_request: AppleAdapterDraftDistillationRequest,
}

fn execute_apple_adapter_sft_artifacts(
    backend: &AppleAdapterTrainingExecutionBackend,
    dataset: &AppleAdapterDatasetContract,
    environment: &AppleAdapterEnvironmentBundle,
    request: &AppleAdapterSftRunRequest,
) -> Result<AppleAdapterSftExecutionArtifacts, AppleAdapterSftError> {
    request.validate()?;
    let mut run = backend.initialize_run()?;
    let initial_groups = backend.snapshot_training_groups(&run)?;
    let mut step_receipts = Vec::new();
    let mut gradient_records = Vec::new();
    for step_index in 0..backend.config().budget.max_steps {
        let batch_index = step_index as usize % backend.batches().len().max(1);
        let started_at_ms =
            request.started_at_ms + step_index.saturating_mul(request.step_duration_ms);
        let finished_at_ms = started_at_ms + request.step_duration_ms;
        let (step_input, gradient_record) =
            backend.produce_step_input(&run, batch_index, started_at_ms, finished_at_ms)?;
        gradient_records.push(gradient_record);
        step_receipts.push(run.apply_step(step_input)?);
    }
    let final_groups = backend.snapshot_training_groups(&run)?;
    let tokenizer_binding = tokenizer_binding(dataset.metadata.tokenizer.clone(), environment);
    let initial_bundle = PortableModelBundle::from_training_groups(
        "apple_adapter_reference",
        backend.config().model.base_model_signature.clone(),
        backend.config().checkpoint_family.clone(),
        Some(format!("checkpoint://{}/initial", backend.config().run_id)),
        initial_groups.as_slice(),
        tokenizer_binding.clone(),
        dataset
            .metadata
            .tokenizer
            .template_digest
            .clone()
            .or_else(|| Some(backend.config().model.prompt_shaping_digest.clone())),
    )?;
    let final_bundle = PortableModelBundle::from_training_groups(
        "apple_adapter_reference",
        backend.config().model.base_model_signature.clone(),
        backend.config().checkpoint_family.clone(),
        Some(format!("checkpoint://{}/final", backend.config().run_id)),
        final_groups.as_slice(),
        tokenizer_binding,
        dataset
            .metadata
            .tokenizer
            .template_digest
            .clone()
            .or_else(|| Some(backend.config().model.prompt_shaping_digest.clone())),
    )?;
    let (initial_bundle_bytes, initial_bundle_receipt) = initial_bundle.export_safetensors()?;
    let (final_bundle_bytes, final_bundle_receipt) = final_bundle.export_safetensors()?;
    let runtime_asset_bytes =
        export_native_apple_runtime_asset_bytes(&backend, final_groups.as_slice())?;
    let adapter_identifier = stable_adapter_identifier(
        request.package_name.as_str(),
        final_bundle_receipt.artifact_digest.as_str(),
    );
    let adapter_delta = crate::PortableModelStateDict::derive_adapter_delta(
        &initial_bundle.state_dict,
        &final_bundle.state_dict,
        adapter_identifier.clone(),
    )?;
    Ok(AppleAdapterSftExecutionArtifacts {
        run,
        step_receipts,
        gradient_records,
        initial_bundle,
        final_bundle,
        initial_bundle_receipt,
        final_bundle_receipt,
        initial_bundle_bytes,
        final_bundle_bytes,
        runtime_asset_bytes,
        adapter_delta,
        adapter_identifier,
    })
}

fn build_apple_adapter_sft_outcome(
    backend: &AppleAdapterTrainingExecutionBackend,
    dataset: &AppleAdapterDatasetContract,
    environment: &AppleAdapterEnvironmentBundle,
    request: &AppleAdapterSftRunRequest,
    artifacts: AppleAdapterSftExecutionArtifacts,
) -> Result<AppleAdapterSftRunOutcome, AppleAdapterSftError> {
    let package =
        build_apple_adapter_package(backend, dataset, environment, request, &artifacts, None)?;
    let summary = AppleAdapterSftTrainingSummary {
        run_summary: artifacts.run.summary(),
        execution_provenance: backend.provenance().clone(),
        dataset_ref: request.dataset_ref.clone(),
        validator_policy_ref: request.validator_policy_ref.clone(),
        benchmark_refs: request.benchmark_refs.clone(),
        base_model_signature: backend.config().model.base_model_signature.clone(),
        initial_state_dict_digest: artifacts.initial_bundle_receipt.state_dict_digest.clone(),
        final_state_dict_digest: artifacts.final_bundle_receipt.state_dict_digest.clone(),
        package_digest: package.package_digest.clone(),
        adapter_identifier: artifacts.adapter_identifier.clone(),
    };
    Ok(AppleAdapterSftRunOutcome {
        step_receipts: artifacts.step_receipts,
        gradient_records: artifacts.gradient_records,
        summary,
        initial_bundle: artifacts.initial_bundle,
        final_bundle: artifacts.final_bundle,
        initial_bundle_receipt: artifacts.initial_bundle_receipt,
        final_bundle_receipt: artifacts.final_bundle_receipt,
        adapter_delta: artifacts.adapter_delta,
        adapter_package: package,
    })
}

fn build_apple_adapter_package(
    backend: &AppleAdapterTrainingExecutionBackend,
    dataset: &AppleAdapterDatasetContract,
    environment: &AppleAdapterEnvironmentBundle,
    request: &AppleAdapterSftRunRequest,
    sft_artifacts: &AppleAdapterSftExecutionArtifacts,
    draft_artifacts: Option<&AppleAdapterDraftExportArtifacts>,
) -> Result<AppleFmAdapterPackage, AppleFmAdapterPackageError> {
    let package_name = normalized_package_name(
        request.package_name.as_str(),
        sft_artifacts.adapter_identifier.as_str(),
    );
    let metadata = export_metadata(
        backend,
        dataset.metadata.tokenizer.clone(),
        environment,
        request,
        sft_artifacts.adapter_identifier.as_str(),
        &sft_artifacts.initial_bundle_receipt,
        &sft_artifacts.final_bundle_receipt,
        draft_artifacts,
    );
    AppleFmAdapterPackage::new(
        package_name,
        metadata,
        sft_artifacts.runtime_asset_bytes.clone(),
        draft_artifacts.map(|artifacts| artifacts.draft_mil_bytes.clone()),
        draft_artifacts.map(|artifacts| artifacts.draft_weights_bytes.clone()),
    )
}

fn export_metadata(
    backend: &AppleAdapterTrainingExecutionBackend,
    tokenizer_digest: TokenizerDigest,
    environment: &AppleAdapterEnvironmentBundle,
    request: &AppleAdapterSftRunRequest,
    adapter_identifier: &str,
    initial_bundle_receipt: &ModelIoArtifactReceipt,
    final_bundle_receipt: &ModelIoArtifactReceipt,
    draft_artifacts: Option<&AppleAdapterDraftExportArtifacts>,
) -> AppleFmAdapterPackageMetadata {
    let mut creator_defined = BTreeMap::new();
    creator_defined.insert(
        String::from("packageFormatVersion"),
        serde_json::Value::String(String::from(
            OPENAGENTS_APPLE_FMADAPTER_PACKAGE_FORMAT_VERSION,
        )),
    );
    creator_defined.insert(
        String::from("tokenizerDigest"),
        serde_json::Value::String(tokenizer_digest.tokenizer_digest.clone()),
    );
    creator_defined.insert(
        String::from("templateDigest"),
        serde_json::Value::String(
            tokenizer_digest
                .template_digest
                .clone()
                .unwrap_or_else(|| backend.config().model.prompt_shaping_digest.clone()),
        ),
    );
    creator_defined.insert(
        String::from("trainingEnvironmentRef"),
        serde_json::Value::String(environment.core_package.key.environment_ref.clone()),
    );
    creator_defined.insert(
        String::from("datasetRef"),
        serde_json::Value::String(request.dataset_ref.clone()),
    );
    creator_defined.insert(
        String::from("benchmarkRefs"),
        serde_json::to_value(&request.benchmark_refs).unwrap_or_else(|_| serde_json::json!([])),
    );
    creator_defined.insert(
        String::from("validatorPolicyRef"),
        serde_json::Value::String(request.validator_policy_ref.clone()),
    );
    creator_defined.insert(
        String::from("draftModelPresent"),
        serde_json::Value::Bool(draft_artifacts.is_some()),
    );
    creator_defined.insert(
        String::from("initialStateDictDigest"),
        serde_json::Value::String(initial_bundle_receipt.state_dict_digest.clone()),
    );
    creator_defined.insert(
        String::from("finalStateDictDigest"),
        serde_json::Value::String(final_bundle_receipt.state_dict_digest.clone()),
    );
    creator_defined.insert(
        String::from("packingPolicyDigest"),
        serde_json::Value::String(backend.provenance().packing_policy_digest.clone()),
    );
    creator_defined.insert(
        String::from("precisionPolicy"),
        serde_json::Value::String(
            serde_json::to_string(&backend.provenance().precision_policy)
                .unwrap_or_else(|_| String::from("\"f32_reference\""))
                .trim_matches('"')
                .to_string(),
        ),
    );
    creator_defined.insert(
        String::from("activationCheckpointPolicy"),
        serde_json::Value::String(
            serde_json::to_string(&backend.provenance().activation_checkpoint_policy)
                .unwrap_or_else(|_| String::from("\"disabled\""))
                .trim_matches('"')
                .to_string(),
        ),
    );
    creator_defined.insert(
        String::from("adapterArtifactDigest"),
        serde_json::Value::String(final_bundle_receipt.artifact_digest.clone()),
    );
    creator_defined.insert(
        String::from("executionFidelityPlan"),
        serde_json::to_value(&backend.provenance().fidelity_plan)
            .unwrap_or(serde_json::Value::Null),
    );
    if let Some(draft_artifacts) = draft_artifacts {
        creator_defined.insert(
            String::from("draftModelId"),
            serde_json::Value::String(draft_artifacts.draft_request.draft_model_id.clone()),
        );
        creator_defined.insert(
            String::from("draftMilDigest"),
            serde_json::Value::String(hex::encode(Sha256::digest(
                draft_artifacts.draft_mil_bytes.as_slice(),
            ))),
        );
        creator_defined.insert(
            String::from("draftWeightsDigest"),
            serde_json::Value::String(draft_artifacts.draft_bundle_receipt.artifact_digest.clone()),
        );
        creator_defined.insert(
            String::from("draftAcceptanceRatio"),
            serde_json::Value::from(mean_f32(
                draft_artifacts
                    .batch_records
                    .iter()
                    .map(|record| record.acceptance_ratio)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )),
        );
        creator_defined.insert(
            String::from("meanTeacherLatencyMs"),
            serde_json::Value::from(mean_u64(
                draft_artifacts
                    .batch_records
                    .iter()
                    .map(|record| record.mean_teacher_latency_ms)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )),
        );
        creator_defined.insert(
            String::from("meanDraftLatencyMs"),
            serde_json::Value::from(mean_u64(
                draft_artifacts
                    .batch_records
                    .iter()
                    .map(|record| record.mean_draft_latency_ms)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )),
        );
    }

    AppleFmAdapterPackageMetadata {
        adapter_identifier: String::from(adapter_identifier),
        author: request.author.clone(),
        base_model_signature: backend.config().model.base_model_signature.clone(),
        creator_defined,
        description: request.description.clone(),
        license: request.license.clone(),
        lora_rank: backend
            .config()
            .model
            .targets
            .iter()
            .map(|target| target.lora_rank as u32)
            .max()
            .unwrap_or(1),
        speculative_decoding_draft_token_count: draft_artifacts
            .map(|artifacts| artifacts.draft_request.draft_token_count)
            .unwrap_or(0),
    }
}

fn run_apple_adapter_draft_export_artifacts(
    backend: &AppleAdapterTrainingExecutionBackend,
    dataset: &AppleAdapterDatasetContract,
    environment: &AppleAdapterEnvironmentBundle,
    request: &AppleAdapterDraftDistillationRequest,
    sft_artifacts: &AppleAdapterSftExecutionArtifacts,
) -> Result<AppleAdapterDraftExportArtifacts, AppleAdapterDraftDistillationError> {
    request.validate()?;
    let hidden_group_id = draft_hidden_group_id(request);
    let output_group_id = draft_output_group_id(request);
    let mut run = FixedBudgetTrainingRun::new(
        draft_run_id(backend, request),
        draft_checkpoint_family(backend, request),
        backend.config().budget,
        vec![
            TrainingParameterGroupState::new(
                hidden_group_id.clone(),
                TrainingParameterClass::Matrix,
                TrainingTensorBuffer::from_f32(
                    hidden_group_id.clone(),
                    draft_hidden_spec(request.hidden_width, backend.config().model.input_width),
                    seeded_matrix(
                        format!(
                            "{}|{}|draft_hidden",
                            sft_artifacts.adapter_identifier, request.draft_model_id
                        )
                        .as_str(),
                        request.hidden_width,
                        backend.config().model.input_width,
                        0.03,
                    ),
                )?,
                request.optimizer.clone(),
                request.optimizer_residency_policy,
            )?,
            TrainingParameterGroupState::new(
                output_group_id.clone(),
                TrainingParameterClass::Matrix,
                TrainingTensorBuffer::from_f32(
                    output_group_id.clone(),
                    draft_output_spec(backend.config().model.output_width, request.hidden_width),
                    seeded_matrix(
                        format!(
                            "{}|{}|draft_output",
                            sft_artifacts.adapter_identifier, request.draft_model_id
                        )
                        .as_str(),
                        backend.config().model.output_width,
                        request.hidden_width,
                        0.02,
                    ),
                )?,
                request.optimizer.clone(),
                request.optimizer_residency_policy,
            )?,
        ],
    )?;
    let mut step_receipts = Vec::new();
    let mut batch_records = Vec::new();
    for step_index in 0..backend.config().budget.max_steps {
        let batch_index = step_index as usize % backend.batches().len().max(1);
        let started_at_ms =
            request.started_at_ms + step_index.saturating_mul(request.step_duration_ms);
        let finished_at_ms = started_at_ms + request.step_duration_ms;
        let (step_input, batch_record) = produce_draft_step_input(
            backend,
            request,
            sft_artifacts,
            &run,
            batch_index,
            started_at_ms,
            finished_at_ms,
        )?;
        batch_records.push(batch_record);
        step_receipts.push(run.apply_step(step_input)?);
    }
    let draft_bundle = PortableModelBundle::from_training_groups(
        "apple_adapter_draft_reference",
        request.draft_model_id.clone(),
        draft_checkpoint_family(backend, request),
        Some(format!(
            "checkpoint://{}/draft/final",
            backend.config().run_id
        )),
        &[
            draft_training_group(&run, hidden_group_id.as_str())?.clone(),
            draft_training_group(&run, output_group_id.as_str())?.clone(),
        ],
        tokenizer_binding(dataset.metadata.tokenizer.clone(), environment),
        dataset
            .metadata
            .tokenizer
            .template_digest
            .clone()
            .or_else(|| Some(backend.config().model.prompt_shaping_digest.clone())),
    )?;
    let (draft_weights_bytes, draft_bundle_receipt) = draft_bundle.export_safetensors()?;
    let runtime_pairing = AppleAdapterDraftRuntimePairing {
        teacher_base_model_signature: backend.config().model.base_model_signature.clone(),
        teacher_adapter_identifier: sft_artifacts.adapter_identifier.clone(),
        teacher_precision_policy: backend.provenance().precision_policy,
        draft_model_id: request.draft_model_id.clone(),
        draft_precision_policy: request.draft_precision_policy,
    };
    let draft_mil_bytes =
        draft_mil_bytes(backend, request, &runtime_pairing, &draft_bundle_receipt);
    Ok(AppleAdapterDraftExportArtifacts {
        run,
        step_receipts,
        batch_records,
        draft_bundle,
        draft_bundle_receipt,
        draft_weights_bytes,
        draft_mil_bytes,
        runtime_pairing,
        draft_request: request.clone(),
    })
}

fn produce_draft_step_input(
    backend: &AppleAdapterTrainingExecutionBackend,
    request: &AppleAdapterDraftDistillationRequest,
    sft_artifacts: &AppleAdapterSftExecutionArtifacts,
    draft_run: &FixedBudgetTrainingRun,
    batch_index: usize,
    started_at_ms: u64,
    finished_at_ms: u64,
) -> Result<
    (TrainingStepInput, AppleAdapterDraftDistillationBatchRecord),
    AppleAdapterDraftDistillationError,
> {
    let batch = backend
        .batches()
        .get(batch_index)
        .ok_or(AppleAdapterDraftDistillationError::UnknownBatchIndex { batch_index })?;
    let hidden_group_id = draft_hidden_group_id(request);
    let output_group_id = draft_output_group_id(request);
    let hidden_group = draft_training_group(draft_run, hidden_group_id.as_str())?;
    let output_group = draft_training_group(draft_run, output_group_id.as_str())?;
    let hidden_values = draft_dense_values(hidden_group, hidden_group_id.as_str())?;
    let output_values = draft_dense_values(output_group, output_group_id.as_str())?;
    let mut grad_hidden = vec![0.0_f32; hidden_values.len()];
    let mut grad_output = vec![0.0_f32; output_values.len()];
    let mut mean_distillation_loss = 0.0_f32;
    let mut accepted = 0_u64;
    let mut total_teacher_latency_ms = 0_u64;
    let mut total_draft_latency_ms = 0_u64;
    for record in &batch.records {
        let teacher_forward = backend.forward_sample(record, &sft_artifacts.run)?;
        let teacher_distribution = softmax_with_temperature(
            teacher_forward.prediction.as_slice(),
            request.teacher_temperature,
        );
        let draft_forward = draft_forward_sample(
            request,
            backend.config().model.output_width,
            hidden_values,
            output_values,
            record.prompt_features.as_slice(),
            teacher_distribution.as_slice(),
        );
        accumulate_draft_gradients(
            grad_hidden.as_mut_slice(),
            grad_output.as_mut_slice(),
            output_values,
            record.prompt_features.as_slice(),
            draft_forward.hidden.as_slice(),
            draft_forward.logits_gradient.as_slice(),
        );
        mean_distillation_loss += draft_forward.loss;
        if cosine_similarity(
            draft_forward.student_distribution.as_slice(),
            teacher_distribution.as_slice(),
        ) >= request.acceptance_cosine_threshold
        {
            accepted = accepted.saturating_add(1);
        }
        total_teacher_latency_ms =
            total_teacher_latency_ms.saturating_add(estimated_teacher_latency_ms(backend, record));
        total_draft_latency_ms = total_draft_latency_ms
            .saturating_add(estimated_draft_latency_ms(backend, request, record));
    }
    let scale = 1.0_f32 / batch.records.len().max(1) as f32;
    for value in &mut grad_hidden {
        *value *= scale;
    }
    for value in &mut grad_output {
        *value *= scale;
    }
    mean_distillation_loss *= scale;
    let mut gradients = BTreeMap::new();
    let mut gradient_norms_l2 = BTreeMap::new();
    gradient_norms_l2.insert(hidden_group_id.clone(), l2_norm(grad_hidden.as_slice()));
    gradient_norms_l2.insert(output_group_id.clone(), l2_norm(grad_output.as_slice()));
    gradients.insert(
        hidden_group_id.clone(),
        TrainingTensorBuffer::from_f32(
            hidden_group_id.clone(),
            hidden_group.parameter.spec.clone(),
            grad_hidden,
        )?,
    );
    gradients.insert(
        output_group_id.clone(),
        TrainingTensorBuffer::from_f32(
            output_group_id.clone(),
            output_group.parameter.spec.clone(),
            grad_output,
        )?,
    );
    let training_batch = TrainingGradientBatch::new(
        format!("{}-draft-gradient", batch.batch_id),
        mean_distillation_loss,
        batch.records.len() as u32,
        gradients,
    );
    let acceptance_ratio = accepted as f32 / batch.records.len().max(1) as f32;
    let mean_teacher_latency_ms = total_teacher_latency_ms / batch.records.len().max(1) as u64;
    let mean_draft_latency_ms = total_draft_latency_ms / batch.records.len().max(1) as u64;
    let execution_digest = stable_draft_execution_digest(
        batch.batch_id.as_str(),
        batch.batch_digest.as_str(),
        &gradient_norms_l2,
        mean_distillation_loss,
        acceptance_ratio,
        mean_teacher_latency_ms,
        mean_draft_latency_ms,
    );
    Ok((
        TrainingStepInput::new(training_batch.clone(), started_at_ms, finished_at_ms),
        AppleAdapterDraftDistillationBatchRecord {
            batch_id: batch.batch_id.clone(),
            batch_digest: batch.batch_digest.clone(),
            training_batch,
            mean_distillation_loss,
            gradient_norms_l2,
            acceptance_ratio,
            mean_teacher_latency_ms,
            mean_draft_latency_ms,
            execution_digest,
        },
    ))
}

#[derive(Clone, Debug)]
struct DraftForwardRecord {
    hidden: Vec<f32>,
    student_distribution: Vec<f32>,
    logits_gradient: Vec<f32>,
    loss: f32,
}

fn draft_forward_sample(
    request: &AppleAdapterDraftDistillationRequest,
    output_width: usize,
    hidden_values: &[f32],
    output_values: &[f32],
    prompt_features: &[f32],
    teacher_distribution: &[f32],
) -> DraftForwardRecord {
    let hidden_pre = mat_vec(
        hidden_values,
        request.hidden_width,
        prompt_features.len(),
        prompt_features,
    );
    let hidden = hidden_pre
        .into_iter()
        .map(|value| value.tanh())
        .collect::<Vec<_>>();
    let logits = mat_vec(
        output_values,
        output_width,
        request.hidden_width,
        hidden.as_slice(),
    );
    let student_distribution =
        softmax_with_temperature(logits.as_slice(), request.student_temperature);
    let loss = soft_cross_entropy(teacher_distribution, student_distribution.as_slice());
    let logits_gradient = student_distribution
        .iter()
        .zip(teacher_distribution)
        .map(|(student, teacher)| {
            (student - teacher) / request.student_temperature.max(f32::EPSILON)
        })
        .collect::<Vec<_>>();
    DraftForwardRecord {
        hidden,
        student_distribution,
        logits_gradient,
        loss,
    }
}

fn accumulate_draft_gradients(
    grad_hidden: &mut [f32],
    grad_output: &mut [f32],
    output_values: &[f32],
    prompt_features: &[f32],
    hidden: &[f32],
    logits_gradient: &[f32],
) {
    let hidden_width = hidden.len();
    let input_width = prompt_features.len();
    for output_index in 0..logits_gradient.len() {
        for hidden_index in 0..hidden_width {
            grad_output[output_index * hidden_width + hidden_index] +=
                logits_gradient[output_index] * hidden[hidden_index];
        }
    }
    let mut hidden_gradient = vec![0.0_f32; hidden_width];
    for hidden_index in 0..hidden_width {
        for output_index in 0..logits_gradient.len() {
            hidden_gradient[hidden_index] += output_values
                [output_index * hidden_width + hidden_index]
                * logits_gradient[output_index];
        }
        hidden_gradient[hidden_index] *= 1.0_f32 - hidden[hidden_index] * hidden[hidden_index];
    }
    for hidden_index in 0..hidden_width {
        for input_index in 0..input_width {
            grad_hidden[hidden_index * input_width + input_index] +=
                hidden_gradient[hidden_index] * prompt_features[input_index];
        }
    }
}

fn draft_training_group<'a>(
    run: &'a FixedBudgetTrainingRun,
    group_id: &str,
) -> Result<&'a TrainingParameterGroupState, AppleAdapterDraftDistillationError> {
    run.parameter_group(group_id).ok_or_else(|| {
        AppleAdapterDraftDistillationError::MissingDraftParameterGroup {
            group_id: String::from(group_id),
        }
    })
}

fn draft_dense_values<'a>(
    group: &'a TrainingParameterGroupState,
    group_id: &str,
) -> Result<&'a [f32], AppleAdapterDraftDistillationError> {
    match &group.parameter.data {
        psionic_core::TensorData::F32(values) => Ok(values.as_slice()),
        psionic_core::TensorData::QuantizedBlocks(_) => {
            Err(AppleAdapterDraftDistillationError::NonDenseDraftGroup {
                group_id: String::from(group_id),
            })
        }
    }
}

fn draft_hidden_group_id(request: &AppleAdapterDraftDistillationRequest) -> String {
    format!("{}.draft.hidden_projection", request.draft_model_id)
}

fn draft_output_group_id(request: &AppleAdapterDraftDistillationRequest) -> String {
    format!("{}.draft.output_projection", request.draft_model_id)
}

fn draft_run_id(
    backend: &AppleAdapterTrainingExecutionBackend,
    request: &AppleAdapterDraftDistillationRequest,
) -> String {
    format!(
        "{}::draft::{}",
        backend.config().run_id,
        request.draft_model_id
    )
}

fn draft_checkpoint_family(
    backend: &AppleAdapterTrainingExecutionBackend,
    request: &AppleAdapterDraftDistillationRequest,
) -> String {
    format!(
        "{}.draft.{}",
        backend.config().checkpoint_family,
        request.draft_model_id
    )
}

fn draft_hidden_spec(hidden_width: usize, input_width: usize) -> TensorSpec {
    TensorSpec::new(
        Shape::new(vec![hidden_width, input_width]),
        DType::F32,
        Device::cpu(),
    )
}

fn draft_output_spec(output_width: usize, hidden_width: usize) -> TensorSpec {
    TensorSpec::new(
        Shape::new(vec![output_width, hidden_width]),
        DType::F32,
        Device::cpu(),
    )
}

fn softmax_with_temperature(logits: &[f32], temperature: f32) -> Vec<f32> {
    let scaled = logits
        .iter()
        .map(|value| *value / temperature.max(f32::EPSILON))
        .collect::<Vec<_>>();
    let max = scaled.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = scaled
        .iter()
        .map(|value| (*value - max).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(f32::EPSILON);
    exp.into_iter().map(|value| value / sum).collect()
}

fn soft_cross_entropy(target: &[f32], prediction: &[f32]) -> f32 {
    target
        .iter()
        .zip(prediction)
        .map(|(expected, actual)| -expected * actual.max(f32::EPSILON).ln())
        .sum()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let numerator = left.iter().zip(right).map(|(l, r)| l * r).sum::<f32>();
    let denominator = (l2_norm(left) * l2_norm(right)).max(f32::EPSILON);
    numerator / denominator
}

fn estimated_teacher_latency_ms(
    backend: &AppleAdapterTrainingExecutionBackend,
    record: &AppleAdapterBatchFeatureRecord,
) -> u64 {
    ceil_div_u64(
        u64::from(record.prompt_tokens.max(1))
            .saturating_mul(backend.config().model.targets.len() as u64)
            .saturating_mul(backend.config().model.output_width as u64),
        32,
    )
    .max(1)
}

fn estimated_draft_latency_ms(
    backend: &AppleAdapterTrainingExecutionBackend,
    request: &AppleAdapterDraftDistillationRequest,
    record: &AppleAdapterBatchFeatureRecord,
) -> u64 {
    ceil_div_u64(
        u64::from(record.prompt_tokens.max(1))
            .saturating_mul(request.hidden_width as u64)
            .saturating_mul(backend.config().model.output_width as u64),
        96,
    )
    .max(1)
}

fn stable_draft_execution_digest(
    batch_id: &str,
    batch_digest: &str,
    gradient_norms_l2: &BTreeMap<String, f32>,
    mean_distillation_loss: f32,
    acceptance_ratio: f32,
    mean_teacher_latency_ms: u64,
    mean_draft_latency_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_apple_adapter_draft_execution|");
    hasher.update(batch_id.as_bytes());
    hasher.update(b"|");
    hasher.update(batch_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(mean_distillation_loss.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(acceptance_ratio.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_teacher_latency_ms.to_le_bytes());
    hasher.update(b"|");
    hasher.update(mean_draft_latency_ms.to_le_bytes());
    for (group_id, norm) in gradient_norms_l2 {
        hasher.update(b"|group|");
        hasher.update(group_id.as_bytes());
        hasher.update(b"|");
        hasher.update(norm.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn draft_mil_bytes(
    backend: &AppleAdapterTrainingExecutionBackend,
    request: &AppleAdapterDraftDistillationRequest,
    runtime_pairing: &AppleAdapterDraftRuntimePairing,
    draft_bundle_receipt: &ModelIoArtifactReceipt,
) -> Vec<u8> {
    format!(
        "// openagents.apple.reference_draft_mil.v1\nteacher_base_model_signature={}\nteacher_adapter_identifier={}\nteacher_precision_policy={:?}\ndraft_model_id={}\ndraft_precision_policy={:?}\ninput_width={}\nhidden_width={}\noutput_width={}\ndraft_token_count={}\nteacher_temperature={:.4}\nstudent_temperature={:.4}\nstate_dict_digest={}\nartifact_digest={}\n",
        runtime_pairing.teacher_base_model_signature,
        runtime_pairing.teacher_adapter_identifier,
        runtime_pairing.teacher_precision_policy,
        runtime_pairing.draft_model_id,
        runtime_pairing.draft_precision_policy,
        backend.config().model.input_width,
        request.hidden_width,
        backend.config().model.output_width,
        request.draft_token_count,
        request.teacher_temperature,
        request.student_temperature,
        draft_bundle_receipt.state_dict_digest,
        draft_bundle_receipt.artifact_digest,
    )
    .into_bytes()
}

fn mean_f32(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

fn mean_u64(values: &[u64]) -> u64 {
    if values.is_empty() {
        0
    } else {
        values.iter().sum::<u64>() / values.len() as u64
    }
}

fn ceil_div_u64(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    numerator.saturating_add(denominator - 1) / denominator
}

fn stable_adapter_identifier(package_name: &str, artifact_digest: &str) -> String {
    let stem = package_name.trim_end_matches(".fmadapter");
    let slug = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let digest = Sha256::digest(artifact_digest.as_bytes());
    format!("fmadapter-{slug}-{}", hex::encode(digest)[..8].to_string())
}

fn normalized_package_name(package_name: &str, adapter_identifier: &str) -> String {
    if package_name.trim().ends_with(".fmadapter") {
        package_name.trim().to_string()
    } else if package_name.trim().is_empty() {
        format!("{adapter_identifier}.fmadapter")
    } else {
        format!("{}.fmadapter", package_name.trim())
    }
}

/// Error surfaced by the higher-level Apple SFT/export lane.
#[derive(Debug, Error)]
pub enum AppleAdapterSftError {
    #[error("Apple adapter SFT export request is missing `dataset_ref`")]
    MissingDatasetRef,
    #[error("Apple adapter SFT export request is missing `validator_policy_ref`")]
    MissingValidatorPolicyRef,
    #[error("Apple adapter SFT export request is missing `package_name`")]
    MissingPackageName,
    #[error("Apple adapter SFT export request requires `step_duration_ms > 0`")]
    InvalidStepDuration,
    #[error("Apple adapter SFT export request contains an empty benchmark ref")]
    InvalidBenchmarkRef,
    #[error(transparent)]
    Execution(#[from] AppleAdapterTrainingExecutionError),
    #[error(transparent)]
    RuntimeAsset(#[from] AppleAdapterRuntimeAssetError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    ModelIo(#[from] crate::ModelIoError),
    #[error(transparent)]
    Package(#[from] AppleFmAdapterPackageError),
}

/// Error surfaced by the optional draft distillation lane.
#[derive(Debug, Error)]
pub enum AppleAdapterDraftDistillationError {
    #[error("Apple draft distillation request is missing `draft_model_id`")]
    MissingDraftModelId,
    #[error("Apple draft distillation request requires `hidden_width > 0`")]
    InvalidDraftHiddenWidth,
    #[error("Apple draft distillation request requires `teacher_temperature > 0`")]
    InvalidTeacherTemperature,
    #[error("Apple draft distillation request requires `student_temperature > 0`")]
    InvalidStudentTemperature,
    #[error("Apple draft distillation request requires `acceptance_cosine_threshold` in `[0, 1]`")]
    InvalidAcceptanceCosineThreshold,
    #[error("Apple draft distillation request requires `draft_token_count > 0`")]
    InvalidDraftTokenCount,
    #[error("Apple draft distillation request requires `step_duration_ms > 0`")]
    InvalidStepDuration,
    #[error("Apple draft distillation does not yet support precision policy `{0:?}`")]
    UnsupportedDraftPrecisionPolicy(AppleAdapterPrecisionPolicy),
    #[error("Apple draft distillation requested unknown batch index `{batch_index}`")]
    UnknownBatchIndex { batch_index: usize },
    #[error("Apple draft distillation is missing parameter group `{group_id}`")]
    MissingDraftParameterGroup { group_id: String },
    #[error("Apple draft parameter group `{group_id}` must use dense `f32` values")]
    NonDenseDraftGroup { group_id: String },
    #[error(transparent)]
    Sft(#[from] AppleAdapterSftError),
    #[error(transparent)]
    Execution(#[from] AppleAdapterTrainingExecutionError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    ModelIo(#[from] crate::ModelIoError),
    #[error(transparent)]
    Package(#[from] AppleFmAdapterPackageError),
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

fn apple_adapter_execution_fidelity_plan() -> AppleAdapterExecutionFidelityPlan {
    AppleAdapterExecutionFidelityPlan {
        plan_id: APPLE_ADAPTER_FIDELITY_PLAN_ID.to_string(),
        token_encoder: String::from("hashed lexical token sequence with unigram and bigram traces"),
        prompt_pooling: String::from(
            "turn-aware position-weighted pooling with explicit role, tool, and schema boundaries",
        ),
        target_supervision: String::from(
            "assistant token-sequence regression over pooled completion traces",
        ),
        faithful_components: vec![
            String::from("multi_turn_prompt_context"),
            String::from("role_aware_prompt_shaping"),
            String::from("tool_and_schema_attachment_encoding"),
            String::from("position_sensitive_token_pooling"),
            String::from("assistant_completion_token_supervision"),
        ],
        bounded_components: vec![
            String::from("repo_owned_lexical_tokenizer_not_apple_exact"),
            String::from("hashed_token_embeddings_not_native_hidden_states"),
            String::from("pooled_sequence_regression_not_full_decoder_loss"),
            String::from("single_host_f32_reference_execution"),
        ],
    }
}

fn prompt_feature_vector(
    sample: &psionic_data::AppleAdapterTrainingSample,
    capture: &AppleAdapterSampleTokenCapture,
    width: usize,
) -> Vec<f32> {
    let mut features = vec![0.0_f32; width];
    let prompt_scale = 1.0_f32 / capture.prompt_tokens.max(1) as f32;
    let total_tokens = capture.total_tokens().max(1) as f32;
    for (turn_index, message) in prompt_messages(sample).iter().enumerate() {
        accumulate_sequence_text(
            features.as_mut_slice(),
            role_label(message.role),
            prompt_scale * 0.35,
            turn_index,
            "role",
        );
        accumulate_sequence_text(
            features.as_mut_slice(),
            message.content.as_str(),
            prompt_scale,
            turn_index,
            role_label(message.role),
        );
        if let Some(response_format) = &message.response_format {
            accumulate_sequence_text(
                features.as_mut_slice(),
                canonical_json(response_format).as_str(),
                capture.response_schema_tokens.max(1) as f32 / total_tokens,
                turn_index,
                "response_schema",
            );
        }
        if !message.tools.is_empty() {
            accumulate_sequence_text(
                features.as_mut_slice(),
                canonical_json(&message.tools).as_str(),
                capture.tool_tokens.max(1) as f32 / total_tokens,
                turn_index,
                "tools",
            );
        }
    }
    accumulate_ratio_feature(
        features.as_mut_slice(),
        "prompt_token_ratio",
        capture.prompt_tokens as f32 / total_tokens,
    );
    accumulate_ratio_feature(
        features.as_mut_slice(),
        "tool_token_ratio",
        capture.tool_tokens as f32 / total_tokens,
    );
    accumulate_ratio_feature(
        features.as_mut_slice(),
        "schema_token_ratio",
        capture.response_schema_tokens as f32 / total_tokens,
    );
    features[sample_kind_bucket(sample.sample_kind, width)] += 0.75;
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
    let total_tokens = capture.total_tokens().max(1) as f32;
    accumulate_sequence_text(
        features.as_mut_slice(),
        assistant.content.as_str(),
        scale,
        0,
        "assistant_target",
    );
    if let Some(structured) = &sample.structured_assistant_output {
        accumulate_sequence_text(
            features.as_mut_slice(),
            canonical_json(structured).as_str(),
            scale,
            0,
            "assistant_structured",
        );
    }
    accumulate_ratio_feature(
        features.as_mut_slice(),
        "completion_token_ratio",
        capture.completion_tokens as f32 / total_tokens,
    );
    features[sample_kind_bucket(sample.sample_kind, width)] += 0.75;
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

fn accumulate_sequence_text(
    out: &mut [f32],
    text: &str,
    scale: f32,
    turn_index: usize,
    channel: &str,
) {
    let tokens = lexical_tokens(text);
    if tokens.is_empty() {
        accumulate_hashed_feature(
            out,
            format!("{channel}|turn={turn_index}|empty").as_str(),
            scale.max(0.05),
        );
        return;
    }

    accumulate_hashed_feature(
        out,
        format!("{channel}|turn={turn_index}|begin").as_str(),
        scale * 0.35,
    );

    let turn_scale = 1.0_f32 / (1.0 + turn_index as f32 * 0.15);
    for (token_index, token) in tokens.iter().enumerate() {
        let position_scale =
            scale * turn_scale * (1.0 + token_index as f32 / tokens.len().max(1) as f32);
        accumulate_hashed_feature(
            out,
            format!("{channel}|turn={turn_index}|token={token_index}|{token}").as_str(),
            position_scale,
        );
        accumulate_hashed_feature(
            out,
            format!(
                "{channel}|turn={turn_index}|class={}",
                lexical_token_class(token.as_str())
            )
            .as_str(),
            position_scale * 0.25,
        );
        if token_index > 0 {
            accumulate_hashed_feature(
                out,
                format!(
                    "{channel}|turn={turn_index}|bigram={}>{}",
                    tokens[token_index - 1],
                    token
                )
                .as_str(),
                position_scale * 0.5,
            );
        }
    }

    accumulate_hashed_feature(
        out,
        format!("{channel}|turn={turn_index}|end|len={}", tokens.len()).as_str(),
        scale * 0.35,
    );
}

fn accumulate_ratio_feature(out: &mut [f32], channel: &str, ratio: f32) {
    let bucket = (ratio.clamp(0.0, 1.0) * 10.0).round() as i32;
    accumulate_hashed_feature(
        out,
        format!("{channel}|bucket={bucket}").as_str(),
        ratio.clamp(0.0, 1.0),
    );
}

fn accumulate_hashed_feature(out: &mut [f32], key: &str, scale: f32) {
    let digest = Sha256::digest(key.as_bytes());
    let index = (((digest[0] as usize) << 8) | digest[1] as usize) % out.len().max(1);
    let sign = if digest[2] & 1 == 0 { 1.0 } else { -1.0 };
    out[index] += sign * scale;
}

fn lexical_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
        tokens.push(ch.to_string());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn lexical_token_class(token: &str) -> &'static str {
    if token.chars().all(|ch| ch.is_ascii_digit()) {
        "number"
    } else if token.len() == 1
        && token
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_alphanumeric() && !ch.is_whitespace())
    {
        "punct"
    } else {
        "word"
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
    hasher.update(b"|");
    hasher.update(APPLE_ADAPTER_FIDELITY_PLAN_ID.as_bytes());
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
    use psionic_adapters::AppleFmAdapterPackage;
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
    use tempfile::tempdir;

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
                input_width: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
                output_width: APPLE_LIVE_REFERENCE_FEATURE_WIDTH,
                targets: vec![
                    AppleAdapterTrainableTarget {
                        target_id: String::from(
                            "layers.segment_0.layer_34.attention.qkv_transform.adapters.base_adapter.lora_0",
                        ),
                        lora_rank: APPLE_LIVE_REFERENCE_LORA_RANK,
                        lora_alpha: APPLE_LIVE_REFERENCE_LORA_RANK as f32,
                        optimizer: TrainingOptimizerConfig::adamw(0.01, 0.9, 0.99, 1e-8)
                            .with_gradient_clip_norm(1.0),
                        optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                    },
                    AppleAdapterTrainableTarget {
                        target_id: String::from(
                            "layers.segment_1.layer_20.attention.q_transform.adapters.base_adapter",
                        ),
                        lora_rank: APPLE_LIVE_REFERENCE_LORA_RANK,
                        lora_alpha: APPLE_LIVE_REFERENCE_LORA_RANK as f32,
                        optimizer: TrainingOptimizerConfig::adamw(0.01, 0.9, 0.99, 1e-8)
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

    #[test]
    fn apple_adapter_sft_lane_trains_and_exports_valid_fmadapter_package()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset = dataset();
        let environment = environment_bundle();
        let backend = AppleAdapterTrainingExecutionBackend::new(
            config(),
            &dataset,
            captures(&dataset).as_slice(),
            &environment,
        )?;
        let request = AppleAdapterSftRunRequest {
            dataset_ref: String::from("fixture.apple_adapter.minimal_sft"),
            benchmark_refs: vec![String::from("apple_adapter_smoke@2026-03-15")],
            validator_policy_ref: String::from("validator.apple_adapter.smoke.v1"),
            package_name: String::from("trained_helpdesk_adapter"),
            author: String::from("openagents"),
            description: String::from("Repo-owned Apple adapter SFT test"),
            license: String::from("internal-test"),
            started_at_ms: 2_000,
            step_duration_ms: 40,
        };
        let outcome = run_apple_adapter_sft_export(&backend, &dataset, &environment, &request)?;
        assert_eq!(outcome.step_receipts.len(), 2);
        assert_eq!(outcome.summary.dataset_ref, request.dataset_ref);
        assert!(!outcome.adapter_delta.tensors.is_empty());

        let temp = tempdir()?;
        let package_path = temp.path().join("trained_helpdesk_adapter.fmadapter");
        outcome.write_package_to_directory(&package_path)?;
        let reread = AppleFmAdapterPackage::read_from_directory(&package_path)?;
        assert_eq!(
            reread.metadata.base_model_signature,
            backend.config().model.base_model_signature
        );
        assert_eq!(
            reread.lineage.dataset_ref.as_deref(),
            Some(request.dataset_ref.as_str())
        );
        assert_eq!(
            reread.lineage.validator_policy_ref.as_deref(),
            Some(request.validator_policy_ref.as_str())
        );
        assert_eq!(
            reread.lineage.package_format_version.as_deref(),
            Some(OPENAGENTS_APPLE_FMADAPTER_PACKAGE_FORMAT_VERSION)
        );
        assert_eq!(reread.package_digest, outcome.summary.package_digest);
        assert_eq!(
            outcome.summary.execution_provenance.fidelity_plan.plan_id,
            APPLE_ADAPTER_FIDELITY_PLAN_ID
        );
        assert!(reread.lineage.extra.contains_key("executionFidelityPlan"));
        Ok(())
    }

    #[test]
    fn apple_adapter_prompt_features_are_order_sensitive() -> Result<(), Box<dyn std::error::Error>>
    {
        let left = AppleAdapterDatasetContract::from_jsonl_str(
            r#"[{"role":"user","content":"alpha beta gamma"},{"role":"assistant","content":"ok"}]"#,
            dataset_metadata(),
        )?;
        let right = AppleAdapterDatasetContract::from_jsonl_str(
            r#"[{"role":"user","content":"gamma beta alpha"},{"role":"assistant","content":"ok"}]"#,
            dataset_metadata(),
        )?;
        let left_capture = left.derive_token_captures()?;
        let right_capture = right.derive_token_captures()?;
        let left_features = prompt_feature_vector(&left.samples[0], &left_capture[0], 24);
        let right_features = prompt_feature_vector(&right.samples[0], &right_capture[0], 24);
        assert_ne!(left_features, right_features);
        Ok(())
    }

    #[test]
    fn apple_adapter_draft_lane_exports_valid_fmadapter_with_draft_payload()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset = dataset();
        let environment = environment_bundle();
        let backend = AppleAdapterTrainingExecutionBackend::new(
            config(),
            &dataset,
            captures(&dataset).as_slice(),
            &environment,
        )?;
        let sft_request = AppleAdapterSftRunRequest {
            dataset_ref: String::from("fixture.apple_adapter.minimal_sft"),
            benchmark_refs: vec![String::from("apple_adapter_smoke@2026-03-15")],
            validator_policy_ref: String::from("validator.apple_adapter.smoke.v1"),
            package_name: String::from("trained_helpdesk_adapter"),
            author: String::from("openagents"),
            description: String::from("Repo-owned Apple adapter SFT + draft test"),
            license: String::from("internal-test"),
            started_at_ms: 3_000,
            step_duration_ms: 40,
        };
        let draft_request = AppleAdapterDraftDistillationRequest {
            draft_model_id: String::from("helpdesk_draft_v1"),
            hidden_width: 6,
            optimizer: TrainingOptimizerConfig::adamw(0.03, 0.9, 0.99, 1e-8)
                .with_gradient_clip_norm(1.0),
            optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
            draft_precision_policy: AppleAdapterPrecisionPolicy::F32Reference,
            teacher_temperature: 1.25,
            student_temperature: 1.0,
            acceptance_cosine_threshold: 0.9,
            draft_token_count: 8,
            started_at_ms: 3_100,
            step_duration_ms: 30,
        };
        let outcome = run_apple_adapter_draft_distillation_export(
            &backend,
            &dataset,
            &environment,
            &sft_request,
            &draft_request,
        )?;
        assert_eq!(outcome.sft_outcome.step_receipts.len(), 2);
        assert_eq!(outcome.draft_step_receipts.len(), 2);
        assert!(outcome.adapter_package.has_draft_payload());
        assert!(outcome.draft_summary.acceptance_ratio > 0.0);
        assert!(
            outcome.draft_summary.mean_teacher_latency_ms
                >= outcome.draft_summary.mean_draft_latency_ms
        );

        let temp = tempdir()?;
        let package_path = temp.path().join("trained_helpdesk_adapter.fmadapter");
        outcome.write_package_to_directory(&package_path)?;
        let reread = AppleFmAdapterPackage::read_from_directory(&package_path)?;
        assert!(reread.has_draft_payload());
        assert_eq!(reread.lineage.draft_model_present, Some(true));
        assert!(reread.lineage.draft_mil_digest.is_some());
        assert!(reread.lineage.draft_weights_digest.is_some());
        assert_eq!(
            reread.metadata.speculative_decoding_draft_token_count,
            draft_request.draft_token_count
        );
        assert_eq!(reread.package_digest, outcome.draft_summary.package_digest);
        Ok(())
    }
}
