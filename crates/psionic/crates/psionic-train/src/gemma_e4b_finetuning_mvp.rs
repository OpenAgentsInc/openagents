use std::collections::BTreeSet;

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
};
use psionic_core::QuantizationMode;
use psionic_data::{TokenizerDigest, TokenizerFamily};
use psionic_models::{golden_prompt_fixture, golden_tokenizer_fixture};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{AdapterTargetIdentity, AdapterWindowContractError};

/// Stable schema version for the first Gemma e4b finetuning MVP contract.
pub const GEMMA_E4B_FINETUNING_MVP_SCHEMA_VERSION: &str = "psionic.gemma4_e4b_finetuning_mvp.v1";
/// Stable training-family identifier for the first Gemma e4b finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_TRAINING_FAMILY_ID: &str = "gemma4.e4b.cuda.adapter_sft.v1";
/// Stable model identifier admitted by the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_MODEL_ID: &str = "gemma4:e4b";
/// Stable model-family label admitted by the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_MODEL_FAMILY: &str = "gemma4";
/// Stable backend label reserved for the first Gemma CUDA adapter trainer.
pub const GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL: &str =
    "open_adapter_backend.cuda.gemma4_e4b_lm_head";
/// Stable adapter family admitted by the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_ADAPTER_FAMILY: &str = "gemma4.e4b.decoder_lm_head_lora";
/// Stable adapter artifact format admitted by the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_ADAPTER_FORMAT: &str = "safetensors";
/// Stable adapter target id admitted by the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_ADAPTER_TARGET_ID: &str = "lm_head";
/// Stable base-model revision bound by the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_BASE_MODEL_REVISION: &str = "v1";
/// Stable checkpoint family label bound by the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_CHECKPOINT_FAMILY: &str = "train.gemma4.e4b.adapter_sft";
/// Stable tokenizer digest bound by the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_TOKENIZER_DIGEST: &str = "gemma4-fixture";
/// Stable promoted-serving posture for the first Gemma finetuning MVP.
pub const GEMMA_E4B_FINETUNING_MVP_PROMOTED_SERVING_POSTURE: &str =
    "optional_promoted_revision_mesh_lane";

/// Declared request modality for one Gemma finetuning attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaFinetuningInputModality {
    /// Text-only supervised finetuning.
    Text,
    /// Vision or video input finetuning.
    Multimodal,
    /// Audio input finetuning.
    Audio,
}

/// Declared update mode for one Gemma finetuning attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaFinetuningUpdateMode {
    /// Adapter-only supervised finetuning.
    AdapterSft,
    /// Full-model supervised finetuning.
    FullModelSft,
}

/// Admission request for the first Gemma finetuning MVP.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bFinetuningMvpRequest {
    /// Requested served model identifier.
    pub model_id: String,
    /// Requested backend label.
    pub execution_backend_label: String,
    /// Requested data modality.
    pub modality: GemmaFinetuningInputModality,
    /// Requested update mode.
    pub update_mode: GemmaFinetuningUpdateMode,
}

impl GemmaE4bFinetuningMvpRequest {
    /// Creates one finetuning admission request.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        execution_backend_label: impl Into<String>,
        modality: GemmaFinetuningInputModality,
        update_mode: GemmaFinetuningUpdateMode,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            execution_backend_label: execution_backend_label.into(),
            modality,
            update_mode,
        }
    }
}

/// Explicit refusal family for the first Gemma finetuning MVP.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaE4bFinetuningRefusalKind {
    UnsupportedModelId,
    UnsupportedExecutionBackend,
    Dense31b,
    Sparse26bA4b,
    Multimodal,
    Audio,
    Metal,
    FullModelFinetuning,
}

/// One explicit refusal retained by the first Gemma finetuning MVP.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bFinetuningRefusal {
    /// Stable refusal id.
    pub refusal_id: String,
    /// Refusal kind.
    pub refusal_kind: GemmaE4bFinetuningRefusalKind,
    /// Machine-legible refusal detail.
    pub detail: String,
}

/// Canonical first Gemma e4b finetuning MVP contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bFinetuningMvpContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable training-family identifier.
    pub training_family_id: String,
    /// Stable admitted model id.
    pub model_id: String,
    /// Stable admitted model family.
    pub model_family: String,
    /// Stable backend label for the first trainer.
    pub execution_backend_label: String,
    /// Stable checkpoint family label owned by the lane.
    pub checkpoint_family: String,
    /// Stable base-model revision the adapter targets.
    pub base_model_revision: String,
    /// Stable tokenizer digest bound to the lane.
    pub tokenizer: TokenizerDigest,
    /// Stable tokenizer-contract digest derived from `tokenizer`.
    pub tokenizer_contract_digest: String,
    /// Stable adapter target identity for the lane.
    pub adapter_target: AdapterTargetIdentity,
    /// Stable adapter artifact kind.
    pub adapter_artifact_kind: AdapterArtifactKind,
    /// Stable adapter artifact format.
    pub adapter_artifact_format: AdapterArtifactFormat,
    /// Stable adapter target family.
    pub adapter_target_family: AdapterTargetFamily,
    /// Whether the lane is async-job-compatible.
    pub async_job_compatible: bool,
    /// Stable promoted-serving posture.
    pub promoted_serving_posture: String,
    /// Explicit non-goals.
    pub non_goals: Vec<String>,
    /// Explicit refusal set.
    pub refusal_set: Vec<GemmaE4bFinetuningRefusal>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl GemmaE4bFinetuningMvpContract {
    /// Returns the stable digest over the contract payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_gemma_e4b_finetuning_mvp_contract|", &clone)
    }

    /// Returns the stable base-model reference used by the adapter target.
    #[must_use]
    pub fn base_model_ref(&self) -> String {
        format!("{}@{}", self.model_id, self.base_model_revision)
    }

    /// Validates the contract against the bounded Gemma fixture and refusal set.
    pub fn validate(&self) -> Result<(), GemmaE4bFinetuningMvpError> {
        if self.schema_version != GEMMA_E4B_FINETUNING_MVP_SCHEMA_VERSION {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    GEMMA_E4B_FINETUNING_MVP_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.training_family_id != GEMMA_E4B_FINETUNING_MVP_TRAINING_FAMILY_ID {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("training-family id drifted"),
            });
        }
        if self.model_id != GEMMA_E4B_FINETUNING_MVP_MODEL_ID
            || self.model_family != GEMMA_E4B_FINETUNING_MVP_MODEL_FAMILY
        {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("model identity drifted"),
            });
        }
        if self.execution_backend_label != GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("execution backend label drifted"),
            });
        }
        if self.checkpoint_family != GEMMA_E4B_FINETUNING_MVP_CHECKPOINT_FAMILY {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("checkpoint family drifted"),
            });
        }
        if self.base_model_revision != GEMMA_E4B_FINETUNING_MVP_BASE_MODEL_REVISION {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("base-model revision drifted"),
            });
        }
        if self.tokenizer.tokenizer_digest != GEMMA_E4B_FINETUNING_MVP_TOKENIZER_DIGEST {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("tokenizer digest drifted"),
            });
        }
        if self.tokenizer.family != TokenizerFamily::SentencePiece {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("Gemma tokenizer family must stay sentencepiece"),
            });
        }
        if self.tokenizer_contract_digest != self.tokenizer.stable_digest() {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("tokenizer contract digest drifted"),
            });
        }
        if self.adapter_target.adapter_target_id != GEMMA_E4B_FINETUNING_MVP_ADAPTER_TARGET_ID
            || self.adapter_target.adapter_family != GEMMA_E4B_FINETUNING_MVP_ADAPTER_FAMILY
            || self.adapter_target.base_model_ref != self.base_model_ref()
            || self.adapter_target.adapter_format != GEMMA_E4B_FINETUNING_MVP_ADAPTER_FORMAT
        {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("adapter target identity drifted"),
            });
        }
        if self.adapter_artifact_kind != AdapterArtifactKind::Lora
            || self.adapter_artifact_format != AdapterArtifactFormat::Safetensors
            || self.adapter_target_family != AdapterTargetFamily::DecoderComposite
        {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("adapter artifact contract drifted"),
            });
        }
        if !self.async_job_compatible {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from(
                    "the first Gemma finetuning lane must stay async-job-compatible",
                ),
            });
        }
        if self.promoted_serving_posture != GEMMA_E4B_FINETUNING_MVP_PROMOTED_SERVING_POSTURE {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("promoted-serving posture drifted"),
            });
        }
        let expected_refusals = BTreeSet::from([
            GemmaE4bFinetuningRefusalKind::UnsupportedModelId,
            GemmaE4bFinetuningRefusalKind::UnsupportedExecutionBackend,
            GemmaE4bFinetuningRefusalKind::Dense31b,
            GemmaE4bFinetuningRefusalKind::Sparse26bA4b,
            GemmaE4bFinetuningRefusalKind::Multimodal,
            GemmaE4bFinetuningRefusalKind::Audio,
            GemmaE4bFinetuningRefusalKind::Metal,
            GemmaE4bFinetuningRefusalKind::FullModelFinetuning,
        ]);
        let actual_refusals = self
            .refusal_set
            .iter()
            .map(|refusal| refusal.refusal_kind)
            .collect::<BTreeSet<_>>();
        if actual_refusals != expected_refusals {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("refusal_set drifted from the bounded Gemma MVP boundary"),
            });
        }
        let tokenizer_fixture = golden_tokenizer_fixture("gemma4_e4b")
            .ok_or(GemmaE4bFinetuningMvpError::MissingTokenizerFixture)?;
        if usize::try_from(self.tokenizer.vocab_size).unwrap_or(usize::MAX)
            != tokenizer_fixture.vocabulary_len
        {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("tokenizer vocabulary length drifted from the Gemma fixture"),
            });
        }
        let prompt_fixture = golden_prompt_fixture("gemma4_e4b")
            .ok_or(GemmaE4bFinetuningMvpError::MissingPromptFixture)?;
        let template_variant = prompt_fixture
            .template_variants
            .first()
            .ok_or(GemmaE4bFinetuningMvpError::MissingPromptTemplateVariant)?;
        if self.tokenizer.template_digest.as_deref() != Some(template_variant.template_digest) {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from(
                    "tokenizer template digest drifted from the Gemma prompt fixture",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(GemmaE4bFinetuningMvpError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }

    /// Admits or refuses one requested finetuning scope against the bounded MVP.
    pub fn admit_request(
        &self,
        request: &GemmaE4bFinetuningMvpRequest,
    ) -> Result<(), GemmaE4bFinetuningMvpError> {
        if request.model_id == "gemma4:31b" {
            return Err(self.refusal_error(GemmaE4bFinetuningRefusalKind::Dense31b));
        }
        if request.model_id == "gemma4:26b" || request.model_id == "gemma4:26b-a4b" {
            return Err(self.refusal_error(GemmaE4bFinetuningRefusalKind::Sparse26bA4b));
        }
        if request.model_id != self.model_id {
            return Err(self.refusal_error(GemmaE4bFinetuningRefusalKind::UnsupportedModelId));
        }
        match request.modality {
            GemmaFinetuningInputModality::Text => {}
            GemmaFinetuningInputModality::Multimodal => {
                return Err(self.refusal_error(GemmaE4bFinetuningRefusalKind::Multimodal));
            }
            GemmaFinetuningInputModality::Audio => {
                return Err(self.refusal_error(GemmaE4bFinetuningRefusalKind::Audio));
            }
        }
        if request.execution_backend_label != self.execution_backend_label {
            if request.execution_backend_label.contains("metal") {
                return Err(self.refusal_error(GemmaE4bFinetuningRefusalKind::Metal));
            }
            return Err(
                self.refusal_error(GemmaE4bFinetuningRefusalKind::UnsupportedExecutionBackend)
            );
        }
        if request.update_mode != GemmaFinetuningUpdateMode::AdapterSft {
            return Err(self.refusal_error(GemmaE4bFinetuningRefusalKind::FullModelFinetuning));
        }
        Ok(())
    }

    /// Builds one adapter artifact identity aligned to the bounded Gemma MVP.
    pub fn adapter_artifact_identity(
        &self,
        adapter_id: impl Into<String>,
        adapter_revision: impl Into<String>,
        base_served_artifact_digest: impl Into<String>,
        artifact_digest: impl Into<String>,
        parameter_count: u64,
    ) -> Result<AdapterArtifactIdentity, GemmaE4bFinetuningMvpError> {
        let adapter_id = required_field("adapter_id", adapter_id.into())?;
        let adapter_revision = required_field("adapter_revision", adapter_revision.into())?;
        let base_served_artifact_digest = required_field(
            "base_served_artifact_digest",
            base_served_artifact_digest.into(),
        )?;
        let artifact_digest = required_field("artifact_digest", artifact_digest.into())?;
        if parameter_count == 0 {
            return Err(GemmaE4bFinetuningMvpError::InvalidParameterCount);
        }
        Ok(AdapterArtifactIdentity::new(
            adapter_id,
            adapter_revision,
            self.adapter_artifact_kind,
            self.adapter_artifact_format,
            self.model_id.clone(),
            self.base_model_revision.clone(),
            base_served_artifact_digest,
            artifact_digest,
            QuantizationMode::None,
            self.adapter_target_family,
            parameter_count,
        )
        .with_provenance_digest(self.contract_digest.clone())
        .with_governance_digest(stable_governance_digest(
            self.training_family_id.as_str(),
            self.checkpoint_family.as_str(),
            self.tokenizer_contract_digest.as_str(),
        )))
    }

    fn refusal(&self, kind: GemmaE4bFinetuningRefusalKind) -> &GemmaE4bFinetuningRefusal {
        self.refusal_set
            .iter()
            .find(|refusal| refusal.refusal_kind == kind)
            .expect("canonical contract should keep every refusal kind")
    }

    fn refusal_error(&self, kind: GemmaE4bFinetuningRefusalKind) -> GemmaE4bFinetuningMvpError {
        let refusal = self.refusal(kind);
        GemmaE4bFinetuningMvpError::Refusal {
            kind: refusal.refusal_kind,
            detail: refusal.detail.clone(),
        }
    }
}

/// Errors surfaced while building or using the first Gemma finetuning MVP contract.
#[derive(Debug, Error)]
pub enum GemmaE4bFinetuningMvpError {
    #[error("missing golden tokenizer fixture `gemma4_e4b`")]
    MissingTokenizerFixture,
    #[error("missing golden prompt fixture `gemma4_e4b`")]
    MissingPromptFixture,
    #[error("golden prompt fixture `gemma4_e4b` is missing its default template variant")]
    MissingPromptTemplateVariant,
    #[error("gemma4 e4b finetuning mvp contract is invalid: {detail}")]
    InvalidContract { detail: String },
    #[error("gemma4 e4b finetuning mvp refuses `{kind:?}`: {detail}")]
    Refusal {
        kind: GemmaE4bFinetuningRefusalKind,
        detail: String,
    },
    #[error("gemma4 e4b finetuning artifact identity is missing `{field}`")]
    MissingArtifactField { field: &'static str },
    #[error("gemma4 e4b finetuning artifact identity requires `parameter_count > 0`")]
    InvalidParameterCount,
    #[error(transparent)]
    AdapterTarget(#[from] AdapterWindowContractError),
}

/// Returns the canonical first Gemma e4b finetuning MVP contract.
pub fn canonical_gemma_e4b_finetuning_mvp_contract()
-> Result<GemmaE4bFinetuningMvpContract, GemmaE4bFinetuningMvpError> {
    let tokenizer_fixture = golden_tokenizer_fixture("gemma4_e4b")
        .ok_or(GemmaE4bFinetuningMvpError::MissingTokenizerFixture)?;
    let prompt_fixture = golden_prompt_fixture("gemma4_e4b")
        .ok_or(GemmaE4bFinetuningMvpError::MissingPromptFixture)?;
    let template_variant = prompt_fixture
        .template_variants
        .first()
        .ok_or(GemmaE4bFinetuningMvpError::MissingPromptTemplateVariant)?;
    let tokenizer = TokenizerDigest::new(
        TokenizerFamily::SentencePiece,
        GEMMA_E4B_FINETUNING_MVP_TOKENIZER_DIGEST,
        u32::try_from(tokenizer_fixture.vocabulary_len).unwrap_or(u32::MAX),
    )
    .with_template_digest(template_variant.template_digest);
    let adapter_target = AdapterTargetIdentity::new(
        GEMMA_E4B_FINETUNING_MVP_ADAPTER_TARGET_ID,
        GEMMA_E4B_FINETUNING_MVP_ADAPTER_FAMILY,
        format!(
            "{}@{}",
            GEMMA_E4B_FINETUNING_MVP_MODEL_ID, GEMMA_E4B_FINETUNING_MVP_BASE_MODEL_REVISION
        ),
        GEMMA_E4B_FINETUNING_MVP_ADAPTER_FORMAT,
    )?;
    let mut contract = GemmaE4bFinetuningMvpContract {
        schema_version: String::from(GEMMA_E4B_FINETUNING_MVP_SCHEMA_VERSION),
        training_family_id: String::from(GEMMA_E4B_FINETUNING_MVP_TRAINING_FAMILY_ID),
        model_id: String::from(GEMMA_E4B_FINETUNING_MVP_MODEL_ID),
        model_family: String::from(GEMMA_E4B_FINETUNING_MVP_MODEL_FAMILY),
        execution_backend_label: String::from(GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL),
        checkpoint_family: String::from(GEMMA_E4B_FINETUNING_MVP_CHECKPOINT_FAMILY),
        base_model_revision: String::from(GEMMA_E4B_FINETUNING_MVP_BASE_MODEL_REVISION),
        tokenizer_contract_digest: tokenizer.stable_digest(),
        tokenizer,
        adapter_target,
        adapter_artifact_kind: AdapterArtifactKind::Lora,
        adapter_artifact_format: AdapterArtifactFormat::Safetensors,
        adapter_target_family: AdapterTargetFamily::DecoderComposite,
        async_job_compatible: true,
        promoted_serving_posture: String::from(GEMMA_E4B_FINETUNING_MVP_PROMOTED_SERVING_POSTURE),
        non_goals: vec![
            String::from("no multimodal Gemma finetuning"),
            String::from("no audio Gemma finetuning"),
            String::from("no dense Gemma 4 31B finetuning"),
            String::from("no sparse Gemma 4 26B A4B finetuning"),
            String::from("no Metal parity requirement"),
            String::from("no full-model finetuning"),
        ],
        refusal_set: vec![
            refusal(
                "gemma_e4b_finetuning.unsupported_model_id",
                GemmaE4bFinetuningRefusalKind::UnsupportedModelId,
                "The first Gemma finetuning MVP admits only `gemma4:e4b` under one bounded CUDA adapter-SFT lane.",
            ),
            refusal(
                "gemma_e4b_finetuning.unsupported_execution_backend",
                GemmaE4bFinetuningRefusalKind::UnsupportedExecutionBackend,
                "The first Gemma finetuning MVP refuses non-CUDA execution backends. The admitted trainer backend label is `open_adapter_backend.cuda.gemma4_e4b_lm_head`.",
            ),
            refusal(
                "gemma_e4b_finetuning.dense_31b",
                GemmaE4bFinetuningRefusalKind::Dense31b,
                "The first Gemma finetuning MVP refuses `gemma4:31b` because the bounded training claim stops at dense `gemma4:e4b`.",
            ),
            refusal(
                "gemma_e4b_finetuning.sparse_26b_a4b",
                GemmaE4bFinetuningRefusalKind::Sparse26bA4b,
                "The first Gemma finetuning MVP refuses sparse `Gemma 4 26B A4B` work because MoE finetuning is outside the bounded dense e4b lane.",
            ),
            refusal(
                "gemma_e4b_finetuning.multimodal",
                GemmaE4bFinetuningRefusalKind::Multimodal,
                "The first Gemma finetuning MVP refuses multimodal finetuning. Only text-only SFT is in scope.",
            ),
            refusal(
                "gemma_e4b_finetuning.audio",
                GemmaE4bFinetuningRefusalKind::Audio,
                "The first Gemma finetuning MVP refuses audio finetuning. Audio remains outside the first text-only lane.",
            ),
            refusal(
                "gemma_e4b_finetuning.metal",
                GemmaE4bFinetuningRefusalKind::Metal,
                "The first Gemma finetuning MVP refuses Metal execution. The first admitted backend is CUDA only.",
            ),
            refusal(
                "gemma_e4b_finetuning.full_model",
                GemmaE4bFinetuningRefusalKind::FullModelFinetuning,
                "The first Gemma finetuning MVP refuses full-model finetuning. Only adapter SFT is admitted.",
            ),
        ],
        claim_boundary: String::from(
            "This contract closes one bounded Gemma finetuning training-family identity in Psionic: `gemma4:e4b`, dense, text only, CUDA only, adapter SFT only, async-job-compatible, with stable tokenizer, base-revision, adapter-target, artifact, checkpoint-family, and explicit refusal truth. It does not claim a landed Gemma trainer, checkpoint refresh into serving, eval-gated promotion, multimodal or audio finetuning, Metal parity, dense 31B finetuning, sparse 26B A4B finetuning, or full-model finetuning.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

fn required_field(
    field: &'static str,
    value: String,
) -> Result<String, GemmaE4bFinetuningMvpError> {
    if value.trim().is_empty() {
        return Err(GemmaE4bFinetuningMvpError::MissingArtifactField { field });
    }
    Ok(value)
}

fn refusal(
    refusal_id: &str,
    refusal_kind: GemmaE4bFinetuningRefusalKind,
    detail: &str,
) -> GemmaE4bFinetuningRefusal {
    GemmaE4bFinetuningRefusal {
        refusal_id: refusal_id.to_string(),
        refusal_kind,
        detail: detail.to_string(),
    }
}

fn stable_governance_digest(
    training_family_id: &str,
    checkpoint_family: &str,
    tokenizer_contract_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_gemma_e4b_finetuning_governance|");
    hasher.update(training_family_id.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_family.as_bytes());
    hasher.update(b"|");
    hasher.update(tokenizer_contract_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_digest(prefix: &[u8], payload: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(payload).expect("contract payload should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemma_e4b_finetuning_mvp_contract_is_machine_checkable()
    -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_gemma_e4b_finetuning_mvp_contract()?;
        assert_eq!(
            contract.training_family_id,
            GEMMA_E4B_FINETUNING_MVP_TRAINING_FAMILY_ID
        );
        assert_eq!(contract.model_id, GEMMA_E4B_FINETUNING_MVP_MODEL_ID);
        assert_eq!(
            contract.execution_backend_label,
            GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL
        );
        assert_eq!(
            contract.adapter_target.adapter_family,
            GEMMA_E4B_FINETUNING_MVP_ADAPTER_FAMILY
        );
        assert_eq!(
            contract.tokenizer.template_digest.as_deref(),
            Some("b41f8277605dd25c150524d580ccfa8f351608a385c01a4211fa0eadec4382c3")
        );
        assert_eq!(
            contract.tokenizer_contract_digest,
            contract.tokenizer.stable_digest()
        );
        contract.validate()?;
        Ok(())
    }

    #[test]
    fn gemma_e4b_finetuning_mvp_refuses_non_mvp_regions() -> Result<(), Box<dyn std::error::Error>>
    {
        let contract = canonical_gemma_e4b_finetuning_mvp_contract()?;
        let dense_31b = contract
            .admit_request(&GemmaE4bFinetuningMvpRequest::new(
                "gemma4:31b",
                GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL,
                GemmaFinetuningInputModality::Text,
                GemmaFinetuningUpdateMode::AdapterSft,
            ))
            .expect_err("31B must refuse");
        assert!(dense_31b.to_string().contains("Dense31b"));

        let sparse_26b = contract
            .admit_request(&GemmaE4bFinetuningMvpRequest::new(
                "gemma4:26b",
                GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL,
                GemmaFinetuningInputModality::Text,
                GemmaFinetuningUpdateMode::AdapterSft,
            ))
            .expect_err("26B A4B must refuse");
        assert!(sparse_26b.to_string().contains("Sparse26bA4b"));

        let multimodal = contract
            .admit_request(&GemmaE4bFinetuningMvpRequest::new(
                GEMMA_E4B_FINETUNING_MVP_MODEL_ID,
                GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL,
                GemmaFinetuningInputModality::Multimodal,
                GemmaFinetuningUpdateMode::AdapterSft,
            ))
            .expect_err("multimodal must refuse");
        assert!(multimodal.to_string().contains("Multimodal"));

        let audio = contract
            .admit_request(&GemmaE4bFinetuningMvpRequest::new(
                GEMMA_E4B_FINETUNING_MVP_MODEL_ID,
                GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL,
                GemmaFinetuningInputModality::Audio,
                GemmaFinetuningUpdateMode::AdapterSft,
            ))
            .expect_err("audio must refuse");
        assert!(audio.to_string().contains("Audio"));

        let metal = contract
            .admit_request(&GemmaE4bFinetuningMvpRequest::new(
                GEMMA_E4B_FINETUNING_MVP_MODEL_ID,
                "open_adapter_backend.metal.gemma4_e4b_lm_head",
                GemmaFinetuningInputModality::Text,
                GemmaFinetuningUpdateMode::AdapterSft,
            ))
            .expect_err("metal must refuse");
        assert!(metal.to_string().contains("Metal"));

        let full_model = contract
            .admit_request(&GemmaE4bFinetuningMvpRequest::new(
                GEMMA_E4B_FINETUNING_MVP_MODEL_ID,
                GEMMA_E4B_FINETUNING_MVP_BACKEND_LABEL,
                GemmaFinetuningInputModality::Text,
                GemmaFinetuningUpdateMode::FullModelSft,
            ))
            .expect_err("full-model must refuse");
        assert!(full_model.to_string().contains("FullModelFinetuning"));
        Ok(())
    }

    #[test]
    fn gemma_e4b_finetuning_mvp_builds_stable_adapter_artifact_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_gemma_e4b_finetuning_mvp_contract()?;
        let identity = contract.adapter_artifact_identity(
            "adapter-gemma4-e4b-helpdesk",
            "r1",
            "sha256:gemma4-e4b-base",
            "sha256:adapter-bytes",
            8_192,
        )?;
        assert_eq!(identity.base_model_id, GEMMA_E4B_FINETUNING_MVP_MODEL_ID);
        assert_eq!(
            identity.base_model_revision,
            GEMMA_E4B_FINETUNING_MVP_BASE_MODEL_REVISION
        );
        assert_eq!(identity.kind, AdapterArtifactKind::Lora);
        assert_eq!(identity.format, AdapterArtifactFormat::Safetensors);
        assert_eq!(
            identity.target_family,
            AdapterTargetFamily::DecoderComposite
        );
        assert_eq!(
            identity.provenance_digest.as_deref(),
            Some(contract.contract_digest.as_str())
        );
        assert!(
            identity
                .governance_digest
                .as_deref()
                .expect("governance digest")
                .len()
                > 16
        );
        Ok(())
    }
}
