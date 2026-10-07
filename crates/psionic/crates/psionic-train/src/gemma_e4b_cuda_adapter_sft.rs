use psionic_adapters::{AdapterArtifactIdentity, LmHeadLoraAdapterArtifact, LmHeadLoraLoadError};
use psionic_data::TokenizerDigest;
use psionic_models::golden_tokenizer_fixture;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, GEMMA_E4B_FINETUNING_MVP_ADAPTER_TARGET_ID,
    GEMMA_E4B_FINETUNING_MVP_BASE_MODEL_REVISION, GEMMA_E4B_FINETUNING_MVP_CHECKPOINT_FAMILY,
    GEMMA_E4B_FINETUNING_MVP_MODEL_ID, GemmaE4bFinetuneDatasetContract,
    GemmaE4bFinetuneEvalPackBinding, GemmaE4bFinetuningMvpContract, GemmaE4bFinetuningMvpError,
    GemmaE4bFinetuningMvpRequest, GemmaFinetuningInputModality, GemmaFinetuningUpdateMode,
    ModelAdapterDelta, ModelIoArtifactReceipt, ModelIoError, OpenAdapterAdmissibleModelFamily,
    OpenAdapterArtifactExportRequest, OpenAdapterGradientBatchRecord, OpenAdapterHiddenStateSample,
    OpenAdapterLmHeadTarget, OpenAdapterPrecisionPolicy, OpenAdapterReferenceModel,
    OpenAdapterSftError, OpenAdapterTrainingExecutionBackend, OpenAdapterTrainingExecutionError,
    PortableModelBundle, PortableTokenizerAssetFormat, PortableTokenizerBinding, TrainingCoreError,
    TrainingLoopBudget, TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy,
    TrainingRunSummary, TrainingStepReceipt, canonical_gemma_e4b_finetuning_mvp_contract,
};

/// Stable identifier for the first explicit Gemma e4b target set.
pub const GEMMA_E4B_CUDA_ADAPTER_TARGET_SET_ID: &str = "gemma4.e4b.cuda.lm_head_lora.v1";
/// Stable schema version for Gemma e4b adapter-training checkpoints.
pub const GEMMA_E4B_CUDA_ADAPTER_CHECKPOINT_SCHEMA_VERSION: &str =
    "psionic.gemma4_e4b_cuda_adapter_checkpoint.v1";
/// Stable LoRA rank frozen for the first Gemma e4b adapter lane.
pub const GEMMA_E4B_CUDA_ADAPTER_LORA_RANK: usize = 8;
/// Stable LoRA alpha frozen for the first Gemma e4b adapter lane.
pub const GEMMA_E4B_CUDA_ADAPTER_LORA_ALPHA: f32 = 16.0;

/// Explicit first target set for the Gemma e4b CUDA adapter lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bCudaAdapterTargetSet {
    /// Stable target-set identifier.
    pub target_set_id: String,
    /// Stable adapter target identifier.
    pub adapter_target_id: String,
    /// Stable LoRA rank.
    pub lora_rank: usize,
    /// Stable LoRA alpha.
    pub lora_alpha: f32,
    /// Honest note about the bounded surface.
    pub detail: String,
}

impl GemmaE4bCudaAdapterTargetSet {
    fn validate(
        &self,
        contract: &GemmaE4bFinetuningMvpContract,
    ) -> Result<(), GemmaE4bCudaAdapterSftError> {
        if self.target_set_id != GEMMA_E4B_CUDA_ADAPTER_TARGET_SET_ID {
            return Err(GemmaE4bCudaAdapterSftError::InvalidTargetSet {
                detail: String::from("target-set id drifted from the bounded Gemma lane"),
            });
        }
        if self.adapter_target_id != contract.adapter_target.adapter_target_id
            || self.adapter_target_id != GEMMA_E4B_FINETUNING_MVP_ADAPTER_TARGET_ID
        {
            return Err(GemmaE4bCudaAdapterSftError::InvalidTargetSet {
                detail: format!(
                    "Gemma e4b CUDA target surface must stay `{}`",
                    GEMMA_E4B_FINETUNING_MVP_ADAPTER_TARGET_ID
                ),
            });
        }
        if self.lora_rank != GEMMA_E4B_CUDA_ADAPTER_LORA_RANK {
            return Err(GemmaE4bCudaAdapterSftError::InvalidTargetSet {
                detail: format!(
                    "Gemma e4b CUDA target set must stay rank {}",
                    GEMMA_E4B_CUDA_ADAPTER_LORA_RANK
                ),
            });
        }
        if (self.lora_alpha - GEMMA_E4B_CUDA_ADAPTER_LORA_ALPHA).abs() > f32::EPSILON {
            return Err(GemmaE4bCudaAdapterSftError::InvalidTargetSet {
                detail: format!(
                    "Gemma e4b CUDA target set must stay alpha {}",
                    GEMMA_E4B_CUDA_ADAPTER_LORA_ALPHA
                ),
            });
        }
        Ok(())
    }

    /// Returns the stable trainable parameter count for the target set.
    #[must_use]
    pub fn parameter_count(&self, hidden_size: usize, vocab_size: usize) -> u64 {
        u64::try_from(
            self.lora_rank
                .saturating_mul(hidden_size.saturating_add(vocab_size)),
        )
        .unwrap_or(u64::MAX)
    }
}

/// Returns the canonical first Gemma e4b CUDA adapter target set.
#[must_use]
pub fn canonical_gemma_e4b_cuda_adapter_target_set() -> GemmaE4bCudaAdapterTargetSet {
    GemmaE4bCudaAdapterTargetSet {
        target_set_id: String::from(GEMMA_E4B_CUDA_ADAPTER_TARGET_SET_ID),
        adapter_target_id: String::from(GEMMA_E4B_FINETUNING_MVP_ADAPTER_TARGET_ID),
        lora_rank: GEMMA_E4B_CUDA_ADAPTER_LORA_RANK,
        lora_alpha: GEMMA_E4B_CUDA_ADAPTER_LORA_ALPHA,
        detail: String::from(
            "The first Gemma e4b adapter lane keeps one explicit trainable surface: LM-head LoRA only, frozen-base semantics, rank 8, alpha 16.",
        ),
    }
}

/// Compatibility binding between the trainer and one served Gemma base lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bServedBaseModelBinding {
    /// Stable model identifier.
    pub model_id: String,
    /// Stable base-model revision.
    pub base_model_revision: String,
    /// Stable served-artifact digest for the frozen base.
    pub base_served_artifact_digest: String,
    /// Tokenizer identity the lane must preserve.
    pub tokenizer: TokenizerDigest,
    /// Hidden width surfaced by the served base.
    pub hidden_size: usize,
}

impl GemmaE4bServedBaseModelBinding {
    /// Returns the served vocabulary width derived from the tokenizer.
    #[must_use]
    pub fn vocab_size(&self) -> usize {
        usize::try_from(self.tokenizer.vocab_size).unwrap_or(usize::MAX)
    }

    fn validate(
        &self,
        contract: &GemmaE4bFinetuningMvpContract,
    ) -> Result<(), GemmaE4bCudaAdapterSftError> {
        if self.model_id != contract.model_id || self.model_id != GEMMA_E4B_FINETUNING_MVP_MODEL_ID
        {
            return Err(GemmaE4bCudaAdapterSftError::Compatibility {
                detail: String::from("served base model id drifted from the bounded Gemma lane"),
            });
        }
        if self.base_model_revision != contract.base_model_revision
            || self.base_model_revision != GEMMA_E4B_FINETUNING_MVP_BASE_MODEL_REVISION
        {
            return Err(GemmaE4bCudaAdapterSftError::Compatibility {
                detail: String::from("served base revision drifted from the bounded Gemma lane"),
            });
        }
        if self.base_served_artifact_digest.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::Compatibility {
                detail: String::from("served base artifact digest must be present"),
            });
        }
        if self.hidden_size == 0 {
            return Err(GemmaE4bCudaAdapterSftError::Compatibility {
                detail: String::from("served hidden size must be greater than zero"),
            });
        }
        if self.tokenizer.stable_digest() != contract.tokenizer_contract_digest {
            return Err(GemmaE4bCudaAdapterSftError::Compatibility {
                detail: String::from(
                    "served tokenizer contract digest drifted from the bounded Gemma fixture",
                ),
            });
        }
        if self.vocab_size() != usize::try_from(contract.tokenizer.vocab_size).unwrap_or(usize::MAX)
        {
            return Err(GemmaE4bCudaAdapterSftError::Compatibility {
                detail: String::from(
                    "served tokenizer vocabulary width drifted from the Gemma lane",
                ),
            });
        }
        Ok(())
    }
}

/// One bounded Gemma e4b LM-head supervision sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bLmHeadSupervisionSample {
    /// Stable sample identifier.
    pub sample_id: String,
    /// Final hidden state emitted by the frozen base before LM-head projection.
    pub final_hidden_state: Vec<f32>,
    /// Target token the adapter should increase likelihood for.
    pub target_token_id: u32,
    /// Approximate source-token count preserved for telemetry.
    pub source_token_count: u32,
}

impl GemmaE4bLmHeadSupervisionSample {
    /// Creates one bounded Gemma LM-head supervision sample.
    #[must_use]
    pub fn new(
        sample_id: impl Into<String>,
        final_hidden_state: Vec<f32>,
        target_token_id: u32,
        source_token_count: u32,
    ) -> Self {
        Self {
            sample_id: sample_id.into(),
            final_hidden_state,
            target_token_id,
            source_token_count,
        }
    }

    fn into_open_adapter_sample(
        self,
    ) -> Result<OpenAdapterHiddenStateSample, OpenAdapterTrainingExecutionError> {
        OpenAdapterHiddenStateSample::new(
            self.sample_id,
            self.final_hidden_state,
            self.target_token_id,
            self.source_token_count,
        )
    }
}

/// Execution config for the first Gemma e4b CUDA adapter trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bCudaAdapterSftConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Fixed training-loop budget.
    pub budget: TrainingLoopBudget,
    /// Deterministic batch size.
    pub batch_size: usize,
    /// Optimizer config applied to the frozen target set.
    pub optimizer: TrainingOptimizerConfig,
    /// Optimizer residency policy.
    pub optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
}

impl GemmaE4bCudaAdapterSftConfig {
    pub(crate) fn validate(&self) -> Result<(), GemmaE4bCudaAdapterSftError> {
        if self.run_id.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingRunId);
        }
        if self.batch_size == 0 {
            return Err(GemmaE4bCudaAdapterSftError::InvalidBatchSize);
        }
        Ok(())
    }
}

/// Artifact-export request for the first Gemma e4b CUDA adapter lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bCudaAdapterExportRequest {
    /// Stable dataset reference.
    pub dataset_ref: String,
    /// Stable validator policy reference.
    pub validator_policy_ref: String,
    /// Stable adapter identifier.
    pub adapter_id: String,
    /// Stable adapter revision.
    pub adapter_revision: String,
}

impl GemmaE4bCudaAdapterExportRequest {
    fn validate(&self) -> Result<(), GemmaE4bCudaAdapterSftError> {
        if self.dataset_ref.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingDatasetRef);
        }
        if self.validator_policy_ref.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingValidatorPolicyRef);
        }
        if self.adapter_id.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingAdapterId);
        }
        if self.adapter_revision.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingAdapterRevision);
        }
        Ok(())
    }
}

/// Higher-level full-run request for the first Gemma e4b CUDA adapter lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bCudaAdapterSftRunRequest {
    /// Stable dataset reference.
    pub dataset_ref: String,
    /// Stable validator policy reference.
    pub validator_policy_ref: String,
    /// Stable adapter identifier.
    pub adapter_id: String,
    /// Stable adapter revision.
    pub adapter_revision: String,
    /// Eval pack bound before the run starts.
    pub eval_pack_binding: GemmaE4bFinetuneEvalPackBinding,
    /// Dataset contract bound before the run starts.
    pub dataset_contract: GemmaE4bFinetuneDatasetContract,
    /// Logical training start timestamp.
    pub started_at_ms: u64,
    /// Logical duration assigned to each trainer step.
    pub step_duration_ms: u64,
}

impl GemmaE4bCudaAdapterSftRunRequest {
    fn validate(&self) -> Result<(), GemmaE4bCudaAdapterSftError> {
        if self.dataset_ref.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingDatasetRef);
        }
        if self.validator_policy_ref.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingValidatorPolicyRef);
        }
        if self.adapter_id.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingAdapterId);
        }
        if self.adapter_revision.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingAdapterRevision);
        }
        self.eval_pack_binding.validate().map_err(|error| {
            GemmaE4bCudaAdapterSftError::FinetuneEval {
                detail: error.to_string(),
            }
        })?;
        self.dataset_contract.validate().map_err(|error| {
            GemmaE4bCudaAdapterSftError::FinetuneEval {
                detail: error.to_string(),
            }
        })?;
        if self.dataset_ref != self.dataset_contract.dataset_ref {
            return Err(GemmaE4bCudaAdapterSftError::FinetuneEval {
                detail: String::from(
                    "run request dataset_ref drifted from the bound dataset contract",
                ),
            });
        }
        if self.step_duration_ms == 0 {
            return Err(GemmaE4bCudaAdapterSftError::InvalidStepDuration);
        }
        Ok(())
    }

    fn export_request(&self) -> GemmaE4bCudaAdapterExportRequest {
        GemmaE4bCudaAdapterExportRequest {
            dataset_ref: self.dataset_ref.clone(),
            validator_policy_ref: self.validator_policy_ref.clone(),
            adapter_id: self.adapter_id.clone(),
            adapter_revision: self.adapter_revision.clone(),
        }
    }
}

/// Typed exported adapter artifact emitted by the Gemma e4b CUDA trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bCudaAdapterExportedArtifact {
    /// Stable Gemma contract digest.
    pub contract_digest: String,
    /// Stable served-compatibility digest.
    pub compatibility_digest: String,
    /// Stable tokenizer contract digest.
    pub tokenizer_contract_digest: String,
    /// Stable adapter identity bound to the Gemma contract.
    pub adapter_identity: AdapterArtifactIdentity,
    /// Stable adapter identity digest.
    pub adapter_identity_digest: String,
    /// Stable adapter-artifact digest.
    pub adapter_artifact_digest: String,
    /// LoRA alpha needed to reload the artifact.
    pub adapter_alpha: f32,
    /// Raw `safetensors` artifact bytes.
    pub adapter_bytes: Vec<u8>,
}

impl GemmaE4bCudaAdapterExportedArtifact {
    /// Reloads the exported artifact through the shared LM-head LoRA parser.
    pub fn load_lm_head_lora_artifact(
        &self,
    ) -> Result<LmHeadLoraAdapterArtifact, GemmaE4bCudaAdapterSftError> {
        Ok(LmHeadLoraAdapterArtifact::from_safetensors_bytes(
            self.adapter_bytes.as_slice(),
            self.adapter_identity.clone(),
            self.adapter_alpha,
        )?)
    }
}

/// Typed checkpoint snapshot for the first Gemma e4b CUDA adapter lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bCudaAdapterCheckpoint {
    /// Stable checkpoint schema version.
    pub schema_version: String,
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable training-family identifier.
    pub training_family_id: String,
    /// Stable Gemma contract digest.
    pub contract_digest: String,
    /// Stable served-compatibility digest.
    pub compatibility_digest: String,
    /// Stable target-set identifier.
    pub target_set_id: String,
    /// Stable served-artifact digest.
    pub base_served_artifact_digest: String,
    /// Stable tokenizer contract digest.
    pub tokenizer_contract_digest: String,
    /// Logical checkpoint timestamp.
    pub saved_at_ms: u64,
    /// Exact serialized run state for continuation.
    pub run: FixedBudgetTrainingRun,
    /// Stable checkpoint digest.
    pub checkpoint_digest: String,
}

impl GemmaE4bCudaAdapterCheckpoint {
    /// Returns the stable digest over the checkpoint payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.checkpoint_digest.clear();
        stable_digest(b"psionic_gemma_e4b_cuda_adapter_checkpoint|", &clone)
    }

    pub(crate) fn validate(&self) -> Result<(), GemmaE4bCudaAdapterSftError> {
        if self.schema_version != GEMMA_E4B_CUDA_ADAPTER_CHECKPOINT_SCHEMA_VERSION {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint schema version drifted"),
            });
        }
        if self.checkpoint_id.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint id must be present"),
            });
        }
        if self.training_family_id.trim().is_empty()
            || self.contract_digest.trim().is_empty()
            || self.compatibility_digest.trim().is_empty()
            || self.target_set_id.trim().is_empty()
            || self.base_served_artifact_digest.trim().is_empty()
            || self.tokenizer_contract_digest.trim().is_empty()
        {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint metadata must stay populated"),
            });
        }
        if self.checkpoint_digest != self.stable_digest() {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint digest drifted"),
            });
        }
        Ok(())
    }
}

/// Progress emitted while advancing one Gemma e4b run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bCudaAdapterRunProgress {
    /// Step receipts emitted while advancing the run.
    pub step_receipts: Vec<TrainingStepReceipt>,
    /// Gradient-production records emitted while advancing the run.
    pub gradient_records: Vec<OpenAdapterGradientBatchRecord>,
}

/// Summary emitted by the higher-level Gemma e4b CUDA adapter lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bCudaAdapterSftSummary {
    /// Final fixed-budget run summary.
    pub run_summary: TrainingRunSummary,
    /// Stable Gemma contract digest.
    pub contract_digest: String,
    /// Stable served-compatibility digest.
    pub compatibility_digest: String,
    /// Stable target-set identifier.
    pub target_set_id: String,
    /// Stable dataset reference.
    pub dataset_ref: String,
    /// Stable validator policy reference.
    pub validator_policy_ref: String,
    /// Stable eval-pack storage key.
    pub eval_pack_storage_key: String,
    /// Stable held-out validation split reference.
    pub held_out_validation_split_ref: String,
    /// Stable final report split reference.
    pub final_report_split_ref: String,
    /// Stable short baseline split reference.
    pub baseline_short_split_ref: String,
    /// Stable adapter-artifact digest.
    pub adapter_artifact_digest: String,
    /// Stable adapter-identity digest.
    pub adapter_identity_digest: String,
    /// Stable initial state-dict digest.
    pub initial_state_dict_digest: String,
    /// Stable final state-dict digest.
    pub final_state_dict_digest: String,
}

/// Full higher-level Gemma e4b CUDA adapter outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct GemmaE4bCudaAdapterSftRunOutcome {
    /// Step receipts emitted during the run.
    pub step_receipts: Vec<TrainingStepReceipt>,
    /// Gradient-production records emitted during the run.
    pub gradient_records: Vec<OpenAdapterGradientBatchRecord>,
    /// Summary and reproducibility metadata.
    pub summary: GemmaE4bCudaAdapterSftSummary,
    /// Initial portable bundle snapshot.
    pub initial_bundle: PortableModelBundle,
    /// Final portable bundle snapshot.
    pub final_bundle: PortableModelBundle,
    /// Initial bundle receipt.
    pub initial_bundle_receipt: ModelIoArtifactReceipt,
    /// Final bundle receipt.
    pub final_bundle_receipt: ModelIoArtifactReceipt,
    /// Typed adapter delta between the initial and final bundles.
    pub adapter_delta: ModelAdapterDelta,
    /// Typed exported artifact.
    pub exported_artifact: GemmaE4bCudaAdapterExportedArtifact,
    /// Final checkpoint snapshot.
    pub final_checkpoint: GemmaE4bCudaAdapterCheckpoint,
}

/// First honest Gemma e4b CUDA adapter-SFT trainer.
#[derive(Clone, Debug)]
pub struct GemmaE4bCudaAdapterSftTrainer {
    contract: GemmaE4bFinetuningMvpContract,
    target_set: GemmaE4bCudaAdapterTargetSet,
    base_binding: GemmaE4bServedBaseModelBinding,
    compatibility_digest: String,
    backend: OpenAdapterTrainingExecutionBackend,
}

impl GemmaE4bCudaAdapterSftTrainer {
    /// Builds the first bounded Gemma e4b CUDA adapter trainer.
    pub fn new(
        config: GemmaE4bCudaAdapterSftConfig,
        target_set: GemmaE4bCudaAdapterTargetSet,
        base_binding: GemmaE4bServedBaseModelBinding,
        samples: Vec<GemmaE4bLmHeadSupervisionSample>,
    ) -> Result<Self, GemmaE4bCudaAdapterSftError> {
        config.validate()?;
        let contract = canonical_gemma_e4b_finetuning_mvp_contract()?;
        contract.admit_request(&GemmaE4bFinetuningMvpRequest::new(
            contract.model_id.clone(),
            contract.execution_backend_label.clone(),
            GemmaFinetuningInputModality::Text,
            GemmaFinetuningUpdateMode::AdapterSft,
        ))?;
        target_set.validate(&contract)?;
        base_binding.validate(&contract)?;
        let compatibility_digest =
            stable_compatibility_digest(&contract, &target_set, &base_binding);
        let open_samples = samples
            .into_iter()
            .map(GemmaE4bLmHeadSupervisionSample::into_open_adapter_sample)
            .collect::<Result<Vec<_>, _>>()?;
        let backend = OpenAdapterTrainingExecutionBackend::new(
            crate::OpenAdapterExecutionConfig {
                run_id: config.run_id,
                checkpoint_family: String::from(GEMMA_E4B_FINETUNING_MVP_CHECKPOINT_FAMILY),
                execution_backend_label: contract.execution_backend_label.clone(),
                admissible_model_family:
                    OpenAdapterAdmissibleModelFamily::Gemma4E4bDecoderLmHeadLora,
                budget: config.budget,
                batch_size: config.batch_size,
                precision_policy: OpenAdapterPrecisionPolicy::F32Reference,
                model: OpenAdapterReferenceModel {
                    base_model_id: base_binding.model_id.clone(),
                    base_model_revision: base_binding.base_model_revision.clone(),
                    base_served_artifact_digest: base_binding.base_served_artifact_digest.clone(),
                    tokenizer: base_binding.tokenizer.clone(),
                    hidden_size: base_binding.hidden_size,
                    vocab_size: base_binding.vocab_size(),
                    target: OpenAdapterLmHeadTarget {
                        target_id: target_set.adapter_target_id.clone(),
                        lora_rank: target_set.lora_rank,
                        lora_alpha: target_set.lora_alpha,
                        optimizer: config.optimizer,
                        optimizer_residency_policy: config.optimizer_residency_policy,
                    },
                },
            },
            open_samples,
        )?;
        Ok(Self {
            contract,
            target_set,
            base_binding,
            compatibility_digest,
            backend,
        })
    }

    /// Returns the bounded Gemma contract.
    #[must_use]
    pub fn contract(&self) -> &GemmaE4bFinetuningMvpContract {
        &self.contract
    }

    /// Returns the explicit target set.
    #[must_use]
    pub fn target_set(&self) -> &GemmaE4bCudaAdapterTargetSet {
        &self.target_set
    }

    /// Returns the served-base binding.
    #[must_use]
    pub fn base_binding(&self) -> &GemmaE4bServedBaseModelBinding {
        &self.base_binding
    }

    /// Returns the stable served-compatibility digest.
    #[must_use]
    pub fn compatibility_digest(&self) -> &str {
        self.compatibility_digest.as_str()
    }

    /// Returns the underlying reusable open-adapter backend.
    #[must_use]
    pub fn backend(&self) -> &OpenAdapterTrainingExecutionBackend {
        &self.backend
    }

    /// Creates a fresh training run.
    pub fn initialize_run(&self) -> Result<FixedBudgetTrainingRun, GemmaE4bCudaAdapterSftError> {
        Ok(self.backend.initialize_run()?)
    }

    /// Creates a fresh training run from one already-materialized adapter artifact.
    pub fn initialize_run_from_loaded_adapter(
        &self,
        adapter: &LmHeadLoraAdapterArtifact,
    ) -> Result<FixedBudgetTrainingRun, GemmaE4bCudaAdapterSftError> {
        Ok(self.backend.initialize_run_from_loaded_adapter(adapter)?)
    }

    /// Advances one run for up to `step_limit` additional steps.
    pub fn advance_run(
        &self,
        run: &mut FixedBudgetTrainingRun,
        step_limit: Option<u64>,
        started_at_ms: u64,
        step_duration_ms: u64,
    ) -> Result<GemmaE4bCudaAdapterRunProgress, GemmaE4bCudaAdapterSftError> {
        if step_duration_ms == 0 {
            return Err(GemmaE4bCudaAdapterSftError::InvalidStepDuration);
        }
        let summary = run.summary();
        let remaining_steps = summary
            .budget
            .max_steps
            .saturating_sub(summary.completed_steps);
        let allowed_steps = step_limit.unwrap_or(remaining_steps).min(remaining_steps);
        let mut step_receipts = Vec::new();
        let mut gradient_records = Vec::new();
        for step_offset in 0..allowed_steps {
            let batch_index = run.completed_steps() as usize % self.backend.batches().len().max(1);
            let step_started_at_ms = started_at_ms + step_offset.saturating_mul(step_duration_ms);
            let step_finished_at_ms = step_started_at_ms + step_duration_ms;
            let (step_input, gradient_record) = self.backend.produce_step_input(
                run,
                batch_index,
                step_started_at_ms,
                step_finished_at_ms,
            )?;
            gradient_records.push(gradient_record);
            step_receipts.push(run.apply_step(step_input)?);
        }
        Ok(GemmaE4bCudaAdapterRunProgress {
            step_receipts,
            gradient_records,
        })
    }

    /// Saves one exact run-state checkpoint for later continuation.
    pub fn save_checkpoint(
        &self,
        checkpoint_id: impl Into<String>,
        run: &FixedBudgetTrainingRun,
        saved_at_ms: u64,
    ) -> Result<GemmaE4bCudaAdapterCheckpoint, GemmaE4bCudaAdapterSftError> {
        let checkpoint_id = checkpoint_id.into();
        if checkpoint_id.trim().is_empty() {
            return Err(GemmaE4bCudaAdapterSftError::MissingCheckpointId);
        }
        let summary = run.summary();
        if summary.checkpoint_family != self.contract.checkpoint_family {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("run checkpoint family does not belong to the Gemma lane"),
            });
        }
        if summary.budget != self.backend.config().budget {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("run budget drifted from the configured Gemma lane"),
            });
        }
        self.backend.snapshot_training_groups(run)?;
        let mut checkpoint = GemmaE4bCudaAdapterCheckpoint {
            schema_version: String::from(GEMMA_E4B_CUDA_ADAPTER_CHECKPOINT_SCHEMA_VERSION),
            checkpoint_id,
            training_family_id: self.contract.training_family_id.clone(),
            contract_digest: self.contract.contract_digest.clone(),
            compatibility_digest: self.compatibility_digest.clone(),
            target_set_id: self.target_set.target_set_id.clone(),
            base_served_artifact_digest: self.base_binding.base_served_artifact_digest.clone(),
            tokenizer_contract_digest: self.contract.tokenizer_contract_digest.clone(),
            saved_at_ms,
            run: run.clone(),
            checkpoint_digest: String::new(),
        };
        checkpoint.checkpoint_digest = checkpoint.stable_digest();
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    /// Restores one previously-saved exact run-state checkpoint.
    pub fn restore_run(
        &self,
        checkpoint: &GemmaE4bCudaAdapterCheckpoint,
    ) -> Result<FixedBudgetTrainingRun, GemmaE4bCudaAdapterSftError> {
        checkpoint.validate()?;
        if checkpoint.training_family_id != self.contract.training_family_id
            || checkpoint.contract_digest != self.contract.contract_digest
        {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint contract drifted from the active Gemma lane"),
            });
        }
        if checkpoint.compatibility_digest != self.compatibility_digest
            || checkpoint.target_set_id != self.target_set.target_set_id
            || checkpoint.base_served_artifact_digest
                != self.base_binding.base_served_artifact_digest
            || checkpoint.tokenizer_contract_digest != self.contract.tokenizer_contract_digest
        {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint compatibility drifted from the active Gemma lane"),
            });
        }
        let summary = checkpoint.run.summary();
        if summary.checkpoint_family != self.contract.checkpoint_family
            || summary.budget != self.backend.config().budget
        {
            return Err(GemmaE4bCudaAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint run shape drifted from the active Gemma lane"),
            });
        }
        self.backend.snapshot_training_groups(&checkpoint.run)?;
        Ok(checkpoint.run.clone())
    }

    /// Exports one typed Gemma adapter artifact from the current run state.
    pub fn export_run_artifact(
        &self,
        run: &FixedBudgetTrainingRun,
        request: &GemmaE4bCudaAdapterExportRequest,
    ) -> Result<GemmaE4bCudaAdapterExportedArtifact, GemmaE4bCudaAdapterSftError> {
        request.validate()?;
        let exported = self.backend.export_run_artifact(
            run,
            &OpenAdapterArtifactExportRequest::new(
                request.dataset_ref.clone(),
                request.validator_policy_ref.clone(),
                request.adapter_id.clone(),
                request.adapter_revision.clone(),
            ),
        )?;
        let adapter_identity = self.contract.adapter_artifact_identity(
            request.adapter_id.clone(),
            request.adapter_revision.clone(),
            self.base_binding.base_served_artifact_digest.clone(),
            exported.adapter_artifact_digest.clone(),
            self.target_set.parameter_count(
                self.base_binding.hidden_size,
                self.base_binding.vocab_size(),
            ),
        )?;
        let adapter_identity_digest = adapter_identity.stable_digest();
        let artifact = GemmaE4bCudaAdapterExportedArtifact {
            contract_digest: self.contract.contract_digest.clone(),
            compatibility_digest: self.compatibility_digest.clone(),
            tokenizer_contract_digest: self.contract.tokenizer_contract_digest.clone(),
            adapter_identity,
            adapter_identity_digest,
            adapter_artifact_digest: exported.adapter_artifact_digest,
            adapter_alpha: exported.adapter_alpha,
            adapter_bytes: exported.adapter_bytes,
        };
        artifact.load_lm_head_lora_artifact()?;
        Ok(artifact)
    }

    /// Runs the full cold-start Gemma e4b adapter-SFT lane.
    pub fn run_sft(
        &self,
        request: &GemmaE4bCudaAdapterSftRunRequest,
    ) -> Result<GemmaE4bCudaAdapterSftRunOutcome, GemmaE4bCudaAdapterSftError> {
        let run = self.initialize_run()?;
        self.run_from_existing_run(run, request, None)
    }

    /// Restores one checkpoint and continues until the fixed budget is reached.
    pub fn run_sft_from_checkpoint(
        &self,
        checkpoint: &GemmaE4bCudaAdapterCheckpoint,
        request: &GemmaE4bCudaAdapterSftRunRequest,
    ) -> Result<GemmaE4bCudaAdapterSftRunOutcome, GemmaE4bCudaAdapterSftError> {
        let run = self.restore_run(checkpoint)?;
        self.run_from_existing_run(run, request, Some(checkpoint.checkpoint_id.as_str()))
    }

    fn run_from_existing_run(
        &self,
        mut run: FixedBudgetTrainingRun,
        request: &GemmaE4bCudaAdapterSftRunRequest,
        resumed_from_checkpoint_id: Option<&str>,
    ) -> Result<GemmaE4bCudaAdapterSftRunOutcome, GemmaE4bCudaAdapterSftError> {
        request.validate()?;
        let initial_groups = self.backend.snapshot_training_groups(&run)?;
        let initial_bundle = self.bundle_from_groups(
            initial_groups.as_slice(),
            format!("checkpoint://{}/initial", self.backend.config().run_id),
        )?;
        let (_, initial_bundle_receipt) = initial_bundle.export_safetensors()?;
        let progress = self.advance_run(
            &mut run,
            None,
            request.started_at_ms,
            request.step_duration_ms,
        )?;
        let final_groups = self.backend.snapshot_training_groups(&run)?;
        let final_bundle = self.bundle_from_groups(
            final_groups.as_slice(),
            format!("checkpoint://{}/final", self.backend.config().run_id),
        )?;
        let (_, final_bundle_receipt) = final_bundle.export_safetensors()?;
        let adapter_delta = crate::PortableModelStateDict::derive_adapter_delta(
            &initial_bundle.state_dict,
            &final_bundle.state_dict,
            request.adapter_id.clone(),
        )?;
        let exported_artifact = self.export_run_artifact(&run, &request.export_request())?;
        let final_checkpoint = self.save_checkpoint(
            checkpoint_id_for(request, resumed_from_checkpoint_id),
            &run,
            progress
                .step_receipts
                .last()
                .map(|receipt| receipt.timing.finished_at_ms)
                .unwrap_or(request.started_at_ms),
        )?;
        let summary = GemmaE4bCudaAdapterSftSummary {
            run_summary: run.summary(),
            contract_digest: self.contract.contract_digest.clone(),
            compatibility_digest: self.compatibility_digest.clone(),
            target_set_id: self.target_set.target_set_id.clone(),
            dataset_ref: request.dataset_ref.clone(),
            validator_policy_ref: request.validator_policy_ref.clone(),
            eval_pack_storage_key: request
                .eval_pack_binding
                .benchmark_package_storage_key
                .clone(),
            held_out_validation_split_ref: request
                .dataset_contract
                .held_out_validation_split_ref
                .clone(),
            final_report_split_ref: request.dataset_contract.final_report_split_ref.clone(),
            baseline_short_split_ref: request.dataset_contract.baseline_short_split_ref.clone(),
            adapter_artifact_digest: exported_artifact.adapter_artifact_digest.clone(),
            adapter_identity_digest: exported_artifact.adapter_identity_digest.clone(),
            initial_state_dict_digest: initial_bundle_receipt.state_dict_digest.clone(),
            final_state_dict_digest: final_bundle_receipt.state_dict_digest.clone(),
        };
        Ok(GemmaE4bCudaAdapterSftRunOutcome {
            step_receipts: progress.step_receipts,
            gradient_records: progress.gradient_records,
            summary,
            initial_bundle,
            final_bundle,
            initial_bundle_receipt,
            final_bundle_receipt,
            adapter_delta,
            exported_artifact,
            final_checkpoint,
        })
    }

    fn bundle_from_groups(
        &self,
        groups: &[crate::TrainingParameterGroupState],
        checkpoint_ref: String,
    ) -> Result<PortableModelBundle, GemmaE4bCudaAdapterSftError> {
        Ok(PortableModelBundle::from_training_groups(
            self.contract.adapter_target.adapter_family.clone(),
            self.contract.base_model_revision.clone(),
            self.contract.checkpoint_family.clone(),
            Some(checkpoint_ref),
            groups,
            self.portable_tokenizer_binding()?,
            self.contract.tokenizer.template_digest.clone(),
        )?)
    }

    fn portable_tokenizer_binding(
        &self,
    ) -> Result<PortableTokenizerBinding, GemmaE4bCudaAdapterSftError> {
        let fixture = golden_tokenizer_fixture("gemma4_e4b")
            .ok_or(GemmaE4bFinetuningMvpError::MissingTokenizerFixture)?;
        Ok(PortableTokenizerBinding::new(
            self.contract.tokenizer.clone(),
            PortableTokenizerAssetFormat::PsionicDigest,
            self.contract.base_model_ref(),
        )
        .with_special_tokens(
            fixture.bos_token_id.map(|token| token.as_u32()),
            fixture
                .eos_token_ids
                .iter()
                .copied()
                .map(|token| token.as_u32())
                .collect(),
            fixture.pad_token_id.map(|token| token.as_u32()),
            fixture.unknown_token_id.map(|token| token.as_u32()),
            fixture.add_bos,
            fixture.add_eos,
        ))
    }
}

/// Error surfaced by the higher-level Gemma e4b CUDA adapter lane.
#[derive(Debug, Error)]
pub enum GemmaE4bCudaAdapterSftError {
    #[error("gemma e4b CUDA adapter config is missing `run_id`")]
    MissingRunId,
    #[error("gemma e4b CUDA adapter config requires `batch_size > 0`")]
    InvalidBatchSize,
    #[error("gemma e4b CUDA adapter request is missing `dataset_ref`")]
    MissingDatasetRef,
    #[error("gemma e4b CUDA adapter request is missing `validator_policy_ref`")]
    MissingValidatorPolicyRef,
    #[error("gemma e4b CUDA adapter request is missing `adapter_id`")]
    MissingAdapterId,
    #[error("gemma e4b CUDA adapter request is missing `adapter_revision`")]
    MissingAdapterRevision,
    #[error("gemma e4b CUDA adapter request requires `step_duration_ms > 0`")]
    InvalidStepDuration,
    #[error("gemma e4b CUDA adapter checkpoint is missing `checkpoint_id`")]
    MissingCheckpointId,
    #[error("Gemma e4b served compatibility mismatch: {detail}")]
    Compatibility { detail: String },
    #[error("Gemma e4b target set is invalid: {detail}")]
    InvalidTargetSet { detail: String },
    #[error("Gemma e4b checkpoint is invalid: {detail}")]
    InvalidCheckpoint { detail: String },
    #[error("Gemma e4b finetune eval contract is invalid: {detail}")]
    FinetuneEval { detail: String },
    #[error(transparent)]
    Mvp(#[from] GemmaE4bFinetuningMvpError),
    #[error(transparent)]
    TrainingExecution(#[from] OpenAdapterTrainingExecutionError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    OpenAdapterSft(#[from] OpenAdapterSftError),
    #[error(transparent)]
    ModelIo(#[from] ModelIoError),
    #[error(transparent)]
    AdapterLoad(#[from] LmHeadLoraLoadError),
}

fn checkpoint_id_for(
    request: &GemmaE4bCudaAdapterSftRunRequest,
    resumed_from_checkpoint_id: Option<&str>,
) -> String {
    match resumed_from_checkpoint_id {
        Some(previous) => format!(
            "{}-{}-continued-from-{}",
            request.adapter_id, request.adapter_revision, previous
        ),
        None => format!("{}-{}-final", request.adapter_id, request.adapter_revision),
    }
}

fn stable_compatibility_digest(
    contract: &GemmaE4bFinetuningMvpContract,
    target_set: &GemmaE4bCudaAdapterTargetSet,
    base_binding: &GemmaE4bServedBaseModelBinding,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_gemma_e4b_cuda_adapter_compatibility|");
    hasher.update(contract.contract_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(target_set.target_set_id.as_bytes());
    hasher.update(b"|");
    hasher.update(target_set.adapter_target_id.as_bytes());
    hasher.update(b"|");
    hasher.update(target_set.lora_rank.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(target_set.lora_alpha.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.base_model_revision.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.base_served_artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.hidden_size.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.tokenizer.stable_digest().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_digest(prefix: &[u8], payload: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(payload).expect("payload should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_binding() -> GemmaE4bServedBaseModelBinding {
        let contract = canonical_gemma_e4b_finetuning_mvp_contract().expect("contract");
        GemmaE4bServedBaseModelBinding {
            model_id: contract.model_id,
            base_model_revision: contract.base_model_revision,
            base_served_artifact_digest: String::from("sha256:gemma4-e4b-base"),
            tokenizer: contract.tokenizer,
            hidden_size: 4,
        }
    }

    fn sample_config() -> GemmaE4bCudaAdapterSftConfig {
        GemmaE4bCudaAdapterSftConfig {
            run_id: String::from("gemma-e4b-adapter-run"),
            budget: TrainingLoopBudget::new(4, 1, 1).expect("budget"),
            batch_size: 2,
            optimizer: TrainingOptimizerConfig::adamw(0.15, 0.9, 0.99, 1e-8)
                .with_gradient_clip_norm(1.0),
            optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
        }
    }

    fn sample_target_set() -> GemmaE4bCudaAdapterTargetSet {
        canonical_gemma_e4b_cuda_adapter_target_set()
    }

    fn sample_supervision() -> Vec<GemmaE4bLmHeadSupervisionSample> {
        vec![
            GemmaE4bLmHeadSupervisionSample::new("a", vec![1.0, 0.0, 0.0, 0.0], 48, 11),
            GemmaE4bLmHeadSupervisionSample::new("b", vec![0.0, 1.0, 0.0, 0.0], 106, 12),
            GemmaE4bLmHeadSupervisionSample::new("c", vec![0.0, 0.0, 1.0, 0.0], 50, 10),
            GemmaE4bLmHeadSupervisionSample::new("d", vec![0.0, 0.0, 0.0, 1.0], 1, 9),
        ]
    }

    fn sample_request() -> GemmaE4bCudaAdapterSftRunRequest {
        GemmaE4bCudaAdapterSftRunRequest {
            dataset_ref: String::from("dataset://openagents/gemma4-e4b-helpdesk@2026.04"),
            validator_policy_ref: String::from("policy://validator/gemma4/e4b-text-sft"),
            adapter_id: String::from("gemma4-e4b-helpdesk"),
            adapter_revision: String::from("r1"),
            eval_pack_binding: crate::canonical_gemma_e4b_finetune_eval_pack_binding()
                .expect("eval-pack binding"),
            dataset_contract: {
                let template_digest = canonical_gemma_e4b_finetuning_mvp_contract()
                    .expect("contract")
                    .tokenizer
                    .template_digest
                    .expect("template digest");
                let mut contract = GemmaE4bFinetuneDatasetContract {
                    schema_version: String::from(
                        crate::GEMMA_E4B_FINETUNE_DATASET_CONTRACT_SCHEMA_VERSION,
                    ),
                    dataset_ref: String::from("dataset://openagents/gemma4-e4b-helpdesk@2026.04"),
                    train_split_ref: String::from("split://gemma4-e4b-helpdesk/train"),
                    held_out_validation_split_ref: String::from(
                        "split://gemma4-e4b-helpdesk/held_out_validation",
                    ),
                    final_report_split_ref: String::from(
                        "split://gemma4-e4b-helpdesk/final_report",
                    ),
                    baseline_short_split_ref: String::from(
                        "split://gemma4-e4b-helpdesk/baseline_short",
                    ),
                    chat_template_digest: template_digest,
                    assistant_mask_kind: crate::GemmaE4bAssistantMaskKind::AssistantResponsesOnly,
                    assistant_mask_coverage_bps: 10_000,
                    benchmark_overlap_check: crate::GemmaE4bBenchmarkOverlapCheck {
                        check_id: String::from("gemma4-e4b-helpdesk-overlap-check"),
                        compared_benchmark_refs: vec![
                            String::from("benchmark://psionic/gemma4/e4b/finetune_eval"),
                            String::from("benchmark://psion/actual_pretraining/checkpoint_eval"),
                        ],
                        exact_overlap_refs: Vec::new(),
                        near_duplicate_overlap_refs: Vec::new(),
                        passed: true,
                        detail: String::from("curated dataset cleared overlap review"),
                    },
                    dataset_digest: String::new(),
                };
                contract.dataset_digest = contract.stable_digest();
                contract
            },
            started_at_ms: 1_000,
            step_duration_ms: 25,
        }
    }

    #[test]
    fn gemma_e4b_cuda_adapter_run_exports_typed_artifact_and_checkpoint()
    -> Result<(), Box<dyn std::error::Error>> {
        let trainer = GemmaE4bCudaAdapterSftTrainer::new(
            sample_config(),
            sample_target_set(),
            sample_binding(),
            sample_supervision(),
        )?;
        let outcome = trainer.run_sft(&sample_request())?;
        assert_eq!(outcome.step_receipts.len(), 4);
        assert_eq!(
            outcome.summary.run_summary.checkpoint_family,
            GEMMA_E4B_FINETUNING_MVP_CHECKPOINT_FAMILY
        );
        assert_eq!(
            outcome.exported_artifact.adapter_identity.base_model_id,
            GEMMA_E4B_FINETUNING_MVP_MODEL_ID
        );
        assert_eq!(
            outcome.exported_artifact.tokenizer_contract_digest,
            trainer.contract().tokenizer_contract_digest
        );
        let loaded = outcome.exported_artifact.load_lm_head_lora_artifact()?;
        assert_eq!(loaded.hidden_size, 4);
        assert_eq!(loaded.vocab_size, sample_binding().vocab_size());
        assert_eq!(
            outcome.final_checkpoint.training_family_id,
            trainer.contract().training_family_id
        );
        assert_eq!(
            outcome.final_checkpoint.run.summary().completed_steps,
            outcome.summary.run_summary.completed_steps
        );
        assert_eq!(
            outcome.summary.eval_pack_storage_key,
            "benchmark://psionic/gemma4/e4b/finetune_eval@2026.04.03"
        );
        assert_eq!(
            outcome.summary.held_out_validation_split_ref,
            "split://gemma4-e4b-helpdesk/held_out_validation"
        );
        Ok(())
    }

    #[test]
    fn gemma_e4b_cuda_adapter_refuses_tokenizer_and_target_surface_mismatches() {
        let mut wrong_tokenizer = sample_binding();
        wrong_tokenizer.tokenizer = wrong_tokenizer
            .tokenizer
            .clone()
            .with_template_digest("drifted-template");
        let tokenizer_error = GemmaE4bCudaAdapterSftTrainer::new(
            sample_config(),
            sample_target_set(),
            wrong_tokenizer,
            sample_supervision(),
        )
        .expect_err("tokenizer drift must refuse");
        assert!(
            tokenizer_error
                .to_string()
                .contains("tokenizer contract digest")
        );

        let mut wrong_target_set = sample_target_set();
        wrong_target_set.adapter_target_id = String::from("layers.0.attention.q_proj");
        let target_error = GemmaE4bCudaAdapterSftTrainer::new(
            sample_config(),
            wrong_target_set,
            sample_binding(),
            sample_supervision(),
        )
        .expect_err("target surface drift must refuse");
        assert!(target_error.to_string().contains("target surface"));
    }

    #[test]
    fn gemma_e4b_cuda_adapter_checkpoint_restores_exact_run_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let trainer = GemmaE4bCudaAdapterSftTrainer::new(
            sample_config(),
            sample_target_set(),
            sample_binding(),
            sample_supervision(),
        )?;
        let mut run = trainer.initialize_run()?;
        let progress = trainer.advance_run(&mut run, Some(2), 1_000, 25)?;
        assert_eq!(progress.step_receipts.len(), 2);
        let checkpoint = trainer.save_checkpoint("gemma-e4b-midpoint", &run, 1_050)?;
        let restored = trainer.restore_run(&checkpoint)?;
        assert_eq!(restored.summary().completed_steps, 2);
        let resumed = trainer.run_sft_from_checkpoint(&checkpoint, &sample_request())?;
        assert_eq!(resumed.summary.run_summary.completed_steps, 4);
        assert_eq!(
            resumed.summary.run_summary.last_receipt_id.as_deref(),
            Some("gemma-e4b-adapter-run-step-4")
        );
        Ok(())
    }

    #[test]
    fn gemma_e4b_cuda_adapter_run_refuses_dataset_contract_drift() {
        let trainer = GemmaE4bCudaAdapterSftTrainer::new(
            sample_config(),
            sample_target_set(),
            sample_binding(),
            sample_supervision(),
        )
        .expect("trainer");
        let mut request = sample_request();
        request.dataset_contract.dataset_ref = String::from("dataset://drifted");
        request.dataset_contract.dataset_digest = request.dataset_contract.stable_digest();
        let error = trainer
            .run_sft(&request)
            .expect_err("dataset contract drift must refuse");
        assert!(error.to_string().contains("dataset_ref drifted"));
    }
}
