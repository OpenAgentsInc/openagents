use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact, LmHeadLoraLoadError,
};
use psionic_core::{DType, Device, QuantizationMode, Shape, TensorSpec};
use psionic_data::TokenizerDigest;
use safetensors::{Dtype as SafeTensorsDType, serialize, tensor::TensorView};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AdapterContributorCapabilityPolicy, AdapterTargetIdentity, AdapterWindowContractError,
    FixedBudgetTrainingRun, ModelAdapterDelta, ModelIoArtifactReceipt, PortableModelBundle,
    PortableTokenizerAssetFormat, PortableTokenizerBinding, TrainingCoreError,
    TrainingGradientBatch, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, TrainingParameterClass, TrainingParameterGroupState,
    TrainingRunSummary, TrainingStepInput, TrainingStepReceipt, TrainingTensorBuffer,
};

/// Canonical backend label for the first open adapter contributor target and the
/// first concrete NVIDIA/CUDA participant in the mixed Apple-plus-NVIDIA
/// cluster experiment.
pub const OPEN_ADAPTER_CUDA_BACKEND_LABEL: &str = "open_adapter_backend.cuda.gpt_oss_lm_head";
/// Canonical adapter family for the first non-Apple decentralized adapter lane.
pub const OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY: &str = "gpt_oss.decoder_lm_head_lora";
/// Canonical adapter format for the first non-Apple decentralized adapter lane.
pub const OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT: &str = "safetensors";

const OPEN_ADAPTER_SAFETENSORS_MANIFEST_KEY: &str = "openagents.open_adapter.manifest";

/// First admitted open model family for decentralized adapter execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAdapterAdmissibleModelFamily {
    /// GPT-OSS-style decoder LM-head LoRA adapters exported as `safetensors`.
    GptOssDecoderLmHeadLora,
}

impl OpenAdapterAdmissibleModelFamily {
    /// Returns the canonical adapter family label.
    #[must_use]
    pub const fn adapter_family(self) -> &'static str {
        match self {
            Self::GptOssDecoderLmHeadLora => OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY,
        }
    }

    /// Returns the canonical adapter format label.
    #[must_use]
    pub const fn adapter_format(self) -> &'static str {
        match self {
            Self::GptOssDecoderLmHeadLora => OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT,
        }
    }
}

/// Precision posture for the first open adapter reference backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAdapterPrecisionPolicy {
    /// Dense `f32` parameters, gradients, and activations.
    F32Reference,
    /// Reserved for later mixed-precision support.
    Bf16Mixed,
}

/// One trainable LM-head LoRA target kept separate from the frozen base model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterLmHeadTarget {
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

impl OpenAdapterLmHeadTarget {
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

    fn validate(&self) -> Result<(), OpenAdapterTrainingExecutionError> {
        if self.target_id.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingTargetId);
        }
        if self.lora_rank == 0 {
            return Err(OpenAdapterTrainingExecutionError::InvalidLoraRank {
                rank: self.lora_rank,
            });
        }
        if !self.lora_alpha.is_finite() || self.lora_alpha <= 0.0 {
            return Err(OpenAdapterTrainingExecutionError::InvalidLoraAlpha {
                alpha: self.lora_alpha,
            });
        }
        Ok(())
    }

    fn scale(&self) -> f32 {
        self.lora_alpha / self.lora_rank as f32
    }
}

/// Minimal frozen-base and trainable-adapter representation for the first open backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterReferenceModel {
    /// Stable base-model identifier.
    pub base_model_id: String,
    /// Stable base-model revision.
    pub base_model_revision: String,
    /// Stable base served-artifact digest the adapter targets.
    pub base_served_artifact_digest: String,
    /// Tokenizer digest that must match later serving/runtime bindings.
    pub tokenizer: TokenizerDigest,
    /// Hidden width surfaced by the admissible decoder family.
    pub hidden_size: usize,
    /// Vocabulary width surfaced by the admissible decoder family.
    pub vocab_size: usize,
    /// Trainable LM-head LoRA target.
    pub target: OpenAdapterLmHeadTarget,
}

impl OpenAdapterReferenceModel {
    fn validate(&self) -> Result<(), OpenAdapterTrainingExecutionError> {
        if self.base_model_id.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingBaseModelId);
        }
        if self.base_model_revision.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingBaseModelRevision);
        }
        if self.base_served_artifact_digest.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingBaseServedArtifactDigest);
        }
        if self.tokenizer.tokenizer_digest.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingTokenizerDigest);
        }
        if self.hidden_size == 0 || self.vocab_size == 0 {
            return Err(OpenAdapterTrainingExecutionError::InvalidModelShape {
                hidden_size: self.hidden_size,
                vocab_size: self.vocab_size,
            });
        }
        self.target.validate()
    }

    fn base_model_ref(&self) -> String {
        format!("{}@{}", self.base_model_id, self.base_model_revision)
    }
}

/// Full execution config for the first open adapter reference backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterExecutionConfig {
    /// Stable run identifier owned by the fixed-budget core.
    pub run_id: String,
    /// Stable checkpoint family label owned by the run.
    pub checkpoint_family: String,
    /// Stable backend label expected in cluster telemetry.
    pub execution_backend_label: String,
    /// Admissible model family implemented by this backend.
    pub admissible_model_family: OpenAdapterAdmissibleModelFamily,
    /// Fixed-budget policy reused by the training core.
    pub budget: TrainingLoopBudget,
    /// Deterministic sample batch size.
    pub batch_size: usize,
    /// Explicit precision posture.
    pub precision_policy: OpenAdapterPrecisionPolicy,
    /// Frozen-base and adapter-target layout.
    pub model: OpenAdapterReferenceModel,
}

impl OpenAdapterExecutionConfig {
    fn validate(&self) -> Result<(), OpenAdapterTrainingExecutionError> {
        if self.run_id.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingRunId);
        }
        if self.checkpoint_family.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingCheckpointFamily);
        }
        if self.execution_backend_label.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingExecutionBackendLabel);
        }
        if self.budget.max_steps == 0 {
            return Err(OpenAdapterTrainingExecutionError::InvalidBudget);
        }
        if self.batch_size == 0 {
            return Err(OpenAdapterTrainingExecutionError::InvalidBatchSize);
        }
        self.model.validate()?;
        match self.precision_policy {
            OpenAdapterPrecisionPolicy::F32Reference => Ok(()),
            unsupported => {
                Err(OpenAdapterTrainingExecutionError::UnsupportedPrecisionPolicy(unsupported))
            }
        }
    }
}

/// One bounded hidden-state supervision record for the first open adapter lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterHiddenStateSample {
    /// Stable sample identifier.
    pub sample_id: String,
    /// Frozen hidden-state vector that the adapter sees.
    pub hidden_state: Vec<f32>,
    /// Supervision token id the adapter should increase likelihood for.
    pub target_token_id: u32,
    /// Approximate source token count preserved for telemetry.
    pub source_token_count: u32,
    /// Stable digest over the sample contents.
    pub sample_digest: String,
}

impl OpenAdapterHiddenStateSample {
    /// Creates one hidden-state supervision sample.
    pub fn new(
        sample_id: impl Into<String>,
        hidden_state: Vec<f32>,
        target_token_id: u32,
        source_token_count: u32,
    ) -> Result<Self, OpenAdapterTrainingExecutionError> {
        let sample_id = sample_id.into();
        if sample_id.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingSampleId);
        }
        if hidden_state.is_empty() {
            return Err(OpenAdapterTrainingExecutionError::EmptyHiddenState {
                sample_id: sample_id.clone(),
            });
        }
        let sample_digest = stable_hidden_state_sample_digest(
            sample_id.as_str(),
            hidden_state.as_slice(),
            target_token_id,
            source_token_count,
        );
        Ok(Self {
            sample_id,
            hidden_state,
            target_token_id,
            source_token_count,
            sample_digest,
        })
    }
}

/// One deterministic packed batch for open adapter execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterPackedTrainingBatch {
    /// Stable batch identifier.
    pub batch_id: String,
    /// Samples assigned to the batch.
    pub samples: Vec<OpenAdapterHiddenStateSample>,
    /// Stable digest over the batch contents.
    pub batch_digest: String,
}

/// Machine-legible provenance frozen by the first open adapter reference backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAdapterExecutionProvenance {
    /// Stable backend label expected in cluster telemetry.
    pub execution_backend_label: String,
    /// Admissible model family for this backend.
    pub admissible_model_family: OpenAdapterAdmissibleModelFamily,
    /// Stable adapter family surfaced into decentralized control flow.
    pub adapter_family: String,
    /// Stable adapter format surfaced into decentralized control flow.
    pub adapter_format: String,
    /// Stable tokenizer digest carried by the model config.
    pub tokenizer_digest: String,
    /// Stable tokenizer contract digest carried by portable bundle lineage.
    pub tokenizer_contract_digest: String,
    /// Supervision sample count frozen into the backend.
    pub sample_count: usize,
}

impl OpenAdapterExecutionProvenance {
    /// Returns the stable digest over the execution provenance contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_open_adapter_execution_provenance|");
        hasher.update(self.execution_backend_label.as_bytes());
        hasher.update(b"|");
        hasher.update(open_adapter_family_label(self.admissible_model_family));
        hasher.update(b"|");
        hasher.update(self.adapter_family.as_bytes());
        hasher.update(b"|");
        hasher.update(self.adapter_format.as_bytes());
        hasher.update(b"|");
        hasher.update(self.tokenizer_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.tokenizer_contract_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.sample_count.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Gradient-production artifact emitted by the first open adapter backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterGradientBatchRecord {
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

/// Repo-owned open adapter hidden-state -> gradient backend.
#[derive(Clone, Debug)]
pub struct OpenAdapterTrainingExecutionBackend {
    config: OpenAdapterExecutionConfig,
    provenance: OpenAdapterExecutionProvenance,
    tokenizer: PortableTokenizerBinding,
    batches: Vec<OpenAdapterPackedTrainingBatch>,
    frozen_base_projection: Vec<f32>,
}

impl OpenAdapterTrainingExecutionBackend {
    /// Builds the repo-owned open adapter execution backend from bounded supervision samples.
    pub fn new(
        config: OpenAdapterExecutionConfig,
        samples: Vec<OpenAdapterHiddenStateSample>,
    ) -> Result<Self, OpenAdapterTrainingExecutionError> {
        config.validate()?;
        if samples.is_empty() {
            return Err(OpenAdapterTrainingExecutionError::EmptySamples);
        }
        for sample in &samples {
            if sample.hidden_state.len() != config.model.hidden_size {
                return Err(OpenAdapterTrainingExecutionError::HiddenSizeMismatch {
                    sample_id: sample.sample_id.clone(),
                    expected: config.model.hidden_size,
                    actual: sample.hidden_state.len(),
                });
            }
            if sample.target_token_id as usize >= config.model.vocab_size {
                return Err(OpenAdapterTrainingExecutionError::TargetTokenOutOfRange {
                    sample_id: sample.sample_id.clone(),
                    target_token_id: sample.target_token_id,
                    vocab_size: config.model.vocab_size,
                });
            }
        }

        let tokenizer = PortableTokenizerBinding::new(
            config.model.tokenizer.clone(),
            PortableTokenizerAssetFormat::PsionicDigest,
            config.model.base_model_ref(),
        );
        let batches = build_open_adapter_batches(samples, config.batch_size);
        let frozen_base_projection = seeded_matrix(
            format!(
                "{}|{}|base_projection|{}x{}",
                config.model.base_model_id,
                config.model.base_model_revision,
                config.model.vocab_size,
                config.model.hidden_size,
            )
            .as_str(),
            config.model.vocab_size,
            config.model.hidden_size,
            0.04,
        );

        Ok(Self {
            provenance: OpenAdapterExecutionProvenance {
                execution_backend_label: config.execution_backend_label.clone(),
                admissible_model_family: config.admissible_model_family,
                adapter_family: config.admissible_model_family.adapter_family().to_string(),
                adapter_format: config.admissible_model_family.adapter_format().to_string(),
                tokenizer_digest: config.model.tokenizer.tokenizer_digest.clone(),
                tokenizer_contract_digest: tokenizer.contract_digest(),
                sample_count: batches.iter().map(|batch| batch.samples.len()).sum(),
            },
            config,
            tokenizer,
            batches,
            frozen_base_projection,
        })
    }

    /// Returns the immutable execution config.
    #[must_use]
    pub fn config(&self) -> &OpenAdapterExecutionConfig {
        &self.config
    }

    /// Returns machine-legible provenance for later summary/export layers.
    #[must_use]
    pub fn provenance(&self) -> &OpenAdapterExecutionProvenance {
        &self.provenance
    }

    /// Returns the packed open-adapter training batches.
    #[must_use]
    pub fn batches(&self) -> &[OpenAdapterPackedTrainingBatch] {
        self.batches.as_slice()
    }

    /// Returns the adapter target identity used by the generic decentralized control plane.
    pub fn adapter_target_identity(
        &self,
    ) -> Result<AdapterTargetIdentity, AdapterWindowContractError> {
        AdapterTargetIdentity::new(
            self.config.model.target.target_id.clone(),
            self.config.admissible_model_family.adapter_family(),
            self.config.model.base_model_ref(),
            self.config.admissible_model_family.adapter_format(),
        )
    }

    /// Returns the contributor capability policy expected for this backend on the cluster plane.
    #[must_use]
    pub fn contributor_capability_policy(
        &self,
        minimum_free_memory_bytes: u64,
    ) -> AdapterContributorCapabilityPolicy {
        AdapterContributorCapabilityPolicy {
            backend_label: self.config.execution_backend_label.clone(),
            minimum_free_memory_bytes,
            require_accelerator: true,
            allow_degraded_backend: false,
            allow_flaky_nodes: false,
        }
    }

    /// Builds the initial trainable adapter groups with frozen-base semantics.
    pub fn initial_training_groups(
        &self,
    ) -> Result<Vec<TrainingParameterGroupState>, OpenAdapterTrainingExecutionError> {
        let target = &self.config.model.target;
        let group_a_id = target.lora_a_group_id();
        let group_b_id = target.lora_b_group_id();
        Ok(vec![
            TrainingParameterGroupState::new(
                group_a_id.clone(),
                TrainingParameterClass::Matrix,
                TrainingTensorBuffer::from_f32(
                    group_a_id.clone(),
                    lora_a_spec(target.lora_rank, self.config.model.hidden_size),
                    seeded_matrix(
                        format!(
                            "{}|{}|{}|lora_a",
                            self.config.model.base_model_id,
                            self.config.model.base_model_revision,
                            target.target_id,
                        )
                        .as_str(),
                        target.lora_rank,
                        self.config.model.hidden_size,
                        0.02,
                    ),
                )?,
                target.optimizer.clone(),
                target.optimizer_residency_policy,
            )?,
            TrainingParameterGroupState::new(
                group_b_id.clone(),
                TrainingParameterClass::Matrix,
                TrainingTensorBuffer::from_f32(
                    group_b_id.clone(),
                    lora_b_spec(self.config.model.vocab_size, target.lora_rank),
                    seeded_matrix(
                        format!(
                            "{}|{}|{}|lora_b",
                            self.config.model.base_model_id,
                            self.config.model.base_model_revision,
                            target.target_id,
                        )
                        .as_str(),
                        self.config.model.vocab_size,
                        target.lora_rank,
                        0.02,
                    ),
                )?,
                target.optimizer.clone(),
                target.optimizer_residency_policy,
            )?,
        ])
    }

    /// Creates a fresh fixed-budget run seeded with trainable adapter-only groups.
    pub fn initialize_run(
        &self,
    ) -> Result<FixedBudgetTrainingRun, OpenAdapterTrainingExecutionError> {
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
    ) -> Result<Vec<TrainingParameterGroupState>, OpenAdapterTrainingExecutionError> {
        let target = &self.config.model.target;
        Ok(vec![
            self.training_group(run, target.lora_a_group_id().as_str())?
                .clone(),
            self.training_group(run, target.lora_b_group_id().as_str())?
                .clone(),
        ])
    }

    /// Produces one gradient batch for the requested packed batch.
    pub fn produce_gradient_batch(
        &self,
        run: &FixedBudgetTrainingRun,
        batch_index: usize,
    ) -> Result<OpenAdapterGradientBatchRecord, OpenAdapterTrainingExecutionError> {
        let batch = self
            .batches
            .get(batch_index)
            .ok_or(OpenAdapterTrainingExecutionError::UnknownBatchIndex { batch_index })?;
        let target = &self.config.model.target;
        let group_a_id = target.lora_a_group_id();
        let group_b_id = target.lora_b_group_id();
        let group_a = self.training_group(run, group_a_id.as_str())?;
        let group_b = self.training_group(run, group_b_id.as_str())?;
        let a_values = dense_values(group_a, group_a_id.as_str())?;
        let b_values = dense_values(group_b, group_b_id.as_str())?;
        let mut grad_a = vec![0.0_f32; a_values.len()];
        let mut grad_b = vec![0.0_f32; b_values.len()];
        let mut mean_loss = 0.0_f32;

        for sample in &batch.samples {
            let forward = self.forward_sample(sample, a_values, b_values);
            mean_loss += forward.loss;
            accumulate_lm_head_gradients(
                grad_a.as_mut_slice(),
                grad_b.as_mut_slice(),
                b_values,
                sample.hidden_state.as_slice(),
                forward.intermediate.as_slice(),
                forward.logits_gradient.as_slice(),
                target.scale(),
            );
        }

        let scale = 1.0_f32 / batch.samples.len() as f32;
        for value in &mut grad_a {
            *value *= scale;
        }
        for value in &mut grad_b {
            *value *= scale;
        }

        mean_loss *= scale;
        let gradient_norms_l2 = BTreeMap::from([
            (group_a_id.clone(), l2_norm(grad_a.as_slice())),
            (group_b_id.clone(), l2_norm(grad_b.as_slice())),
        ]);
        let training_batch = TrainingGradientBatch::new(
            format!("{}-gradient", batch.batch_id),
            mean_loss,
            batch.samples.len() as u32,
            BTreeMap::from([
                (
                    group_a_id.clone(),
                    TrainingTensorBuffer::from_f32(
                        group_a_id.clone(),
                        group_a.parameter.spec.clone(),
                        grad_a,
                    )?,
                ),
                (
                    group_b_id.clone(),
                    TrainingTensorBuffer::from_f32(
                        group_b_id.clone(),
                        group_b.parameter.spec.clone(),
                        grad_b,
                    )?,
                ),
            ]),
        );
        let execution_digest = stable_gradient_execution_digest(
            batch.batch_id.as_str(),
            batch.batch_digest.as_str(),
            &gradient_norms_l2,
            mean_loss,
        );
        Ok(OpenAdapterGradientBatchRecord {
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
        (TrainingStepInput, OpenAdapterGradientBatchRecord),
        OpenAdapterTrainingExecutionError,
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
    ) -> Result<&'a TrainingParameterGroupState, OpenAdapterTrainingExecutionError> {
        run.parameter_group(group_id).ok_or_else(|| {
            OpenAdapterTrainingExecutionError::MissingParameterGroup {
                group_id: group_id.to_string(),
            }
        })
    }

    fn forward_sample(
        &self,
        sample: &OpenAdapterHiddenStateSample,
        a_values: &[f32],
        b_values: &[f32],
    ) -> OpenAdapterForwardRecord {
        let mut logits = mat_vec(
            self.frozen_base_projection.as_slice(),
            self.config.model.vocab_size,
            self.config.model.hidden_size,
            sample.hidden_state.as_slice(),
        );
        let intermediate = mat_vec(
            a_values,
            self.config.model.target.lora_rank,
            self.config.model.hidden_size,
            sample.hidden_state.as_slice(),
        );
        let adapter_logits = mat_vec(
            b_values,
            self.config.model.vocab_size,
            self.config.model.target.lora_rank,
            intermediate.as_slice(),
        );
        add_scaled(
            logits.as_mut_slice(),
            adapter_logits.as_slice(),
            self.config.model.target.scale(),
        );
        let distribution = softmax(logits.as_slice());
        let mut logits_gradient = distribution.clone();
        if let Some(target) = logits_gradient.get_mut(sample.target_token_id as usize) {
            *target -= 1.0;
        }
        let loss = -distribution[sample.target_token_id as usize]
            .max(f32::EPSILON)
            .ln();
        OpenAdapterForwardRecord {
            intermediate,
            logits_gradient,
            loss,
        }
    }
}

#[derive(Clone, Debug)]
struct OpenAdapterForwardRecord {
    intermediate: Vec<f32>,
    logits_gradient: Vec<f32>,
    loss: f32,
}

/// Higher-level export request layered on top of the repo-owned open adapter backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterSftRunRequest {
    /// Stable dataset ref carried into export lineage.
    pub dataset_ref: String,
    /// Stable validator policy ref carried into export lineage.
    pub validator_policy_ref: String,
    /// Stable adapter identifier for the exported artifact.
    pub adapter_id: String,
    /// Stable adapter revision for the exported artifact.
    pub adapter_revision: String,
    /// Logical training start timestamp for the first step.
    pub started_at_ms: u64,
    /// Duration applied to each produced step receipt.
    pub step_duration_ms: u64,
}

impl OpenAdapterSftRunRequest {
    fn validate(&self) -> Result<(), OpenAdapterSftError> {
        if self.dataset_ref.trim().is_empty() {
            return Err(OpenAdapterSftError::MissingDatasetRef);
        }
        if self.validator_policy_ref.trim().is_empty() {
            return Err(OpenAdapterSftError::MissingValidatorPolicyRef);
        }
        if self.adapter_id.trim().is_empty() {
            return Err(OpenAdapterSftError::MissingAdapterId);
        }
        if self.adapter_revision.trim().is_empty() {
            return Err(OpenAdapterSftError::MissingAdapterRevision);
        }
        if self.step_duration_ms == 0 {
            return Err(OpenAdapterSftError::InvalidStepDuration);
        }
        Ok(())
    }
}

/// Typed training summary emitted by the higher-level open adapter lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterSftTrainingSummary {
    /// Fixed-budget training summary.
    pub run_summary: TrainingRunSummary,
    /// Repo-owned open adapter execution provenance.
    pub execution_provenance: OpenAdapterExecutionProvenance,
    /// Stable dataset ref carried into export lineage.
    pub dataset_ref: String,
    /// Stable validator policy ref carried into export lineage.
    pub validator_policy_ref: String,
    /// Stable adapter-artifact digest.
    pub adapter_artifact_digest: String,
    /// Stable adapter-identity digest.
    pub adapter_identity_digest: String,
    /// Stable initial adapter state-dict digest.
    pub initial_state_dict_digest: String,
    /// Stable final adapter state-dict digest.
    pub final_state_dict_digest: String,
    /// LoRA alpha needed to reload the exported adapter.
    pub lora_alpha: f32,
}

/// Full higher-level open adapter outcome including exported `safetensors`.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenAdapterSftRunOutcome {
    /// Step receipts emitted by the fixed-budget core.
    pub step_receipts: Vec<TrainingStepReceipt>,
    /// Gradient-production records emitted by the repo-owned open backend.
    pub gradient_records: Vec<OpenAdapterGradientBatchRecord>,
    /// Summary and reproducibility metadata.
    pub summary: OpenAdapterSftTrainingSummary,
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
    /// Stable adapter identity for the exported `safetensors`.
    pub adapter_identity: AdapterArtifactIdentity,
    /// Final `safetensors` artifact held in memory.
    pub adapter_bytes: Vec<u8>,
}

impl OpenAdapterSftRunOutcome {
    /// Writes the final `safetensors` artifact to disk.
    pub fn write_artifact_to_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), OpenAdapterSftError> {
        std::fs::write(path.as_ref(), self.adapter_bytes.as_slice()).map_err(|error| {
            OpenAdapterSftError::WriteArtifact {
                path: path.as_ref().display().to_string(),
                message: error.to_string(),
            }
        })
    }

    /// Reloads the exported adapter through the shared adapter-runtime parser.
    pub fn load_lm_head_lora_artifact(
        &self,
    ) -> Result<LmHeadLoraAdapterArtifact, OpenAdapterSftError> {
        Ok(LmHeadLoraAdapterArtifact::from_safetensors_bytes(
            self.adapter_bytes.as_slice(),
            self.adapter_identity.clone(),
            self.summary.lora_alpha,
        )?)
    }
}

/// Runs the first honest non-Apple open adapter SFT/export lane.
pub fn run_open_adapter_sft_export(
    backend: &OpenAdapterTrainingExecutionBackend,
    request: &OpenAdapterSftRunRequest,
) -> Result<OpenAdapterSftRunOutcome, OpenAdapterSftError> {
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
    let initial_bundle = PortableModelBundle::from_training_groups(
        backend.config().admissible_model_family.adapter_family(),
        backend.config().model.base_model_revision.clone(),
        backend.config().checkpoint_family.clone(),
        Some(format!("checkpoint://{}/initial", backend.config().run_id)),
        initial_groups.as_slice(),
        backend.tokenizer.clone(),
        backend.config().model.tokenizer.template_digest.clone(),
    )?;
    let final_bundle = PortableModelBundle::from_training_groups(
        backend.config().admissible_model_family.adapter_family(),
        backend.config().model.base_model_revision.clone(),
        backend.config().checkpoint_family.clone(),
        Some(format!("checkpoint://{}/final", backend.config().run_id)),
        final_groups.as_slice(),
        backend.tokenizer.clone(),
        backend.config().model.tokenizer.template_digest.clone(),
    )?;
    let (_, initial_bundle_receipt) = initial_bundle.export_safetensors()?;
    let (_, final_bundle_receipt) = final_bundle.export_safetensors()?;
    let adapter_delta = crate::PortableModelStateDict::derive_adapter_delta(
        &initial_bundle.state_dict,
        &final_bundle.state_dict,
        request.adapter_id.clone(),
    )?;
    let manifest = OpenAdapterSafetensorsManifest {
        abi_version: String::from("openagents.open_adapter.safetensors.v1"),
        execution_backend_label: backend.config().execution_backend_label.clone(),
        admissible_model_family: backend.config().admissible_model_family,
        adapter_family: backend
            .config()
            .admissible_model_family
            .adapter_family()
            .to_string(),
        adapter_format: backend
            .config()
            .admissible_model_family
            .adapter_format()
            .to_string(),
        dataset_ref: request.dataset_ref.clone(),
        validator_policy_ref: request.validator_policy_ref.clone(),
        adapter_id: request.adapter_id.clone(),
        adapter_revision: request.adapter_revision.clone(),
        base_model_id: backend.config().model.base_model_id.clone(),
        base_model_revision: backend.config().model.base_model_revision.clone(),
        base_served_artifact_digest: backend.config().model.base_served_artifact_digest.clone(),
        tokenizer_digest: backend.config().model.tokenizer.tokenizer_digest.clone(),
        tokenizer_contract_digest: backend.tokenizer.contract_digest(),
        checkpoint_family: backend.config().checkpoint_family.clone(),
        run_id: backend.config().run_id.clone(),
        lora_rank: backend.config().model.target.lora_rank,
        lora_alpha: backend.config().model.target.lora_alpha,
        hidden_size: backend.config().model.hidden_size,
        vocab_size: backend.config().model.vocab_size,
        final_state_dict_digest: final_bundle_receipt.state_dict_digest.clone(),
        execution_provenance_digest: backend.provenance().stable_digest(),
    };
    let adapter_bytes = export_open_adapter_safetensors(backend, &run, &manifest)?;
    let adapter_artifact_digest = hex::encode(Sha256::digest(adapter_bytes.as_slice()));
    let adapter_identity = AdapterArtifactIdentity::new(
        request.adapter_id.clone(),
        request.adapter_revision.clone(),
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        backend.config().model.base_model_id.clone(),
        backend.config().model.base_model_revision.clone(),
        backend.config().model.base_served_artifact_digest.clone(),
        adapter_artifact_digest.clone(),
        QuantizationMode::None,
        AdapterTargetFamily::DecoderComposite,
        u64::try_from(backend.config().model.target.lora_rank.saturating_mul(
            backend.config().model.hidden_size + backend.config().model.vocab_size,
        ))
        .unwrap_or(u64::MAX),
    )
    .with_provenance_digest(backend.provenance().stable_digest())
    .with_governance_digest(stable_governance_digest(
        request.dataset_ref.as_str(),
        request.validator_policy_ref.as_str(),
    ));
    let summary = OpenAdapterSftTrainingSummary {
        run_summary: run.summary(),
        execution_provenance: backend.provenance().clone(),
        dataset_ref: request.dataset_ref.clone(),
        validator_policy_ref: request.validator_policy_ref.clone(),
        adapter_artifact_digest,
        adapter_identity_digest: adapter_identity.stable_digest(),
        initial_state_dict_digest: initial_bundle_receipt.state_dict_digest.clone(),
        final_state_dict_digest: final_bundle_receipt.state_dict_digest.clone(),
        lora_alpha: backend.config().model.target.lora_alpha,
    };

    let outcome = OpenAdapterSftRunOutcome {
        step_receipts,
        gradient_records,
        summary,
        initial_bundle,
        final_bundle,
        initial_bundle_receipt,
        final_bundle_receipt,
        adapter_delta,
        adapter_identity,
        adapter_bytes,
    };
    outcome.load_lm_head_lora_artifact()?;
    Ok(outcome)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct OpenAdapterSafetensorsManifest {
    abi_version: String,
    execution_backend_label: String,
    admissible_model_family: OpenAdapterAdmissibleModelFamily,
    adapter_family: String,
    adapter_format: String,
    dataset_ref: String,
    validator_policy_ref: String,
    adapter_id: String,
    adapter_revision: String,
    base_model_id: String,
    base_model_revision: String,
    base_served_artifact_digest: String,
    tokenizer_digest: String,
    tokenizer_contract_digest: String,
    checkpoint_family: String,
    run_id: String,
    lora_rank: usize,
    lora_alpha: f32,
    hidden_size: usize,
    vocab_size: usize,
    final_state_dict_digest: String,
    execution_provenance_digest: String,
}

fn build_open_adapter_batches(
    samples: Vec<OpenAdapterHiddenStateSample>,
    batch_size: usize,
) -> Vec<OpenAdapterPackedTrainingBatch> {
    samples
        .chunks(batch_size)
        .enumerate()
        .map(|(index, batch_samples)| OpenAdapterPackedTrainingBatch {
            batch_id: format!("open-adapter-batch-{}", index + 1),
            batch_digest: stable_batch_digest(
                format!("open-adapter-batch-{}", index + 1).as_str(),
                batch_samples
                    .iter()
                    .map(|sample| sample.sample_digest.as_str())
                    .collect::<Vec<_>>()
                    .as_slice(),
            ),
            samples: batch_samples.to_vec(),
        })
        .collect()
}

fn export_open_adapter_safetensors(
    backend: &OpenAdapterTrainingExecutionBackend,
    run: &FixedBudgetTrainingRun,
    manifest: &OpenAdapterSafetensorsManifest,
) -> Result<Vec<u8>, OpenAdapterSftError> {
    let target = &backend.config().model.target;
    let group_a = backend.training_group(run, target.lora_a_group_id().as_str())?;
    let group_b = backend.training_group(run, target.lora_b_group_id().as_str())?;
    let a_values = dense_values(group_a, target.lora_a_group_id().as_str())?;
    let b_values = dense_values(group_b, target.lora_b_group_id().as_str())?;
    let manifest_json =
        serde_json::to_string(manifest).map_err(|error| OpenAdapterSftError::Serialization {
            context: "open adapter manifest export",
            message: error.to_string(),
        })?;
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from(OPEN_ADAPTER_SAFETENSORS_MANIFEST_KEY),
        manifest_json,
    );

    let raw_a = encode_f32_bytes(a_values);
    let raw_b = encode_f32_bytes(b_values);
    let view_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![
            backend.config().model.target.lora_rank,
            backend.config().model.hidden_size,
        ],
        raw_a.as_slice(),
    )
    .map_err(safetensors_error)?;
    let view_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![
            backend.config().model.vocab_size,
            backend.config().model.target.lora_rank,
        ],
        raw_b.as_slice(),
    )
    .map_err(safetensors_error)?;
    serialize(
        [
            ("lm_head.lora_A.weight", view_a),
            ("lm_head.lora_B.weight", view_b),
        ],
        Some(metadata),
    )
    .map_err(safetensors_error)
}

fn accumulate_lm_head_gradients(
    grad_a: &mut [f32],
    grad_b: &mut [f32],
    b_values: &[f32],
    hidden_state: &[f32],
    intermediate: &[f32],
    logits_gradient: &[f32],
    scale: f32,
) {
    let rank = intermediate.len();
    let hidden_size = hidden_state.len();
    for (token_index, token_gradient) in logits_gradient.iter().enumerate() {
        for rank_index in 0..rank {
            grad_b[token_index * rank + rank_index] +=
                token_gradient * intermediate[rank_index] * scale;
        }
    }
    let mut propagated = vec![0.0_f32; rank];
    for rank_index in 0..rank {
        for (token_index, token_gradient) in logits_gradient.iter().enumerate() {
            propagated[rank_index] +=
                b_values[token_index * rank + rank_index] * token_gradient * scale;
        }
    }
    for rank_index in 0..rank {
        for hidden_index in 0..hidden_size {
            grad_a[rank_index * hidden_size + hidden_index] +=
                propagated[rank_index] * hidden_state[hidden_index];
        }
    }
}

fn dense_values<'a>(
    group: &'a TrainingParameterGroupState,
    group_id: &str,
) -> Result<&'a [f32], OpenAdapterTrainingExecutionError> {
    match &group.parameter.data {
        psionic_core::TensorData::F32(values) => Ok(values.as_slice()),
        psionic_core::TensorData::QuantizedBlocks(_) => {
            Err(OpenAdapterTrainingExecutionError::NonDenseGroup {
                group_id: group_id.to_string(),
            })
        }
    }
}

fn lora_a_spec(rank: usize, hidden_size: usize) -> TensorSpec {
    TensorSpec::new(
        Shape::new(vec![rank, hidden_size]),
        DType::F32,
        Device::cpu(),
    )
}

fn lora_b_spec(vocab_size: usize, rank: usize) -> TensorSpec {
    TensorSpec::new(
        Shape::new(vec![vocab_size, rank]),
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

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = logits
        .iter()
        .map(|value| (*value - max).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(f32::EPSILON);
    exp.into_iter().map(|value| value / sum).collect()
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
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

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * std::mem::size_of::<f32>());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn stable_hidden_state_sample_digest(
    sample_id: &str,
    hidden_state: &[f32],
    target_token_id: u32,
    source_token_count: u32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_open_adapter_sample|");
    hasher.update(sample_id.as_bytes());
    hasher.update(b"|");
    hasher.update(target_token_id.to_le_bytes());
    hasher.update(b"|");
    hasher.update(source_token_count.to_le_bytes());
    for value in hidden_state {
        hasher.update(b"|");
        hasher.update(value.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_batch_digest(batch_id: &str, sample_digests: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_open_adapter_batch|");
    hasher.update(batch_id.as_bytes());
    for digest in sample_digests {
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
    hasher.update(b"psionic_open_adapter_gradient_execution|");
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

fn stable_governance_digest(dataset_ref: &str, validator_policy_ref: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_open_adapter_governance|");
    hasher.update(dataset_ref.as_bytes());
    hasher.update(b"|");
    hasher.update(validator_policy_ref.as_bytes());
    hex::encode(hasher.finalize())
}

fn open_adapter_family_label(family: OpenAdapterAdmissibleModelFamily) -> &'static [u8] {
    match family {
        OpenAdapterAdmissibleModelFamily::GptOssDecoderLmHeadLora => {
            b"gpt_oss_decoder_lm_head_lora"
        }
    }
}

fn safetensors_error(error: safetensors::SafeTensorError) -> OpenAdapterSftError {
    OpenAdapterSftError::Serialization {
        context: "open adapter safetensors export",
        message: error.to_string(),
    }
}

/// Error surfaced by the repo-owned open adapter execution backend.
#[derive(Debug, Error)]
pub enum OpenAdapterTrainingExecutionError {
    #[error("open adapter execution config is missing `run_id`")]
    MissingRunId,
    #[error("open adapter execution config is missing `checkpoint_family`")]
    MissingCheckpointFamily,
    #[error("open adapter execution config is missing `execution_backend_label`")]
    MissingExecutionBackendLabel,
    #[error("open adapter execution config requires `batch_size > 0`")]
    InvalidBatchSize,
    #[error("open adapter execution config requires `budget.max_steps > 0`")]
    InvalidBudget,
    #[error("open adapter backend does not yet support precision policy `{0:?}`")]
    UnsupportedPrecisionPolicy(OpenAdapterPrecisionPolicy),
    #[error("open adapter model is missing `base_model_id`")]
    MissingBaseModelId,
    #[error("open adapter model is missing `base_model_revision`")]
    MissingBaseModelRevision,
    #[error("open adapter model is missing `base_served_artifact_digest`")]
    MissingBaseServedArtifactDigest,
    #[error("open adapter model is missing tokenizer digest")]
    MissingTokenizerDigest,
    #[error(
        "open adapter model requires `hidden_size > 0` and `vocab_size > 0`, found hidden_size={hidden_size} vocab_size={vocab_size}"
    )]
    InvalidModelShape {
        hidden_size: usize,
        vocab_size: usize,
    },
    #[error("open adapter target is missing `target_id`")]
    MissingTargetId,
    #[error("open adapter target requires `lora_rank > 0`, found {rank}")]
    InvalidLoraRank { rank: usize },
    #[error("open adapter target requires positive finite `lora_alpha`, found {alpha}")]
    InvalidLoraAlpha { alpha: f32 },
    #[error("open adapter training requires at least one supervision sample")]
    EmptySamples,
    #[error("open adapter sample is missing `sample_id`")]
    MissingSampleId,
    #[error("open adapter sample `{sample_id}` has an empty hidden state")]
    EmptyHiddenState { sample_id: String },
    #[error(
        "open adapter sample `{sample_id}` hidden width mismatch: expected {expected}, found {actual}"
    )]
    HiddenSizeMismatch {
        sample_id: String,
        expected: usize,
        actual: usize,
    },
    #[error(
        "open adapter sample `{sample_id}` target token `{target_token_id}` exceeds vocab size {vocab_size}"
    )]
    TargetTokenOutOfRange {
        sample_id: String,
        target_token_id: u32,
        vocab_size: usize,
    },
    #[error("open adapter requested unknown batch index `{batch_index}`")]
    UnknownBatchIndex { batch_index: usize },
    #[error("open adapter run is missing parameter group `{group_id}`")]
    MissingParameterGroup { group_id: String },
    #[error("open adapter parameter group `{group_id}` is not dense f32 data")]
    NonDenseGroup { group_id: String },
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
}

/// Error surfaced by the higher-level open adapter SFT/export lane.
#[derive(Debug, Error)]
pub enum OpenAdapterSftError {
    #[error("open adapter SFT export request is missing `dataset_ref`")]
    MissingDatasetRef,
    #[error("open adapter SFT export request is missing `validator_policy_ref`")]
    MissingValidatorPolicyRef,
    #[error("open adapter SFT export request is missing `adapter_id`")]
    MissingAdapterId,
    #[error("open adapter SFT export request is missing `adapter_revision`")]
    MissingAdapterRevision,
    #[error("open adapter SFT export request requires `step_duration_ms > 0`")]
    InvalidStepDuration,
    #[error("failed to serialize {context}: {message}")]
    Serialization {
        context: &'static str,
        message: String,
    },
    #[error("failed to write exported open adapter artifact `{path}`: {message}")]
    WriteArtifact { path: String, message: String },
    #[error(transparent)]
    TrainingExecution(#[from] OpenAdapterTrainingExecutionError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    ModelIo(#[from] crate::ModelIoError),
    #[error(transparent)]
    AdapterLoad(#[from] LmHeadLoraLoadError),
}

#[cfg(test)]
mod tests {
    use psionic_cluster::{
        AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
        ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
        ClusterSnapshot, ClusterStabilityPosture, ClusterState, NodeEpoch, NodeId, NodeRole,
    };
    use psionic_data::{TokenizerDigest, TokenizerFamily};

    use super::*;
    use crate::{
        AdapterDatasetSliceIdentity, AdapterTrainingClusterCoordinator, CheckpointPointer,
        CheckpointScopeBinding, CheckpointScopeKind, PolicyRevision, TrainingRunState,
    };

    const GIB_BYTES: u64 = 1024 * 1024 * 1024;

    fn sample_tokenizer() -> TokenizerDigest {
        TokenizerDigest::new(TokenizerFamily::BytePairEncoding, "gpt-oss-tok-v1", 32)
            .with_template_digest("gpt-oss-template-v1")
    }

    fn config() -> OpenAdapterExecutionConfig {
        OpenAdapterExecutionConfig {
            run_id: "open-adapter-run".to_string(),
            checkpoint_family: "open_adapter.reference".to_string(),
            execution_backend_label: OPEN_ADAPTER_CUDA_BACKEND_LABEL.to_string(),
            admissible_model_family: OpenAdapterAdmissibleModelFamily::GptOssDecoderLmHeadLora,
            budget: TrainingLoopBudget::new(8, 1, 1).expect("budget"),
            batch_size: 2,
            precision_policy: OpenAdapterPrecisionPolicy::F32Reference,
            model: OpenAdapterReferenceModel {
                base_model_id: "gpt-oss-20b".to_string(),
                base_model_revision: "2026-03".to_string(),
                base_served_artifact_digest: "sha256:gpt-oss-base".to_string(),
                tokenizer: sample_tokenizer(),
                hidden_size: 4,
                vocab_size: 4,
                target: OpenAdapterLmHeadTarget {
                    target_id: "lm_head".to_string(),
                    lora_rank: 2,
                    lora_alpha: 8.0,
                    optimizer: TrainingOptimizerConfig::adamw(0.2, 0.9, 0.99, 1e-8)
                        .with_gradient_clip_norm(1.0),
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
            },
        }
    }

    fn samples() -> Vec<OpenAdapterHiddenStateSample> {
        vec![
            OpenAdapterHiddenStateSample::new("sample-a", vec![1.0, 0.0, 0.0, 0.0], 2, 12)
                .expect("sample"),
            OpenAdapterHiddenStateSample::new("sample-b", vec![0.0, 1.0, 0.0, 0.0], 3, 11)
                .expect("sample"),
            OpenAdapterHiddenStateSample::new("sample-c", vec![1.0, 0.0, 0.0, 0.0], 2, 10)
                .expect("sample"),
            OpenAdapterHiddenStateSample::new("sample-d", vec![0.0, 1.0, 0.0, 0.0], 3, 9)
                .expect("sample"),
        ]
    }

    fn cluster_state(
        memberships: &[(&str, NodeRole, ClusterMembershipStatus)],
        telemetry: &[(&str, u64, u16, ClusterBackendReadinessStatus)],
    ) -> ClusterState {
        let cluster_id = ClusterId::new(
            &ClusterNamespace::new("open-adapter-cluster"),
            &AdmissionToken::new("shared-secret"),
        );
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = memberships
            .iter()
            .map(|(node_id, role, status)| {
                (
                    NodeId::new(*node_id),
                    ClusterMembershipRecord::new(
                        ClusterNodeIdentity {
                            cluster_id: cluster_id.clone(),
                            node_id: NodeId::new(*node_id),
                            node_epoch: NodeEpoch::initial(),
                            role: *role,
                            auth_public_key: format!("{node_id}-pk"),
                            attestation: None,
                        },
                        None,
                        *status,
                    ),
                )
            })
            .collect();
        snapshot.telemetry = telemetry
            .iter()
            .map(
                |(node_id, free_memory_gib, accelerator_count, backend_status)| {
                    (
                        NodeId::new(*node_id),
                        ClusterNodeTelemetry::new(NodeId::new(*node_id))
                            .with_memory(
                                Some(free_memory_gib.saturating_mul(GIB_BYTES)),
                                Some(free_memory_gib.saturating_mul(GIB_BYTES)),
                            )
                            .with_accelerator_count(*accelerator_count)
                            .with_backend_readiness(
                                OPEN_ADAPTER_CUDA_BACKEND_LABEL,
                                *backend_status,
                            )
                            .with_stability_posture(ClusterStabilityPosture::Stable),
                    )
                },
            )
            .collect();
        ClusterState::from_snapshot(snapshot)
    }

    fn checkpoint_reference(
        checkpoint_ref: &str,
        started_at_ms: u64,
    ) -> psionic_runtime::TrainingCheckpointReference {
        psionic_runtime::TrainingCheckpointReference::new(
            "open.adapter.policy",
            format!("stream://{checkpoint_ref}"),
            format!("manifest://{checkpoint_ref}"),
            format!("object://{checkpoint_ref}"),
            "node-a",
            3,
            "cluster-digest-open-adapter",
            "topology-digest-open-adapter",
            started_at_ms,
        )
        .with_checkpoint_ref(checkpoint_ref)
        .with_step(12)
    }

    #[test]
    fn open_adapter_backend_produces_repo_owned_gradients_and_steps()
    -> Result<(), Box<dyn std::error::Error>> {
        let backend = OpenAdapterTrainingExecutionBackend::new(config(), samples())?;
        assert_eq!(backend.batches().len(), 2);
        assert_eq!(
            backend.provenance().adapter_family,
            OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY
        );

        let mut run = backend.initialize_run()?;
        let (step_input, gradient_record) = backend.produce_step_input(&run, 0, 1_000, 1_020)?;
        assert_eq!(gradient_record.training_batch.sample_count, 2);
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
        assert_eq!(run.completed_steps(), 1);
        Ok(())
    }

    #[test]
    fn open_adapter_sft_lane_exports_loadable_lm_head_lora_artifact()
    -> Result<(), Box<dyn std::error::Error>> {
        let backend = OpenAdapterTrainingExecutionBackend::new(config(), samples())?;
        let outcome = run_open_adapter_sft_export(
            &backend,
            &OpenAdapterSftRunRequest {
                dataset_ref: "dataset://openagents/open_adapter_reference@2026.03".to_string(),
                validator_policy_ref: "policy://validator/open_adapter/gpt_oss_decoder_lm_head"
                    .to_string(),
                adapter_id: "explainer-open-adapter".to_string(),
                adapter_revision: "r1".to_string(),
                started_at_ms: 2_000,
                step_duration_ms: 25,
            },
        )?;
        assert_eq!(outcome.step_receipts.len(), 8);
        assert!(!outcome.adapter_bytes.is_empty());
        assert_eq!(
            outcome.adapter_identity.format,
            AdapterArtifactFormat::Safetensors
        );

        let adapter = outcome.load_lm_head_lora_artifact()?;
        let mut logits = vec![0.0_f32; backend.config().model.vocab_size];
        adapter.apply_to_logits(&[1.0, 0.0, 0.0, 0.0], logits.as_mut_slice())?;
        let target = logits
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.partial_cmp(right.1).expect("finite logits"))
            .map(|(index, _)| index)
            .expect("non-empty logits");
        assert_eq!(target, 2);
        assert_eq!(
            outcome.summary.execution_provenance.adapter_format,
            OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT
        );
        Ok(())
    }

    #[test]
    fn open_adapter_backend_reuses_generic_cluster_window_planning()
    -> Result<(), Box<dyn std::error::Error>> {
        let backend = OpenAdapterTrainingExecutionBackend::new(config(), samples())?;
        let state = cluster_state(
            &[
                (
                    "trainer-a",
                    NodeRole::CoordinatorOnly,
                    ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-a",
                    NodeRole::ExecutorOnly,
                    ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-b",
                    NodeRole::ExecutorOnly,
                    ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-c",
                    NodeRole::ExecutorOnly,
                    ClusterMembershipStatus::Ready,
                ),
            ],
            &[
                ("trainer-a", 24, 1, ClusterBackendReadinessStatus::Ready),
                ("worker-a", 12, 1, ClusterBackendReadinessStatus::Ready),
                ("worker-b", 28, 1, ClusterBackendReadinessStatus::Ready),
                ("worker-c", 8, 1, ClusterBackendReadinessStatus::Ready),
            ],
        );
        let run = TrainingRunState::new(
            "open-adapter-window-run",
            "adapter-sft",
            state.cluster_id().as_str(),
            "open.adapter.policy",
            psionic_environments::EnvironmentPackageKey::new("oa.open.adapter", "2026.03"),
        )?;
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            run,
            backend.adapter_target_identity()?,
            PolicyRevision::new(
                "open.adapter.policy",
                "policy-r3",
                "policy-digest-r3",
                1_000,
            ),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-open-adapter-1"),
                "open.adapter.policy",
                checkpoint_reference("checkpoint/open_adapter/r3", 1_000),
                "manifest-digest-r3",
                1_001,
            )?,
            backend.contributor_capability_policy(10 * GIB_BYTES),
        );
        let receipt = coordinator.observe_cluster_state(&state, 1_010)?.clone();
        assert_eq!(
            receipt
                .contributor_statuses
                .iter()
                .find(|status| status.node_id == "worker-c")
                .expect("worker-c exists")
                .eligibility,
            crate::AdapterContributorEligibility::InsufficientFreeMemory
        );

        let window = coordinator.plan_next_window(
            vec![
                AdapterDatasetSliceIdentity::new(
                    "dataset.open_adapter",
                    "train",
                    "slice-a",
                    "slice-digest-a",
                )?,
                AdapterDatasetSliceIdentity::new(
                    "dataset.open_adapter",
                    "train",
                    "slice-b",
                    "slice-digest-b",
                )?,
            ],
            2,
            1_020,
        )?;
        assert_eq!(window.plan.selected_node_ids, vec!["worker-b", "worker-a"]);
        assert_eq!(
            window.plan.adapter_target.adapter_family,
            OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY
        );
        assert_eq!(
            window.plan.adapter_target.adapter_format,
            OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT
        );
        Ok(())
    }
}
