use std::collections::{BTreeMap, HashMap};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterResidencyMode,
    AdapterServingBinding, AdapterTargetFamily, LmHeadLoraAdapterArtifact, LmHeadLoraLoadError,
    LmHeadLoraRuntimeError,
};
use psionic_core::QuantizationMode;
use psionic_models::{
    QWEN38_27B_MODEL_ID, QWEN38_27B_SERVED_MODEL_ID, QWEN38_27B_UPSTREAM_REVISION,
    canonical_qwen38_27b_artifact_facts,
};
use safetensors::{Dtype as SafeTensorsDType, SafeTensors, serialize, tensor::TensorView};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::PsionicTrainCpuBudget;

pub const QWEN38_TRAINING_ADMISSION_SCHEMA_VERSION: &str =
    "psionic.qwen38.training_adapter_admission.v1";
pub const QWEN38_TRAINING_BASE_IDENTITY_SCHEMA_VERSION: &str =
    "psionic.qwen38.training_base_identity.v1";
pub const QWEN38_ADAPTER_IDENTITY_BINDING_SCHEMA_VERSION: &str =
    "psionic.qwen38.adapter_identity_binding.v1";
pub const QWEN38_LM_HEAD_BACKWARD_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen38.lm_head_lora_backward_receipt.v1";
pub const QWEN38_TRAINING_EVIDENCE_SCHEMA_VERSION: &str =
    "psionic.qwen38.training_adapter_evidence.v1";
pub const QWEN38_LM_HEAD_CHECKPOINT_SCHEMA_VERSION: &str =
    "psionic.qwen38.lm_head_lora_checkpoint.v1";
pub const QWEN38_LM_HEAD_RECOVERY_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen38.lm_head_lora_recovery_receipt.v1";
pub const QWEN38_LM_HEAD_ADAPTER_ARTIFACT_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen38.lm_head_lora_adapter_artifact_receipt.v1";
pub const QWEN38_LM_HEAD_ADAPTER_ARTIFACT_METADATA_SCHEMA_VERSION: &str =
    "psionic.qwen38.lm_head_lora_adapter_artifact_metadata.v1";
pub const QWEN38_LM_HEAD_TARGET: &str = "lm_head.weight";
pub const QWEN38_LM_HEAD_BACKWARD_CONTRACT: &str = "qwen38_lm_head_lora_f32_reference_backward_v1";
pub const QWEN38_LM_HEAD_ADAPTER_ARTIFACT_REF: &str =
    "fixtures/qwen38/adapters/qwen38_lm_head_lora_tiny_v1.safetensors";
pub const QWEN38_DEFERRED_ADAPTER_TARGETS: &[&str] = &[
    "q_proj",
    "k_proj",
    "v_proj",
    "o_proj",
    "gate_proj",
    "up_proj",
    "down_proj",
    "linear_attn.in_proj_qkv",
    "linear_attn.in_proj_z",
    "linear_attn.in_proj_a",
    "linear_attn.in_proj_b",
    "linear_attn.out_proj",
    "visual.merger.linear_fc2",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38TrainingExecutionMode {
    TinyReferenceCpu,
    NativeCpu,
    NativeCuda,
    NativeMetal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38TrainingAdmissionStatus {
    Admitted,
    Refused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38TrainingRefusalCode {
    SchemaMismatch,
    UnsupportedModel,
    BaseRevisionMismatch,
    BaseArtifactIdentityMismatch,
    InheritedAdapterIdentity,
    UnsupportedTarget,
    UnsupportedExecutionMode,
    InvalidAdapterConfig,
    MissingLineage,
    InvalidResourceBudget,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen38TrainingRefusal {
    pub code: Qwen38TrainingRefusalCode,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen38TrainingBaseIdentity {
    pub schema_version: String,
    pub model_id: String,
    pub served_model_id: String,
    pub upstream_revision: String,
    pub artifact_facts_sha256: String,
    pub safetensors_index_sha256: String,
    pub base_artifact_identity_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen38AdapterIdentityBinding {
    pub schema_version: String,
    pub adapter_id: String,
    pub adapter_revision: String,
    pub base_model_id: String,
    pub base_model_revision: String,
    pub base_artifact_identity_digest: String,
    pub target_modules: Vec<String>,
    pub binding_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38TrainingAdapterRequest {
    pub schema_version: String,
    pub request_id: String,
    pub model_id: String,
    pub upstream_revision: String,
    pub base_artifact_identity_digest: String,
    pub adapter_id: String,
    pub adapter_revision: String,
    pub target_modules: Vec<String>,
    pub lora_rank: usize,
    pub lora_alpha: f32,
    pub execution_mode: Qwen38TrainingExecutionMode,
    pub corpus_manifest_sha256: String,
    pub evaluation_manifest_sha256: String,
    pub seed: u64,
    pub cpu_budget: PsionicTrainCpuBudget,
}

impl Default for Qwen38TrainingAdapterRequest {
    fn default() -> Self {
        Self {
            schema_version: String::from(QWEN38_TRAINING_ADMISSION_SCHEMA_VERSION),
            request_id: String::from("qwen38-lm-head-reference-001"),
            model_id: String::from(QWEN38_27B_MODEL_ID),
            upstream_revision: String::from(QWEN38_27B_UPSTREAM_REVISION),
            base_artifact_identity_digest: qwen38_training_base_identity()
                .base_artifact_identity_digest,
            adapter_id: String::from("qwen38-lm-head-reference-001"),
            adapter_revision: String::from("reference-step-001"),
            target_modules: vec![String::from(QWEN38_LM_HEAD_TARGET)],
            lora_rank: 2,
            lora_alpha: 4.0,
            execution_mode: Qwen38TrainingExecutionMode::TinyReferenceCpu,
            corpus_manifest_sha256: sha256_bytes(b"qwen38-reference-corpus-v1"),
            evaluation_manifest_sha256: sha256_bytes(b"qwen38-reference-eval-v1"),
            seed: 38,
            cpu_budget: PsionicTrainCpuBudget::BoundedSingleCore,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38TrainingAdapterPlan {
    pub schema_version: String,
    pub request_id: String,
    pub status: Qwen38TrainingAdmissionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Qwen38TrainingRefusal>,
    pub base_identity: Qwen38TrainingBaseIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_binding: Option<Qwen38AdapterIdentityBinding>,
    pub execution_mode: Qwen38TrainingExecutionMode,
    pub exact_backward_contract: Option<String>,
    pub deferred_target_modules: Vec<String>,
    pub corpus_manifest_sha256: String,
    pub evaluation_manifest_sha256: String,
    pub seed: u64,
    pub cpu_budget: PsionicTrainCpuBudget,
    pub claim_boundary: String,
    pub plan_digest: String,
}

impl Qwen38TrainingAdapterPlan {
    #[must_use]
    pub fn is_admitted(&self) -> bool {
        self.status == Qwen38TrainingAdmissionStatus::Admitted
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadLoraBackwardFixture {
    pub fixture_id: String,
    pub hidden: Vec<f32>,
    pub base_logits: Vec<f32>,
    pub lora_rank: usize,
    pub lora_alpha: f32,
    pub lora_a: Vec<f32>,
    pub lora_b: Vec<f32>,
    pub target_token_id: usize,
    pub learning_rate: f32,
    pub finite_difference_epsilon: f32,
    pub gradient_tolerance: f32,
}

impl Default for Qwen38LmHeadLoraBackwardFixture {
    fn default() -> Self {
        Self {
            fixture_id: String::from("qwen38-lm-head-lora-tiny-v1"),
            hidden: vec![0.75, -0.5, 0.25, 1.0],
            base_logits: vec![0.2, -0.1, 0.05],
            lora_rank: 2,
            lora_alpha: 4.0,
            lora_a: vec![0.1, -0.2, 0.05, 0.3, -0.15, 0.25, 0.2, -0.1],
            lora_b: vec![0.2, -0.1, -0.15, 0.3, 0.1, 0.25],
            target_token_id: 2,
            learning_rate: 0.05,
            finite_difference_epsilon: 0.001,
            gradient_tolerance: 0.0002,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadLoraBackwardReceipt {
    pub schema_version: String,
    pub fixture_id: String,
    pub contract: String,
    pub hidden_size: usize,
    pub vocabulary_size: usize,
    pub lora_rank: usize,
    pub lora_alpha: f32,
    pub target_token_id: usize,
    pub learning_rate: f32,
    pub initial_loss: f32,
    pub updated_loss: f32,
    pub loss_improved: bool,
    pub initial_logits_sha256: String,
    pub updated_logits_sha256: String,
    pub lora_a_gradient_sha256: String,
    pub lora_b_gradient_sha256: String,
    pub updated_lora_a_sha256: String,
    pub updated_lora_b_sha256: String,
    pub finite_difference_epsilon: f32,
    pub gradient_max_abs_error: f32,
    pub gradient_tolerance: f32,
    pub gradient_check_passed: bool,
    pub base_weights_frozen: bool,
    pub deterministic_replay: bool,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadAdamWConfig {
    pub learning_rate: f32,
    pub beta1: f32,
    pub beta2: f32,
    pub epsilon: f32,
    pub weight_decay: f32,
}

impl Default for Qwen38LmHeadAdamWConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            beta1: 0.9,
            beta2: 0.999,
            epsilon: 1.0e-8,
            weight_decay: 0.01,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadLoraTrainingState {
    pub step: u64,
    pub lora_a: Vec<f32>,
    pub lora_b: Vec<f32>,
    pub adam_m_a: Vec<f32>,
    pub adam_v_a: Vec<f32>,
    pub adam_m_b: Vec<f32>,
    pub adam_v_b: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadAdamWStepReceipt {
    pub step: u64,
    pub loss_before: f32,
    pub loss_after: f32,
    pub loss_improved: bool,
    pub gradient_a_sha256: String,
    pub gradient_b_sha256: String,
    pub state_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadLoraCheckpoint {
    pub schema_version: String,
    pub fixture_id: String,
    pub base_artifact_identity_digest: String,
    pub adapter_binding_digest: String,
    pub corpus_manifest_sha256: String,
    pub evaluation_manifest_sha256: String,
    pub seed: u64,
    pub optimizer: Qwen38LmHeadAdamWConfig,
    pub state: Qwen38LmHeadLoraTrainingState,
    pub state_digest: String,
    pub checkpoint_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadLoraRecoveryReceipt {
    pub schema_version: String,
    pub fixture_id: String,
    pub checkpoint: Qwen38LmHeadLoraCheckpoint,
    pub checkpoint_bytes_sha256: String,
    pub checkpoint_step: u64,
    pub resumed_step: u64,
    pub uninterrupted_state_digest: String,
    pub resumed_state_digest: String,
    pub exact_state_match: bool,
    pub optimizer_state_exact_match: bool,
    pub uninterrupted_second_step_loss: f32,
    pub resumed_second_step_loss: f32,
    pub second_step_loss_exact_match: bool,
    pub tampered_checkpoint_refused: bool,
    pub claim_boundary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadLoraAdapterArtifactReceipt {
    pub schema_version: String,
    pub fixture_id: String,
    pub artifact_ref: String,
    pub trained_step: u64,
    pub tensor_names: Vec<String>,
    pub lora_a_shape: Vec<usize>,
    pub lora_b_shape: Vec<usize>,
    pub artifact_byte_length: usize,
    pub artifact_sha256: String,
    pub metadata: Qwen38LmHeadLoraAdapterArtifactMetadata,
    pub adapter_identity: AdapterArtifactIdentity,
    pub adapter_identity_digest: String,
    pub serving_binding: AdapterServingBinding,
    pub loaded_rank: usize,
    pub loaded_hidden_size: usize,
    pub loaded_vocabulary_size: usize,
    pub loaded_lora_a_sha256: String,
    pub loaded_lora_b_sha256: String,
    pub expected_logits_sha256: String,
    pub served_logits_sha256: String,
    pub save_load_exact_match: bool,
    pub serve_logits_exact_match: bool,
    pub deterministic_serve_replay: bool,
    pub tampered_artifact_refused: bool,
    pub metadata_drift_refused: bool,
    pub claim_boundary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38LmHeadLoraAdapterArtifactMetadata {
    pub schema_version: String,
    pub fixture_id: String,
    pub base_model_id: String,
    pub base_model_revision: String,
    pub base_artifact_identity_digest: String,
    pub adapter_id: String,
    pub adapter_revision: String,
    pub adapter_binding_digest: String,
    pub plan_digest: String,
    pub corpus_manifest_sha256: String,
    pub evaluation_manifest_sha256: String,
    pub seed: u64,
    pub trained_step: u64,
    pub target: String,
    pub lora_rank: usize,
    pub lora_alpha: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen38TrainingAdapterEvidenceReport {
    pub schema_version: String,
    pub phase: String,
    pub status: String,
    pub base_identity: Qwen38TrainingBaseIdentity,
    pub admitted_plan: Qwen38TrainingAdapterPlan,
    pub backward_receipt: Qwen38LmHeadLoraBackwardReceipt,
    pub checkpoint_recovery: Qwen38LmHeadLoraRecoveryReceipt,
    pub adapter_artifact: Qwen38LmHeadLoraAdapterArtifactReceipt,
    pub refusals: BTreeMap<String, Qwen38TrainingAdapterPlan>,
    pub real_checkpoint_training_admitted: bool,
    pub native_backward_admitted: bool,
    pub adapter_artifact_written: bool,
    pub tiny_reference_adapter_serving_parity_admitted: bool,
    pub adapter_serving_admitted: bool,
    pub tiny_reference_checkpoint_recovery_admitted: bool,
    pub checkpoint_recovery_admitted: bool,
    pub promotion_admitted: bool,
    pub claim_boundary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum Qwen38LmHeadBackwardError {
    #[error("invalid Qwen3.8 LM-head backward fixture: {0}")]
    InvalidFixture(String),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum Qwen38AdapterIdentityError {
    #[error("Qwen3.8 adapter plan was not admitted")]
    PlanNotAdmitted,
    #[error("Qwen3.8 adapter artifact digest must be a lowercase sha256 hex digest")]
    InvalidArtifactDigest,
    #[error("adapter base model `{actual}` does not match Qwen3.8 `{expected}`")]
    BaseModelMismatch { expected: String, actual: String },
    #[error("adapter base revision `{actual}` does not match Qwen3.8 `{expected}`")]
    BaseRevisionMismatch { expected: String, actual: String },
    #[error("adapter base artifact digest `{actual}` does not match Qwen3.8 `{expected}`")]
    BaseArtifactMismatch { expected: String, actual: String },
    #[error("adapter kind or format is not an admitted Qwen3.8 LoRA safetensors artifact")]
    UnsupportedArtifactShape,
}

#[derive(Debug, Error)]
pub enum Qwen38TrainingRecoveryError {
    #[error(transparent)]
    Backward(#[from] Qwen38LmHeadBackwardError),
    #[error("Qwen3.8 recovery requires an admitted adapter plan")]
    PlanNotAdmitted,
    #[error("invalid Qwen3.8 AdamW config: {0}")]
    InvalidOptimizer(String),
    #[error("invalid Qwen3.8 training state: {0}")]
    InvalidState(String),
    #[error("invalid Qwen3.8 checkpoint: {0}")]
    InvalidCheckpoint(String),
    #[error("Qwen3.8 checkpoint JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum Qwen38AdapterArtifactError {
    #[error(transparent)]
    Recovery(#[from] Qwen38TrainingRecoveryError),
    #[error(transparent)]
    Identity(#[from] Qwen38AdapterIdentityError),
    #[error("Qwen3.8 adapter safetensors encoding failed: {0}")]
    Safetensors(String),
    #[error("invalid Qwen3.8 adapter artifact metadata: {0}")]
    ArtifactMetadata(String),
    #[error(transparent)]
    Load(#[from] LmHeadLoraLoadError),
    #[error(transparent)]
    Runtime(#[from] LmHeadLoraRuntimeError),
    #[error("Qwen3.8 adapter artifact digest mismatch: expected {expected}, actual {actual}")]
    ArtifactDigestMismatch { expected: String, actual: String },
    #[error("Qwen3.8 adapter parameter count mismatch: expected {expected}, actual {actual}")]
    ParameterCountMismatch { expected: u64, actual: u64 },
}

#[derive(Debug, Error)]
pub enum Qwen38TrainingEvidenceError {
    #[error(transparent)]
    Backward(#[from] Qwen38LmHeadBackwardError),
    #[error(transparent)]
    Recovery(#[from] Qwen38TrainingRecoveryError),
    #[error(transparent)]
    Artifact(#[from] Qwen38AdapterArtifactError),
}

#[must_use]
pub fn qwen38_training_base_identity() -> Qwen38TrainingBaseIdentity {
    let facts = canonical_qwen38_27b_artifact_facts();
    let artifact_facts_sha256 = facts.canonical_sha256();
    let mut identity = Qwen38TrainingBaseIdentity {
        schema_version: String::from(QWEN38_TRAINING_BASE_IDENTITY_SCHEMA_VERSION),
        model_id: String::from(QWEN38_27B_MODEL_ID),
        served_model_id: String::from(QWEN38_27B_SERVED_MODEL_ID),
        upstream_revision: String::from(QWEN38_27B_UPSTREAM_REVISION),
        artifact_facts_sha256,
        safetensors_index_sha256: facts.digests.safetensors_index_sha256,
        base_artifact_identity_digest: String::new(),
    };
    identity.base_artifact_identity_digest =
        stable_json_digest(b"qwen38_training_base_identity|", &identity);
    identity
}

#[must_use]
pub fn admit_qwen38_training_adapter(
    request: &Qwen38TrainingAdapterRequest,
) -> Qwen38TrainingAdapterPlan {
    let base_identity = qwen38_training_base_identity();
    if request.schema_version != QWEN38_TRAINING_ADMISSION_SCHEMA_VERSION {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::SchemaMismatch,
            format!(
                "expected schema `{QWEN38_TRAINING_ADMISSION_SCHEMA_VERSION}`, found `{}`",
                request.schema_version
            ),
        );
    }
    if request.model_id != QWEN38_27B_MODEL_ID {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::UnsupportedModel,
            format!(
                "Qwen3.8 training requires exact model id `{QWEN38_27B_MODEL_ID}`; `{}` is not inherited or normalized",
                request.model_id
            ),
        );
    }
    if request.upstream_revision != QWEN38_27B_UPSTREAM_REVISION {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::BaseRevisionMismatch,
            "Qwen3.8 training revision does not match the canonical upstream revision",
        );
    }
    if request.base_artifact_identity_digest != base_identity.base_artifact_identity_digest {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::BaseArtifactIdentityMismatch,
            "Qwen3.8 training base artifact identity digest does not match canonical facts",
        );
    }
    if names_inherited_qwen_adapter(request.adapter_id.as_str()) {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::InheritedAdapterIdentity,
            "Qwen3.5/Qwen3.6 adapter ids cannot be relabeled as Qwen3.8 adapters",
        );
    }
    if request.request_id.trim().is_empty()
        || request.adapter_id.trim().is_empty()
        || request.adapter_revision.trim().is_empty()
        || request.lora_rank == 0
        || request.lora_rank > 64
        || !request.lora_alpha.is_finite()
        || request.lora_alpha <= 0.0
    {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::InvalidAdapterConfig,
            "request/adapter ids, revision, rank in 1..=64, and positive finite alpha are required",
        );
    }
    if request.target_modules.as_slice() != [QWEN38_LM_HEAD_TARGET] {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::UnsupportedTarget,
            format!(
                "only `{QWEN38_LM_HEAD_TARGET}` has an exact Qwen3.8 reference backward contract; requested {:?}",
                request.target_modules
            ),
        );
    }
    if request.execution_mode != Qwen38TrainingExecutionMode::TinyReferenceCpu {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::UnsupportedExecutionMode,
            "native CPU, CUDA, and Metal Qwen3.8 backward execution is not implemented",
        );
    }
    if !is_sha256(&request.corpus_manifest_sha256)
        || !is_sha256(&request.evaluation_manifest_sha256)
    {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::MissingLineage,
            "corpus and evaluation manifests require lowercase sha256 digests",
        );
    }
    if let Err(error) = request.cpu_budget.validate() {
        return refused_plan(
            request,
            base_identity,
            Qwen38TrainingRefusalCode::InvalidResourceBudget,
            error.to_string(),
        );
    }

    let binding = adapter_binding(request, &base_identity);
    finalize_plan(Qwen38TrainingAdapterPlan {
        schema_version: String::from(QWEN38_TRAINING_ADMISSION_SCHEMA_VERSION),
        request_id: request.request_id.clone(),
        status: Qwen38TrainingAdmissionStatus::Admitted,
        refusal: None,
        base_identity,
        adapter_binding: Some(binding),
        execution_mode: request.execution_mode,
        exact_backward_contract: Some(String::from(QWEN38_LM_HEAD_BACKWARD_CONTRACT)),
        deferred_target_modules: deferred_targets(),
        corpus_manifest_sha256: request.corpus_manifest_sha256.clone(),
        evaluation_manifest_sha256: request.evaluation_manifest_sha256.clone(),
        seed: request.seed,
        cpu_budget: request.cpu_budget,
        claim_boundary: String::from(
            "This admits deterministic tiny-reference CPU math for an F32 Qwen3.8 LM-head LoRA adapter. It does not admit real-checkpoint training, native CPU/CUDA/Metal backward execution, decoder-layer or vision targets, native adapter serving, real/native checkpoint recovery, evaluation gains, or promotion.",
        ),
        plan_digest: String::new(),
    })
}

pub fn finalize_qwen38_lm_head_adapter_identity(
    plan: &Qwen38TrainingAdapterPlan,
    artifact_digest: &str,
    parameter_count: u64,
) -> Result<AdapterArtifactIdentity, Qwen38AdapterIdentityError> {
    if !plan.is_admitted() {
        return Err(Qwen38AdapterIdentityError::PlanNotAdmitted);
    }
    if !is_sha256(artifact_digest) {
        return Err(Qwen38AdapterIdentityError::InvalidArtifactDigest);
    }
    let binding = plan
        .adapter_binding
        .as_ref()
        .ok_or(Qwen38AdapterIdentityError::PlanNotAdmitted)?;
    Ok(AdapterArtifactIdentity::new(
        binding.adapter_id.clone(),
        binding.adapter_revision.clone(),
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        binding.base_model_id.clone(),
        binding.base_model_revision.clone(),
        binding.base_artifact_identity_digest.clone(),
        String::from(artifact_digest),
        QuantizationMode::None,
        AdapterTargetFamily::DecoderComposite,
        parameter_count,
    )
    .with_provenance_digest(plan.plan_digest.clone()))
}

pub fn validate_qwen38_adapter_identity(
    adapter: &AdapterArtifactIdentity,
) -> Result<(), Qwen38AdapterIdentityError> {
    let base = qwen38_training_base_identity();
    if adapter.base_model_id != QWEN38_27B_MODEL_ID {
        return Err(Qwen38AdapterIdentityError::BaseModelMismatch {
            expected: String::from(QWEN38_27B_MODEL_ID),
            actual: adapter.base_model_id.clone(),
        });
    }
    if adapter.base_model_revision != QWEN38_27B_UPSTREAM_REVISION {
        return Err(Qwen38AdapterIdentityError::BaseRevisionMismatch {
            expected: String::from(QWEN38_27B_UPSTREAM_REVISION),
            actual: adapter.base_model_revision.clone(),
        });
    }
    if adapter.base_served_artifact_digest != base.base_artifact_identity_digest {
        return Err(Qwen38AdapterIdentityError::BaseArtifactMismatch {
            expected: base.base_artifact_identity_digest,
            actual: adapter.base_served_artifact_digest.clone(),
        });
    }
    if adapter.kind != AdapterArtifactKind::Lora
        || adapter.format != AdapterArtifactFormat::Safetensors
        || adapter.quantization != QuantizationMode::None
        || adapter.target_family != AdapterTargetFamily::DecoderComposite
    {
        return Err(Qwen38AdapterIdentityError::UnsupportedArtifactShape);
    }
    if !is_sha256(&adapter.artifact_digest) {
        return Err(Qwen38AdapterIdentityError::InvalidArtifactDigest);
    }
    Ok(())
}

pub fn run_qwen38_lm_head_lora_backward_reference(
    fixture: &Qwen38LmHeadLoraBackwardFixture,
) -> Result<Qwen38LmHeadLoraBackwardReceipt, Qwen38LmHeadBackwardError> {
    validate_backward_fixture(fixture)?;
    let (initial_logits, initial_loss) = loss_and_logits(
        fixture,
        fixture.lora_a.as_slice(),
        fixture.lora_b.as_slice(),
    );
    let (gradient_a, gradient_b) = analytic_gradients(
        fixture,
        &initial_logits,
        fixture.lora_a.as_slice(),
        fixture.lora_b.as_slice(),
    );
    let finite_difference_a = finite_difference_gradient(fixture, true);
    let finite_difference_b = finite_difference_gradient(fixture, false);
    let gradient_max_abs_error = gradient_a
        .iter()
        .zip(finite_difference_a.iter())
        .chain(gradient_b.iter().zip(finite_difference_b.iter()))
        .map(|(analytic, numeric)| (analytic - numeric).abs())
        .fold(0.0_f32, f32::max);
    let updated_a = fixture
        .lora_a
        .iter()
        .zip(gradient_a.iter())
        .map(|(weight, gradient)| weight - fixture.learning_rate * gradient)
        .collect::<Vec<_>>();
    let updated_b = fixture
        .lora_b
        .iter()
        .zip(gradient_b.iter())
        .map(|(weight, gradient)| weight - fixture.learning_rate * gradient)
        .collect::<Vec<_>>();
    let (updated_logits, updated_loss) =
        loss_and_logits(fixture, updated_a.as_slice(), updated_b.as_slice());
    let replay = loss_and_logits(fixture, updated_a.as_slice(), updated_b.as_slice());
    let mut receipt = Qwen38LmHeadLoraBackwardReceipt {
        schema_version: String::from(QWEN38_LM_HEAD_BACKWARD_RECEIPT_SCHEMA_VERSION),
        fixture_id: fixture.fixture_id.clone(),
        contract: String::from(QWEN38_LM_HEAD_BACKWARD_CONTRACT),
        hidden_size: fixture.hidden.len(),
        vocabulary_size: fixture.base_logits.len(),
        lora_rank: fixture.lora_rank,
        lora_alpha: fixture.lora_alpha,
        target_token_id: fixture.target_token_id,
        learning_rate: fixture.learning_rate,
        initial_loss,
        updated_loss,
        loss_improved: updated_loss < initial_loss,
        initial_logits_sha256: sha256_f32(&initial_logits),
        updated_logits_sha256: sha256_f32(&updated_logits),
        lora_a_gradient_sha256: sha256_f32(&gradient_a),
        lora_b_gradient_sha256: sha256_f32(&gradient_b),
        updated_lora_a_sha256: sha256_f32(&updated_a),
        updated_lora_b_sha256: sha256_f32(&updated_b),
        finite_difference_epsilon: fixture.finite_difference_epsilon,
        gradient_max_abs_error,
        gradient_tolerance: fixture.gradient_tolerance,
        gradient_check_passed: gradient_max_abs_error <= fixture.gradient_tolerance,
        base_weights_frozen: true,
        deterministic_replay: replay.0 == updated_logits && replay.1 == updated_loss,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_json_digest(b"qwen38_lm_head_backward_receipt|", &receipt);
    Ok(receipt)
}

pub fn qwen38_lm_head_initial_training_state(
    fixture: &Qwen38LmHeadLoraBackwardFixture,
) -> Result<Qwen38LmHeadLoraTrainingState, Qwen38TrainingRecoveryError> {
    validate_backward_fixture(fixture)?;
    Ok(Qwen38LmHeadLoraTrainingState {
        step: 0,
        lora_a: fixture.lora_a.clone(),
        lora_b: fixture.lora_b.clone(),
        adam_m_a: vec![0.0; fixture.lora_a.len()],
        adam_v_a: vec![0.0; fixture.lora_a.len()],
        adam_m_b: vec![0.0; fixture.lora_b.len()],
        adam_v_b: vec![0.0; fixture.lora_b.len()],
    })
}

pub fn run_qwen38_lm_head_adamw_step(
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    optimizer: &Qwen38LmHeadAdamWConfig,
    state: &mut Qwen38LmHeadLoraTrainingState,
) -> Result<Qwen38LmHeadAdamWStepReceipt, Qwen38TrainingRecoveryError> {
    validate_backward_fixture(fixture)?;
    validate_optimizer(optimizer)?;
    validate_training_state(fixture, state)?;
    let (logits_before, loss_before) =
        loss_and_logits(fixture, state.lora_a.as_slice(), state.lora_b.as_slice());
    let (gradient_a, gradient_b) = analytic_gradients(
        fixture,
        logits_before.as_slice(),
        state.lora_a.as_slice(),
        state.lora_b.as_slice(),
    );
    state.step = state.step.saturating_add(1);
    adamw_update(
        state.lora_a.as_mut_slice(),
        state.adam_m_a.as_mut_slice(),
        state.adam_v_a.as_mut_slice(),
        gradient_a.as_slice(),
        state.step,
        optimizer,
    );
    adamw_update(
        state.lora_b.as_mut_slice(),
        state.adam_m_b.as_mut_slice(),
        state.adam_v_b.as_mut_slice(),
        gradient_b.as_slice(),
        state.step,
        optimizer,
    );
    let (_, loss_after) =
        loss_and_logits(fixture, state.lora_a.as_slice(), state.lora_b.as_slice());
    Ok(Qwen38LmHeadAdamWStepReceipt {
        step: state.step,
        loss_before,
        loss_after,
        loss_improved: loss_after < loss_before,
        gradient_a_sha256: sha256_f32(&gradient_a),
        gradient_b_sha256: sha256_f32(&gradient_b),
        state_digest: training_state_digest(state),
    })
}

pub fn export_qwen38_lm_head_checkpoint(
    plan: &Qwen38TrainingAdapterPlan,
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    optimizer: &Qwen38LmHeadAdamWConfig,
    state: &Qwen38LmHeadLoraTrainingState,
) -> Result<Vec<u8>, Qwen38TrainingRecoveryError> {
    validate_checkpoint_inputs(plan, fixture, optimizer, state)?;
    let binding = plan
        .adapter_binding
        .as_ref()
        .ok_or(Qwen38TrainingRecoveryError::PlanNotAdmitted)?;
    let mut checkpoint = Qwen38LmHeadLoraCheckpoint {
        schema_version: String::from(QWEN38_LM_HEAD_CHECKPOINT_SCHEMA_VERSION),
        fixture_id: fixture.fixture_id.clone(),
        base_artifact_identity_digest: plan.base_identity.base_artifact_identity_digest.clone(),
        adapter_binding_digest: binding.binding_digest.clone(),
        corpus_manifest_sha256: plan.corpus_manifest_sha256.clone(),
        evaluation_manifest_sha256: plan.evaluation_manifest_sha256.clone(),
        seed: plan.seed,
        optimizer: optimizer.clone(),
        state: state.clone(),
        state_digest: training_state_digest(state),
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest = checkpoint_digest(&checkpoint);
    Ok(serde_json::to_vec(&checkpoint)?)
}

pub fn restore_qwen38_lm_head_checkpoint(
    bytes: &[u8],
    plan: &Qwen38TrainingAdapterPlan,
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    optimizer: &Qwen38LmHeadAdamWConfig,
) -> Result<Qwen38LmHeadLoraTrainingState, Qwen38TrainingRecoveryError> {
    if !plan.is_admitted() {
        return Err(Qwen38TrainingRecoveryError::PlanNotAdmitted);
    }
    let binding = plan
        .adapter_binding
        .as_ref()
        .ok_or(Qwen38TrainingRecoveryError::PlanNotAdmitted)?;
    let checkpoint = serde_json::from_slice::<Qwen38LmHeadLoraCheckpoint>(bytes)?;
    if checkpoint.schema_version != QWEN38_LM_HEAD_CHECKPOINT_SCHEMA_VERSION
        || checkpoint.fixture_id != fixture.fixture_id
        || checkpoint.base_artifact_identity_digest
            != plan.base_identity.base_artifact_identity_digest
        || checkpoint.adapter_binding_digest != binding.binding_digest
        || checkpoint.corpus_manifest_sha256 != plan.corpus_manifest_sha256
        || checkpoint.evaluation_manifest_sha256 != plan.evaluation_manifest_sha256
        || checkpoint.seed != plan.seed
        || checkpoint.optimizer != *optimizer
    {
        return Err(Qwen38TrainingRecoveryError::InvalidCheckpoint(
            String::from(
                "checkpoint identity, lineage, seed, or optimizer does not match the admitted plan",
            ),
        ));
    }
    validate_training_state(fixture, &checkpoint.state)?;
    if checkpoint.state_digest != training_state_digest(&checkpoint.state) {
        return Err(Qwen38TrainingRecoveryError::InvalidCheckpoint(
            String::from("checkpoint training-state digest mismatch"),
        ));
    }
    if checkpoint.checkpoint_digest != checkpoint_digest(&checkpoint) {
        return Err(Qwen38TrainingRecoveryError::InvalidCheckpoint(
            String::from("checkpoint envelope digest mismatch"),
        ));
    }
    Ok(checkpoint.state)
}

pub fn qwen38_lm_head_checkpoint_recovery_evidence(
    plan: &Qwen38TrainingAdapterPlan,
    fixture: &Qwen38LmHeadLoraBackwardFixture,
) -> Result<Qwen38LmHeadLoraRecoveryReceipt, Qwen38TrainingRecoveryError> {
    let optimizer = Qwen38LmHeadAdamWConfig::default();
    let mut uninterrupted = qwen38_lm_head_initial_training_state(fixture)?;
    run_qwen38_lm_head_adamw_step(fixture, &optimizer, &mut uninterrupted)?;
    let uninterrupted_second =
        run_qwen38_lm_head_adamw_step(fixture, &optimizer, &mut uninterrupted)?;

    let mut staged = qwen38_lm_head_initial_training_state(fixture)?;
    run_qwen38_lm_head_adamw_step(fixture, &optimizer, &mut staged)?;
    let checkpoint_bytes = export_qwen38_lm_head_checkpoint(plan, fixture, &optimizer, &staged)?;
    let checkpoint =
        serde_json::from_slice::<Qwen38LmHeadLoraCheckpoint>(checkpoint_bytes.as_slice())?;
    let mut resumed =
        restore_qwen38_lm_head_checkpoint(checkpoint_bytes.as_slice(), plan, fixture, &optimizer)?;
    let optimizer_state_after_restore = (
        resumed.adam_m_a.clone(),
        resumed.adam_v_a.clone(),
        resumed.adam_m_b.clone(),
        resumed.adam_v_b.clone(),
    );
    let optimizer_state_before_checkpoint = (
        staged.adam_m_a.clone(),
        staged.adam_v_a.clone(),
        staged.adam_m_b.clone(),
        staged.adam_v_b.clone(),
    );
    let resumed_second = run_qwen38_lm_head_adamw_step(fixture, &optimizer, &mut resumed)?;

    let mut tampered = serde_json::from_slice::<serde_json::Value>(checkpoint_bytes.as_slice())?;
    tampered["state"]["step"] = serde_json::Value::from(99_u64);
    let tampered_bytes = serde_json::to_vec(&tampered)?;
    let tampered_checkpoint_refused =
        restore_qwen38_lm_head_checkpoint(tampered_bytes.as_slice(), plan, fixture, &optimizer)
            .is_err();
    let uninterrupted_state_digest = training_state_digest(&uninterrupted);
    let resumed_state_digest = training_state_digest(&resumed);
    let exact_state_match = uninterrupted == resumed;
    let mut receipt = Qwen38LmHeadLoraRecoveryReceipt {
        schema_version: String::from(QWEN38_LM_HEAD_RECOVERY_RECEIPT_SCHEMA_VERSION),
        fixture_id: fixture.fixture_id.clone(),
        checkpoint,
        checkpoint_bytes_sha256: sha256_bytes(checkpoint_bytes.as_slice()),
        checkpoint_step: staged.step,
        resumed_step: resumed.step,
        uninterrupted_state_digest,
        resumed_state_digest,
        exact_state_match,
        optimizer_state_exact_match: optimizer_state_before_checkpoint
            == optimizer_state_after_restore,
        uninterrupted_second_step_loss: uninterrupted_second.loss_after,
        resumed_second_step_loss: resumed_second.loss_after,
        second_step_loss_exact_match: uninterrupted_second.loss_after == resumed_second.loss_after,
        tampered_checkpoint_refused,
        claim_boundary: String::from(
            "This proves exact JSON checkpoint and AdamW-state recovery for the tiny Qwen3.8 LM-head reference lane only. It is not a real-checkpoint or native-backend training recovery claim.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_json_digest(b"qwen38_lm_head_recovery_receipt|", &receipt);
    Ok(receipt)
}

pub fn export_qwen38_lm_head_adapter_safetensors(
    plan: &Qwen38TrainingAdapterPlan,
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    state: &Qwen38LmHeadLoraTrainingState,
) -> Result<Vec<u8>, Qwen38AdapterArtifactError> {
    if !plan.is_admitted() || plan.adapter_binding.is_none() {
        return Err(Qwen38TrainingRecoveryError::PlanNotAdmitted.into());
    }
    validate_backward_fixture(fixture).map_err(Qwen38TrainingRecoveryError::from)?;
    validate_training_state(fixture, state)?;
    if state.step == 0 {
        return Err(Qwen38TrainingRecoveryError::InvalidState(String::from(
            "adapter export requires at least one completed optimizer step",
        ))
        .into());
    }

    let binding = plan
        .adapter_binding
        .as_ref()
        .ok_or(Qwen38TrainingRecoveryError::PlanNotAdmitted)?;
    let artifact_metadata = Qwen38LmHeadLoraAdapterArtifactMetadata {
        schema_version: String::from(QWEN38_LM_HEAD_ADAPTER_ARTIFACT_METADATA_SCHEMA_VERSION),
        fixture_id: fixture.fixture_id.clone(),
        base_model_id: plan.base_identity.model_id.clone(),
        base_model_revision: plan.base_identity.upstream_revision.clone(),
        base_artifact_identity_digest: plan.base_identity.base_artifact_identity_digest.clone(),
        adapter_id: binding.adapter_id.clone(),
        adapter_revision: binding.adapter_revision.clone(),
        adapter_binding_digest: binding.binding_digest.clone(),
        plan_digest: plan.plan_digest.clone(),
        corpus_manifest_sha256: plan.corpus_manifest_sha256.clone(),
        evaluation_manifest_sha256: plan.evaluation_manifest_sha256.clone(),
        seed: plan.seed,
        trained_step: state.step,
        target: String::from(QWEN38_LM_HEAD_TARGET),
        lora_rank: fixture.lora_rank,
        lora_alpha: fixture.lora_alpha,
    };
    let mut safetensors_metadata = HashMap::new();
    safetensors_metadata.insert(
        String::from("psionic.qwen38_lm_head_lora"),
        serde_json::to_string(&artifact_metadata)
            .map_err(|error| Qwen38AdapterArtifactError::ArtifactMetadata(error.to_string()))?,
    );
    let lora_a_bytes = encode_f32_bytes(state.lora_a.as_slice());
    let lora_b_bytes = encode_f32_bytes(state.lora_b.as_slice());
    let view_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![fixture.lora_rank, fixture.hidden.len()],
        lora_a_bytes.as_slice(),
    )
    .map_err(|error| Qwen38AdapterArtifactError::Safetensors(error.to_string()))?;
    let view_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![fixture.base_logits.len(), fixture.lora_rank],
        lora_b_bytes.as_slice(),
    )
    .map_err(|error| Qwen38AdapterArtifactError::Safetensors(error.to_string()))?;
    serialize(
        [
            ("lm_head.lora_A.weight", view_a),
            ("lm_head.lora_B.weight", view_b),
        ],
        Some(safetensors_metadata),
    )
    .map_err(|error| Qwen38AdapterArtifactError::Safetensors(error.to_string()))
}

pub fn load_qwen38_lm_head_adapter_safetensors(
    bytes: &[u8],
    identity: AdapterArtifactIdentity,
    alpha: f32,
) -> Result<LmHeadLoraAdapterArtifact, Qwen38AdapterArtifactError> {
    validate_qwen38_adapter_identity(&identity)?;
    let actual_digest = sha256_bytes(bytes);
    if actual_digest != identity.artifact_digest {
        return Err(Qwen38AdapterArtifactError::ArtifactDigestMismatch {
            expected: identity.artifact_digest.clone(),
            actual: actual_digest,
        });
    }
    let metadata = read_qwen38_lm_head_adapter_metadata(bytes)?;
    let expected_provenance = identity.provenance_digest.as_deref().unwrap_or_default();
    if metadata.schema_version != QWEN38_LM_HEAD_ADAPTER_ARTIFACT_METADATA_SCHEMA_VERSION
        || metadata.base_model_id != identity.base_model_id
        || metadata.base_model_revision != identity.base_model_revision
        || metadata.base_artifact_identity_digest != identity.base_served_artifact_digest
        || metadata.adapter_id != identity.adapter_id
        || metadata.adapter_revision != identity.adapter_revision
        || metadata.plan_digest != expected_provenance
        || metadata.target != QWEN38_LM_HEAD_TARGET
        || !is_sha256(&metadata.adapter_binding_digest)
        || !is_sha256(&metadata.corpus_manifest_sha256)
        || !is_sha256(&metadata.evaluation_manifest_sha256)
        || metadata.trained_step == 0
        || metadata.lora_rank == 0
        || !metadata.lora_alpha.is_finite()
        || metadata.lora_alpha <= 0.0
        || metadata.lora_alpha.to_bits() != alpha.to_bits()
    {
        return Err(Qwen38AdapterArtifactError::ArtifactMetadata(String::from(
            "artifact identity, plan, lineage, target, rank, alpha, or trained step does not match the admitted Qwen3.8 adapter",
        )));
    }
    let artifact = LmHeadLoraAdapterArtifact::from_safetensors_bytes(bytes, identity, alpha)?;
    if artifact.rank != metadata.lora_rank {
        return Err(Qwen38AdapterArtifactError::ArtifactMetadata(String::from(
            "artifact metadata rank does not match the LoRA tensor rank",
        )));
    }
    let actual_parameter_count = artifact
        .lora_a()
        .len()
        .saturating_add(artifact.lora_b().len()) as u64;
    if actual_parameter_count != artifact.identity.parameter_count {
        return Err(Qwen38AdapterArtifactError::ParameterCountMismatch {
            expected: artifact.identity.parameter_count,
            actual: actual_parameter_count,
        });
    }
    Ok(artifact)
}

pub fn qwen38_lm_head_adapter_artifact_evidence(
    plan: &Qwen38TrainingAdapterPlan,
    fixture: &Qwen38LmHeadLoraBackwardFixture,
) -> Result<(Qwen38LmHeadLoraAdapterArtifactReceipt, Vec<u8>), Qwen38AdapterArtifactError> {
    let optimizer = Qwen38LmHeadAdamWConfig::default();
    let mut state = qwen38_lm_head_initial_training_state(fixture)?;
    run_qwen38_lm_head_adamw_step(fixture, &optimizer, &mut state)?;
    run_qwen38_lm_head_adamw_step(fixture, &optimizer, &mut state)?;
    let artifact_bytes = export_qwen38_lm_head_adapter_safetensors(plan, fixture, &state)?;
    let artifact_sha256 = sha256_bytes(artifact_bytes.as_slice());
    let parameter_count = state.lora_a.len().saturating_add(state.lora_b.len()) as u64;
    let identity =
        finalize_qwen38_lm_head_adapter_identity(plan, artifact_sha256.as_str(), parameter_count)?;
    let loaded = load_qwen38_lm_head_adapter_safetensors(
        artifact_bytes.as_slice(),
        identity.clone(),
        fixture.lora_alpha,
    )?;
    let metadata = read_qwen38_lm_head_adapter_metadata(artifact_bytes.as_slice())?;
    let serving_binding = AdapterServingBinding::new(
        "qwen38-lm-head-reference-serving-v1",
        plan.base_identity.model_id.clone(),
        plan.base_identity.upstream_revision.clone(),
        plan.base_identity.base_artifact_identity_digest.clone(),
        AdapterResidencyMode::HotSwapOverlay,
        vec![identity.clone()],
    );
    let expected_logits =
        loss_and_logits(fixture, state.lora_a.as_slice(), state.lora_b.as_slice()).0;
    let mut served_logits = fixture.base_logits.clone();
    loaded.apply_to_logits(fixture.hidden.as_slice(), served_logits.as_mut_slice())?;
    let mut replay_logits = fixture.base_logits.clone();
    loaded.apply_to_logits(fixture.hidden.as_slice(), replay_logits.as_mut_slice())?;

    let mut tampered_bytes = artifact_bytes.clone();
    if let Some(last) = tampered_bytes.last_mut() {
        *last ^= 1;
    }
    let tampered_artifact_refused = matches!(
        load_qwen38_lm_head_adapter_safetensors(
            tampered_bytes.as_slice(),
            identity.clone(),
            fixture.lora_alpha,
        ),
        Err(Qwen38AdapterArtifactError::ArtifactDigestMismatch { .. })
    );
    let metadata_drift_refused = matches!(
        load_qwen38_lm_head_adapter_safetensors(
            artifact_bytes.as_slice(),
            identity.clone(),
            fixture.lora_alpha + 1.0,
        ),
        Err(Qwen38AdapterArtifactError::ArtifactMetadata(_))
    );
    let mut receipt = Qwen38LmHeadLoraAdapterArtifactReceipt {
        schema_version: String::from(QWEN38_LM_HEAD_ADAPTER_ARTIFACT_RECEIPT_SCHEMA_VERSION),
        fixture_id: fixture.fixture_id.clone(),
        artifact_ref: String::from(QWEN38_LM_HEAD_ADAPTER_ARTIFACT_REF),
        trained_step: state.step,
        tensor_names: vec![
            String::from("lm_head.lora_A.weight"),
            String::from("lm_head.lora_B.weight"),
        ],
        lora_a_shape: vec![fixture.lora_rank, fixture.hidden.len()],
        lora_b_shape: vec![fixture.base_logits.len(), fixture.lora_rank],
        artifact_byte_length: artifact_bytes.len(),
        artifact_sha256,
        metadata,
        adapter_identity_digest: identity.stable_digest(),
        adapter_identity: identity,
        serving_binding,
        loaded_rank: loaded.rank,
        loaded_hidden_size: loaded.hidden_size,
        loaded_vocabulary_size: loaded.vocab_size,
        loaded_lora_a_sha256: sha256_f32(loaded.lora_a()),
        loaded_lora_b_sha256: sha256_f32(loaded.lora_b()),
        expected_logits_sha256: sha256_f32(expected_logits.as_slice()),
        served_logits_sha256: sha256_f32(served_logits.as_slice()),
        save_load_exact_match: loaded.lora_a() == state.lora_a.as_slice()
            && loaded.lora_b() == state.lora_b.as_slice(),
        serve_logits_exact_match: served_logits == expected_logits,
        deterministic_serve_replay: replay_logits == served_logits,
        tampered_artifact_refused,
        metadata_drift_refused,
        claim_boundary: String::from(
            "This proves Qwen3.8-bound tiny-reference safetensors save/load parity and logits-overlay parity through the generic LM-head adapter runtime. It does not prove adapter execution on the native Qwen3.8 CPU, CUDA, or Metal decoder runtime.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest =
        stable_json_digest(b"qwen38_lm_head_adapter_artifact_receipt|", &receipt);
    Ok((receipt, artifact_bytes))
}

pub fn qwen38_training_adapter_artifact_bytes() -> Result<Vec<u8>, Qwen38AdapterArtifactError> {
    let plan = admit_qwen38_training_adapter(&Qwen38TrainingAdapterRequest::default());
    let (_, bytes) = qwen38_lm_head_adapter_artifact_evidence(
        &plan,
        &Qwen38LmHeadLoraBackwardFixture::default(),
    )?;
    Ok(bytes)
}

pub fn qwen38_training_adapter_evidence_report()
-> Result<Qwen38TrainingAdapterEvidenceReport, Qwen38TrainingEvidenceError> {
    let request = Qwen38TrainingAdapterRequest::default();
    let admitted_plan = admit_qwen38_training_adapter(&request);
    let backward_receipt =
        run_qwen38_lm_head_lora_backward_reference(&Qwen38LmHeadLoraBackwardFixture::default())?;
    let checkpoint_recovery = qwen38_lm_head_checkpoint_recovery_evidence(
        &admitted_plan,
        &Qwen38LmHeadLoraBackwardFixture::default(),
    )?;
    let (adapter_artifact, _) = qwen38_lm_head_adapter_artifact_evidence(
        &admitted_plan,
        &Qwen38LmHeadLoraBackwardFixture::default(),
    )?;
    let refusals = [
        (
            "inherited_model",
            Qwen38TrainingAdapterRequest {
                model_id: String::from("Qwen/Qwen3.6-27B"),
                ..request.clone()
            },
        ),
        (
            "inherited_adapter",
            Qwen38TrainingAdapterRequest {
                adapter_id: String::from("qwen36-legal-lora-001"),
                ..request.clone()
            },
        ),
        (
            "decoder_target",
            Qwen38TrainingAdapterRequest {
                target_modules: vec![String::from("q_proj")],
                ..request.clone()
            },
        ),
        (
            "native_cuda",
            Qwen38TrainingAdapterRequest {
                execution_mode: Qwen38TrainingExecutionMode::NativeCuda,
                ..request.clone()
            },
        ),
        (
            "artifact_drift",
            Qwen38TrainingAdapterRequest {
                base_artifact_identity_digest: sha256_bytes(b"different-base"),
                ..request.clone()
            },
        ),
        (
            "missing_lineage",
            Qwen38TrainingAdapterRequest {
                corpus_manifest_sha256: String::from("missing"),
                ..request
            },
        ),
    ]
    .into_iter()
    .map(|(name, request)| (String::from(name), admit_qwen38_training_adapter(&request)))
    .collect();
    let mut report = Qwen38TrainingAdapterEvidenceReport {
        schema_version: String::from(QWEN38_TRAINING_EVIDENCE_SCHEMA_VERSION),
        phase: String::from("R12"),
        status: String::from("implemented_early"),
        base_identity: qwen38_training_base_identity(),
        admitted_plan,
        backward_receipt,
        checkpoint_recovery,
        adapter_artifact,
        refusals,
        real_checkpoint_training_admitted: false,
        native_backward_admitted: false,
        adapter_artifact_written: true,
        tiny_reference_adapter_serving_parity_admitted: true,
        adapter_serving_admitted: false,
        tiny_reference_checkpoint_recovery_admitted: true,
        checkpoint_recovery_admitted: false,
        promotion_admitted: false,
        claim_boundary: String::from(
            "This report proves Qwen3.8-specific base/adapter admission, deterministic tiny-reference F32 LM-head LoRA gradients, exact tiny-reference AdamW checkpoint recovery, and generic LM-head safetensors save/load/overlay parity. It makes no real-checkpoint training, native backward or recovery, native Qwen3.8 adapter serving, evaluation, or promotion claim.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_json_digest(b"qwen38_training_adapter_evidence|", &report);
    Ok(report)
}

fn refused_plan(
    request: &Qwen38TrainingAdapterRequest,
    base_identity: Qwen38TrainingBaseIdentity,
    code: Qwen38TrainingRefusalCode,
    detail: impl Into<String>,
) -> Qwen38TrainingAdapterPlan {
    finalize_plan(Qwen38TrainingAdapterPlan {
        schema_version: String::from(QWEN38_TRAINING_ADMISSION_SCHEMA_VERSION),
        request_id: request.request_id.clone(),
        status: Qwen38TrainingAdmissionStatus::Refused,
        refusal: Some(Qwen38TrainingRefusal {
            code,
            detail: detail.into(),
        }),
        base_identity,
        adapter_binding: None,
        execution_mode: request.execution_mode,
        exact_backward_contract: None,
        deferred_target_modules: deferred_targets(),
        corpus_manifest_sha256: request.corpus_manifest_sha256.clone(),
        evaluation_manifest_sha256: request.evaluation_manifest_sha256.clone(),
        seed: request.seed,
        cpu_budget: request.cpu_budget,
        claim_boundary: String::from(
            "The request was refused before training. No adapter, checkpoint, evaluation, serving binding, or promotion claim was produced.",
        ),
        plan_digest: String::new(),
    })
}

fn finalize_plan(mut plan: Qwen38TrainingAdapterPlan) -> Qwen38TrainingAdapterPlan {
    plan.plan_digest = stable_json_digest(b"qwen38_training_adapter_plan|", &plan);
    plan
}

fn adapter_binding(
    request: &Qwen38TrainingAdapterRequest,
    base: &Qwen38TrainingBaseIdentity,
) -> Qwen38AdapterIdentityBinding {
    let mut binding = Qwen38AdapterIdentityBinding {
        schema_version: String::from(QWEN38_ADAPTER_IDENTITY_BINDING_SCHEMA_VERSION),
        adapter_id: request.adapter_id.clone(),
        adapter_revision: request.adapter_revision.clone(),
        base_model_id: base.model_id.clone(),
        base_model_revision: base.upstream_revision.clone(),
        base_artifact_identity_digest: base.base_artifact_identity_digest.clone(),
        target_modules: request.target_modules.clone(),
        binding_digest: String::new(),
    };
    binding.binding_digest = stable_json_digest(b"qwen38_adapter_identity_binding|", &binding);
    binding
}

fn deferred_targets() -> Vec<String> {
    QWEN38_DEFERRED_ADAPTER_TARGETS
        .iter()
        .map(|target| String::from(*target))
        .collect()
}

fn names_inherited_qwen_adapter(adapter_id: &str) -> bool {
    let normalized = adapter_id.to_ascii_lowercase();
    ["qwen3.5", "qwen35", "qwen3.6", "qwen36"]
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn validate_backward_fixture(
    fixture: &Qwen38LmHeadLoraBackwardFixture,
) -> Result<(), Qwen38LmHeadBackwardError> {
    let hidden_size = fixture.hidden.len();
    let vocabulary_size = fixture.base_logits.len();
    if fixture.fixture_id.trim().is_empty()
        || hidden_size == 0
        || vocabulary_size < 2
        || fixture.lora_rank == 0
        || fixture.lora_a.len() != fixture.lora_rank.saturating_mul(hidden_size)
        || fixture.lora_b.len() != vocabulary_size.saturating_mul(fixture.lora_rank)
        || fixture.target_token_id >= vocabulary_size
    {
        return Err(Qwen38LmHeadBackwardError::InvalidFixture(String::from(
            "ids, dimensions, rank, matrix shapes, and target token must be valid",
        )));
    }
    if !fixture.lora_alpha.is_finite()
        || fixture.lora_alpha <= 0.0
        || !fixture.learning_rate.is_finite()
        || fixture.learning_rate <= 0.0
        || !fixture.finite_difference_epsilon.is_finite()
        || fixture.finite_difference_epsilon <= 0.0
        || !fixture.gradient_tolerance.is_finite()
        || fixture.gradient_tolerance <= 0.0
        || fixture
            .hidden
            .iter()
            .chain(fixture.base_logits.iter())
            .chain(fixture.lora_a.iter())
            .chain(fixture.lora_b.iter())
            .any(|value| !value.is_finite())
    {
        return Err(Qwen38LmHeadBackwardError::InvalidFixture(String::from(
            "all values and positive hyperparameters must be finite",
        )));
    }
    Ok(())
}

fn validate_checkpoint_inputs(
    plan: &Qwen38TrainingAdapterPlan,
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    optimizer: &Qwen38LmHeadAdamWConfig,
    state: &Qwen38LmHeadLoraTrainingState,
) -> Result<(), Qwen38TrainingRecoveryError> {
    if !plan.is_admitted() || plan.adapter_binding.is_none() {
        return Err(Qwen38TrainingRecoveryError::PlanNotAdmitted);
    }
    validate_backward_fixture(fixture)?;
    validate_optimizer(optimizer)?;
    validate_training_state(fixture, state)
}

fn validate_optimizer(
    optimizer: &Qwen38LmHeadAdamWConfig,
) -> Result<(), Qwen38TrainingRecoveryError> {
    if !optimizer.learning_rate.is_finite()
        || optimizer.learning_rate <= 0.0
        || !optimizer.beta1.is_finite()
        || !(0.0..1.0).contains(&optimizer.beta1)
        || !optimizer.beta2.is_finite()
        || !(0.0..1.0).contains(&optimizer.beta2)
        || !optimizer.epsilon.is_finite()
        || optimizer.epsilon <= 0.0
        || !optimizer.weight_decay.is_finite()
        || optimizer.weight_decay < 0.0
    {
        return Err(Qwen38TrainingRecoveryError::InvalidOptimizer(String::from(
            "AdamW values must be finite and inside their admitted ranges",
        )));
    }
    Ok(())
}

fn validate_training_state(
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    state: &Qwen38LmHeadLoraTrainingState,
) -> Result<(), Qwen38TrainingRecoveryError> {
    let a_len = fixture.lora_a.len();
    let b_len = fixture.lora_b.len();
    if state.lora_a.len() != a_len
        || state.adam_m_a.len() != a_len
        || state.adam_v_a.len() != a_len
        || state.lora_b.len() != b_len
        || state.adam_m_b.len() != b_len
        || state.adam_v_b.len() != b_len
    {
        return Err(Qwen38TrainingRecoveryError::InvalidState(String::from(
            "LoRA and AdamW state shapes do not match the admitted fixture",
        )));
    }
    if state
        .lora_a
        .iter()
        .chain(state.lora_b.iter())
        .chain(state.adam_m_a.iter())
        .chain(state.adam_v_a.iter())
        .chain(state.adam_m_b.iter())
        .chain(state.adam_v_b.iter())
        .any(|value| !value.is_finite())
    {
        return Err(Qwen38TrainingRecoveryError::InvalidState(String::from(
            "LoRA and AdamW state values must be finite",
        )));
    }
    Ok(())
}

fn adamw_update(
    weights: &mut [f32],
    first_moment: &mut [f32],
    second_moment: &mut [f32],
    gradients: &[f32],
    step: u64,
    optimizer: &Qwen38LmHeadAdamWConfig,
) {
    let first_correction = 1.0 - optimizer.beta1.powf(step as f32);
    let second_correction = 1.0 - optimizer.beta2.powf(step as f32);
    for index in 0..weights.len() {
        first_moment[index] =
            optimizer.beta1 * first_moment[index] + (1.0 - optimizer.beta1) * gradients[index];
        second_moment[index] = optimizer.beta2 * second_moment[index]
            + (1.0 - optimizer.beta2) * gradients[index] * gradients[index];
        let corrected_first = first_moment[index] / first_correction;
        let corrected_second = second_moment[index] / second_correction;
        let update = corrected_first / (corrected_second.sqrt() + optimizer.epsilon)
            + optimizer.weight_decay * weights[index];
        weights[index] -= optimizer.learning_rate * update;
    }
}

fn training_state_digest(state: &Qwen38LmHeadLoraTrainingState) -> String {
    stable_json_digest(b"qwen38_lm_head_training_state|", state)
}

fn checkpoint_digest(checkpoint: &Qwen38LmHeadLoraCheckpoint) -> String {
    let mut digestible = checkpoint.clone();
    digestible.checkpoint_digest.clear();
    stable_json_digest(b"qwen38_lm_head_checkpoint|", &digestible)
}

fn read_qwen38_lm_head_adapter_metadata(
    bytes: &[u8],
) -> Result<Qwen38LmHeadLoraAdapterArtifactMetadata, Qwen38AdapterArtifactError> {
    let (_, metadata) = SafeTensors::read_metadata(bytes)
        .map_err(|error| Qwen38AdapterArtifactError::Safetensors(error.to_string()))?;
    let encoded = metadata
        .metadata()
        .as_ref()
        .and_then(|values| values.get("psionic.qwen38_lm_head_lora"))
        .ok_or_else(|| {
            Qwen38AdapterArtifactError::ArtifactMetadata(String::from(
                "missing `psionic.qwen38_lm_head_lora` metadata",
            ))
        })?;
    serde_json::from_str(encoded)
        .map_err(|error| Qwen38AdapterArtifactError::ArtifactMetadata(error.to_string()))
}

fn loss_and_logits(
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    lora_a: &[f32],
    lora_b: &[f32],
) -> (Vec<f32>, f32) {
    let hidden_size = fixture.hidden.len();
    let rank = fixture.lora_rank;
    let scale = fixture.lora_alpha / rank as f32;
    let intermediate = lora_a
        .chunks_exact(hidden_size)
        .map(|row| dot(row, fixture.hidden.as_slice()))
        .collect::<Vec<_>>();
    let logits = fixture
        .base_logits
        .iter()
        .zip(lora_b.chunks_exact(rank))
        .map(|(base, row)| base + scale * dot(row, intermediate.as_slice()))
        .collect::<Vec<_>>();
    let probabilities = softmax(logits.as_slice());
    (logits, -probabilities[fixture.target_token_id].ln())
}

fn analytic_gradients(
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    logits: &[f32],
    lora_a: &[f32],
    lora_b: &[f32],
) -> (Vec<f32>, Vec<f32>) {
    let hidden_size = fixture.hidden.len();
    let rank = fixture.lora_rank;
    let scale = fixture.lora_alpha / rank as f32;
    let intermediate = lora_a
        .chunks_exact(hidden_size)
        .map(|row| dot(row, fixture.hidden.as_slice()))
        .collect::<Vec<_>>();
    let mut d_logits = softmax(logits);
    d_logits[fixture.target_token_id] -= 1.0;
    let mut gradient_b = vec![0.0; lora_b.len()];
    for (vocabulary_index, d_logit) in d_logits.iter().copied().enumerate() {
        for rank_index in 0..rank {
            gradient_b[vocabulary_index * rank + rank_index] =
                scale * d_logit * intermediate[rank_index];
        }
    }
    let mut d_intermediate = vec![0.0; rank];
    for rank_index in 0..rank {
        d_intermediate[rank_index] = scale
            * d_logits
                .iter()
                .enumerate()
                .map(|(vocabulary_index, d_logit)| {
                    lora_b[vocabulary_index * rank + rank_index] * d_logit
                })
                .sum::<f32>();
    }
    let mut gradient_a = vec![0.0; lora_a.len()];
    for rank_index in 0..rank {
        for hidden_index in 0..hidden_size {
            gradient_a[rank_index * hidden_size + hidden_index] =
                d_intermediate[rank_index] * fixture.hidden[hidden_index];
        }
    }
    (gradient_a, gradient_b)
}

fn finite_difference_gradient(
    fixture: &Qwen38LmHeadLoraBackwardFixture,
    for_lora_a: bool,
) -> Vec<f32> {
    let weights = if for_lora_a {
        fixture.lora_a.as_slice()
    } else {
        fixture.lora_b.as_slice()
    };
    (0..weights.len())
        .map(|index| {
            let mut positive_a = fixture.lora_a.clone();
            let mut negative_a = fixture.lora_a.clone();
            let mut positive_b = fixture.lora_b.clone();
            let mut negative_b = fixture.lora_b.clone();
            if for_lora_a {
                positive_a[index] += fixture.finite_difference_epsilon;
                negative_a[index] -= fixture.finite_difference_epsilon;
            } else {
                positive_b[index] += fixture.finite_difference_epsilon;
                negative_b[index] -= fixture.finite_difference_epsilon;
            }
            let positive = loss_and_logits(fixture, positive_a.as_slice(), positive_b.as_slice()).1;
            let negative = loss_and_logits(fixture, negative_a.as_slice(), negative_b.as_slice()).1;
            (positive - negative) / (2.0 * fixture.finite_difference_epsilon)
        })
        .collect()
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let maximum = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut values = logits
        .iter()
        .map(|logit| (logit - maximum).exp())
        .collect::<Vec<_>>();
    let total = values.iter().sum::<f32>();
    for value in &mut values {
        *value /= total;
    }
    values
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn stable_json_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn sha256_f32(values: &[f32]) -> String {
    let bytes = encode_f32_bytes(values);
    sha256_bytes(bytes.as_slice())
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen38_training_admits_only_exact_tiny_lm_head_contract() {
        let request = Qwen38TrainingAdapterRequest::default();
        let first = admit_qwen38_training_adapter(&request);
        let second = admit_qwen38_training_adapter(&request);

        assert!(first.is_admitted());
        assert_eq!(first, second);
        assert_eq!(
            first.exact_backward_contract.as_deref(),
            Some(QWEN38_LM_HEAD_BACKWARD_CONTRACT)
        );
        assert_eq!(first.cpu_budget.effective_cores(), 1);
        assert_eq!(first.cpu_budget.max_parallel_workers(), 1);
        assert!(!first.plan_digest.is_empty());
        assert_eq!(
            first
                .adapter_binding
                .as_ref()
                .expect("admitted binding")
                .base_model_id,
            QWEN38_27B_MODEL_ID
        );
    }

    #[test]
    fn qwen38_training_refuses_inherited_models_adapters_and_targets() {
        let inherited_model = Qwen38TrainingAdapterRequest {
            model_id: String::from("Qwen/Qwen3.6-27B"),
            ..Qwen38TrainingAdapterRequest::default()
        };
        let inherited_adapter = Qwen38TrainingAdapterRequest {
            adapter_id: String::from("qwen36-legal-lora-001"),
            ..Qwen38TrainingAdapterRequest::default()
        };
        let unsupported_target = Qwen38TrainingAdapterRequest {
            target_modules: vec![String::from("q_proj")],
            ..Qwen38TrainingAdapterRequest::default()
        };

        assert_eq!(
            admit_qwen38_training_adapter(&inherited_model)
                .refusal
                .expect("model refusal")
                .code,
            Qwen38TrainingRefusalCode::UnsupportedModel
        );
        assert_eq!(
            admit_qwen38_training_adapter(&inherited_adapter)
                .refusal
                .expect("adapter refusal")
                .code,
            Qwen38TrainingRefusalCode::InheritedAdapterIdentity
        );
        assert_eq!(
            admit_qwen38_training_adapter(&unsupported_target)
                .refusal
                .expect("target refusal")
                .code,
            Qwen38TrainingRefusalCode::UnsupportedTarget
        );
    }

    #[test]
    fn qwen38_training_refuses_identity_lineage_backend_and_budget_drift() {
        let identity_drift = Qwen38TrainingAdapterRequest {
            base_artifact_identity_digest: sha256_bytes(b"different-base"),
            ..Qwen38TrainingAdapterRequest::default()
        };
        let missing_lineage = Qwen38TrainingAdapterRequest {
            corpus_manifest_sha256: String::from("missing"),
            ..Qwen38TrainingAdapterRequest::default()
        };
        let native_cuda = Qwen38TrainingAdapterRequest {
            execution_mode: Qwen38TrainingExecutionMode::NativeCuda,
            ..Qwen38TrainingAdapterRequest::default()
        };
        let invalid_budget = Qwen38TrainingAdapterRequest {
            cpu_budget: PsionicTrainCpuBudget::ExplicitCores {
                cores: 4,
                opted_in: false,
            },
            ..Qwen38TrainingAdapterRequest::default()
        };

        assert_eq!(
            admit_qwen38_training_adapter(&identity_drift)
                .refusal
                .expect("identity refusal")
                .code,
            Qwen38TrainingRefusalCode::BaseArtifactIdentityMismatch
        );
        assert_eq!(
            admit_qwen38_training_adapter(&missing_lineage)
                .refusal
                .expect("lineage refusal")
                .code,
            Qwen38TrainingRefusalCode::MissingLineage
        );
        assert_eq!(
            admit_qwen38_training_adapter(&native_cuda)
                .refusal
                .expect("backend refusal")
                .code,
            Qwen38TrainingRefusalCode::UnsupportedExecutionMode
        );
        assert_eq!(
            admit_qwen38_training_adapter(&invalid_budget)
                .refusal
                .expect("budget refusal")
                .code,
            Qwen38TrainingRefusalCode::InvalidResourceBudget
        );
    }

    #[test]
    fn qwen38_lm_head_backward_matches_finite_difference_and_improves_loss() {
        let receipt =
            run_qwen38_lm_head_lora_backward_reference(&Qwen38LmHeadLoraBackwardFixture::default())
                .expect("reference backward");

        assert!(receipt.gradient_check_passed);
        assert!(receipt.loss_improved);
        assert!(receipt.deterministic_replay);
        assert!(receipt.base_weights_frozen);
        assert!(receipt.gradient_max_abs_error <= receipt.gradient_tolerance);
        assert_ne!(receipt.initial_logits_sha256, receipt.updated_logits_sha256);
    }

    #[test]
    fn qwen38_lm_head_checkpoint_restores_exact_adamw_state() {
        let plan = admit_qwen38_training_adapter(&Qwen38TrainingAdapterRequest::default());
        let receipt = qwen38_lm_head_checkpoint_recovery_evidence(
            &plan,
            &Qwen38LmHeadLoraBackwardFixture::default(),
        )
        .expect("checkpoint recovery");

        assert_eq!(receipt.checkpoint_step, 1);
        assert_eq!(receipt.resumed_step, 2);
        assert!(receipt.exact_state_match);
        assert!(receipt.optimizer_state_exact_match);
        assert!(receipt.second_step_loss_exact_match);
        assert!(receipt.tampered_checkpoint_refused);
        assert_eq!(
            receipt.uninterrupted_state_digest,
            receipt.resumed_state_digest
        );
    }

    #[test]
    fn qwen38_lm_head_adapter_roundtrips_and_serves_exact_reference_logits() {
        let plan = admit_qwen38_training_adapter(&Qwen38TrainingAdapterRequest::default());
        let (receipt, bytes) = qwen38_lm_head_adapter_artifact_evidence(
            &plan,
            &Qwen38LmHeadLoraBackwardFixture::default(),
        )
        .expect("adapter artifact evidence");

        assert_eq!(receipt.trained_step, 2);
        assert!(receipt.save_load_exact_match);
        assert!(receipt.serve_logits_exact_match);
        assert!(receipt.deterministic_serve_replay);
        assert!(receipt.tampered_artifact_refused);
        assert!(receipt.metadata_drift_refused);
        assert_eq!(receipt.artifact_sha256, sha256_bytes(bytes.as_slice()));
        assert_eq!(
            receipt.serving_binding.base_served_artifact_digest,
            plan.base_identity.base_artifact_identity_digest
        );
        assert_eq!(receipt.serving_binding.adapters.len(), 1);
    }

    #[test]
    fn qwen38_finalized_adapter_identity_cannot_bind_to_qwen36() {
        let plan = admit_qwen38_training_adapter(&Qwen38TrainingAdapterRequest::default());
        let artifact_digest = sha256_bytes(b"qwen38-adapter");
        let adapter = finalize_qwen38_lm_head_adapter_identity(&plan, artifact_digest.as_str(), 16)
            .expect("adapter identity");
        validate_qwen38_adapter_identity(&adapter).expect("Qwen3.8 identity");

        let mut inherited = adapter;
        inherited.base_model_id = String::from("Qwen/Qwen3.6-27B");
        assert!(matches!(
            validate_qwen38_adapter_identity(&inherited),
            Err(Qwen38AdapterIdentityError::BaseModelMismatch { .. })
        ));
    }
}
