use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    io::{Read, Write},
    path::Path,
};

use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use half::f16;
use psionic_data::{
    ParameterGolfDataError, ParameterGolfSentencePieceByteLuts,
    ParameterGolfSentencePieceTokenEntry, ParameterGolfSentencePieceTokenKind,
};
use psionic_eval::{
    ParameterGolfValidationEvalError, ParameterGolfValidationEvalReport,
    evaluate_parameter_golf_validation,
};
use psionic_models::{
    PARAMETER_GOLF_BASELINE_VOCAB_SIZE, PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_SCHEMA_VERSION,
    PARAMETER_GOLF_PROMOTED_GENERATION_CONFIG_SCHEMA_VERSION,
    PARAMETER_GOLF_PROMOTED_TOKENIZER_ASSET_SCHEMA_VERSION, ParameterGolfBankedWeights,
    ParameterGolfExecutionError, ParameterGolfModelDescriptor, ParameterGolfModelError,
    ParameterGolfParameterVector, ParameterGolfPromotedBundleArtifactRef,
    ParameterGolfPromotedBundleArtifacts, ParameterGolfPromotedBundleLineage,
    ParameterGolfPromotedBundleManifest, ParameterGolfPromotedGenerationConfig,
    ParameterGolfPromotedProfileContract, ParameterGolfPromotedProfileKind,
    ParameterGolfPromotedTokenizerAsset, ParameterGolfPromotedTokenizerAssetFormat,
    ParameterGolfPromotedTokenizerFamily, ParameterGolfPromotedTokenizerToken,
    ParameterGolfPromotedTokenizerTokenKind, ParameterGolfReferenceModel,
};
use psionic_runtime::TrainingCheckpointReference;
use safetensors::{Dtype as SafeTensorsDType, SafeTensors, tensor::TensorView};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zstd::stream::{decode_all as zstd_decode_all, encode_all as zstd_encode_all};

use crate::{
    AsyncCheckpointWritebackError, AsyncCheckpointWritebackFile, AsyncCheckpointWritebackOptions,
    AsyncCheckpointWritebackPayload, AsyncCheckpointWritebackReceipt,
    AsyncCheckpointWritebackTicket, AsyncCheckpointWritebackWorker, LocalTrainMetricEvent,
    LocalTrainMetricFanout, LocalTrainMetricPhase, LocalTrainMetricSinkError,
    LocalTrainMetricValue, PARAMETER_GOLF_CONTROL_TENSOR_NAME_PATTERNS, ParameterGolfMuonConfig,
    ParameterGolfMuonState, ParameterGolfOptimizerExecution, ParameterGolfTrainError,
    ParameterGolfTrainingHyperparameters, PortableModelProfileContract, TrainingOptimizerConfig,
    TrainingOptimizerError, TrainingOptimizerState, apply_parameter_golf_muon_step,
    parameter_golf_optimizer_plan,
};

const PARAMETER_GOLF_CHECKPOINT_MANIFEST_KEY: &str = "psionic.parameter_golf.checkpoint_manifest";
const PARAMETER_GOLF_WEIGHT_SURFACE_KEY: &str = "psionic.parameter_golf.weight_surface";
const PARAMETER_GOLF_SPLIT_WEIGHT_SURFACE: &str = "split_f32_v1";
const PARAMETER_GOLF_BANKED_WEIGHT_SURFACE: &str = "banked_f32_v1";
const PARAMETER_GOLF_INT8_ZLIB_FORMAT: &str = "int8_clean_per_row_v1";
const PARAMETER_GOLF_INT6_GPTQ_LITE_FORMAT: &str = "int6_gptq_lite_per_row_v1";
const PARAMETER_GOLF_ZLIB_COMPRESSION_FORMAT: &str = "zlib_v1";
const PARAMETER_GOLF_ZSTD_COMPRESSION_FORMAT: &str = "zstd_v1";
const PARAMETER_GOLF_INT8_KEEP_FLOAT_MAX_NUMEL: usize = 65_536;
const PARAMETER_GOLF_INT8_CLIP_Q: f32 = 0.999_998_4;
const PARAMETER_GOLF_INT6_GPTQ_LITE_CLIP_CANDIDATES: &[f32] =
    &[0.999, 0.9995, 0.9999, 0.99999, 1.0];
const PARAMETER_GOLF_COMPETITIVE_ZSTD_LEVEL: i32 = 22;

/// Returns the frozen promoted-family profile contract for one PGOLF-shaped
/// small-decoder profile.
#[must_use]
pub fn parameter_golf_promoted_profile_contract(
    kind: ParameterGolfPromotedProfileKind,
) -> ParameterGolfPromotedProfileContract {
    match kind {
        ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder => {
            ParameterGolfPromotedProfileContract::general_psion_small_decoder_v0()
        }
        ParameterGolfPromotedProfileKind::StrictPgolfChallenge => {
            ParameterGolfPromotedProfileContract::strict_pgolf_challenge_v0()
        }
    }
}

/// Returns the portable-bundle profile contract for the promoted general Psion
/// small-decoder profile.
#[must_use]
pub fn parameter_golf_general_psion_small_decoder_profile_contract() -> PortableModelProfileContract
{
    portable_parameter_golf_profile_contract(parameter_golf_promoted_profile_contract(
        ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder,
    ))
}

/// Returns the portable-bundle profile contract for the strict PGOLF challenge
/// overlay.
#[must_use]
pub fn parameter_golf_strict_challenge_profile_contract() -> PortableModelProfileContract {
    portable_parameter_golf_profile_contract(parameter_golf_promoted_profile_contract(
        ParameterGolfPromotedProfileKind::StrictPgolfChallenge,
    ))
}

const PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_FILE: &str =
    "parameter_golf_promoted_bundle_manifest.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_DESCRIPTOR_FILE: &str = "descriptor.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_MODEL_FILE: &str = "model.safetensors";
const PARAMETER_GOLF_PROMOTED_BUNDLE_TOKENIZER_FILE: &str = "tokenizer.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_GENERATION_CONFIG_FILE: &str = "generation_config.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_PROFILE_CONTRACT_FILE: &str = "profile_contract.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_TRAINING_CONFIG_FILE: &str = "training_config.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_SUMMARY_FILE: &str = "summary.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_CHECKPOINT_MANIFEST_FILE: &str = "checkpoint_manifest.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_CHECKPOINT_SURFACE_REPORT_FILE: &str =
    "checkpoint_surface_report.json";
const PARAMETER_GOLF_PROMOTED_BUNDLE_RESUME_PROOF_FILE: &str = "resume_proof.json";

fn portable_parameter_golf_profile_contract(
    contract: ParameterGolfPromotedProfileContract,
) -> PortableModelProfileContract {
    let mut shared_capabilities = BTreeMap::new();
    shared_capabilities.insert(
        String::from("tied_embeddings"),
        contract.shared_capabilities.tied_embeddings,
    );
    shared_capabilities.insert(
        String::from("grouped_query_attention"),
        contract.shared_capabilities.grouped_query_attention,
    );
    shared_capabilities.insert(String::from("rope"), contract.shared_capabilities.rope);
    shared_capabilities.insert(
        String::from("partial_rope"),
        contract.shared_capabilities.partial_rope,
    );
    shared_capabilities.insert(
        String::from("skip_weights"),
        contract.shared_capabilities.skip_weights,
    );
    shared_capabilities.insert(
        String::from("optional_bigram_hash"),
        contract.shared_capabilities.optional_bigram_hash,
    );
    shared_capabilities.insert(
        String::from("optional_value_embeddings"),
        contract.shared_capabilities.optional_value_embeddings,
    );
    shared_capabilities.insert(
        String::from("optional_xsa"),
        contract.shared_capabilities.optional_xsa,
    );
    shared_capabilities.insert(
        String::from("relu_squared"),
        contract.shared_capabilities.relu_squared,
    );
    shared_capabilities.insert(
        String::from("leaky_relu_squared_point_five"),
        contract.shared_capabilities.leaky_relu_squared_point_five,
    );
    shared_capabilities.insert(
        String::from("parameter_banking"),
        contract.shared_capabilities.parameter_banking,
    );
    shared_capabilities.insert(
        String::from("quantized_export"),
        contract.shared_capabilities.quantized_export,
    );

    let mut overlay_requirements = BTreeMap::new();
    overlay_requirements.insert(
        String::from("exact_sp1024_tokenizer"),
        contract.challenge_overlay.exact_sp1024_tokenizer,
    );
    overlay_requirements.insert(
        String::from("exact_fineweb_challenge_data"),
        contract.challenge_overlay.exact_fineweb_challenge_data,
    );
    overlay_requirements.insert(
        String::from("exact_16_mib_compressed_artifact_cap"),
        contract
            .challenge_overlay
            .exact_16_mib_compressed_artifact_cap,
    );
    overlay_requirements.insert(
        String::from("score_first_ttt"),
        contract.challenge_overlay.score_first_ttt,
    );
    overlay_requirements.insert(
        String::from("contest_bpb_accounting"),
        contract.challenge_overlay.contest_bpb_accounting,
    );

    PortableModelProfileContract {
        profile_id: contract.profile_id,
        family_id: contract.family_id,
        baseline_model_id: contract.baseline_model_id,
        baseline_revision: contract.baseline_revision,
        profile_kind: match contract.kind {
            ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder => {
                String::from("general_psion_small_decoder")
            }
            ParameterGolfPromotedProfileKind::StrictPgolfChallenge => {
                String::from("strict_pgolf_challenge")
            }
        },
        shared_capabilities,
        overlay_requirements,
    }
}

/// Explicit final-artifact quantization posture for the PGOLF score lane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfFinalArtifactQuantizationFormat {
    #[default]
    Int8CleanPerRow,
    Int6GptqLitePerRow,
}

impl ParameterGolfFinalArtifactQuantizationFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Int8CleanPerRow => PARAMETER_GOLF_INT8_ZLIB_FORMAT,
            Self::Int6GptqLitePerRow => PARAMETER_GOLF_INT6_GPTQ_LITE_FORMAT,
        }
    }
}

/// Explicit final-artifact compression posture for the PGOLF score lane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfFinalArtifactCompressionFormat {
    #[default]
    Zlib,
    Zstd,
}

impl ParameterGolfFinalArtifactCompressionFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zlib => PARAMETER_GOLF_ZLIB_COMPRESSION_FORMAT,
            Self::Zstd => PARAMETER_GOLF_ZSTD_COMPRESSION_FORMAT,
        }
    }
}

/// Typed final-artifact export contract for the PGOLF score lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfFinalArtifactConfig {
    #[serde(default)]
    pub quantization: ParameterGolfFinalArtifactQuantizationFormat,
    #[serde(default)]
    pub compression: ParameterGolfFinalArtifactCompressionFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression_level: Option<i32>,
}

impl Default for ParameterGolfFinalArtifactConfig {
    fn default() -> Self {
        Self {
            quantization: ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow,
            compression: ParameterGolfFinalArtifactCompressionFormat::Zlib,
            compression_level: None,
        }
    }
}

impl ParameterGolfFinalArtifactConfig {
    #[must_use]
    pub const fn competitive_defaults() -> Self {
        Self {
            quantization: ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow,
            compression: ParameterGolfFinalArtifactCompressionFormat::Zstd,
            compression_level: Some(PARAMETER_GOLF_COMPETITIVE_ZSTD_LEVEL),
        }
    }

    #[must_use]
    pub const fn metric_source(&self) -> &'static str {
        match (self.quantization, self.compression) {
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zlib,
            ) => "int8_zlib_roundtrip",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zstd,
            ) => "int8_zstd_roundtrip",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zlib,
            ) => "int6_zlib_roundtrip",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zstd,
            ) => "int6_zstd_roundtrip",
        }
    }

    #[must_use]
    pub const fn artifact_kind(&self) -> &'static str {
        match (self.quantization, self.compression) {
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zlib,
            ) => "parameter_golf_model_int8_zlib",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zstd,
            ) => "parameter_golf_model_int8_zstd",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zlib,
            ) => "parameter_golf_model_int6_zlib",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zstd,
            ) => "parameter_golf_model_int6_zstd",
        }
    }

    #[must_use]
    pub const fn artifact_extension(&self) -> &'static str {
        match (self.quantization, self.compression) {
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zlib,
            ) => "int8.ptz",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zstd,
            ) => "int8.zst",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zlib,
            ) => "int6.ptz",
            (
                ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow,
                ParameterGolfFinalArtifactCompressionFormat::Zstd,
            ) => "int6.zst",
        }
    }

    #[must_use]
    pub const fn resolved_compression_level(&self) -> Option<i32> {
        match self.compression {
            ParameterGolfFinalArtifactCompressionFormat::Zlib => None,
            ParameterGolfFinalArtifactCompressionFormat::Zstd => self.compression_level,
        }
    }
}

/// Stable single-device batch geometry for the Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfBatchGeometry {
    /// Distributed world size encoded by the upstream challenge script.
    pub world_size: usize,
    /// Global train-token budget per step.
    pub train_batch_tokens: usize,
    /// Global validation-token budget per eval batch.
    pub validation_batch_tokens: usize,
    /// Sequence length for both training and validation.
    pub train_sequence_length: usize,
    /// Gradient-accumulation steps per optimizer step.
    pub grad_accum_steps: usize,
}

impl ParameterGolfBatchGeometry {
    /// Returns the current public single-device challenge defaults from `train_gpt.py`.
    #[must_use]
    pub const fn challenge_single_device_defaults() -> Self {
        Self {
            world_size: 1,
            train_batch_tokens: 524_288,
            validation_batch_tokens: 524_288,
            train_sequence_length: 1024,
            grad_accum_steps: 8,
        }
    }

    /// Returns the bounded local-reference defaults used by repo-owned tests.
    #[must_use]
    pub const fn local_reference_defaults() -> Self {
        Self {
            world_size: 1,
            train_batch_tokens: 32,
            validation_batch_tokens: 32,
            train_sequence_length: 4,
            grad_accum_steps: 8,
        }
    }

    /// Returns the current public `8xH100` challenge defaults from `train_gpt.py`.
    #[must_use]
    pub const fn challenge_distributed_8xh100_defaults() -> Self {
        Self {
            world_size: 8,
            train_batch_tokens: 524_288,
            validation_batch_tokens: 524_288,
            train_sequence_length: 1024,
            grad_accum_steps: 1,
        }
    }

    /// Returns the per-rank, per-microbatch train-token count.
    #[must_use]
    pub const fn local_train_batch_tokens(&self) -> usize {
        self.train_batch_tokens / (self.world_size * self.grad_accum_steps)
    }

    /// Returns the per-rank, per-microbatch validation-token count.
    #[must_use]
    pub const fn local_validation_batch_tokens(&self) -> usize {
        self.validation_batch_tokens / (self.world_size * self.grad_accum_steps)
    }

    /// Returns the number of local train sequences per microbatch.
    #[must_use]
    pub const fn local_train_batch_sequences(&self) -> usize {
        self.local_train_batch_tokens() / self.train_sequence_length
    }

    /// Returns the number of local validation sequences per batch.
    #[must_use]
    pub const fn local_validation_batch_sequences(&self) -> usize {
        self.local_validation_batch_tokens() / self.train_sequence_length
    }

    pub(crate) fn validate(&self) -> Result<(), ParameterGolfReferenceTrainingError> {
        if self.world_size != 1 {
            return Err(ParameterGolfReferenceTrainingError::InvalidBatchGeometry {
                message: format!(
                    "parameter golf local reference trainer only supports world_size=1, found {}",
                    self.world_size
                ),
            });
        }
        if self.train_sequence_length == 0 {
            return Err(ParameterGolfReferenceTrainingError::InvalidBatchGeometry {
                message: String::from("train_sequence_length must be positive"),
            });
        }
        if self.grad_accum_steps == 0 {
            return Err(ParameterGolfReferenceTrainingError::InvalidBatchGeometry {
                message: String::from("grad_accum_steps must be positive"),
            });
        }
        let denom = self.world_size.saturating_mul(self.grad_accum_steps);
        if self.train_batch_tokens == 0
            || self.train_batch_tokens % denom != 0
            || self.local_train_batch_tokens() < self.train_sequence_length
            || self.local_train_batch_tokens() % self.train_sequence_length != 0
        {
            return Err(ParameterGolfReferenceTrainingError::InvalidBatchGeometry {
                message: format!(
                    "train_batch_tokens={} must divide cleanly across world_size={} grad_accum_steps={} and admit at least one full sequence of length {}",
                    self.train_batch_tokens,
                    self.world_size,
                    self.grad_accum_steps,
                    self.train_sequence_length
                ),
            });
        }
        if self.validation_batch_tokens == 0
            || self.validation_batch_tokens % denom != 0
            || self.local_validation_batch_tokens() < self.train_sequence_length
            || self.local_validation_batch_tokens() % self.train_sequence_length != 0
        {
            return Err(ParameterGolfReferenceTrainingError::InvalidBatchGeometry {
                message: format!(
                    "validation_batch_tokens={} must divide cleanly across world_size={} grad_accum_steps={} and admit at least one full sequence of length {}",
                    self.validation_batch_tokens,
                    self.world_size,
                    self.grad_accum_steps,
                    self.train_sequence_length
                ),
            });
        }
        Ok(())
    }
}

/// Repo-owned bounded fixture for the local Parameter Golf reference trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfLocalReferenceFixture {
    /// Human-readable fixture description.
    pub description: String,
    /// Tokenizer vocabulary size.
    pub tokenizer_vocab_size: usize,
    /// Tokenizer entries used by the eval byte-accounting LUTs.
    pub sentencepiece_entries: Vec<ParameterGolfSentencePieceTokenEntry>,
    /// Flat training-token stream.
    pub training_tokens: Vec<u16>,
    /// Flat validation-token stream.
    pub validation_tokens: Vec<u16>,
}

impl ParameterGolfLocalReferenceFixture {
    /// Returns the canonical repo-owned local-reference fixture.
    pub fn reference() -> Result<Self, ParameterGolfReferenceTrainingError> {
        let fixture: Self = serde_json::from_str(include_str!(
            "../../../fixtures/parameter_golf/train/parameter_golf_local_reference_fixture.json"
        ))
        .map_err(|error| ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf local reference fixture load",
            message: error.to_string(),
        })?;
        fixture.validate()?;
        Ok(fixture)
    }

    /// Builds the SentencePiece byte-accounting LUTs for the fixture tokenizer.
    pub fn byte_luts(
        &self,
    ) -> Result<ParameterGolfSentencePieceByteLuts, ParameterGolfReferenceTrainingError> {
        Ok(ParameterGolfSentencePieceByteLuts::build(
            self.tokenizer_vocab_size,
            self.sentencepiece_entries.as_slice(),
        )?)
    }

    /// Returns a stable digest over the training token stream.
    #[must_use]
    pub fn training_digest(&self) -> String {
        stable_digest(
            b"psionic_parameter_golf_training_tokens|",
            &self.training_tokens,
        )
    }

    /// Returns a stable digest over the validation token stream.
    #[must_use]
    pub fn validation_digest(&self) -> String {
        stable_digest(
            b"psionic_parameter_golf_validation_tokens|",
            &self.validation_tokens,
        )
    }

    fn validate(&self) -> Result<(), ParameterGolfReferenceTrainingError> {
        if self.tokenizer_vocab_size == 0 {
            return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
                message: String::from("tokenizer_vocab_size must be positive"),
            });
        }
        if self.training_tokens.len() < 2 {
            return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
                message: String::from("training_tokens must contain at least two tokens"),
            });
        }
        if self.validation_tokens.len() < 2 {
            return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
                message: String::from("validation_tokens must contain at least two tokens"),
            });
        }
        for &token_id in self
            .training_tokens
            .iter()
            .chain(self.validation_tokens.iter())
        {
            if token_id as usize >= self.tokenizer_vocab_size {
                return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
                    message: format!(
                        "token id {} exceeds tokenizer_vocab_size {}",
                        token_id, self.tokenizer_vocab_size
                    ),
                });
            }
        }
        Ok(())
    }
}

/// One selected trainable coordinate for the bounded local-reference trainer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParameterGolfTrainableCoordinate {
    /// Stable tensor identifier.
    pub parameter_id: String,
    /// Flat row-major index inside the tensor.
    pub flat_index: usize,
}

/// Explicit tokenizer identity for one promoted PGOLF training profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedTokenizerIdentity {
    /// Repo-owned bounded local-reference SentencePiece surface.
    RepoLocalReferenceSentencePiece,
    /// Exact public challenge SP1024 SentencePiece surface.
    ChallengeSp1024SentencePiece,
}

/// Explicit dataset identity for one promoted PGOLF training profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedDatasetIdentity {
    /// Repo-owned bounded local-reference token stream.
    RepoLocalReferenceFixture,
    /// Exact public FineWeb challenge lane.
    ChallengeFinewebSp1024,
}

/// Explicit evaluation identity for one promoted PGOLF training profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedEvaluationIdentity {
    /// Bounded local-reference validation on the repo-owned fixture.
    LocalReferenceValidation,
    /// Challenge-style bits-per-byte evaluation on the public challenge lane.
    ChallengeBitsPerByte,
}

/// Explicit evaluation policy for one promoted PGOLF training profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedEvaluationPolicy {
    /// Stable evaluation identity.
    pub evaluation_identity: ParameterGolfPromotedEvaluationIdentity,
    /// Whether legal score-first TTT is required.
    pub legal_score_first_ttt_required: bool,
    /// Whether contest bits-per-byte accounting is required.
    pub contest_bits_per_byte_accounting_required: bool,
}

/// Explicit artifact policy for one promoted PGOLF training profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedArtifactPolicy {
    /// Whether the exact public compressed artifact cap is required.
    pub exact_compressed_artifact_cap_required: bool,
    /// Required compressed artifact cap in bytes when challenge posture is
    /// requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressed_artifact_cap_bytes: Option<u64>,
}

/// Explicit promoted training profile for one PGOLF-shaped small-decoder lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedTrainingProfile {
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Stable promoted profile kind.
    pub kind: ParameterGolfPromotedProfileKind,
    /// Stable tokenizer identity.
    pub tokenizer_identity: ParameterGolfPromotedTokenizerIdentity,
    /// Stable dataset identity.
    pub dataset_identity: ParameterGolfPromotedDatasetIdentity,
    /// Stable evaluation policy.
    pub evaluation_policy: ParameterGolfPromotedEvaluationPolicy,
    /// Stable artifact policy.
    pub artifact_policy: ParameterGolfPromotedArtifactPolicy,
}

impl Default for ParameterGolfPromotedTrainingProfile {
    fn default() -> Self {
        Self::general_psion_small_decoder()
    }
}

impl ParameterGolfPromotedTrainingProfile {
    /// Returns the general Psion small-decoder training profile.
    #[must_use]
    pub fn general_psion_small_decoder() -> Self {
        Self {
            profile_id: String::from(
                ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder.profile_id(),
            ),
            kind: ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder,
            tokenizer_identity:
                ParameterGolfPromotedTokenizerIdentity::RepoLocalReferenceSentencePiece,
            dataset_identity: ParameterGolfPromotedDatasetIdentity::RepoLocalReferenceFixture,
            evaluation_policy: ParameterGolfPromotedEvaluationPolicy {
                evaluation_identity:
                    ParameterGolfPromotedEvaluationIdentity::LocalReferenceValidation,
                legal_score_first_ttt_required: false,
                contest_bits_per_byte_accounting_required: false,
            },
            artifact_policy: ParameterGolfPromotedArtifactPolicy {
                exact_compressed_artifact_cap_required: false,
                compressed_artifact_cap_bytes: None,
            },
        }
    }

    /// Returns the strict PGOLF challenge overlay training profile.
    #[must_use]
    pub fn strict_pgolf_challenge() -> Self {
        Self {
            profile_id: String::from(
                ParameterGolfPromotedProfileKind::StrictPgolfChallenge.profile_id(),
            ),
            kind: ParameterGolfPromotedProfileKind::StrictPgolfChallenge,
            tokenizer_identity:
                ParameterGolfPromotedTokenizerIdentity::ChallengeSp1024SentencePiece,
            dataset_identity: ParameterGolfPromotedDatasetIdentity::ChallengeFinewebSp1024,
            evaluation_policy: ParameterGolfPromotedEvaluationPolicy {
                evaluation_identity: ParameterGolfPromotedEvaluationIdentity::ChallengeBitsPerByte,
                legal_score_first_ttt_required: true,
                contest_bits_per_byte_accounting_required: true,
            },
            artifact_policy: ParameterGolfPromotedArtifactPolicy {
                exact_compressed_artifact_cap_required: true,
                compressed_artifact_cap_bytes: Some(16_000_000),
            },
        }
    }

    fn validate_contract_alignment(&self) -> Result<(), ParameterGolfReferenceTrainingError> {
        let expected_profile_id = self.kind.profile_id();
        if self.profile_id != expected_profile_id {
            return Err(ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted training profile",
                message: format!(
                    "profile id `{}` drifted from frozen kind `{expected_profile_id}`",
                    self.profile_id
                ),
            });
        }
        Ok(())
    }

    fn validate_local_reference_lane(
        &self,
        fixture: &ParameterGolfLocalReferenceFixture,
    ) -> Result<(), ParameterGolfReferenceTrainingError> {
        self.validate_contract_alignment()?;
        if fixture.tokenizer_vocab_size != PARAMETER_GOLF_BASELINE_VOCAB_SIZE {
            return Err(
                ParameterGolfReferenceTrainingError::PromotedProfileRefusal {
                    profile_id: self.profile_id.clone(),
                    detail: format!(
                        "local-reference fixture vocab size {} drifted from promoted baseline vocab size {}",
                        fixture.tokenizer_vocab_size, PARAMETER_GOLF_BASELINE_VOCAB_SIZE
                    ),
                },
            );
        }
        let mut missing = Vec::new();
        if self.tokenizer_identity
            != ParameterGolfPromotedTokenizerIdentity::RepoLocalReferenceSentencePiece
        {
            missing.push(String::from(
                "strict challenge tokenizer identity requires the exact public challenge SP1024 tokenizer, but the current lane is the repo-owned local-reference SentencePiece surface",
            ));
        }
        if self.dataset_identity != ParameterGolfPromotedDatasetIdentity::RepoLocalReferenceFixture
        {
            missing.push(String::from(
                "strict challenge dataset identity requires the public FineWeb SP1024 lane, but the current lane is the repo-owned local-reference fixture",
            ));
        }
        if self.evaluation_policy.legal_score_first_ttt_required {
            missing.push(String::from(
                "strict challenge evaluation requires legal score-first TTT, but the local-reference lane does not execute that overlay",
            ));
        }
        if self
            .evaluation_policy
            .contest_bits_per_byte_accounting_required
        {
            missing.push(String::from(
                "strict challenge evaluation requires contest bits-per-byte accounting, but the local-reference lane only claims bounded local-reference validation",
            ));
        }
        if self.artifact_policy.exact_compressed_artifact_cap_required {
            missing.push(format!(
                "strict challenge artifact posture requires an exact compressed artifact cap of {} bytes, but the local-reference lane does not enforce exported-folder submission accounting",
                self.artifact_policy
                    .compressed_artifact_cap_bytes
                    .unwrap_or(16_000_000)
            ));
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(
                ParameterGolfReferenceTrainingError::PromotedProfileRefusal {
                    profile_id: self.profile_id.clone(),
                    detail: missing.join("; "),
                },
            )
        }
    }
}

/// Config for the bounded Parameter Golf local-reference trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfReferenceTrainingConfig {
    /// Stable training run identifier.
    pub run_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Logical training start time.
    pub started_at_ms: u64,
    /// Logical per-step duration.
    pub step_duration_ms: u64,
    /// Fixed maximum step count.
    pub max_steps: u64,
    /// Batch geometry contract for the run.
    pub geometry: ParameterGolfBatchGeometry,
    /// Baseline optimizer and schedule hyperparameters.
    pub hyperparameters: ParameterGolfTrainingHyperparameters,
    /// Finite-difference epsilon used for selected coordinates.
    pub finite_difference_epsilon: f32,
    /// Explicit promoted profile and policy surface for this run.
    #[serde(default)]
    pub promoted_profile: ParameterGolfPromotedTrainingProfile,
    /// Optional bounded inference window surfaced into the emitted promoted
    /// bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_attention_window_tokens: Option<usize>,
    /// Selected trainable coordinates.
    pub selected_coordinates: Vec<ParameterGolfTrainableCoordinate>,
}

impl ParameterGolfReferenceTrainingConfig {
    /// Returns the canonical bounded local-reference config used by repo-owned tests.
    #[must_use]
    pub fn local_reference() -> Self {
        let geometry = ParameterGolfBatchGeometry::local_reference_defaults();
        let model_dim = 512;
        Self {
            run_id: String::from("parameter-golf-local-reference-run"),
            checkpoint_family: String::from("train.parameter_golf.local_reference"),
            started_at_ms: 1_774_320_000_000,
            step_duration_ms: 50,
            max_steps: 2,
            geometry,
            hyperparameters: ParameterGolfTrainingHyperparameters::baseline_defaults(),
            finite_difference_epsilon: 0.01,
            promoted_profile: ParameterGolfPromotedTrainingProfile::general_psion_small_decoder(),
            inference_attention_window_tokens: None,
            selected_coordinates: vec![
                ParameterGolfTrainableCoordinate {
                    parameter_id: String::from("tok_emb.weight"),
                    flat_index: model_dim,
                },
                ParameterGolfTrainableCoordinate {
                    parameter_id: String::from("blocks.0.attn.c_q.weight"),
                    flat_index: 0,
                },
                ParameterGolfTrainableCoordinate {
                    parameter_id: String::from("skip_weights"),
                    flat_index: 0,
                },
            ],
        }
    }

    /// Returns the canonical promoted first-model proof config for the general
    /// Psion PGOLF-shaped small-decoder profile.
    #[must_use]
    pub fn promoted_general_small_decoder() -> Self {
        let mut config = Self::local_reference();
        config.run_id = String::from("parameter-golf-promoted-general-proof-run");
        config.checkpoint_family = String::from("train.parameter_golf.promoted_general");
        config
    }

    /// Returns a stronger bounded XTRAIN baseline for the promoted general
    /// small-decoder lane. This stays inside the repo-owned fixture and
    /// finite-difference training boundary, but expands the step budget and the
    /// coordinate surface enough to produce a meaningfully inferable local
    /// model.
    #[must_use]
    pub fn xtrain_promoted_general_small_decoder_baseline() -> Self {
        let mut config = Self::promoted_general_small_decoder();
        config.run_id = String::from("parameter-golf-promoted-general-xtrain-baseline");
        config.checkpoint_family = String::from("train.parameter_golf.promoted_general_xtrain");
        config.step_duration_ms = 75;
        config.max_steps = 16;
        config.finite_difference_epsilon = 0.005;
        config.geometry = ParameterGolfBatchGeometry {
            world_size: 1,
            train_batch_tokens: 16,
            validation_batch_tokens: 16,
            train_sequence_length: 4,
            grad_accum_steps: 1,
        };
        config.hyperparameters.tied_embed_lr = 0.2;
        config.hyperparameters.matrix_lr = 0.08;
        config.hyperparameters.scalar_lr = 0.08;
        config.inference_attention_window_tokens = Some(1);
        config.selected_coordinates = promoted_xtrain_coordinate_budget();
        config
    }

    /// Returns the strict PGOLF challenge overlay config. The bounded
    /// local-reference lane is expected to refuse this profile explicitly until
    /// the exact challenge prerequisites are present.
    #[must_use]
    pub fn strict_pgolf_challenge() -> Self {
        let mut config = Self::local_reference();
        config.run_id = String::from("parameter-golf-strict-pgolf-challenge");
        config.checkpoint_family = String::from("train.parameter_golf.strict_pgolf_challenge");
        config.promoted_profile = ParameterGolfPromotedTrainingProfile::strict_pgolf_challenge();
        config
    }

    fn validate(&self) -> Result<(), ParameterGolfReferenceTrainingError> {
        if self.run_id.trim().is_empty() {
            return Err(ParameterGolfReferenceTrainingError::MissingRunId);
        }
        if self.checkpoint_family.trim().is_empty() {
            return Err(ParameterGolfReferenceTrainingError::MissingCheckpointFamily);
        }
        if self.step_duration_ms == 0 {
            return Err(ParameterGolfReferenceTrainingError::InvalidStepDuration);
        }
        if self.max_steps == 0 {
            return Err(ParameterGolfReferenceTrainingError::InvalidMaxSteps);
        }
        if !self.finite_difference_epsilon.is_finite() || self.finite_difference_epsilon <= 0.0 {
            return Err(
                ParameterGolfReferenceTrainingError::InvalidFiniteDifferenceEpsilon {
                    epsilon: self.finite_difference_epsilon,
                },
            );
        }
        if self.selected_coordinates.is_empty() {
            return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
                message: String::from("selected_coordinates must not be empty"),
            });
        }
        if self.inference_attention_window_tokens == Some(0) {
            return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
                message: String::from(
                    "inference_attention_window_tokens must be positive when present",
                ),
            });
        }
        self.promoted_profile.validate_contract_alignment()?;
        self.geometry.validate()?;
        Ok(())
    }
}

fn promoted_xtrain_coordinate_budget() -> Vec<ParameterGolfTrainableCoordinate> {
    let fixture = ParameterGolfLocalReferenceFixture::reference()
        .expect("repo-owned parameter golf local reference fixture must remain loadable");
    let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
        .expect("repo-owned parameter golf baseline fixture must remain loadable");
    let descriptor = model.descriptor();
    let model_dim = descriptor.config.model_dim;
    let mut coordinates = Vec::new();

    for token_id in promoted_xtrain_reference_token_ids(&fixture, 8) {
        for dim in evenly_spaced_flat_indices(model_dim, 4) {
            coordinates.push(ParameterGolfTrainableCoordinate {
                parameter_id: String::from("tok_emb.weight"),
                flat_index: token_id.saturating_mul(model_dim).saturating_add(dim),
            });
        }
    }
    push_evenly_spaced_parameter_coordinates(&mut coordinates, &model, "skip_weights", 4);
    for layer_index in 0..1 {
        push_evenly_spaced_parameter_coordinates(
            &mut coordinates,
            &model,
            format!("blocks.{layer_index}.attn.q_gain").as_str(),
            6,
        );
        push_evenly_spaced_parameter_coordinates(
            &mut coordinates,
            &model,
            format!("blocks.{layer_index}.attn_scale").as_str(),
            4,
        );
        push_evenly_spaced_parameter_coordinates(
            &mut coordinates,
            &model,
            format!("blocks.{layer_index}.mlp_scale").as_str(),
            4,
        );
        push_evenly_spaced_parameter_coordinates(
            &mut coordinates,
            &model,
            format!("blocks.{layer_index}.resid_mix").as_str(),
            4,
        );
    }

    coordinates.sort();
    coordinates.dedup();
    coordinates
}

fn promoted_xtrain_reference_token_ids(
    fixture: &ParameterGolfLocalReferenceFixture,
    max_tokens: usize,
) -> Vec<usize> {
    let mut counts = BTreeMap::<usize, usize>::new();
    for token_id in &fixture.training_tokens {
        *counts.entry(*token_id as usize).or_default() += 1;
    }
    let mut rows = counts.into_iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows.into_iter()
        .take(max_tokens)
        .map(|(token_id, _)| token_id)
        .collect()
}

fn push_evenly_spaced_parameter_coordinates(
    coordinates: &mut Vec<ParameterGolfTrainableCoordinate>,
    model: &ParameterGolfReferenceModel,
    parameter_id: &str,
    count: usize,
) {
    let Some(parameter) = model
        .weights()
        .parameter_vector(&model.descriptor().config, parameter_id)
    else {
        panic!("parameter golf promoted XTRAIN budget expected tensor `{parameter_id}`");
    };
    for flat_index in evenly_spaced_flat_indices(parameter.values.len(), count) {
        coordinates.push(ParameterGolfTrainableCoordinate {
            parameter_id: String::from(parameter_id),
            flat_index,
        });
    }
}

fn evenly_spaced_flat_indices(parameter_len: usize, count: usize) -> Vec<usize> {
    if parameter_len == 0 || count == 0 {
        return Vec::new();
    }
    let actual = parameter_len.min(count);
    if actual == 1 {
        return vec![0];
    }
    let mut indices = BTreeSet::new();
    for position in 0..actual {
        let numerator = position.saturating_mul(parameter_len.saturating_sub(1));
        let denominator = actual.saturating_sub(1).max(1);
        indices.insert(numerator / denominator);
    }
    indices.into_iter().collect()
}

/// One serialized artifact emitted by the local-reference trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfTrainingArtifact {
    /// Stable artifact kind.
    pub artifact_kind: String,
    /// Stable artifact reference.
    pub artifact_ref: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Serialized artifact bytes.
    pub bytes: Vec<u8>,
}

impl ParameterGolfTrainingArtifact {
    pub(crate) fn new(
        artifact_kind: impl Into<String>,
        artifact_ref: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        let artifact_kind = artifact_kind.into();
        let artifact_ref = artifact_ref.into();
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_parameter_golf_training_artifact|");
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

/// Serializable optimizer-state payload for one trainable tensor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum ParameterGolfReferenceOptimizerState {
    /// Adam-family state over selected coordinates.
    Adam {
        /// Optimizer state for the selected coordinate vector.
        state: TrainingOptimizerState,
    },
    /// Muon state over a dense matrix tensor.
    Muon {
        /// Muon momentum buffer.
        state: ParameterGolfMuonState,
    },
}

/// Serializable checkpoint view for one trainable tensor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfCheckpointTrainableTensor {
    /// Stable tensor identifier.
    pub parameter_id: String,
    /// Logical tensor shape.
    pub shape: Vec<usize>,
    /// Selected flat coordinates used for finite-difference gradients.
    pub selected_indices: Vec<usize>,
    /// Reconstructed optimizer execution for the tensor.
    pub execution: ParameterGolfOptimizerExecution,
    /// Mutable optimizer state for the tensor.
    pub optimizer_state: ParameterGolfReferenceOptimizerState,
}

/// JSON manifest paired with one raw checkpoint export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfCheckpointManifest {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable checkpoint reference.
    pub checkpoint_ref: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Explicit promoted profile and policy surface.
    #[serde(default)]
    pub promoted_profile: ParameterGolfPromotedTrainingProfile,
    /// Logical checkpoint step.
    pub step: u64,
    /// Logical training start time.
    pub started_at_ms: u64,
    /// Logical per-step duration.
    pub step_duration_ms: u64,
    /// Fixed-budget step count.
    pub max_steps: u64,
    /// Single-device batch geometry.
    pub geometry: ParameterGolfBatchGeometry,
    /// Baseline optimizer and schedule hyperparameters.
    pub hyperparameters: ParameterGolfTrainingHyperparameters,
    /// Finite-difference epsilon for the run.
    pub finite_difference_epsilon: f32,
    /// Stable descriptor digest for the seeded baseline model.
    pub base_descriptor_digest: String,
    /// Stable descriptor digest for the current checkpointed model.
    pub current_descriptor_digest: String,
    /// Stable digest over the optimizer split.
    pub optimizer_plan_digest: String,
    /// Stable digest over the training split.
    pub training_dataset_digest: String,
    /// Stable digest over the validation split.
    pub validation_dataset_digest: String,
    /// Current validation-eval digest.
    pub validation_eval_digest: String,
    /// Step metrics accumulated so far.
    pub step_metrics: Vec<ParameterGolfReferenceTrainingStepMetrics>,
    /// Trainable tensor runtime state.
    pub trainable_tensors: Vec<ParameterGolfCheckpointTrainableTensor>,
    /// Optional parent checkpoint reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_checkpoint_ref: Option<String>,
    /// Optional parent manifest digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_manifest_digest: Option<String>,
}

impl ParameterGolfCheckpointManifest {
    /// Returns a stable digest over the manifest payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_parameter_golf_checkpoint_manifest|", self)
    }
}

/// One persisted checkpoint plus lineage refs for the local-reference trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfCheckpointArtifact {
    /// Raw-model weights artifact.
    pub weights_artifact: ParameterGolfTrainingArtifact,
    /// JSON manifest artifact.
    pub manifest_artifact: ParameterGolfTrainingArtifact,
    /// Structured manifest.
    pub manifest: ParameterGolfCheckpointManifest,
    /// Runtime-visible checkpoint reference.
    pub checkpoint: TrainingCheckpointReference,
}

/// Higher-level per-step metrics for the local-reference trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfReferenceTrainingStepMetrics {
    /// One-based global step.
    pub global_step: u64,
    /// Mean microbatch loss across the gradient-accumulation window.
    pub mean_microbatch_loss: f32,
    /// Validation mean loss after the step.
    pub validation_mean_loss: f64,
    /// Validation bits per byte after the step.
    pub validation_bits_per_byte: f64,
    /// Effective LR multiplier at the step.
    pub learning_rate_multiplier: f32,
    /// Effective Muon momentum at the step.
    pub muon_momentum: f32,
}

/// Final machine-readable summary for one bounded local-reference run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfReferenceTrainingSummary {
    /// Initial validation mean loss.
    pub initial_validation_mean_loss: f64,
    /// Final validation mean loss.
    pub final_validation_mean_loss: f64,
    /// Final validation bits per byte.
    pub final_validation_bits_per_byte: f64,
    /// Raw roundtrip validation mean loss.
    pub raw_roundtrip_validation_mean_loss: f64,
    /// Int8+zlib roundtrip validation mean loss.
    pub int8_zlib_roundtrip_validation_mean_loss: f64,
    /// Stable digest of the initial checkpoint manifest.
    pub initial_checkpoint_manifest_digest: String,
    /// Stable digest of the final checkpoint manifest.
    pub final_checkpoint_manifest_digest: String,
    /// Stable digest of the raw model artifact.
    pub raw_model_artifact_digest: String,
    /// Stable digest of the int8+zlib model artifact.
    pub int8_zlib_model_artifact_digest: String,
}

/// Full bounded local-reference training outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfReferenceTrainingOutcome {
    /// Seeded baseline model before any step.
    pub initial_model: ParameterGolfReferenceModel,
    /// Final stepped model after the bounded run.
    pub trained_model: ParameterGolfReferenceModel,
    /// Accumulated per-step metrics.
    pub step_metrics: Vec<ParameterGolfReferenceTrainingStepMetrics>,
    /// Initial checkpoint artifact.
    pub initial_checkpoint: ParameterGolfCheckpointArtifact,
    /// Final checkpoint artifact.
    pub final_checkpoint: ParameterGolfCheckpointArtifact,
    /// Final raw full-precision model artifact.
    pub raw_model_artifact: ParameterGolfTrainingArtifact,
    /// Final int8+zlib model artifact.
    pub int8_zlib_model_artifact: ParameterGolfTrainingArtifact,
    /// Initial validation report.
    pub initial_validation_eval: ParameterGolfValidationEvalReport,
    /// Final validation report.
    pub final_validation_eval: ParameterGolfValidationEvalReport,
    /// Validation report after raw restore.
    pub raw_roundtrip_validation_eval: ParameterGolfValidationEvalReport,
    /// Validation report after int8+zlib restore.
    pub int8_zlib_roundtrip_validation_eval: ParameterGolfValidationEvalReport,
    /// Durable checkpoint writeback receipts when the run used async writeback.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoint_writeback_receipts: Vec<AsyncCheckpointWritebackReceipt>,
    /// Final summary.
    pub summary: ParameterGolfReferenceTrainingSummary,
}

/// One expected or observed tensor entry in the promoted checkpoint surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfCheckpointSurfaceTensorEntry {
    /// Stable tensor name.
    pub name: String,
    /// Stable tensor shape.
    pub shape: Vec<usize>,
}

/// One tensor shape mismatch between the promoted descriptor and the emitted
/// checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfCheckpointSurfaceShapeMismatch {
    /// Stable tensor name.
    pub name: String,
    /// Expected logical shape from the promoted descriptor.
    pub expected: Vec<usize>,
    /// Actual logical shape from the emitted checkpoint.
    pub actual: Vec<usize>,
}

/// Machine-readable report proving whether the emitted checkpoint tensor surface
/// matches the promoted PGOLF descriptor exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedCheckpointSurfaceReport {
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Stable descriptor digest for the promoted decoder.
    pub descriptor_digest: String,
    /// Stable checkpoint artifact digest.
    pub checkpoint_artifact_digest: String,
    /// Ordered expected tensors from the promoted descriptor.
    pub expected_tensors: Vec<ParameterGolfCheckpointSurfaceTensorEntry>,
    /// Ordered actual tensors from the emitted checkpoint artifact.
    pub actual_tensors: Vec<ParameterGolfCheckpointSurfaceTensorEntry>,
    /// Tensor names declared by the descriptor but missing from the artifact.
    pub missing_tensors: Vec<String>,
    /// Tensor names present in the artifact but not declared by the descriptor.
    pub unexpected_tensors: Vec<String>,
    /// Tensor names whose shapes drifted.
    pub shape_mismatches: Vec<ParameterGolfCheckpointSurfaceShapeMismatch>,
    /// Whether tensor-name coverage matches exactly.
    pub exact_name_match: bool,
    /// Whether tensor shapes match exactly.
    pub exact_shape_match: bool,
    /// Whether the full tensor surface matches exactly.
    pub exact_match: bool,
    /// Human-readable explanation of the report.
    pub detail: String,
    /// Stable digest over the report payload.
    pub report_digest: String,
}

impl ParameterGolfPromotedCheckpointSurfaceReport {
    /// Returns a stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(
            b"psionic_parameter_golf_promoted_checkpoint_surface_report|",
            self,
        )
    }
}

/// Machine-readable proof that the promoted checkpoint can be restored and
/// resumed to the same final state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedResumeProof {
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Checkpoint ref used as the restore source.
    pub restore_source_checkpoint_ref: String,
    /// Stable manifest digest for the restore source checkpoint.
    pub restore_source_manifest_digest: String,
    /// Final checkpoint ref emitted by the continuous source run.
    pub continuous_final_checkpoint_ref: String,
    /// Final checkpoint manifest digest emitted by the continuous source run.
    pub continuous_final_manifest_digest: String,
    /// Final checkpoint ref emitted by the restored-and-resumed run.
    pub resumed_final_checkpoint_ref: String,
    /// Final checkpoint manifest digest emitted by the restored-and-resumed run.
    pub resumed_final_manifest_digest: String,
    /// One-based number of steps replayed after restore.
    pub replayed_steps: u64,
    /// Whether restore/resume reached exact final parity.
    pub exact_final_parity: bool,
    /// Human-readable explanation of the proof.
    pub detail: String,
    /// Stable digest over the proof payload.
    pub proof_digest: String,
}

impl ParameterGolfPromotedResumeProof {
    /// Returns a stable digest over the proof payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_parameter_golf_promoted_resume_proof|", self)
    }
}

/// Final machine-readable summary for the promoted first-model PGOLF proof run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedReferenceRunSummary {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Stable promoted profile kind label.
    pub profile_kind: String,
    /// Stable promoted run id.
    pub run_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Stable descriptor digest.
    pub descriptor_digest: String,
    /// Stable training fixture digest.
    pub training_dataset_digest: String,
    /// Stable validation fixture digest.
    pub validation_dataset_digest: String,
    /// Initial checkpoint ref.
    pub initial_checkpoint_ref: String,
    /// Final checkpoint ref.
    pub final_checkpoint_ref: String,
    /// Final checkpoint manifest digest.
    pub final_checkpoint_manifest_digest: String,
    /// Stable checkpoint surface report digest.
    pub checkpoint_surface_report_digest: String,
    /// Stable resume proof digest.
    pub resume_proof_digest: String,
    /// Bounded training summary.
    pub training_summary: ParameterGolfReferenceTrainingSummary,
    /// Human-readable explanation of the run boundary.
    pub detail: String,
}

/// Full promoted PGOLF-shaped first-model proof run.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterGolfPromotedReferenceRun {
    /// Frozen promoted profile contract used by the run.
    pub profile_contract: ParameterGolfPromotedProfileContract,
    /// Concrete training config used by the run.
    pub training_config: ParameterGolfReferenceTrainingConfig,
    /// Stable trained-model descriptor.
    pub model_descriptor: ParameterGolfModelDescriptor,
    /// Full bounded training outcome.
    pub training_outcome: ParameterGolfReferenceTrainingOutcome,
    /// Exact checkpoint-surface verification report.
    pub checkpoint_surface_report: ParameterGolfPromotedCheckpointSurfaceReport,
    /// Restore/resume parity proof.
    pub resume_proof: ParameterGolfPromotedResumeProof,
    /// Runtime-loadable tokenizer asset for the promoted bundle.
    pub tokenizer_asset: ParameterGolfPromotedTokenizerAsset,
    /// Default generation config for the promoted bundle.
    pub generation_config: ParameterGolfPromotedGenerationConfig,
    /// Canonical on-disk bundle manifest for the promoted runtime bundle.
    pub bundle_manifest: ParameterGolfPromotedBundleManifest,
    /// Final summary for operator-facing proof-run output.
    pub summary: ParameterGolfPromotedReferenceRunSummary,
}

/// Failure for the bounded Parameter Golf local-reference trainer.
#[derive(Debug, Error)]
pub enum ParameterGolfReferenceTrainingError {
    #[error("parameter golf local reference training requires a non-empty run id")]
    MissingRunId,
    #[error("parameter golf local reference training requires a non-empty checkpoint family")]
    MissingCheckpointFamily,
    #[error("parameter golf local reference training requires a non-zero step duration")]
    InvalidStepDuration,
    #[error("parameter golf local reference training requires max_steps > 0")]
    InvalidMaxSteps,
    #[error(
        "parameter golf local reference training requires a positive finite-difference epsilon, got {epsilon}"
    )]
    InvalidFiniteDifferenceEpsilon { epsilon: f32 },
    #[error("parameter golf local reference batch geometry is invalid: {message}")]
    InvalidBatchGeometry { message: String },
    #[error("parameter golf local reference fixture is invalid: {message}")]
    InvalidFixture { message: String },
    #[error(
        "parameter golf local reference fixture vocab size {fixture_vocab_size} does not match model vocab size {model_vocab_size}"
    )]
    FixtureVocabMismatch {
        fixture_vocab_size: usize,
        model_vocab_size: usize,
    },
    #[error("parameter golf coordinate `{parameter_id}`[{flat_index}] is duplicated")]
    DuplicateCoordinate {
        parameter_id: String,
        flat_index: usize,
    },
    #[error(
        "parameter golf coordinate `{parameter_id}`[{flat_index}] exceeds tensor length {parameter_len}"
    )]
    CoordinateOutOfRange {
        parameter_id: String,
        flat_index: usize,
        parameter_len: usize,
    },
    #[error("parameter golf optimizer split did not classify tensor `{parameter_id}`")]
    MissingOptimizerGroup { parameter_id: String },
    #[error("parameter golf local reference runner `{run_id}` already completed")]
    AlreadyCompleted { run_id: String },
    #[error(
        "parameter golf local reference runner ended early: completed {completed_steps} of {max_steps} steps"
    )]
    IncompleteRun {
        completed_steps: u64,
        max_steps: u64,
    },
    #[error("parameter golf promoted profile `{profile_id}` refused this lane: {detail}")]
    PromotedProfileRefusal { profile_id: String, detail: String },
    #[error("parameter golf artifact is missing tensor `{parameter_id}`")]
    MissingArtifactTensor { parameter_id: String },
    #[error(
        "parameter golf artifact tensor `{parameter_id}` had shape {actual:?}; expected {expected:?}"
    )]
    ArtifactTensorShape {
        parameter_id: String,
        actual: Vec<usize>,
        expected: Vec<usize>,
    },
    #[error("{context}: {message}")]
    Serialization {
        context: &'static str,
        message: String,
    },
    #[error(transparent)]
    Data(#[from] ParameterGolfDataError),
    #[error(transparent)]
    Model(#[from] ParameterGolfModelError),
    #[error(transparent)]
    Execution(#[from] ParameterGolfExecutionError),
    #[error(transparent)]
    Eval(#[from] ParameterGolfValidationEvalError),
    #[error(transparent)]
    Train(#[from] ParameterGolfTrainError),
    #[error(transparent)]
    Optimizer(#[from] TrainingOptimizerError),
    #[error(transparent)]
    AsyncCheckpointWriteback(#[from] AsyncCheckpointWritebackError),
    #[error(transparent)]
    LocalMetricSink(#[from] LocalTrainMetricSinkError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug, PartialEq)]
enum TrainableTensorRuntime {
    AdamSparse {
        parameter_id: String,
        shape: Vec<usize>,
        selected_indices: Vec<usize>,
        selected_values: Vec<f32>,
        optimizer: TrainingOptimizerConfig,
        optimizer_state: TrainingOptimizerState,
    },
    MuonDense {
        parameter_id: String,
        shape: Vec<usize>,
        selected_indices: Vec<usize>,
        values: Vec<f32>,
        optimizer: ParameterGolfMuonConfig,
        optimizer_state: ParameterGolfMuonState,
    },
}

impl TrainableTensorRuntime {
    fn parameter_id(&self) -> &str {
        match self {
            Self::AdamSparse { parameter_id, .. } | Self::MuonDense { parameter_id, .. } => {
                parameter_id.as_str()
            }
        }
    }

    fn full_values(
        &self,
        baseline_vectors: &BTreeMap<String, ParameterGolfParameterVector>,
    ) -> Vec<f32> {
        match self {
            Self::AdamSparse {
                parameter_id,
                selected_indices,
                selected_values,
                ..
            } => {
                let mut values = baseline_vectors
                    .get(parameter_id)
                    .expect("baseline vector should exist")
                    .values
                    .clone();
                for (flat_index, value) in selected_indices.iter().zip(selected_values.iter()) {
                    values[*flat_index] = *value;
                }
                values
            }
            Self::MuonDense { values, .. } => values.clone(),
        }
    }

    fn checkpoint_tensor(&self) -> ParameterGolfCheckpointTrainableTensor {
        match self {
            Self::AdamSparse {
                parameter_id,
                shape,
                selected_indices,
                optimizer,
                optimizer_state,
                ..
            } => ParameterGolfCheckpointTrainableTensor {
                parameter_id: parameter_id.clone(),
                shape: shape.clone(),
                selected_indices: selected_indices.clone(),
                execution: ParameterGolfOptimizerExecution::Adam {
                    optimizer: optimizer.clone(),
                },
                optimizer_state: ParameterGolfReferenceOptimizerState::Adam {
                    state: optimizer_state.clone(),
                },
            },
            Self::MuonDense {
                parameter_id,
                shape,
                selected_indices,
                optimizer,
                optimizer_state,
                ..
            } => ParameterGolfCheckpointTrainableTensor {
                parameter_id: parameter_id.clone(),
                shape: shape.clone(),
                selected_indices: selected_indices.clone(),
                execution: ParameterGolfOptimizerExecution::Muon {
                    optimizer: optimizer.clone(),
                },
                optimizer_state: ParameterGolfReferenceOptimizerState::Muon {
                    state: optimizer_state.clone(),
                },
            },
        }
    }

    fn apply_gradients(
        &mut self,
        gradients: &[f32],
        learning_rate_multiplier: f32,
        muon_momentum: f32,
        step_number: u64,
    ) -> Result<(), ParameterGolfReferenceTrainingError> {
        match self {
            Self::AdamSparse {
                selected_values,
                optimizer,
                optimizer_state,
                ..
            } => {
                let mut effective_optimizer = optimizer.clone();
                effective_optimizer.learning_rate *= learning_rate_multiplier;
                effective_optimizer.apply_step(
                    selected_values.as_mut_slice(),
                    gradients,
                    optimizer_state,
                    step_number,
                )?;
                Ok(())
            }
            Self::MuonDense {
                shape,
                values,
                optimizer,
                optimizer_state,
                ..
            } => {
                let mut effective_optimizer = optimizer.clone();
                effective_optimizer.learning_rate *= learning_rate_multiplier;
                effective_optimizer.momentum = muon_momentum;
                apply_parameter_golf_muon_step(
                    values.as_mut_slice(),
                    shape.as_slice(),
                    gradients,
                    &effective_optimizer,
                    optimizer_state,
                )?;
                Ok(())
            }
        }
    }
}

/// Stepwise in-memory runner for the bounded Parameter Golf local-reference lane.
#[derive(Debug)]
pub struct ParameterGolfReferenceTrainingRunner {
    fixture: ParameterGolfLocalReferenceFixture,
    config: ParameterGolfReferenceTrainingConfig,
    initial_model: ParameterGolfReferenceModel,
    current_model: ParameterGolfReferenceModel,
    byte_luts: ParameterGolfSentencePieceByteLuts,
    baseline_vectors: BTreeMap<String, ParameterGolfParameterVector>,
    trainable_tensors: Vec<TrainableTensorRuntime>,
    optimizer_plan_digest: String,
    completed_steps: u64,
    step_metrics: Vec<ParameterGolfReferenceTrainingStepMetrics>,
    initial_checkpoint: ParameterGolfCheckpointArtifact,
    latest_checkpoint: ParameterGolfCheckpointArtifact,
    initial_validation_eval: ParameterGolfValidationEvalReport,
    current_validation_eval: ParameterGolfValidationEvalReport,
}

impl ParameterGolfReferenceTrainingRunner {
    /// Seeds one bounded local-reference runner.
    pub fn new(
        fixture: &ParameterGolfLocalReferenceFixture,
        config: &ParameterGolfReferenceTrainingConfig,
    ) -> Result<Self, ParameterGolfReferenceTrainingError> {
        config.validate()?;
        fixture.validate()?;
        config
            .promoted_profile
            .validate_local_reference_lane(fixture)?;

        let initial_model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        if fixture.tokenizer_vocab_size != initial_model.descriptor().config.vocab_size {
            return Err(ParameterGolfReferenceTrainingError::FixtureVocabMismatch {
                fixture_vocab_size: fixture.tokenizer_vocab_size,
                model_vocab_size: initial_model.descriptor().config.vocab_size,
            });
        }
        if fixture.training_tokens.len() < config.geometry.local_train_batch_tokens() + 1 {
            return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
                message: format!(
                    "training_tokens must contain at least {} tokens",
                    config.geometry.local_train_batch_tokens() + 1
                ),
            });
        }
        if fixture.validation_tokens.len() <= config.geometry.train_sequence_length {
            return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
                message: format!(
                    "validation_tokens must contain at least {} tokens",
                    config.geometry.train_sequence_length + 1
                ),
            });
        }

        let optimizer_plan =
            parameter_golf_optimizer_plan(initial_model.descriptor(), &config.hyperparameters)?;
        let optimizer_plan_digest =
            stable_digest(b"psionic_parameter_golf_optimizer_plan|", &optimizer_plan);
        let coordinate_map = coordinate_map(config.selected_coordinates.as_slice())?;
        let mut baseline_vectors = BTreeMap::new();
        let mut trainable_tensors = Vec::with_capacity(coordinate_map.len());
        for (parameter_id, selected_indices) in coordinate_map {
            let parameter = initial_model
                .weights()
                .parameter_vector(&initial_model.descriptor().config, parameter_id.as_str())
                .ok_or_else(
                    || ParameterGolfReferenceTrainingError::MissingOptimizerGroup {
                        parameter_id: parameter_id.clone(),
                    },
                )?;
            validate_selected_indices(&parameter_id, &selected_indices, parameter.values.len())?;
            let group = optimizer_plan
                .groups
                .iter()
                .find(|group| group.tensor_names.iter().any(|name| name == &parameter_id))
                .ok_or_else(
                    || ParameterGolfReferenceTrainingError::MissingOptimizerGroup {
                        parameter_id: parameter_id.clone(),
                    },
                )?;
            baseline_vectors.insert(parameter_id.clone(), parameter.clone());
            match &group.execution {
                ParameterGolfOptimizerExecution::Adam { optimizer } => {
                    let selected_len = selected_indices.len();
                    let selected_values = selected_indices
                        .iter()
                        .map(|flat_index| parameter.values[*flat_index])
                        .collect::<Vec<_>>();
                    trainable_tensors.push(TrainableTensorRuntime::AdamSparse {
                        parameter_id,
                        shape: parameter.shape.dims().to_vec(),
                        selected_indices,
                        selected_values,
                        optimizer: optimizer.clone(),
                        optimizer_state: optimizer.initialize_state(selected_len),
                    });
                }
                ParameterGolfOptimizerExecution::Muon { optimizer } => {
                    let shape = parameter.shape.dims().to_vec();
                    let rows = shape.first().copied().unwrap_or(0);
                    let cols = shape.get(1).copied().unwrap_or(0);
                    trainable_tensors.push(TrainableTensorRuntime::MuonDense {
                        parameter_id,
                        shape,
                        selected_indices,
                        values: parameter.values,
                        optimizer: optimizer.clone(),
                        optimizer_state: ParameterGolfMuonState::zeros(rows, cols),
                    });
                }
            }
        }
        let byte_luts = fixture.byte_luts()?;
        let initial_validation_eval = evaluate_parameter_golf_validation(
            &initial_model,
            fixture.validation_tokens.as_slice(),
            config.geometry.train_sequence_length,
            config.geometry.local_validation_batch_tokens(),
            &byte_luts,
        )?;
        let initial_checkpoint = export_checkpoint(
            &initial_model,
            &initial_model,
            trainable_tensors.as_slice(),
            fixture,
            config,
            0,
            &initial_validation_eval,
            &[],
            optimizer_plan_digest.as_str(),
            None,
        )?;
        Ok(Self {
            fixture: fixture.clone(),
            config: config.clone(),
            initial_model: initial_model.clone(),
            current_model: initial_model,
            byte_luts,
            baseline_vectors,
            trainable_tensors,
            optimizer_plan_digest,
            completed_steps: 0,
            step_metrics: Vec::new(),
            initial_checkpoint: initial_checkpoint.clone(),
            latest_checkpoint: initial_checkpoint,
            initial_validation_eval: initial_validation_eval.clone(),
            current_validation_eval: initial_validation_eval,
        })
    }

    /// Returns whether the runner already exhausted its fixed budget.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.completed_steps >= self.config.max_steps
    }

    /// Returns the current stepped model.
    #[must_use]
    pub fn current_model(&self) -> &ParameterGolfReferenceModel {
        &self.current_model
    }

    /// Returns the latest checkpoint artifact.
    #[must_use]
    pub fn latest_checkpoint(&self) -> &ParameterGolfCheckpointArtifact {
        &self.latest_checkpoint
    }

    /// Returns the accumulated per-step metrics.
    #[must_use]
    pub fn step_metrics(&self) -> &[ParameterGolfReferenceTrainingStepMetrics] {
        self.step_metrics.as_slice()
    }

    /// Advances the trainer by exactly one optimizer step.
    pub fn step(&mut self) -> Result<(), ParameterGolfReferenceTrainingError> {
        if self.is_complete() {
            return Err(ParameterGolfReferenceTrainingError::AlreadyCompleted {
                run_id: self.config.run_id.clone(),
            });
        }

        let step_index = self.completed_steps;
        let mut accumulated_gradients = self
            .trainable_tensors
            .iter()
            .map(|tensor| match tensor {
                TrainableTensorRuntime::AdamSparse {
                    selected_values, ..
                } => vec![0.0_f32; selected_values.len()],
                TrainableTensorRuntime::MuonDense { values, .. } => vec![0.0_f32; values.len()],
            })
            .collect::<Vec<_>>();
        let mut microbatch_loss_sum = 0.0_f32;

        for micro_step in 0..self.config.geometry.grad_accum_steps {
            let global_micro_step =
                (step_index as usize * self.config.geometry.grad_accum_steps) + micro_step;
            let (input_ids, target_ids) = take_training_microbatch(
                self.fixture.training_tokens.as_slice(),
                &self.config.geometry,
                global_micro_step,
            )?;
            let microbatch_loss = self
                .current_model
                .loss(input_ids.as_slice(), target_ids.as_slice())?;
            microbatch_loss_sum += microbatch_loss;
            for (tensor_index, tensor) in self.trainable_tensors.iter().enumerate() {
                let gradients = finite_difference_gradients(
                    &self.current_model,
                    tensor,
                    &self.baseline_vectors,
                    self.config.finite_difference_epsilon,
                    input_ids.as_slice(),
                    target_ids.as_slice(),
                )?;
                for (accumulated, gradient) in accumulated_gradients[tensor_index]
                    .iter_mut()
                    .zip(gradients.iter())
                {
                    *accumulated += *gradient / self.config.geometry.grad_accum_steps as f32;
                }
            }
        }

        clip_accumulated_gradients(
            accumulated_gradients.as_mut_slice(),
            self.config.hyperparameters.grad_clip_norm,
        );

        let elapsed_ms = step_index.saturating_mul(self.config.step_duration_ms) as f32;
        let learning_rate_multiplier = self
            .config
            .hyperparameters
            .learning_rate_multiplier(step_index, elapsed_ms);
        let muon_momentum = self
            .config
            .hyperparameters
            .muon_momentum_at_step(step_index);
        let step_number = step_index.saturating_add(1);
        for (tensor, gradients) in self
            .trainable_tensors
            .iter_mut()
            .zip(accumulated_gradients.iter())
        {
            tensor.apply_gradients(
                gradients.as_slice(),
                learning_rate_multiplier,
                muon_momentum,
                step_number,
            )?;
        }

        self.current_model = materialize_model(
            &self.initial_model,
            self.trainable_tensors.as_slice(),
            &self.baseline_vectors,
        )?;
        self.completed_steps = step_number;
        self.current_validation_eval = evaluate_parameter_golf_validation(
            &self.current_model,
            self.fixture.validation_tokens.as_slice(),
            self.config.geometry.train_sequence_length,
            self.config.geometry.local_validation_batch_tokens(),
            &self.byte_luts,
        )?;
        let step_metrics = ParameterGolfReferenceTrainingStepMetrics {
            global_step: step_number,
            mean_microbatch_loss: microbatch_loss_sum
                / self.config.geometry.grad_accum_steps as f32,
            validation_mean_loss: self.current_validation_eval.mean_loss,
            validation_bits_per_byte: self.current_validation_eval.bits_per_byte,
            learning_rate_multiplier,
            muon_momentum,
        };
        self.step_metrics.push(step_metrics);
        self.latest_checkpoint = export_checkpoint(
            &self.initial_model,
            &self.current_model,
            self.trainable_tensors.as_slice(),
            &self.fixture,
            &self.config,
            step_number,
            &self.current_validation_eval,
            self.step_metrics.as_slice(),
            self.optimizer_plan_digest.as_str(),
            Some(&self.latest_checkpoint),
        )?;
        Ok(())
    }

    /// Consumes a completed runner and returns the whole-run outcome.
    pub fn into_outcome(
        self,
    ) -> Result<ParameterGolfReferenceTrainingOutcome, ParameterGolfReferenceTrainingError> {
        if !self.is_complete() {
            return Err(ParameterGolfReferenceTrainingError::IncompleteRun {
                completed_steps: self.completed_steps,
                max_steps: self.config.max_steps,
            });
        }

        let raw_model_artifact = self.latest_checkpoint.weights_artifact.clone();
        let raw_roundtrip_model = restore_parameter_golf_model_from_safetensors(
            &self.initial_model,
            raw_model_artifact.bytes.as_slice(),
        )?;
        let raw_roundtrip_validation_eval = evaluate_parameter_golf_validation(
            &raw_roundtrip_model,
            self.fixture.validation_tokens.as_slice(),
            self.config.geometry.train_sequence_length,
            self.config.geometry.local_validation_batch_tokens(),
            &self.byte_luts,
        )?;
        let int8_zlib_model_artifact = export_parameter_golf_int8_zlib_model_artifact(
            &self.current_model,
            self.config.run_id.as_str(),
            self.completed_steps,
        )?;
        let int8_zlib_roundtrip_model = restore_parameter_golf_model_from_int8_zlib(
            &self.initial_model,
            int8_zlib_model_artifact.bytes.as_slice(),
        )?;
        let int8_zlib_roundtrip_validation_eval = evaluate_parameter_golf_validation(
            &int8_zlib_roundtrip_model,
            self.fixture.validation_tokens.as_slice(),
            self.config.geometry.train_sequence_length,
            self.config.geometry.local_validation_batch_tokens(),
            &self.byte_luts,
        )?;
        let summary = ParameterGolfReferenceTrainingSummary {
            initial_validation_mean_loss: self.initial_validation_eval.mean_loss,
            final_validation_mean_loss: self.current_validation_eval.mean_loss,
            final_validation_bits_per_byte: self.current_validation_eval.bits_per_byte,
            raw_roundtrip_validation_mean_loss: raw_roundtrip_validation_eval.mean_loss,
            int8_zlib_roundtrip_validation_mean_loss: int8_zlib_roundtrip_validation_eval.mean_loss,
            initial_checkpoint_manifest_digest: self.initial_checkpoint.manifest.stable_digest(),
            final_checkpoint_manifest_digest: self.latest_checkpoint.manifest.stable_digest(),
            raw_model_artifact_digest: raw_model_artifact.artifact_digest.clone(),
            int8_zlib_model_artifact_digest: int8_zlib_model_artifact.artifact_digest.clone(),
        };
        Ok(ParameterGolfReferenceTrainingOutcome {
            initial_model: self.initial_model,
            trained_model: self.current_model,
            step_metrics: self.step_metrics,
            initial_checkpoint: self.initial_checkpoint,
            final_checkpoint: self.latest_checkpoint,
            raw_model_artifact,
            int8_zlib_model_artifact,
            initial_validation_eval: self.initial_validation_eval,
            final_validation_eval: self.current_validation_eval,
            raw_roundtrip_validation_eval,
            int8_zlib_roundtrip_validation_eval,
            checkpoint_writeback_receipts: Vec::new(),
            summary,
        })
    }
}

/// Runs the bounded local-reference trainer end to end.
pub fn train_parameter_golf_local_reference(
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
) -> Result<ParameterGolfReferenceTrainingOutcome, ParameterGolfReferenceTrainingError> {
    let mut runner = ParameterGolfReferenceTrainingRunner::new(fixture, config)?;
    while !runner.is_complete() {
        runner.step()?;
    }
    runner.into_outcome()
}

/// Runs the bounded local-reference trainer end to end and fans typed local
/// telemetry into the provided metric sink surface.
pub fn train_parameter_golf_local_reference_with_metric_sink(
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
    metric_sink: &mut LocalTrainMetricFanout,
) -> Result<ParameterGolfReferenceTrainingOutcome, ParameterGolfReferenceTrainingError> {
    let mut runner = ParameterGolfReferenceTrainingRunner::new(fixture, config)?;
    while !runner.is_complete() {
        runner.step()?;
        emit_parameter_golf_step_metrics(
            metric_sink,
            config.run_id.as_str(),
            runner.step_metrics(),
        )?;
    }
    metric_sink.flush()?;
    runner.into_outcome()
}

/// Runs the bounded local-reference trainer end to end and persists each emitted
/// checkpoint through the shared async writeback worker.
pub fn train_parameter_golf_local_reference_with_async_checkpoint_writeback(
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
    checkpoint_output_root: &Path,
    writeback_options: AsyncCheckpointWritebackOptions,
) -> Result<ParameterGolfReferenceTrainingOutcome, ParameterGolfReferenceTrainingError> {
    let mut runner = ParameterGolfReferenceTrainingRunner::new(fixture, config)?;
    let max_in_flight_writes = writeback_options.max_in_flight_writes();
    let mut worker = AsyncCheckpointWritebackWorker::new(writeback_options)?;
    let mut pending_tickets = VecDeque::new();
    let mut receipts = Vec::new();

    submit_checkpoint_async_writeback(
        &worker,
        runner.latest_checkpoint(),
        checkpoint_output_root,
        max_in_flight_writes,
        &mut pending_tickets,
        &mut receipts,
    )?;
    while !runner.is_complete() {
        runner.step()?;
        submit_checkpoint_async_writeback(
            &worker,
            runner.latest_checkpoint(),
            checkpoint_output_root,
            max_in_flight_writes,
            &mut pending_tickets,
            &mut receipts,
        )?;
    }
    drain_checkpoint_async_writeback_tickets(&mut pending_tickets, &mut receipts)?;
    let shutdown_receipts = worker.shutdown_flush()?;

    let mut outcome = runner.into_outcome()?;
    outcome.checkpoint_writeback_receipts =
        merge_checkpoint_writeback_receipts(receipts, shutdown_receipts);
    Ok(outcome)
}

/// Runs the promoted PGOLF-shaped first-model proof lane and verifies that the
/// emitted checkpoint matches the promoted descriptor exactly.
pub fn run_parameter_golf_promoted_reference_run(
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
) -> Result<ParameterGolfPromotedReferenceRun, ParameterGolfReferenceTrainingError> {
    run_parameter_golf_promoted_reference_run_with_resume_policy(fixture, config, true)
}

/// Runs the promoted PGOLF-shaped bounded XTRAIN lane and preserves the resume
/// proof as a retained signal without requiring exact parity. This keeps the
/// train-to-infer handoff usable when the stronger reference config no longer
/// fits the original proof-lane exact replay claim.
pub fn run_parameter_golf_promoted_xtrain_reference_run(
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
) -> Result<ParameterGolfPromotedReferenceRun, ParameterGolfReferenceTrainingError> {
    run_parameter_golf_promoted_reference_run_with_resume_policy(fixture, config, false)
}

fn run_parameter_golf_promoted_reference_run_with_resume_policy(
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
    strict_resume_parity: bool,
) -> Result<ParameterGolfPromotedReferenceRun, ParameterGolfReferenceTrainingError> {
    let profile_contract = parameter_golf_promoted_profile_contract(config.promoted_profile.kind);
    let mut training_outcome = train_parameter_golf_local_reference(fixture, config)?;
    training_outcome.final_checkpoint.manifest = normalize_json_roundtrip(
        &training_outcome.final_checkpoint.manifest,
        "parameter golf promoted final checkpoint manifest normalization",
    )?;
    training_outcome.final_checkpoint.manifest_artifact = ParameterGolfTrainingArtifact::new(
        training_outcome
            .final_checkpoint
            .manifest_artifact
            .artifact_kind
            .clone(),
        training_outcome
            .final_checkpoint
            .manifest_artifact
            .artifact_ref
            .clone(),
        json_bytes_pretty(
            &training_outcome.final_checkpoint.manifest,
            "parameter golf promoted final checkpoint manifest export",
        )?,
    );
    training_outcome.summary.final_checkpoint_manifest_digest =
        training_outcome.final_checkpoint.manifest.stable_digest();
    validate_promoted_reference_descriptor(
        &profile_contract,
        training_outcome.initial_model.descriptor(),
    )?;
    validate_promoted_reference_descriptor(
        &profile_contract,
        training_outcome.trained_model.descriptor(),
    )?;

    let model_descriptor = training_outcome.trained_model.descriptor().clone();
    let checkpoint_surface_report = promoted_checkpoint_surface_report(
        &profile_contract,
        &model_descriptor,
        &training_outcome.final_checkpoint.weights_artifact,
    )?;
    if !checkpoint_surface_report.exact_match {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted checkpoint surface",
            message: format!(
                "emitted checkpoint drifted from promoted descriptor: missing={:?} unexpected={:?} shape_mismatches={:?}",
                checkpoint_surface_report.missing_tensors,
                checkpoint_surface_report.unexpected_tensors,
                checkpoint_surface_report.shape_mismatches
            ),
        });
    }

    let resume_proof = promoted_resume_proof(
        fixture,
        &profile_contract,
        &training_outcome,
        strict_resume_parity,
    )?;
    let tokenizer_asset = build_parameter_golf_promoted_tokenizer_asset(fixture, config)?;
    let generation_config =
        build_parameter_golf_promoted_generation_config(config, &model_descriptor)?;
    let summary = ParameterGolfPromotedReferenceRunSummary {
        schema_version: String::from("psionic.parameter_golf_promoted_reference_run.v1"),
        profile_id: profile_contract.profile_id.clone(),
        profile_kind: profile_kind_label(config.promoted_profile.kind),
        run_id: config.run_id.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        descriptor_digest: model_descriptor.stable_digest(),
        training_dataset_digest: fixture.training_digest(),
        validation_dataset_digest: fixture.validation_digest(),
        initial_checkpoint_ref: training_outcome
            .initial_checkpoint
            .manifest
            .checkpoint_ref
            .clone(),
        final_checkpoint_ref: training_outcome
            .final_checkpoint
            .manifest
            .checkpoint_ref
            .clone(),
        final_checkpoint_manifest_digest: training_outcome
            .final_checkpoint
            .manifest
            .stable_digest(),
        checkpoint_surface_report_digest: checkpoint_surface_report.report_digest.clone(),
        resume_proof_digest: resume_proof.proof_digest.clone(),
        training_summary: training_outcome.summary.clone(),
        detail: if strict_resume_parity {
            String::from(
                "Promoted PGOLF first-model proof run trained the full parameter_golf_decoder baseline, verified that the emitted checkpoint surface exactly matched the promoted descriptor, and proved restore-plus-resume parity from the emitted checkpoint lineage.",
            )
        } else {
            String::from(
                "Promoted PGOLF bounded XTRAIN run trained the full parameter_golf_decoder baseline under a stronger local-reference budget, verified that the emitted checkpoint surface exactly matched the promoted descriptor, and retained restore-plus-resume evidence without treating exact replay parity as a blocking claim.",
            )
        },
    };
    let mut run = ParameterGolfPromotedReferenceRun {
        profile_contract,
        training_config: config.clone(),
        model_descriptor,
        training_outcome,
        checkpoint_surface_report,
        resume_proof,
        tokenizer_asset,
        generation_config,
        bundle_manifest: ParameterGolfPromotedBundleManifest {
            schema_version: String::new(),
            bundle_id: String::new(),
            family_id: String::new(),
            model_family: String::new(),
            model_id: String::new(),
            model_revision: String::new(),
            profile_id: String::new(),
            profile_kind: String::new(),
            artifacts: ParameterGolfPromotedBundleArtifacts {
                descriptor: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                model: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                tokenizer_asset: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                generation_config: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                profile_contract: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                training_config: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                summary: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                checkpoint_manifest: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                checkpoint_surface_report: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
                resume_proof: ParameterGolfPromotedBundleArtifactRef {
                    relative_path: String::new(),
                    sha256: String::new(),
                },
            },
            lineage: ParameterGolfPromotedBundleLineage {
                run_id: String::new(),
                checkpoint_family: String::new(),
                final_checkpoint_ref: String::new(),
                final_checkpoint_manifest_digest: String::new(),
                checkpoint_artifact_digest: String::new(),
                descriptor_digest: String::new(),
                training_dataset_digest: String::new(),
                validation_dataset_digest: String::new(),
                profile_id: String::new(),
                profile_kind: String::new(),
                detail: String::new(),
            },
            detail: String::new(),
            bundle_digest: String::new(),
        },
        summary,
    };
    run.bundle_manifest = build_parameter_golf_promoted_bundle_manifest(&run)?;
    Ok(run)
}

/// Writes one promoted PGOLF first-model proof run into a self-contained output
/// directory.
pub fn write_parameter_golf_promoted_reference_run(
    run: &ParameterGolfPromotedReferenceRun,
    output_dir: &Path,
) -> Result<(), ParameterGolfReferenceTrainingError> {
    std::fs::create_dir_all(output_dir)?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_FILE)
            .as_path(),
        &run.bundle_manifest,
        "parameter golf promoted bundle manifest export",
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_DESCRIPTOR_FILE)
            .as_path(),
        &run.model_descriptor,
        "parameter golf promoted bundle descriptor export",
    )?;
    std::fs::write(
        output_dir.join(PARAMETER_GOLF_PROMOTED_BUNDLE_MODEL_FILE),
        run.training_outcome
            .final_checkpoint
            .weights_artifact
            .bytes
            .as_slice(),
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_TOKENIZER_FILE)
            .as_path(),
        &run.tokenizer_asset,
        "parameter golf promoted bundle tokenizer asset export",
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_GENERATION_CONFIG_FILE)
            .as_path(),
        &run.generation_config,
        "parameter golf promoted bundle generation config export",
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_PROFILE_CONTRACT_FILE)
            .as_path(),
        &run.profile_contract,
        "parameter golf promoted bundle profile contract export",
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_TRAINING_CONFIG_FILE)
            .as_path(),
        &run.training_config,
        "parameter golf promoted bundle training config export",
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_SUMMARY_FILE)
            .as_path(),
        &run.summary,
        "parameter golf promoted bundle summary export",
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_CHECKPOINT_MANIFEST_FILE)
            .as_path(),
        &run.training_outcome.final_checkpoint.manifest,
        "parameter golf promoted bundle checkpoint manifest export",
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_CHECKPOINT_SURFACE_REPORT_FILE)
            .as_path(),
        &run.checkpoint_surface_report,
        "parameter golf promoted bundle checkpoint surface report export",
    )?;
    write_json_file(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_RESUME_PROOF_FILE)
            .as_path(),
        &run.resume_proof,
        "parameter golf promoted bundle resume proof export",
    )?;
    write_json_file(
        output_dir
            .join("parameter_golf_promoted_profile_contract.json")
            .as_path(),
        &run.profile_contract,
        "parameter golf promoted profile contract export",
    )?;
    write_json_file(
        output_dir
            .join("parameter_golf_promoted_training_config.json")
            .as_path(),
        &run.training_config,
        "parameter golf promoted training config export",
    )?;
    write_json_file(
        output_dir
            .join("parameter_golf_promoted_model_descriptor.json")
            .as_path(),
        &run.model_descriptor,
        "parameter golf promoted model descriptor export",
    )?;
    write_json_file(
        output_dir
            .join("parameter_golf_promoted_checkpoint_surface_report.json")
            .as_path(),
        &run.checkpoint_surface_report,
        "parameter golf promoted checkpoint surface report export",
    )?;
    write_json_file(
        output_dir
            .join("parameter_golf_promoted_resume_proof.json")
            .as_path(),
        &run.resume_proof,
        "parameter golf promoted resume proof export",
    )?;
    write_json_file(
        output_dir
            .join("parameter_golf_promoted_summary.json")
            .as_path(),
        &run.summary,
        "parameter golf promoted summary export",
    )?;
    write_json_file(
        output_dir
            .join("parameter_golf_initial_checkpoint_manifest.json")
            .as_path(),
        &run.training_outcome.initial_checkpoint.manifest,
        "parameter golf initial checkpoint manifest export",
    )?;
    write_json_file(
        output_dir
            .join("parameter_golf_final_checkpoint_manifest.json")
            .as_path(),
        &run.training_outcome.final_checkpoint.manifest,
        "parameter golf final checkpoint manifest export",
    )?;
    std::fs::write(
        output_dir.join("parameter_golf_initial_checkpoint.safetensors"),
        run.training_outcome
            .initial_checkpoint
            .weights_artifact
            .bytes
            .as_slice(),
    )?;
    std::fs::write(
        output_dir.join("parameter_golf_final_checkpoint.safetensors"),
        run.training_outcome
            .final_checkpoint
            .weights_artifact
            .bytes
            .as_slice(),
    )?;
    std::fs::write(
        output_dir.join("parameter_golf_final_model_int8_zlib.st"),
        run.training_outcome
            .int8_zlib_model_artifact
            .bytes
            .as_slice(),
    )?;
    check_parameter_golf_promoted_bundle(output_dir)?;
    Ok(())
}

fn write_json_file<T: Serialize>(
    path: &Path,
    value: &T,
    context: &'static str,
) -> Result<(), ParameterGolfReferenceTrainingError> {
    let bytes = json_bytes_pretty(value, context)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

fn json_bytes_pretty<T: Serialize>(
    value: &T,
    context: &'static str,
) -> Result<Vec<u8>, ParameterGolfReferenceTrainingError> {
    serde_json::to_vec_pretty(value).map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context,
            message: error.to_string(),
        }
    })
}

fn normalize_json_roundtrip<T: Serialize + DeserializeOwned>(
    value: &T,
    context: &'static str,
) -> Result<T, ParameterGolfReferenceTrainingError> {
    let bytes = json_bytes_pretty(value, context)?;
    serde_json::from_slice(bytes.as_slice()).map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context,
            message: error.to_string(),
        }
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn profile_kind_label(kind: ParameterGolfPromotedProfileKind) -> String {
    match kind {
        ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder => {
            String::from("general_psion_small_decoder")
        }
        ParameterGolfPromotedProfileKind::StrictPgolfChallenge => {
            String::from("strict_pgolf_challenge")
        }
    }
}

fn map_promoted_token_kind(
    kind: ParameterGolfSentencePieceTokenKind,
) -> ParameterGolfPromotedTokenizerTokenKind {
    match kind {
        ParameterGolfSentencePieceTokenKind::Normal => {
            ParameterGolfPromotedTokenizerTokenKind::Normal
        }
        ParameterGolfSentencePieceTokenKind::Byte => ParameterGolfPromotedTokenizerTokenKind::Byte,
        ParameterGolfSentencePieceTokenKind::Control => {
            ParameterGolfPromotedTokenizerTokenKind::Control
        }
        ParameterGolfSentencePieceTokenKind::Unknown => {
            ParameterGolfPromotedTokenizerTokenKind::Unknown
        }
        ParameterGolfSentencePieceTokenKind::Unused => {
            ParameterGolfPromotedTokenizerTokenKind::Unused
        }
    }
}

fn build_parameter_golf_promoted_tokenizer_asset(
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
) -> Result<ParameterGolfPromotedTokenizerAsset, ParameterGolfReferenceTrainingError> {
    let mut pieces_by_id = fixture
        .sentencepiece_entries
        .iter()
        .map(|entry| {
            (
                entry.token_id as usize,
                ParameterGolfPromotedTokenizerToken {
                    token_id: entry.token_id,
                    piece: entry.piece.clone(),
                    kind: map_promoted_token_kind(entry.kind),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    pieces_by_id
        .entry(0)
        .or_insert_with(|| ParameterGolfPromotedTokenizerToken {
            token_id: 0,
            piece: String::from("<unk>"),
            kind: ParameterGolfPromotedTokenizerTokenKind::Unknown,
        });

    let pieces = (0..fixture.tokenizer_vocab_size)
        .map(|token_id| {
            pieces_by_id
                .remove(&token_id)
                .unwrap_or(ParameterGolfPromotedTokenizerToken {
                    token_id: token_id as u32,
                    piece: format!("<reserved_{token_id:04}>"),
                    kind: ParameterGolfPromotedTokenizerTokenKind::Unused,
                })
        })
        .collect::<Vec<_>>();

    let tokenizer_id = match config.promoted_profile.tokenizer_identity {
        ParameterGolfPromotedTokenizerIdentity::RepoLocalReferenceSentencePiece => {
            format!(
                "{}.local_reference_sentencepiece",
                config.promoted_profile.profile_id
            )
        }
        ParameterGolfPromotedTokenizerIdentity::ChallengeSp1024SentencePiece => {
            format!(
                "{}.challenge_sp1024_sentencepiece",
                config.promoted_profile.profile_id
            )
        }
    };
    let mut asset = ParameterGolfPromotedTokenizerAsset {
        schema_version: String::from(PARAMETER_GOLF_PROMOTED_TOKENIZER_ASSET_SCHEMA_VERSION),
        profile_id: config.promoted_profile.profile_id.clone(),
        tokenizer_id,
        tokenizer_version: String::from("repo-2026-03-26"),
        family: ParameterGolfPromotedTokenizerFamily::SentencePiece,
        asset_format: ParameterGolfPromotedTokenizerAssetFormat::SentencePiecePieceTableJson,
        vocab_size: fixture.tokenizer_vocab_size as u32,
        add_bos: false,
        add_eos: false,
        bos_token_id: None,
        eos_token_ids: Vec::new(),
        pad_token_id: None,
        unknown_token_id: Some(0),
        pieces,
        tokenizer_digest: String::new(),
        asset_digest: String::new(),
        detail: String::from(
            "Runtime-loadable SentencePiece-style piece table for the promoted PGOLF-shaped local-reference bundle.",
        ),
    };
    asset.tokenizer_digest = asset.tokenizer_contract_digest();
    asset.asset_digest = asset.stable_digest();
    asset
        .validate()
        .map_err(|error| ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted tokenizer asset",
            message: error.to_string(),
        })?;
    Ok(asset)
}

fn build_parameter_golf_promoted_generation_config(
    config: &ParameterGolfReferenceTrainingConfig,
    descriptor: &ParameterGolfModelDescriptor,
) -> Result<ParameterGolfPromotedGenerationConfig, ParameterGolfReferenceTrainingError> {
    let mut generation_config = ParameterGolfPromotedGenerationConfig {
        schema_version: String::from(PARAMETER_GOLF_PROMOTED_GENERATION_CONFIG_SCHEMA_VERSION),
        profile_id: config.promoted_profile.profile_id.clone(),
        prompt_format: String::from("plain_text"),
        max_context: descriptor.config.max_context,
        default_max_new_tokens: descriptor.config.max_context.min(32),
        default_sampling_mode: String::from("greedy"),
        default_temperature: 0.0,
        default_top_k: None,
        bounded_attention_window_tokens: config.inference_attention_window_tokens,
        stop_on_eos: false,
        config_digest: String::new(),
        detail: match config.inference_attention_window_tokens {
            Some(window_tokens) => format!(
                "Default CPU-first generation posture for the promoted PGOLF-shaped local-reference bundle with one bounded trailing attention window of {window_tokens} tokens."
            ),
            None => String::from(
                "Default CPU-first generation posture for the promoted PGOLF-shaped local-reference bundle.",
            ),
        },
    };
    generation_config.config_digest = generation_config.stable_digest();
    generation_config.validate().map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted generation config",
            message: error.to_string(),
        }
    })?;
    Ok(generation_config)
}

fn build_bundle_artifact_ref(
    relative_path: &str,
    bytes: &[u8],
) -> ParameterGolfPromotedBundleArtifactRef {
    ParameterGolfPromotedBundleArtifactRef {
        relative_path: String::from(relative_path),
        sha256: sha256_hex(bytes),
    }
}

fn build_parameter_golf_promoted_bundle_manifest(
    run: &ParameterGolfPromotedReferenceRun,
) -> Result<ParameterGolfPromotedBundleManifest, ParameterGolfReferenceTrainingError> {
    let descriptor_bytes = json_bytes_pretty(
        &run.model_descriptor,
        "parameter golf promoted bundle descriptor export",
    )?;
    let tokenizer_asset_bytes = json_bytes_pretty(
        &run.tokenizer_asset,
        "parameter golf promoted bundle tokenizer export",
    )?;
    let generation_config_bytes = json_bytes_pretty(
        &run.generation_config,
        "parameter golf promoted bundle generation config export",
    )?;
    let profile_contract_bytes = json_bytes_pretty(
        &run.profile_contract,
        "parameter golf promoted bundle profile contract export",
    )?;
    let training_config_bytes = json_bytes_pretty(
        &run.training_config,
        "parameter golf promoted bundle training config export",
    )?;
    let summary_bytes = json_bytes_pretty(
        &run.summary,
        "parameter golf promoted bundle summary export",
    )?;
    let checkpoint_manifest_bytes = json_bytes_pretty(
        &run.training_outcome.final_checkpoint.manifest,
        "parameter golf promoted bundle checkpoint manifest export",
    )?;
    let checkpoint_surface_report_bytes = json_bytes_pretty(
        &run.checkpoint_surface_report,
        "parameter golf promoted bundle checkpoint surface report export",
    )?;
    let resume_proof_bytes = json_bytes_pretty(
        &run.resume_proof,
        "parameter golf promoted bundle resume proof export",
    )?;

    let profile_kind = profile_kind_label(run.profile_contract.kind);
    let mut manifest = ParameterGolfPromotedBundleManifest {
        schema_version: String::from(PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_SCHEMA_VERSION),
        bundle_id: format!(
            "{}:{}",
            run.profile_contract.profile_id, run.summary.final_checkpoint_ref
        ),
        family_id: run.profile_contract.family_id.clone(),
        model_family: run.model_descriptor.model.family.clone(),
        model_id: run.model_descriptor.model.model_id.clone(),
        model_revision: run.model_descriptor.model.revision.clone(),
        profile_id: run.profile_contract.profile_id.clone(),
        profile_kind: profile_kind.clone(),
        artifacts: ParameterGolfPromotedBundleArtifacts {
            descriptor: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_DESCRIPTOR_FILE,
                descriptor_bytes.as_slice(),
            ),
            model: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_MODEL_FILE,
                run.training_outcome
                    .final_checkpoint
                    .weights_artifact
                    .bytes
                    .as_slice(),
            ),
            tokenizer_asset: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_TOKENIZER_FILE,
                tokenizer_asset_bytes.as_slice(),
            ),
            generation_config: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_GENERATION_CONFIG_FILE,
                generation_config_bytes.as_slice(),
            ),
            profile_contract: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_PROFILE_CONTRACT_FILE,
                profile_contract_bytes.as_slice(),
            ),
            training_config: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_TRAINING_CONFIG_FILE,
                training_config_bytes.as_slice(),
            ),
            summary: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_SUMMARY_FILE,
                summary_bytes.as_slice(),
            ),
            checkpoint_manifest: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_CHECKPOINT_MANIFEST_FILE,
                checkpoint_manifest_bytes.as_slice(),
            ),
            checkpoint_surface_report: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_CHECKPOINT_SURFACE_REPORT_FILE,
                checkpoint_surface_report_bytes.as_slice(),
            ),
            resume_proof: build_bundle_artifact_ref(
                PARAMETER_GOLF_PROMOTED_BUNDLE_RESUME_PROOF_FILE,
                resume_proof_bytes.as_slice(),
            ),
        },
        lineage: ParameterGolfPromotedBundleLineage {
            run_id: run.summary.run_id.clone(),
            checkpoint_family: run.summary.checkpoint_family.clone(),
            final_checkpoint_ref: run.summary.final_checkpoint_ref.clone(),
            final_checkpoint_manifest_digest: run.summary.final_checkpoint_manifest_digest.clone(),
            checkpoint_artifact_digest: run
                .training_outcome
                .final_checkpoint
                .weights_artifact
                .artifact_digest
                .clone(),
            descriptor_digest: run.summary.descriptor_digest.clone(),
            training_dataset_digest: run.summary.training_dataset_digest.clone(),
            validation_dataset_digest: run.summary.validation_dataset_digest.clone(),
            profile_id: run.summary.profile_id.clone(),
            profile_kind,
            detail: String::from(
                "This promoted bundle is the exact runtime handoff from the bounded PGOLF-shaped proof run into later load, prompt, and serve work.",
            ),
        },
        detail: String::from(
            "Self-contained promoted PGOLF-shaped model bundle carrying descriptor, weights, runtime tokenizer asset, generation defaults, and proof-lineage metadata.",
        ),
        bundle_digest: String::new(),
    };
    manifest.bundle_digest = manifest.stable_digest();
    manifest
        .validate()
        .map_err(|error| ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle manifest",
            message: error.to_string(),
        })?;
    Ok(manifest)
}

pub fn check_parameter_golf_promoted_bundle(
    output_dir: &Path,
) -> Result<ParameterGolfPromotedBundleManifest, ParameterGolfReferenceTrainingError> {
    let manifest_bytes = std::fs::read(
        output_dir
            .join(PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_FILE)
            .as_path(),
    )?;
    let manifest: ParameterGolfPromotedBundleManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle manifest import",
            message: error.to_string(),
        })?;
    manifest
        .validate()
        .map_err(|error| ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle manifest import",
            message: error.to_string(),
        })?;

    let load_bytes = |artifact: &ParameterGolfPromotedBundleArtifactRef,
                      context: &'static str|
     -> Result<Vec<u8>, ParameterGolfReferenceTrainingError> {
        let bytes = std::fs::read(output_dir.join(artifact.relative_path.as_str()).as_path())?;
        let actual_sha256 = sha256_hex(bytes.as_slice());
        if actual_sha256 != artifact.sha256 {
            return Err(ParameterGolfReferenceTrainingError::Serialization {
                context,
                message: format!(
                    "artifact `{}` SHA-256 drifted: expected `{}`, found `{actual_sha256}`",
                    artifact.relative_path, artifact.sha256
                ),
            });
        }
        Ok(bytes)
    };

    let descriptor_bytes = load_bytes(
        &manifest.artifacts.descriptor,
        "parameter golf promoted bundle descriptor",
    )?;
    let model_bytes = load_bytes(
        &manifest.artifacts.model,
        "parameter golf promoted bundle model",
    )?;
    let tokenizer_asset_bytes = load_bytes(
        &manifest.artifacts.tokenizer_asset,
        "parameter golf promoted bundle tokenizer asset",
    )?;
    let generation_config_bytes = load_bytes(
        &manifest.artifacts.generation_config,
        "parameter golf promoted bundle generation config",
    )?;
    let profile_contract_bytes = load_bytes(
        &manifest.artifacts.profile_contract,
        "parameter golf promoted bundle profile contract",
    )?;
    let training_config_bytes = load_bytes(
        &manifest.artifacts.training_config,
        "parameter golf promoted bundle training config",
    )?;
    let summary_bytes = load_bytes(
        &manifest.artifacts.summary,
        "parameter golf promoted bundle summary",
    )?;
    let checkpoint_manifest_bytes = load_bytes(
        &manifest.artifacts.checkpoint_manifest,
        "parameter golf promoted bundle checkpoint manifest",
    )?;
    let checkpoint_surface_report_bytes = load_bytes(
        &manifest.artifacts.checkpoint_surface_report,
        "parameter golf promoted bundle checkpoint surface report",
    )?;
    let resume_proof_bytes = load_bytes(
        &manifest.artifacts.resume_proof,
        "parameter golf promoted bundle resume proof",
    )?;

    let descriptor: ParameterGolfModelDescriptor =
        serde_json::from_slice(descriptor_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle descriptor import",
                message: error.to_string(),
            }
        })?;
    let tokenizer_asset: ParameterGolfPromotedTokenizerAsset =
        serde_json::from_slice(tokenizer_asset_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle tokenizer asset import",
                message: error.to_string(),
            }
        })?;
    tokenizer_asset.validate().map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle tokenizer asset import",
            message: error.to_string(),
        }
    })?;
    let generation_config: ParameterGolfPromotedGenerationConfig =
        serde_json::from_slice(generation_config_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle generation config import",
                message: error.to_string(),
            }
        })?;
    generation_config.validate().map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle generation config import",
            message: error.to_string(),
        }
    })?;
    let profile_contract: ParameterGolfPromotedProfileContract =
        serde_json::from_slice(profile_contract_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle profile contract import",
                message: error.to_string(),
            }
        })?;
    let training_config: ParameterGolfReferenceTrainingConfig =
        serde_json::from_slice(training_config_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle training config import",
                message: error.to_string(),
            }
        })?;
    let summary: ParameterGolfPromotedReferenceRunSummary =
        serde_json::from_slice(summary_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle summary import",
                message: error.to_string(),
            }
        })?;
    let checkpoint_manifest: ParameterGolfCheckpointManifest =
        serde_json::from_slice(checkpoint_manifest_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle checkpoint manifest import",
                message: error.to_string(),
            }
        })?;
    let checkpoint_surface_report: ParameterGolfPromotedCheckpointSurfaceReport =
        serde_json::from_slice(checkpoint_surface_report_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle checkpoint surface report import",
                message: error.to_string(),
            }
        })?;
    let resume_proof: ParameterGolfPromotedResumeProof =
        serde_json::from_slice(resume_proof_bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted bundle resume proof import",
                message: error.to_string(),
            }
        })?;

    if descriptor.stable_digest() != manifest.lineage.descriptor_digest {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle descriptor import",
            message: String::from("descriptor digest drifted from bundle lineage"),
        });
    }
    if tokenizer_asset.profile_id != manifest.profile_id {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle tokenizer asset import",
            message: String::from("tokenizer asset profile id drifted from bundle manifest"),
        });
    }
    if generation_config.profile_id != manifest.profile_id {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle generation config import",
            message: String::from("generation config profile id drifted from bundle manifest"),
        });
    }
    if profile_contract.profile_id != manifest.profile_id
        || training_config.promoted_profile.profile_id != manifest.profile_id
        || summary.profile_id != manifest.profile_id
        || checkpoint_manifest.promoted_profile.profile_id != manifest.profile_id
        || checkpoint_surface_report.profile_id != manifest.profile_id
        || resume_proof.profile_id != manifest.profile_id
    {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle profile alignment",
            message: String::from("bundle profile identity drifted across emitted artifacts"),
        });
    }
    if summary.final_checkpoint_ref != manifest.lineage.final_checkpoint_ref
        || checkpoint_manifest.checkpoint_ref != manifest.lineage.final_checkpoint_ref
    {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle checkpoint alignment",
            message: String::from("final checkpoint ref drifted across emitted artifacts"),
        });
    }
    if checkpoint_manifest.stable_digest() != manifest.lineage.final_checkpoint_manifest_digest
        || summary.final_checkpoint_manifest_digest
            != manifest.lineage.final_checkpoint_manifest_digest
    {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle checkpoint alignment",
            message: String::from(
                "final checkpoint manifest digest drifted across emitted artifacts",
            ),
        });
    }
    if summary.training_dataset_digest != manifest.lineage.training_dataset_digest
        || summary.validation_dataset_digest != manifest.lineage.validation_dataset_digest
        || checkpoint_manifest.training_dataset_digest != manifest.lineage.training_dataset_digest
        || checkpoint_manifest.validation_dataset_digest
            != manifest.lineage.validation_dataset_digest
    {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle lineage alignment",
            message: String::from("dataset lineage drifted across emitted artifacts"),
        });
    }
    let recomputed_surface_report = promoted_checkpoint_surface_report(
        &profile_contract,
        &descriptor,
        &ParameterGolfTrainingArtifact::new(
            "parameter_golf_model_safetensors",
            manifest.artifacts.model.relative_path.clone(),
            model_bytes,
        ),
    )?;
    if recomputed_surface_report.expected_tensors != checkpoint_surface_report.expected_tensors
        || recomputed_surface_report.actual_tensors != checkpoint_surface_report.actual_tensors
        || recomputed_surface_report.missing_tensors != checkpoint_surface_report.missing_tensors
        || recomputed_surface_report.unexpected_tensors
            != checkpoint_surface_report.unexpected_tensors
        || recomputed_surface_report.shape_mismatches != checkpoint_surface_report.shape_mismatches
        || recomputed_surface_report.exact_name_match != checkpoint_surface_report.exact_name_match
        || recomputed_surface_report.exact_shape_match
            != checkpoint_surface_report.exact_shape_match
        || recomputed_surface_report.exact_match != checkpoint_surface_report.exact_match
        || recomputed_surface_report.detail != checkpoint_surface_report.detail
    {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted bundle checkpoint surface report",
            message: String::from("checkpoint surface report drifted from emitted model bytes"),
        });
    }
    Ok(manifest)
}

fn validate_promoted_reference_descriptor(
    profile_contract: &ParameterGolfPromotedProfileContract,
    descriptor: &ParameterGolfModelDescriptor,
) -> Result<(), ParameterGolfReferenceTrainingError> {
    if descriptor.model.model_id != profile_contract.baseline_model_id {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted reference descriptor",
            message: format!(
                "baseline model id drifted: descriptor `{}` vs promoted `{}`",
                descriptor.model.model_id, profile_contract.baseline_model_id
            ),
        });
    }
    if descriptor.model.revision != profile_contract.baseline_revision {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted reference descriptor",
            message: format!(
                "baseline revision drifted: descriptor `{}` vs promoted `{}`",
                descriptor.model.revision, profile_contract.baseline_revision
            ),
        });
    }
    if descriptor.config != profile_contract.baseline_config {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted reference descriptor",
            message: String::from("baseline config drifted from the promoted family contract"),
        });
    }
    Ok(())
}

fn promoted_checkpoint_surface_report(
    profile_contract: &ParameterGolfPromotedProfileContract,
    descriptor: &ParameterGolfModelDescriptor,
    checkpoint_artifact: &ParameterGolfTrainingArtifact,
) -> Result<ParameterGolfPromotedCheckpointSurfaceReport, ParameterGolfReferenceTrainingError> {
    let safetensors =
        SafeTensors::deserialize(checkpoint_artifact.bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted checkpoint surface",
                message: error.to_string(),
            }
        })?;
    let mut expected_tensors = descriptor
        .weights
        .tensors
        .iter()
        .map(|tensor| ParameterGolfCheckpointSurfaceTensorEntry {
            name: tensor.name.clone(),
            shape: tensor.shape.dims().to_vec(),
        })
        .collect::<Vec<_>>();
    expected_tensors.sort_by(|left, right| left.name.cmp(&right.name));

    let mut actual_tensors = Vec::new();
    for name in safetensors.names() {
        let tensor = safetensors.tensor(name).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf promoted checkpoint surface",
                message: error.to_string(),
            }
        })?;
        actual_tensors.push(ParameterGolfCheckpointSurfaceTensorEntry {
            name: String::from(name),
            shape: tensor.shape().to_vec(),
        });
    }
    actual_tensors.sort_by(|left, right| left.name.cmp(&right.name));

    let expected_by_name = expected_tensors
        .iter()
        .map(|tensor| (tensor.name.as_str(), tensor.shape.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let actual_by_name = actual_tensors
        .iter()
        .map(|tensor| (tensor.name.as_str(), tensor.shape.as_slice()))
        .collect::<BTreeMap<_, _>>();

    let missing_tensors = expected_by_name
        .keys()
        .filter(|name| !actual_by_name.contains_key(**name))
        .map(|name| String::from(*name))
        .collect::<Vec<_>>();
    let unexpected_tensors = actual_by_name
        .keys()
        .filter(|name| !expected_by_name.contains_key(**name))
        .map(|name| String::from(*name))
        .collect::<Vec<_>>();
    let shape_mismatches = expected_tensors
        .iter()
        .filter_map(|expected| {
            let actual_shape = actual_by_name.get(expected.name.as_str())?;
            if *actual_shape == expected.shape.as_slice() {
                None
            } else {
                Some(ParameterGolfCheckpointSurfaceShapeMismatch {
                    name: expected.name.clone(),
                    expected: expected.shape.clone(),
                    actual: (*actual_shape).to_vec(),
                })
            }
        })
        .collect::<Vec<_>>();
    let exact_name_match = missing_tensors.is_empty() && unexpected_tensors.is_empty();
    let exact_shape_match = shape_mismatches.is_empty();
    let exact_match = exact_name_match && exact_shape_match;
    let detail = if exact_match {
        String::from(
            "Emitted checkpoint tensor names and shapes exactly match the promoted PGOLF descriptor layout.",
        )
    } else {
        String::from(
            "Emitted checkpoint tensor surface drifted from the promoted PGOLF descriptor and is not claimable for first-model inference.",
        )
    };
    let mut report = ParameterGolfPromotedCheckpointSurfaceReport {
        profile_id: profile_contract.profile_id.clone(),
        descriptor_digest: descriptor.stable_digest(),
        checkpoint_artifact_digest: checkpoint_artifact.artifact_digest.clone(),
        expected_tensors,
        actual_tensors,
        missing_tensors,
        unexpected_tensors,
        shape_mismatches,
        exact_name_match,
        exact_shape_match,
        exact_match,
        detail,
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    Ok(report)
}

fn promoted_resume_proof(
    fixture: &ParameterGolfLocalReferenceFixture,
    profile_contract: &ParameterGolfPromotedProfileContract,
    outcome: &ParameterGolfReferenceTrainingOutcome,
    strict_exact_parity: bool,
) -> Result<ParameterGolfPromotedResumeProof, ParameterGolfReferenceTrainingError> {
    let mut restored =
        restore_parameter_golf_local_reference_checkpoint(fixture, &outcome.initial_checkpoint)?;
    while !restored.is_complete() {
        restored.step()?;
    }
    let resumed_outcome = restored.into_outcome()?;
    let continuous_final_manifest_digest = outcome.final_checkpoint.manifest.stable_digest();
    let resumed_final_manifest_digest = resumed_outcome.final_checkpoint.manifest.stable_digest();
    let exact_final_parity = resumed_outcome.trained_model == outcome.trained_model
        && resumed_outcome.final_validation_eval == outcome.final_validation_eval
        && resumed_final_manifest_digest == continuous_final_manifest_digest;
    if strict_exact_parity && !exact_final_parity {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf promoted resume proof",
            message: String::from(
                "restored-and-resumed proof run diverged from the continuous source run",
            ),
        });
    }
    let replayed_steps = outcome.step_metrics.len().saturating_sub(
        outcome
            .initial_checkpoint
            .manifest
            .step
            .try_into()
            .unwrap_or(usize::MAX),
    ) as u64;
    let mut proof = ParameterGolfPromotedResumeProof {
        profile_id: profile_contract.profile_id.clone(),
        restore_source_checkpoint_ref: outcome.initial_checkpoint.manifest.checkpoint_ref.clone(),
        restore_source_manifest_digest: outcome.initial_checkpoint.manifest.stable_digest(),
        continuous_final_checkpoint_ref: outcome.final_checkpoint.manifest.checkpoint_ref.clone(),
        continuous_final_manifest_digest,
        resumed_final_checkpoint_ref: resumed_outcome.final_checkpoint.manifest.checkpoint_ref,
        resumed_final_manifest_digest,
        replayed_steps,
        exact_final_parity,
        detail: if exact_final_parity {
            String::from(
                "Restoring the emitted promoted checkpoint and replaying the remaining bounded steps produced the same final checkpoint and validation state as the continuous source run.",
            )
        } else {
            String::from(
                "Restoring the emitted promoted checkpoint and replaying the bounded steps did not reproduce exact final parity. The stronger bounded XTRAIN lane retains that divergence as evidence and does not upgrade it into a replay-parity claim.",
            )
        },
        proof_digest: String::new(),
    };
    proof.proof_digest = proof.stable_digest();
    Ok(proof)
}

fn submit_checkpoint_async_writeback(
    worker: &AsyncCheckpointWritebackWorker,
    checkpoint: &ParameterGolfCheckpointArtifact,
    checkpoint_output_root: &Path,
    max_in_flight_writes: usize,
    pending_tickets: &mut VecDeque<AsyncCheckpointWritebackTicket>,
    receipts: &mut Vec<AsyncCheckpointWritebackReceipt>,
) -> Result<(), ParameterGolfReferenceTrainingError> {
    while pending_tickets.len() >= max_in_flight_writes {
        complete_oldest_checkpoint_async_writeback(pending_tickets, receipts)?;
    }

    let payload = checkpoint_async_writeback_payload(checkpoint, checkpoint_output_root)?;
    match worker.submit(payload) {
        Ok(ticket) => {
            pending_tickets.push_back(ticket);
            Ok(())
        }
        Err(AsyncCheckpointWritebackError::QueueFull { .. }) => {
            complete_oldest_checkpoint_async_writeback(pending_tickets, receipts)?;
            let payload = checkpoint_async_writeback_payload(checkpoint, checkpoint_output_root)?;
            let ticket = worker.submit(payload)?;
            pending_tickets.push_back(ticket);
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn drain_checkpoint_async_writeback_tickets(
    pending_tickets: &mut VecDeque<AsyncCheckpointWritebackTicket>,
    receipts: &mut Vec<AsyncCheckpointWritebackReceipt>,
) -> Result<(), ParameterGolfReferenceTrainingError> {
    while !pending_tickets.is_empty() {
        complete_oldest_checkpoint_async_writeback(pending_tickets, receipts)?;
    }
    Ok(())
}

fn complete_oldest_checkpoint_async_writeback(
    pending_tickets: &mut VecDeque<AsyncCheckpointWritebackTicket>,
    receipts: &mut Vec<AsyncCheckpointWritebackReceipt>,
) -> Result<(), ParameterGolfReferenceTrainingError> {
    let ticket = pending_tickets.pop_front().ok_or_else(|| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf async checkpoint writeback",
            message: String::from("bounded queue saturated without an in-flight ticket"),
        }
    })?;
    receipts.push(ticket.wait()?);
    Ok(())
}

fn checkpoint_async_writeback_payload(
    checkpoint: &ParameterGolfCheckpointArtifact,
    checkpoint_output_root: &Path,
) -> Result<AsyncCheckpointWritebackPayload, ParameterGolfReferenceTrainingError> {
    let step = checkpoint.manifest.step;
    let checkpoint_ref = checkpoint
        .checkpoint
        .checkpoint_ref
        .clone()
        .unwrap_or_else(|| checkpoint.manifest.checkpoint_ref.clone());
    AsyncCheckpointWritebackPayload::new(
        format!("{}-step-{step:05}", checkpoint.manifest.run_id),
        checkpoint_ref,
        checkpoint.checkpoint.checkpoint_family.clone(),
        checkpoint
            .checkpoint
            .stream_id
            .strip_suffix("/checkpoint_model.safetensors")
            .map_or_else(
                || {
                    checkpoint_output_root
                        .join(checkpoint.manifest.run_id.as_str())
                        .join(format!("step-{step:05}"))
                },
                |stream_prefix| checkpoint_output_root.join(stream_prefix),
            ),
        vec![
            AsyncCheckpointWritebackFile::new(
                "checkpoint_model.safetensors",
                checkpoint.weights_artifact.artifact_digest.clone(),
                checkpoint.weights_artifact.bytes.clone(),
            )?,
            AsyncCheckpointWritebackFile::new(
                "checkpoint_manifest.json",
                checkpoint.manifest_artifact.artifact_digest.clone(),
                checkpoint.manifest_artifact.bytes.clone(),
            )?,
        ],
    )
    .map_err(Into::into)
}

fn emit_parameter_golf_step_metrics(
    metric_sink: &mut LocalTrainMetricFanout,
    run_id: &str,
    step_metrics: &[ParameterGolfReferenceTrainingStepMetrics],
) -> Result<(), ParameterGolfReferenceTrainingError> {
    let Some(step_metrics) = step_metrics.last() else {
        return Ok(());
    };
    let step = step_metrics.global_step;
    metric_sink.record(LocalTrainMetricEvent::new(
        run_id,
        LocalTrainMetricPhase::Train,
        step,
        "mean_microbatch_loss",
        LocalTrainMetricValue::F32(step_metrics.mean_microbatch_loss),
    ))?;
    metric_sink.flush()?;
    metric_sink.record(LocalTrainMetricEvent::new(
        run_id,
        LocalTrainMetricPhase::Validation,
        step,
        "validation_mean_loss",
        LocalTrainMetricValue::F64(step_metrics.validation_mean_loss),
    ))?;
    metric_sink.record(LocalTrainMetricEvent::new(
        run_id,
        LocalTrainMetricPhase::Validation,
        step,
        "validation_bits_per_byte",
        LocalTrainMetricValue::F64(step_metrics.validation_bits_per_byte),
    ))?;
    metric_sink.flush()?;
    Ok(())
}

fn merge_checkpoint_writeback_receipts(
    completed: Vec<AsyncCheckpointWritebackReceipt>,
    shutdown: Vec<AsyncCheckpointWritebackReceipt>,
) -> Vec<AsyncCheckpointWritebackReceipt> {
    let mut seen = BTreeSet::new();
    completed
        .into_iter()
        .chain(shutdown)
        .filter(|receipt| seen.insert(receipt.write_id.clone()))
        .collect()
}

#[cfg(test)]
fn read_parameter_golf_checkpoint_from_directory(
    directory: &Path,
    checkpoint: &TrainingCheckpointReference,
) -> Result<ParameterGolfCheckpointArtifact, ParameterGolfReferenceTrainingError> {
    let weights_path = directory.join("checkpoint_model.safetensors");
    let manifest_path = directory.join("checkpoint_manifest.json");
    let weights_bytes = std::fs::read(weights_path.as_path())?;
    let manifest_bytes = std::fs::read(manifest_path.as_path())?;
    let manifest =
        serde_json::from_slice::<ParameterGolfCheckpointManifest>(manifest_bytes.as_slice())
            .map_err(|error| ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf checkpoint manifest import",
                message: error.to_string(),
            })?;
    if manifest.checkpoint_family != checkpoint.checkpoint_family {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf checkpoint import",
            message: format!(
                "checkpoint family mismatch: manifest `{}` vs runtime `{}`",
                manifest.checkpoint_family, checkpoint.checkpoint_family
            ),
        });
    }
    if checkpoint.checkpoint_ref.as_ref() != Some(&manifest.checkpoint_ref) {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf checkpoint import",
            message: format!(
                "checkpoint ref mismatch: manifest `{}` vs runtime `{:?}`",
                manifest.checkpoint_ref, checkpoint.checkpoint_ref
            ),
        });
    }
    if checkpoint.step != Some(manifest.step) {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf checkpoint import",
            message: format!(
                "checkpoint step mismatch: manifest `{}` vs runtime `{:?}`",
                manifest.step, checkpoint.step
            ),
        });
    }
    let weights_artifact = ParameterGolfTrainingArtifact::new(
        "parameter_golf_model_safetensors",
        format!(
            "{}/step-{:05}/checkpoint_model.safetensors",
            manifest.run_id, manifest.step
        ),
        weights_bytes,
    );
    let manifest_artifact = ParameterGolfTrainingArtifact::new(
        "parameter_golf_checkpoint_manifest",
        format!(
            "{}/step-{:05}/checkpoint_manifest.json",
            manifest.run_id, manifest.step
        ),
        manifest_bytes,
    );
    Ok(ParameterGolfCheckpointArtifact {
        weights_artifact,
        manifest_artifact,
        manifest,
        checkpoint: checkpoint.clone(),
    })
}

/// Restores one local-reference runner from a persisted checkpoint.
pub fn restore_parameter_golf_local_reference_checkpoint(
    fixture: &ParameterGolfLocalReferenceFixture,
    checkpoint: &ParameterGolfCheckpointArtifact,
) -> Result<ParameterGolfReferenceTrainingRunner, ParameterGolfReferenceTrainingError> {
    fixture.validate()?;
    let manifest = &checkpoint.manifest;
    let config = ParameterGolfReferenceTrainingConfig {
        run_id: manifest.run_id.clone(),
        checkpoint_family: manifest.checkpoint_family.clone(),
        started_at_ms: manifest.started_at_ms,
        step_duration_ms: manifest.step_duration_ms,
        max_steps: manifest.max_steps,
        geometry: manifest.geometry.clone(),
        hyperparameters: manifest.hyperparameters.clone(),
        finite_difference_epsilon: manifest.finite_difference_epsilon,
        promoted_profile: manifest.promoted_profile.clone(),
        inference_attention_window_tokens: None,
        selected_coordinates: manifest
            .trainable_tensors
            .iter()
            .flat_map(|tensor| {
                tensor
                    .selected_indices
                    .iter()
                    .map(|flat_index| ParameterGolfTrainableCoordinate {
                        parameter_id: tensor.parameter_id.clone(),
                        flat_index: *flat_index,
                    })
                    .collect::<Vec<_>>()
            })
            .collect(),
    };
    config.validate()?;
    if fixture.training_digest() != manifest.training_dataset_digest {
        return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
            message: String::from("training fixture digest does not match checkpoint manifest"),
        });
    }
    if fixture.validation_digest() != manifest.validation_dataset_digest {
        return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
            message: String::from("validation fixture digest does not match checkpoint manifest"),
        });
    }

    let initial_model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
    if initial_model.descriptor().stable_digest() != manifest.base_descriptor_digest {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf checkpoint restore",
            message: String::from("baseline descriptor digest does not match checkpoint manifest"),
        });
    }
    let optimizer_plan =
        parameter_golf_optimizer_plan(initial_model.descriptor(), &manifest.hyperparameters)?;
    let optimizer_plan_digest =
        stable_digest(b"psionic_parameter_golf_optimizer_plan|", &optimizer_plan);
    if optimizer_plan_digest != manifest.optimizer_plan_digest {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf checkpoint restore",
            message: String::from("optimizer plan digest does not match checkpoint manifest"),
        });
    }

    let current_model = restore_parameter_golf_model_from_safetensors(
        &initial_model,
        checkpoint.weights_artifact.bytes.as_slice(),
    )?;
    if current_model.descriptor().stable_digest() != manifest.current_descriptor_digest {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf checkpoint restore",
            message: String::from("current descriptor digest does not match checkpoint manifest"),
        });
    }

    let mut baseline_vectors = BTreeMap::new();
    let mut trainable_tensors = Vec::with_capacity(manifest.trainable_tensors.len());
    for tensor in &manifest.trainable_tensors {
        let baseline_vector = initial_model
            .weights()
            .parameter_vector(
                &initial_model.descriptor().config,
                tensor.parameter_id.as_str(),
            )
            .ok_or_else(
                || ParameterGolfReferenceTrainingError::MissingOptimizerGroup {
                    parameter_id: tensor.parameter_id.clone(),
                },
            )?;
        validate_selected_indices(
            tensor.parameter_id.as_str(),
            tensor.selected_indices.as_slice(),
            baseline_vector.values.len(),
        )?;
        baseline_vectors.insert(tensor.parameter_id.clone(), baseline_vector);
        let current_vector = current_model
            .weights()
            .parameter_vector(
                &current_model.descriptor().config,
                tensor.parameter_id.as_str(),
            )
            .ok_or_else(
                || ParameterGolfReferenceTrainingError::MissingArtifactTensor {
                    parameter_id: tensor.parameter_id.clone(),
                },
            )?;
        match (&tensor.execution, &tensor.optimizer_state) {
            (
                ParameterGolfOptimizerExecution::Adam { optimizer },
                ParameterGolfReferenceOptimizerState::Adam { state },
            ) => {
                let selected_values = tensor
                    .selected_indices
                    .iter()
                    .map(|flat_index| current_vector.values[*flat_index])
                    .collect::<Vec<_>>();
                trainable_tensors.push(TrainableTensorRuntime::AdamSparse {
                    parameter_id: tensor.parameter_id.clone(),
                    shape: tensor.shape.clone(),
                    selected_indices: tensor.selected_indices.clone(),
                    selected_values,
                    optimizer: optimizer.clone(),
                    optimizer_state: state.clone(),
                });
            }
            (
                ParameterGolfOptimizerExecution::Muon { optimizer },
                ParameterGolfReferenceOptimizerState::Muon { state },
            ) => {
                trainable_tensors.push(TrainableTensorRuntime::MuonDense {
                    parameter_id: tensor.parameter_id.clone(),
                    shape: tensor.shape.clone(),
                    selected_indices: tensor.selected_indices.clone(),
                    values: current_vector.values,
                    optimizer: optimizer.clone(),
                    optimizer_state: state.clone(),
                });
            }
            _ => {
                return Err(ParameterGolfReferenceTrainingError::Serialization {
                    context: "parameter golf checkpoint restore",
                    message: format!(
                        "optimizer execution/state mismatch for tensor `{}`",
                        tensor.parameter_id
                    ),
                });
            }
        }
    }

    let byte_luts = fixture.byte_luts()?;
    let initial_validation_eval = evaluate_parameter_golf_validation(
        &initial_model,
        fixture.validation_tokens.as_slice(),
        config.geometry.train_sequence_length,
        config.geometry.local_validation_batch_tokens(),
        &byte_luts,
    )?;
    let current_validation_eval = evaluate_parameter_golf_validation(
        &current_model,
        fixture.validation_tokens.as_slice(),
        config.geometry.train_sequence_length,
        config.geometry.local_validation_batch_tokens(),
        &byte_luts,
    )?;
    if current_validation_eval.stable_digest() != manifest.validation_eval_digest {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf checkpoint restore",
            message: String::from("validation eval digest does not match checkpoint manifest"),
        });
    }
    let initial_checkpoint = export_checkpoint(
        &initial_model,
        &initial_model,
        trainable_tensors.as_slice(),
        fixture,
        &config,
        0,
        &initial_validation_eval,
        &[],
        optimizer_plan_digest.as_str(),
        None,
    )?;
    Ok(ParameterGolfReferenceTrainingRunner {
        fixture: fixture.clone(),
        config,
        initial_model,
        current_model,
        byte_luts,
        baseline_vectors,
        trainable_tensors,
        optimizer_plan_digest,
        completed_steps: manifest.step,
        step_metrics: manifest.step_metrics.clone(),
        initial_checkpoint,
        latest_checkpoint: checkpoint.clone(),
        initial_validation_eval,
        current_validation_eval,
    })
}

/// Restores one exact raw full-precision model export.
pub fn restore_parameter_golf_model_from_safetensors(
    baseline_model: &ParameterGolfReferenceModel,
    weights_bytes: &[u8],
) -> Result<ParameterGolfReferenceModel, ParameterGolfReferenceTrainingError> {
    let (_, metadata) = SafeTensors::read_metadata(weights_bytes).map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf raw safetensors restore",
            message: error.to_string(),
        }
    })?;
    let safetensors = SafeTensors::deserialize(weights_bytes).map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf raw safetensors restore",
            message: error.to_string(),
        }
    })?;
    let weight_surface = metadata
        .metadata()
        .as_ref()
        .and_then(|metadata| metadata.get(PARAMETER_GOLF_WEIGHT_SURFACE_KEY))
        .map(String::as_str)
        .unwrap_or(PARAMETER_GOLF_SPLIT_WEIGHT_SURFACE);
    match weight_surface {
        PARAMETER_GOLF_SPLIT_WEIGHT_SURFACE => {
            restore_parameter_golf_model_from_split_safetensors(baseline_model, &safetensors)
        }
        PARAMETER_GOLF_BANKED_WEIGHT_SURFACE => {
            restore_parameter_golf_model_from_banked_safetensors_impl(baseline_model, &safetensors)
        }
        other => Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf raw safetensors restore",
            message: format!("unsupported parameter golf weight surface `{other}`"),
        }),
    }
}

/// Restores the explicit upstream-style banked runtime-weight surface when the
/// safetensors payload was exported in the banked score-path format.
pub fn restore_parameter_golf_banked_weights_from_safetensors(
    baseline_model: &ParameterGolfReferenceModel,
    weights_bytes: &[u8],
) -> Result<Option<ParameterGolfBankedWeights>, ParameterGolfReferenceTrainingError> {
    let (_, metadata) = SafeTensors::read_metadata(weights_bytes).map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf banked safetensors restore",
            message: error.to_string(),
        }
    })?;
    let safetensors = SafeTensors::deserialize(weights_bytes).map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf banked safetensors restore",
            message: error.to_string(),
        }
    })?;
    let weight_surface = metadata
        .metadata()
        .as_ref()
        .and_then(|metadata| metadata.get(PARAMETER_GOLF_WEIGHT_SURFACE_KEY))
        .map(String::as_str)
        .unwrap_or(PARAMETER_GOLF_SPLIT_WEIGHT_SURFACE);
    match weight_surface {
        PARAMETER_GOLF_SPLIT_WEIGHT_SURFACE => Ok(None),
        PARAMETER_GOLF_BANKED_WEIGHT_SURFACE => Ok(Some(
            restore_parameter_golf_banked_weights_from_banked_safetensors_impl(
                baseline_model,
                &safetensors,
            )?,
        )),
        other => Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf banked safetensors restore",
            message: format!("unsupported parameter golf weight surface `{other}`"),
        }),
    }
}

fn restore_parameter_golf_model_from_split_safetensors(
    baseline_model: &ParameterGolfReferenceModel,
    safetensors: &SafeTensors<'_>,
) -> Result<ParameterGolfReferenceModel, ParameterGolfReferenceTrainingError> {
    let mut overrides = BTreeMap::new();
    for parameter in baseline_model
        .weights()
        .parameter_vectors(&baseline_model.descriptor().config)
    {
        let tensor = safetensors
            .tensor(parameter.parameter_id.as_str())
            .map_err(
                |_| ParameterGolfReferenceTrainingError::MissingArtifactTensor {
                    parameter_id: parameter.parameter_id.clone(),
                },
            )?;
        validate_tensor_shape(
            parameter.parameter_id.as_str(),
            tensor.shape(),
            parameter.shape.dims(),
        )?;
        overrides.insert(
            parameter.parameter_id.clone(),
            decode_float_tensor(
                parameter.parameter_id.as_str(),
                tensor.dtype(),
                tensor.data(),
                tensor.shape(),
            )?,
        );
    }
    let weights = baseline_model
        .weights()
        .with_parameter_overrides(&baseline_model.descriptor().config, &overrides)?;
    Ok(ParameterGolfReferenceModel::new(
        baseline_model.descriptor().model.clone(),
        baseline_model.descriptor().config.clone(),
        weights,
    )?)
}

fn restore_parameter_golf_model_from_banked_safetensors_impl(
    baseline_model: &ParameterGolfReferenceModel,
    safetensors: &SafeTensors<'_>,
) -> Result<ParameterGolfReferenceModel, ParameterGolfReferenceTrainingError> {
    let weights = restore_parameter_golf_banked_weights_from_banked_safetensors_impl(
        baseline_model,
        safetensors,
    )?
    .to_split(&baseline_model.descriptor().config)?;
    Ok(ParameterGolfReferenceModel::new(
        baseline_model.descriptor().model.clone(),
        baseline_model.descriptor().config.clone(),
        weights,
    )?)
}

fn restore_parameter_golf_banked_weights_from_banked_safetensors_impl(
    baseline_model: &ParameterGolfReferenceModel,
    safetensors: &SafeTensors<'_>,
) -> Result<ParameterGolfBankedWeights, ParameterGolfReferenceTrainingError> {
    let config = &baseline_model.descriptor().config;
    let banked_weights = baseline_model.banked_weights()?;
    let mut overrides = BTreeMap::new();
    for parameter in banked_weights.parameter_vectors(config) {
        let tensor = safetensors
            .tensor(parameter.parameter_id.as_str())
            .map_err(
                |_| ParameterGolfReferenceTrainingError::MissingArtifactTensor {
                    parameter_id: parameter.parameter_id.clone(),
                },
            )?;
        validate_tensor_shape(
            parameter.parameter_id.as_str(),
            tensor.shape(),
            parameter.shape.dims(),
        )?;
        overrides.insert(
            parameter.parameter_id.clone(),
            decode_float_tensor(
                parameter.parameter_id.as_str(),
                tensor.dtype(),
                tensor.data(),
                tensor.shape(),
            )?,
        );
    }
    Ok(banked_weights.with_parameter_overrides(config, &overrides)?)
}

/// Restores one typed quantized PGOLF artifact back into the full-precision
/// reference family.
pub fn restore_parameter_golf_model_from_quantized_artifact(
    baseline_model: &ParameterGolfReferenceModel,
    artifact_bytes: &[u8],
) -> Result<ParameterGolfReferenceModel, ParameterGolfReferenceTrainingError> {
    let (detected_compression, raw_bytes) = decode_quantized_artifact_bytes(artifact_bytes)?;
    let metadata = read_safetensors_metadata(raw_bytes.as_slice())?;
    let safetensors = SafeTensors::deserialize(raw_bytes.as_slice()).map_err(|error| {
        ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf quantized artifact restore",
            message: error.to_string(),
        }
    })?;
    let artifact_config =
        quantized_artifact_config_from_metadata(metadata.as_ref(), detected_compression)?;
    let mut overrides = BTreeMap::new();
    for parameter in baseline_model
        .weights()
        .parameter_vectors(&baseline_model.descriptor().config)
    {
        let parameter_id = parameter.parameter_id.as_str();
        let restored = match safetensors.tensor(parameter_id) {
            Ok(tensor) => {
                validate_tensor_shape(parameter_id, tensor.shape(), parameter.shape.dims())?;
                decode_float_tensor(parameter_id, tensor.dtype(), tensor.data(), tensor.shape())?
            }
            Err(_) => {
                let quantized_name = format!("{parameter_id}.__q");
                let scale_name = format!("{parameter_id}.__scale");
                let quantized = safetensors.tensor(quantized_name.as_str()).map_err(|_| {
                    ParameterGolfReferenceTrainingError::MissingArtifactTensor {
                        parameter_id: parameter.parameter_id.clone(),
                    }
                })?;
                let scale = safetensors.tensor(scale_name.as_str()).map_err(|_| {
                    ParameterGolfReferenceTrainingError::MissingArtifactTensor {
                        parameter_id: scale_name.clone(),
                    }
                })?;
                match artifact_config.quantization {
                    ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow => {
                        validate_tensor_shape(
                            parameter_id,
                            quantized.shape(),
                            parameter.shape.dims(),
                        )?;
                        dequantize_int8_tensor(
                            parameter_id,
                            parameter.shape.dims(),
                            quantized.data(),
                            scale.dtype(),
                            scale.data(),
                            scale.shape(),
                        )?
                    }
                    ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow => {
                        validate_tensor_shape(
                            parameter_id,
                            quantized.shape(),
                            parameter.shape.dims(),
                        )?;
                        dequantize_int6_tensor(
                            parameter_id,
                            parameter.shape.dims(),
                            quantized.data(),
                            scale.dtype(),
                            scale.data(),
                            scale.shape(),
                        )?
                    }
                }
            }
        };
        overrides.insert(parameter.parameter_id.clone(), restored);
    }
    let weights = baseline_model
        .weights()
        .with_parameter_overrides(&baseline_model.descriptor().config, &overrides)?;
    Ok(ParameterGolfReferenceModel::new(
        baseline_model.descriptor().model.clone(),
        baseline_model.descriptor().config.clone(),
        weights,
    )?)
}

/// Restores one int8+zlib model export back into the full-precision reference family.
pub fn restore_parameter_golf_model_from_int8_zlib(
    baseline_model: &ParameterGolfReferenceModel,
    artifact_bytes: &[u8],
) -> Result<ParameterGolfReferenceModel, ParameterGolfReferenceTrainingError> {
    restore_parameter_golf_model_from_quantized_artifact(baseline_model, artifact_bytes)
}

fn coordinate_map(
    coordinates: &[ParameterGolfTrainableCoordinate],
) -> Result<BTreeMap<String, Vec<usize>>, ParameterGolfReferenceTrainingError> {
    let mut seen = BTreeSet::new();
    let mut grouped = BTreeMap::new();
    for coordinate in coordinates {
        let key = format!("{}:{}", coordinate.parameter_id, coordinate.flat_index);
        if !seen.insert(key) {
            return Err(ParameterGolfReferenceTrainingError::DuplicateCoordinate {
                parameter_id: coordinate.parameter_id.clone(),
                flat_index: coordinate.flat_index,
            });
        }
        grouped
            .entry(coordinate.parameter_id.clone())
            .or_insert_with(Vec::new)
            .push(coordinate.flat_index);
    }
    for values in grouped.values_mut() {
        values.sort_unstable();
    }
    Ok(grouped)
}

fn validate_selected_indices(
    parameter_id: &str,
    selected_indices: &[usize],
    parameter_len: usize,
) -> Result<(), ParameterGolfReferenceTrainingError> {
    for &flat_index in selected_indices {
        if flat_index >= parameter_len {
            return Err(ParameterGolfReferenceTrainingError::CoordinateOutOfRange {
                parameter_id: String::from(parameter_id),
                flat_index,
                parameter_len,
            });
        }
    }
    Ok(())
}

fn materialize_model(
    initial_model: &ParameterGolfReferenceModel,
    trainable_tensors: &[TrainableTensorRuntime],
    baseline_vectors: &BTreeMap<String, ParameterGolfParameterVector>,
) -> Result<ParameterGolfReferenceModel, ParameterGolfReferenceTrainingError> {
    let mut overrides = BTreeMap::new();
    for tensor in trainable_tensors {
        overrides.insert(
            String::from(tensor.parameter_id()),
            tensor.full_values(baseline_vectors),
        );
    }
    let weights = initial_model
        .weights()
        .with_parameter_overrides(&initial_model.descriptor().config, &overrides)?;
    Ok(ParameterGolfReferenceModel::new(
        initial_model.descriptor().model.clone(),
        initial_model.descriptor().config.clone(),
        weights,
    )?)
}

fn take_training_microbatch(
    training_tokens: &[u16],
    geometry: &ParameterGolfBatchGeometry,
    global_micro_step: usize,
) -> Result<(Vec<Vec<u32>>, Vec<Vec<u32>>), ParameterGolfReferenceTrainingError> {
    let per_rank_span = geometry.local_train_batch_tokens() + 1;
    let start = (global_micro_step * per_rank_span) % training_tokens.len();
    let chunk = wraparound_slice(training_tokens, start, per_rank_span);
    microbatch_as_sequences(chunk.as_slice(), geometry.train_sequence_length)
}

fn wraparound_slice(tokens: &[u16], start: usize, len: usize) -> Vec<u16> {
    if start + len <= tokens.len() {
        return tokens[start..start + len].to_vec();
    }
    let mut output = Vec::with_capacity(len);
    output.extend_from_slice(&tokens[start..]);
    output.extend_from_slice(&tokens[..len - (tokens.len() - start)]);
    output
}

fn microbatch_as_sequences(
    tokens: &[u16],
    seq_len: usize,
) -> Result<(Vec<Vec<u32>>, Vec<Vec<u32>>), ParameterGolfReferenceTrainingError> {
    if tokens.len() <= seq_len {
        return Err(ParameterGolfReferenceTrainingError::InvalidFixture {
            message: format!(
                "microbatch requires at least {} tokens, found {}",
                seq_len + 1,
                tokens.len()
            ),
        });
    }
    let input_ids = tokens[..tokens.len() - 1]
        .chunks(seq_len)
        .map(|row| {
            row.iter()
                .map(|token| u32::from(*token))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let target_ids = tokens[1..]
        .chunks(seq_len)
        .map(|row| {
            row.iter()
                .map(|token| u32::from(*token))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    Ok((input_ids, target_ids))
}

fn finite_difference_gradients(
    current_model: &ParameterGolfReferenceModel,
    tensor: &TrainableTensorRuntime,
    baseline_vectors: &BTreeMap<String, ParameterGolfParameterVector>,
    epsilon: f32,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<Vec<f32>, ParameterGolfReferenceTrainingError> {
    let parameter_id = tensor.parameter_id();
    let current_values = tensor.full_values(baseline_vectors);
    match tensor {
        TrainableTensorRuntime::AdamSparse {
            selected_indices, ..
        } => {
            let mut gradients = vec![0.0_f32; selected_indices.len()];
            for (gradient_index, flat_index) in selected_indices.iter().enumerate() {
                gradients[gradient_index] = symmetric_finite_difference(
                    current_model,
                    parameter_id,
                    current_values.as_slice(),
                    *flat_index,
                    epsilon,
                    input_ids,
                    target_ids,
                )?;
            }
            Ok(gradients)
        }
        TrainableTensorRuntime::MuonDense {
            selected_indices, ..
        } => {
            let mut gradients = vec![0.0_f32; current_values.len()];
            for flat_index in selected_indices {
                gradients[*flat_index] = symmetric_finite_difference(
                    current_model,
                    parameter_id,
                    current_values.as_slice(),
                    *flat_index,
                    epsilon,
                    input_ids,
                    target_ids,
                )?;
            }
            Ok(gradients)
        }
    }
}

fn symmetric_finite_difference(
    current_model: &ParameterGolfReferenceModel,
    parameter_id: &str,
    values: &[f32],
    flat_index: usize,
    epsilon: f32,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<f32, ParameterGolfReferenceTrainingError> {
    let mut plus = values.to_vec();
    plus[flat_index] += epsilon;
    let plus_loss =
        loss_with_parameter_override(current_model, parameter_id, plus, input_ids, target_ids)?;
    let mut minus = values.to_vec();
    minus[flat_index] -= epsilon;
    let minus_loss =
        loss_with_parameter_override(current_model, parameter_id, minus, input_ids, target_ids)?;
    Ok((plus_loss - minus_loss) / (2.0 * epsilon))
}

fn loss_with_parameter_override(
    current_model: &ParameterGolfReferenceModel,
    parameter_id: &str,
    values: Vec<f32>,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<f32, ParameterGolfReferenceTrainingError> {
    let mut overrides = BTreeMap::new();
    overrides.insert(String::from(parameter_id), values);
    let weights = current_model
        .weights()
        .with_parameter_overrides(&current_model.descriptor().config, &overrides)?;
    let perturbed = current_model.with_execution_weights_unchecked(weights);
    Ok(perturbed.loss(input_ids, target_ids)?)
}

fn clip_accumulated_gradients(accumulated_gradients: &mut [Vec<f32>], grad_clip_norm: f32) {
    if !(grad_clip_norm.is_finite() && grad_clip_norm > 0.0) {
        return;
    }
    let norm = accumulated_gradients
        .iter()
        .flat_map(|values| values.iter())
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if norm <= grad_clip_norm || norm <= f32::EPSILON {
        return;
    }
    let scale = grad_clip_norm / norm;
    for values in accumulated_gradients {
        for value in values {
            *value *= scale;
        }
    }
}

fn export_checkpoint(
    initial_model: &ParameterGolfReferenceModel,
    current_model: &ParameterGolfReferenceModel,
    trainable_tensors: &[TrainableTensorRuntime],
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
    step: u64,
    validation_eval: &ParameterGolfValidationEvalReport,
    step_metrics: &[ParameterGolfReferenceTrainingStepMetrics],
    optimizer_plan_digest: &str,
    previous: Option<&ParameterGolfCheckpointArtifact>,
) -> Result<ParameterGolfCheckpointArtifact, ParameterGolfReferenceTrainingError> {
    let weights_artifact = ParameterGolfTrainingArtifact::new(
        "parameter_golf_model_safetensors",
        format!(
            "{}/step-{step:05}/checkpoint_model.safetensors",
            config.run_id
        ),
        export_full_precision_model_bytes(current_model)?,
    );
    let checkpoint_ref = format!("{}:step-{step:05}", config.run_id);
    let manifest = ParameterGolfCheckpointManifest {
        schema_version: 1,
        checkpoint_ref: checkpoint_ref.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        run_id: config.run_id.clone(),
        promoted_profile: config.promoted_profile.clone(),
        step,
        started_at_ms: config.started_at_ms,
        step_duration_ms: config.step_duration_ms,
        max_steps: config.max_steps,
        geometry: config.geometry.clone(),
        hyperparameters: config.hyperparameters.clone(),
        finite_difference_epsilon: config.finite_difference_epsilon,
        base_descriptor_digest: initial_model.descriptor().stable_digest(),
        current_descriptor_digest: current_model.descriptor().stable_digest(),
        optimizer_plan_digest: String::from(optimizer_plan_digest),
        training_dataset_digest: fixture.training_digest(),
        validation_dataset_digest: fixture.validation_digest(),
        validation_eval_digest: validation_eval.stable_digest(),
        step_metrics: step_metrics.to_vec(),
        trainable_tensors: trainable_tensors
            .iter()
            .map(TrainableTensorRuntime::checkpoint_tensor)
            .collect(),
        parent_checkpoint_ref: previous
            .and_then(|artifact| artifact.checkpoint.checkpoint_ref.clone()),
        parent_manifest_digest: previous.map(|artifact| artifact.manifest.stable_digest()),
    };
    let manifest_artifact = ParameterGolfTrainingArtifact::new(
        "parameter_golf_checkpoint_manifest",
        format!("{}/step-{step:05}/checkpoint_manifest.json", config.run_id),
        serde_json::to_vec_pretty(&manifest).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf checkpoint manifest export",
                message: error.to_string(),
            }
        })?,
    );
    let started_at_ms = config
        .started_at_ms
        .saturating_add(step.saturating_mul(config.step_duration_ms));
    let cluster_state_digest = stable_digest(
        b"psionic_parameter_golf_local_reference_cluster|",
        &config.run_id,
    );
    let topology_digest = stable_digest(
        b"psionic_parameter_golf_local_reference_topology|",
        &config.geometry,
    );
    let checkpoint = TrainingCheckpointReference::new(
        config.checkpoint_family.clone(),
        weights_artifact.artifact_ref.clone(),
        manifest_artifact.artifact_digest.clone(),
        weights_artifact.artifact_digest.clone(),
        "local-reference",
        0,
        cluster_state_digest,
        topology_digest,
        started_at_ms,
    )
    .with_checkpoint_ref(checkpoint_ref)
    .with_step(step)
    .with_durable_at_ms(started_at_ms);
    Ok(ParameterGolfCheckpointArtifact {
        weights_artifact,
        manifest_artifact,
        manifest,
        checkpoint,
    })
}

fn export_full_precision_model_bytes(
    model: &ParameterGolfReferenceModel,
) -> Result<Vec<u8>, ParameterGolfReferenceTrainingError> {
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from(PARAMETER_GOLF_CHECKPOINT_MANIFEST_KEY),
        model.descriptor().stable_digest(),
    );
    metadata.insert(
        String::from(PARAMETER_GOLF_WEIGHT_SURFACE_KEY),
        String::from(PARAMETER_GOLF_SPLIT_WEIGHT_SURFACE),
    );
    let raw_tensors = model
        .weights()
        .parameter_vectors(&model.descriptor().config)
        .into_iter()
        .map(|parameter| {
            (
                parameter.parameter_id,
                SafeTensorsDType::F32,
                parameter.shape.dims().to_vec(),
                encode_f32_bytes(parameter.values.as_slice()),
            )
        })
        .collect::<Vec<_>>();
    serialize_tensors(
        raw_tensors,
        Some(metadata),
        "parameter golf raw safetensors export",
    )
}

fn export_quantized_model_artifact(
    model: &ParameterGolfReferenceModel,
    artifact_config: &ParameterGolfFinalArtifactConfig,
    run_id: &str,
    step: u64,
) -> Result<ParameterGolfTrainingArtifact, ParameterGolfReferenceTrainingError> {
    let mut encoded_tensors = Vec::new();
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from("psionic.parameter_golf.quant_format"),
        String::from(artifact_config.quantization.as_str()),
    );
    metadata.insert(
        String::from("psionic.parameter_golf.compression_format"),
        String::from(artifact_config.compression.as_str()),
    );
    if let Some(level) = artifact_config.resolved_compression_level() {
        metadata.insert(
            String::from("psionic.parameter_golf.compression_level"),
            level.to_string(),
        );
    }
    for parameter in model
        .weights()
        .parameter_vectors(&model.descriptor().config)
    {
        let shape = parameter.shape.dims().to_vec();
        if keep_float_tensor(parameter.parameter_id.as_str(), parameter.values.len()) {
            if is_control_tensor_name(parameter.parameter_id.as_str()) {
                encoded_tensors.push((
                    parameter.parameter_id,
                    SafeTensorsDType::F32,
                    shape,
                    encode_f32_bytes(parameter.values.as_slice()),
                ));
            } else {
                encoded_tensors.push((
                    parameter.parameter_id,
                    SafeTensorsDType::F16,
                    shape,
                    encode_f16_bytes(parameter.values.as_slice()),
                ));
            }
            continue;
        }

        let quantized_name = format!("{}.__q", parameter.parameter_id);
        let scale_name = format!("{}.__scale", parameter.parameter_id);
        let (quantized_bytes, scale_bytes, scale_shape) = match artifact_config.quantization {
            ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow => {
                quantize_int8_tensor(parameter.values.as_slice(), shape.as_slice())
            }
            ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow => {
                quantize_int6_gptq_lite_tensor(parameter.values.as_slice(), shape.as_slice())
            }
        };
        encoded_tensors.push((quantized_name, SafeTensorsDType::I8, shape, quantized_bytes));
        encoded_tensors.push((scale_name, SafeTensorsDType::F16, scale_shape, scale_bytes));
    }
    let raw = serialize_tensors(
        encoded_tensors,
        Some(metadata),
        "parameter golf quantized safetensors export",
    )?;
    let compressed = match artifact_config.compression {
        ParameterGolfFinalArtifactCompressionFormat::Zlib => {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
            encoder.write_all(raw.as_slice())?;
            encoder.finish()?
        }
        ParameterGolfFinalArtifactCompressionFormat::Zstd => zstd_encode_all(
            raw.as_slice(),
            artifact_config
                .resolved_compression_level()
                .unwrap_or(PARAMETER_GOLF_COMPETITIVE_ZSTD_LEVEL),
        )?,
    };
    Ok(ParameterGolfTrainingArtifact::new(
        artifact_config.artifact_kind(),
        format!(
            "{run_id}/step-{step:05}/final_model.{}",
            artifact_config.artifact_extension()
        ),
        compressed,
    ))
}

/// Exports one Parameter Golf model into an explicit quantized artifact surface.
pub fn export_parameter_golf_quantized_model_artifact(
    model: &ParameterGolfReferenceModel,
    artifact_config: &ParameterGolfFinalArtifactConfig,
    run_id: &str,
    step: u64,
) -> Result<ParameterGolfTrainingArtifact, ParameterGolfReferenceTrainingError> {
    export_quantized_model_artifact(model, artifact_config, run_id, step)
}

/// Exports one Parameter Golf model into the canonical int8-plus-zlib artifact
/// surface used by the bounded benchmark and submission lanes.
pub fn export_parameter_golf_int8_zlib_model_artifact(
    model: &ParameterGolfReferenceModel,
    run_id: &str,
    step: u64,
) -> Result<ParameterGolfTrainingArtifact, ParameterGolfReferenceTrainingError> {
    export_quantized_model_artifact(
        model,
        &ParameterGolfFinalArtifactConfig::default(),
        run_id,
        step,
    )
}

/// Exports one Parameter Golf model into the canonical raw safetensors bytes
/// surface used by checkpoint and distributed-runtime seams.
pub fn export_parameter_golf_full_precision_model_bytes(
    model: &ParameterGolfReferenceModel,
) -> Result<Vec<u8>, ParameterGolfReferenceTrainingError> {
    export_full_precision_model_bytes(model)
}

/// Exports one Parameter Golf model into the upstream-style banked full-precision
/// safetensors surface used by the distributed score path.
pub fn export_parameter_golf_banked_full_precision_model_bytes(
    model: &ParameterGolfReferenceModel,
) -> Result<Vec<u8>, ParameterGolfReferenceTrainingError> {
    let banked_weights = model.banked_weights()?;
    export_parameter_golf_banked_full_precision_weights_bytes(model, &banked_weights)
}

/// Exports one explicit banked runtime-weight surface into the canonical
/// safetensors bytes used by the distributed score path.
pub fn export_parameter_golf_banked_full_precision_weights_bytes(
    baseline_model: &ParameterGolfReferenceModel,
    banked_weights: &ParameterGolfBankedWeights,
) -> Result<Vec<u8>, ParameterGolfReferenceTrainingError> {
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from(PARAMETER_GOLF_CHECKPOINT_MANIFEST_KEY),
        baseline_model.descriptor().stable_digest(),
    );
    metadata.insert(
        String::from(PARAMETER_GOLF_WEIGHT_SURFACE_KEY),
        String::from(PARAMETER_GOLF_BANKED_WEIGHT_SURFACE),
    );
    let raw_tensors = banked_weights
        .parameter_vectors(&baseline_model.descriptor().config)
        .into_iter()
        .map(|parameter| {
            (
                parameter.parameter_id,
                SafeTensorsDType::F32,
                parameter.shape.dims().to_vec(),
                encode_f32_bytes(parameter.values.as_slice()),
            )
        })
        .collect::<Vec<_>>();
    serialize_tensors(
        raw_tensors,
        Some(metadata),
        "parameter golf raw banked safetensors export",
    )
}

fn serialize_tensors(
    tensors: Vec<(String, SafeTensorsDType, Vec<usize>, Vec<u8>)>,
    metadata: Option<HashMap<String, String>>,
    context: &'static str,
) -> Result<Vec<u8>, ParameterGolfReferenceTrainingError> {
    // Upstream `safetensors::serialize` writes `__metadata__` in `HashMap`
    // iteration order, which is non-deterministic across processes and even
    // across map instances. Parameter Golf exports are digest-bearing
    // replay-safe artifacts, so build the exact safetensors byte layout here
    // with sorted metadata keys and insertion-ordered tensors instead.
    let mut views = Vec::with_capacity(tensors.len());
    for (name, dtype, shape, bytes) in &tensors {
        let view = TensorView::new(*dtype, shape.clone(), bytes.as_slice()).map_err(|error| {
            ParameterGolfReferenceTrainingError::Serialization {
                context,
                message: error.to_string(),
            }
        })?;
        views.push((name.clone(), view));
    }

    let serialization_error =
        |message: String| ParameterGolfReferenceTrainingError::Serialization { context, message };
    let mut header = String::from("{");
    if let Some(metadata) = &metadata {
        let sorted_metadata = metadata.iter().collect::<BTreeMap<_, _>>();
        header.push_str("\"__metadata__\":");
        header.push_str(
            &serde_json::to_string(&sorted_metadata)
                .map_err(|error| serialization_error(error.to_string()))?,
        );
        if !views.is_empty() {
            header.push(',');
        }
    }
    let mut data_offset = 0_usize;
    for (index, (name, view)) in views.iter().enumerate() {
        let data_len = view.data().len();
        let entry = serde_json::json!({
            "dtype": view.dtype(),
            "shape": view.shape(),
            "data_offsets": [data_offset, data_offset + data_len],
        });
        header.push_str(
            &serde_json::to_string(name).map_err(|error| serialization_error(error.to_string()))?,
        );
        header.push(':');
        header.push_str(
            &serde_json::to_string(&entry)
                .map_err(|error| serialization_error(error.to_string()))?,
        );
        if index + 1 != views.len() {
            header.push(',');
        }
        data_offset += data_len;
    }
    header.push('}');

    let mut header_bytes = header.into_bytes();
    let aligned_header_len = header_bytes.len().next_multiple_of(8);
    header_bytes.resize(aligned_header_len, b' ');

    let mut output = Vec::with_capacity(8 + header_bytes.len() + data_offset);
    output.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(header_bytes.as_slice());
    for (_, view) in &views {
        output.extend_from_slice(view.data());
    }
    Ok(output)
}

fn keep_float_tensor(parameter_id: &str, parameter_len: usize) -> bool {
    is_control_tensor_name(parameter_id)
        || parameter_len <= PARAMETER_GOLF_INT8_KEEP_FLOAT_MAX_NUMEL
}

fn decode_quantized_artifact_bytes(
    artifact_bytes: &[u8],
) -> Result<
    (ParameterGolfFinalArtifactCompressionFormat, Vec<u8>),
    ParameterGolfReferenceTrainingError,
> {
    let mut zlib_decoder = ZlibDecoder::new(artifact_bytes);
    let mut zlib_bytes = Vec::new();
    match zlib_decoder.read_to_end(&mut zlib_bytes) {
        Ok(_) => Ok((
            ParameterGolfFinalArtifactCompressionFormat::Zlib,
            zlib_bytes,
        )),
        Err(zlib_error) => {
            let zstd_bytes = zstd_decode_all(artifact_bytes).map_err(|zstd_error| {
                ParameterGolfReferenceTrainingError::Serialization {
                    context: "parameter golf quantized artifact decode",
                    message: format!(
                        "failed zlib decode: {zlib_error}; failed zstd decode: {zstd_error}"
                    ),
                }
            })?;
            Ok((
                ParameterGolfFinalArtifactCompressionFormat::Zstd,
                zstd_bytes,
            ))
        }
    }
}

fn read_safetensors_metadata(
    raw_bytes: &[u8],
) -> Result<Option<HashMap<String, String>>, ParameterGolfReferenceTrainingError> {
    if raw_bytes.len() < 8 {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf safetensors metadata",
            message: String::from("missing safetensors header length prefix"),
        });
    }
    let header_len = u64::from_le_bytes(
        raw_bytes[..8]
            .try_into()
            .expect("slice length already checked"),
    ) as usize;
    if raw_bytes.len() < 8 + header_len {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf safetensors metadata",
            message: format!(
                "header length {} exceeds artifact bytes {}",
                header_len,
                raw_bytes.len()
            ),
        });
    }
    let header_json = serde_json::from_slice::<serde_json::Value>(&raw_bytes[8..8 + header_len])
        .map_err(|error| ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf safetensors metadata",
            message: error.to_string(),
        })?;
    Ok(header_json
        .get("__metadata__")
        .and_then(serde_json::Value::as_object)
        .map(|metadata| {
            metadata
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.into())))
                .collect::<HashMap<_, _>>()
        }))
}

fn quantized_artifact_config_from_metadata(
    metadata: Option<&HashMap<String, String>>,
    detected_compression: ParameterGolfFinalArtifactCompressionFormat,
) -> Result<ParameterGolfFinalArtifactConfig, ParameterGolfReferenceTrainingError> {
    let quantization = match metadata
        .and_then(|metadata| metadata.get("psionic.parameter_golf.quant_format"))
        .map(String::as_str)
        .unwrap_or(PARAMETER_GOLF_INT8_ZLIB_FORMAT)
    {
        PARAMETER_GOLF_INT8_ZLIB_FORMAT => {
            ParameterGolfFinalArtifactQuantizationFormat::Int8CleanPerRow
        }
        PARAMETER_GOLF_INT6_GPTQ_LITE_FORMAT => {
            ParameterGolfFinalArtifactQuantizationFormat::Int6GptqLitePerRow
        }
        actual => {
            return Err(ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf quantized artifact metadata",
                message: format!("unsupported quantization format `{actual}`"),
            });
        }
    };
    let compression = match metadata
        .and_then(|metadata| metadata.get("psionic.parameter_golf.compression_format"))
        .map(String::as_str)
    {
        Some(PARAMETER_GOLF_ZLIB_COMPRESSION_FORMAT) => {
            ParameterGolfFinalArtifactCompressionFormat::Zlib
        }
        Some(PARAMETER_GOLF_ZSTD_COMPRESSION_FORMAT) => {
            ParameterGolfFinalArtifactCompressionFormat::Zstd
        }
        Some(actual) => {
            return Err(ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf quantized artifact metadata",
                message: format!("unsupported compression format `{actual}`"),
            });
        }
        None => detected_compression,
    };
    let compression_level = metadata
        .and_then(|metadata| metadata.get("psionic.parameter_golf.compression_level"))
        .map(|value| {
            value.parse::<i32>().map_err(|error| {
                ParameterGolfReferenceTrainingError::Serialization {
                    context: "parameter golf quantized artifact metadata",
                    message: format!("invalid compression level `{value}`: {error}"),
                }
            })
        })
        .transpose()?;
    Ok(ParameterGolfFinalArtifactConfig {
        quantization,
        compression,
        compression_level,
    })
}

fn is_control_tensor_name(parameter_id: &str) -> bool {
    PARAMETER_GOLF_CONTROL_TENSOR_NAME_PATTERNS
        .iter()
        .any(|pattern| parameter_id.contains(pattern))
}

fn quantize_int8_tensor(values: &[f32], shape: &[usize]) -> (Vec<u8>, Vec<u8>, Vec<usize>) {
    if shape.len() == 2 {
        let rows = shape[0];
        let cols = shape[1];
        let mut quantized = Vec::with_capacity(values.len());
        let mut scales = Vec::with_capacity(rows);
        for row in 0..rows {
            let row_values = &values[row * cols..(row + 1) * cols];
            let clip_abs = quantile_abs(row_values, PARAMETER_GOLF_INT8_CLIP_Q);
            let scale = if clip_abs > 0.0 {
                clip_abs / 127.0
            } else {
                1.0
            };
            scales.push(scale);
            for value in row_values {
                let clipped = value.clamp(-clip_abs, clip_abs);
                let q = (clipped / scale).round().clamp(-127.0, 127.0) as i8;
                quantized.push(q as u8);
            }
        }
        return (quantized, encode_f16_bytes(scales.as_slice()), vec![rows]);
    }

    let clip_abs = quantile_abs(values, PARAMETER_GOLF_INT8_CLIP_Q);
    let scale = if clip_abs > 0.0 {
        clip_abs / 127.0
    } else {
        1.0
    };
    let quantized = values
        .iter()
        .map(|value| {
            let clipped = value.clamp(-clip_abs, clip_abs);
            (clipped / scale).round().clamp(-127.0, 127.0) as i8 as u8
        })
        .collect::<Vec<_>>();
    (quantized, encode_f16_bytes(&[scale]), vec![1])
}

fn quantize_int6_gptq_lite_tensor(
    values: &[f32],
    shape: &[usize],
) -> (Vec<u8>, Vec<u8>, Vec<usize>) {
    if shape.len() == 2 {
        let rows = shape[0];
        let cols = shape[1];
        let mut quantized = Vec::with_capacity(values.len());
        let mut scales = Vec::with_capacity(rows);
        for row in 0..rows {
            let row_values = &values[row * cols..(row + 1) * cols];
            let (best_quantized, best_scale) = quantize_int6_row_gptq_lite(row_values);
            quantized.extend_from_slice(best_quantized.as_slice());
            scales.push(best_scale);
        }
        return (quantized, encode_f16_bytes(scales.as_slice()), vec![rows]);
    }

    let (quantized, scale) = quantize_int6_row_gptq_lite(values);
    (quantized, encode_f16_bytes(&[scale]), vec![1])
}

fn quantize_int6_row_gptq_lite(values: &[f32]) -> (Vec<u8>, f32) {
    let mut best_quantized = Vec::new();
    let mut best_scale = 1.0_f32;
    let mut best_mse = f32::INFINITY;
    for &clip_quantile in PARAMETER_GOLF_INT6_GPTQ_LITE_CLIP_CANDIDATES {
        let clip_abs = quantile_abs(values, clip_quantile);
        let scale = if clip_abs > 0.0 { clip_abs / 31.0 } else { 1.0 };
        let quantized = values
            .iter()
            .map(|value| {
                let clipped = value.clamp(-clip_abs, clip_abs);
                (clipped / scale).round().clamp(-31.0, 31.0) as i8 as u8
            })
            .collect::<Vec<_>>();
        let mse = values
            .iter()
            .zip(quantized.iter())
            .map(|(value, quantized)| {
                let restored = (*quantized as i8 as f32) * scale;
                let error = restored - *value;
                error * error
            })
            .sum::<f32>()
            / values.len().max(1) as f32;
        if mse < best_mse {
            best_mse = mse;
            best_scale = scale;
            best_quantized = quantized;
        }
    }
    (best_quantized, best_scale)
}

fn dequantize_int8_tensor(
    parameter_id: &str,
    expected_shape: &[usize],
    quantized_bytes: &[u8],
    scale_dtype: SafeTensorsDType,
    scale_bytes: &[u8],
    scale_shape: &[usize],
) -> Result<Vec<f32>, ParameterGolfReferenceTrainingError> {
    let element_count = expected_shape.iter().product::<usize>();
    if quantized_bytes.len() != element_count {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf int8 restore",
            message: format!(
                "tensor `{parameter_id}` had {} int8 bytes for {} elements",
                quantized_bytes.len(),
                element_count
            ),
        });
    }
    let scales = decode_float_tensor(parameter_id, scale_dtype, scale_bytes, scale_shape)?;
    if expected_shape.len() == 2 {
        let rows = expected_shape[0];
        let cols = expected_shape[1];
        if scales.len() != rows {
            return Err(ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf int8 restore",
                message: format!(
                    "tensor `{parameter_id}` expected {rows} row scales, found {}",
                    scales.len()
                ),
            });
        }
        let mut output = vec![0.0_f32; element_count];
        for row in 0..rows {
            let scale = scales[row];
            for col in 0..cols {
                let index = row * cols + col;
                output[index] = (quantized_bytes[index] as i8 as f32) * scale;
            }
        }
        return Ok(output);
    }
    let scale = scales.first().copied().unwrap_or(1.0);
    Ok(quantized_bytes
        .iter()
        .map(|value| (*value as i8 as f32) * scale)
        .collect())
}

fn dequantize_int6_tensor(
    parameter_id: &str,
    expected_shape: &[usize],
    quantized_bytes: &[u8],
    scale_dtype: SafeTensorsDType,
    scale_bytes: &[u8],
    scale_shape: &[usize],
) -> Result<Vec<f32>, ParameterGolfReferenceTrainingError> {
    let element_count = expected_shape.iter().product::<usize>();
    if quantized_bytes.len() != element_count {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf int6 restore",
            message: format!(
                "tensor `{parameter_id}` had {} int6 bytes for {} elements",
                quantized_bytes.len(),
                element_count
            ),
        });
    }
    let scales = decode_float_tensor(parameter_id, scale_dtype, scale_bytes, scale_shape)?;
    if expected_shape.len() == 2 {
        let rows = expected_shape[0];
        let cols = expected_shape[1];
        if scales.len() != rows {
            return Err(ParameterGolfReferenceTrainingError::Serialization {
                context: "parameter golf int6 restore",
                message: format!(
                    "tensor `{parameter_id}` expected {rows} row scales, found {}",
                    scales.len()
                ),
            });
        }
        let mut output = vec![0.0_f32; element_count];
        for row in 0..rows {
            let scale = scales[row];
            for col in 0..cols {
                let index = row * cols + col;
                output[index] = (quantized_bytes[index] as i8 as f32) * scale;
            }
        }
        return Ok(output);
    }
    let scale = scales.first().copied().unwrap_or(1.0);
    Ok(quantized_bytes
        .iter()
        .map(|value| (*value as i8 as f32) * scale)
        .collect())
}

fn validate_tensor_shape(
    parameter_id: &str,
    actual: &[usize],
    expected: &[usize],
) -> Result<(), ParameterGolfReferenceTrainingError> {
    if actual != expected {
        return Err(ParameterGolfReferenceTrainingError::ArtifactTensorShape {
            parameter_id: String::from(parameter_id),
            actual: actual.to_vec(),
            expected: expected.to_vec(),
        });
    }
    Ok(())
}

fn decode_float_tensor(
    parameter_id: &str,
    dtype: SafeTensorsDType,
    bytes: &[u8],
    shape: &[usize],
) -> Result<Vec<f32>, ParameterGolfReferenceTrainingError> {
    let expected_len = shape.iter().product::<usize>();
    match dtype {
        SafeTensorsDType::F32 => decode_f32_bytes(parameter_id, bytes, expected_len),
        SafeTensorsDType::F16 => decode_f16_bytes(parameter_id, bytes, expected_len),
        _ => Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf tensor decode",
            message: format!("tensor `{parameter_id}` had unsupported dtype `{dtype}`"),
        }),
    }
}

fn decode_f32_bytes(
    parameter_id: &str,
    bytes: &[u8],
    expected_len: usize,
) -> Result<Vec<f32>, ParameterGolfReferenceTrainingError> {
    if bytes.len() != expected_len.saturating_mul(4) {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf f32 decode",
            message: format!(
                "tensor `{parameter_id}` had {} bytes; expected {}",
                bytes.len(),
                expected_len.saturating_mul(4)
            ),
        });
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn decode_f16_bytes(
    parameter_id: &str,
    bytes: &[u8],
    expected_len: usize,
) -> Result<Vec<f32>, ParameterGolfReferenceTrainingError> {
    if bytes.len() != expected_len.saturating_mul(2) {
        return Err(ParameterGolfReferenceTrainingError::Serialization {
            context: "parameter golf f16 decode",
            message: format!(
                "tensor `{parameter_id}` had {} bytes; expected {}",
                bytes.len(),
                expected_len.saturating_mul(2)
            ),
        });
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|chunk| f16::from_le_bytes([chunk[0], chunk[1]]).to_f32())
        .collect())
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn encode_f16_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 2);
    for value in values {
        bytes.extend_from_slice(&f16::from_f32(*value).to_le_bytes());
    }
    bytes
}

fn quantile_abs(values: &[f32], quantile: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.iter().map(|value| value.abs()).collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let index = ((sorted.len().saturating_sub(1) as f32) * quantile)
        .round()
        .clamp(0.0, sorted.len().saturating_sub(1) as f32) as usize;
    sorted[index]
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        io::{self, Write},
        sync::{Arc, Mutex},
        time::Instant,
    };

    use tempfile::tempdir;

    use crate::{
        AsyncCheckpointWritebackOptions, AsyncCheckpointWritebackWorker, LocalTrainMetricCollector,
        LocalTrainMetricFanout, LocalTrainMetricJsonlSink, LocalTrainMetricProgressSink,
        LocalTrainMetricStructuredLogSink, LocalTrainMetricValue,
        async_checkpoint_writeback::write_checkpoint_payload_sync_with_options,
    };

    use super::{
        ParameterGolfFinalArtifactConfig, ParameterGolfLocalReferenceFixture,
        ParameterGolfReferenceTrainingConfig, ParameterGolfReferenceTrainingError,
        ParameterGolfReferenceTrainingRunner, check_parameter_golf_promoted_bundle,
        checkpoint_async_writeback_payload,
        export_parameter_golf_banked_full_precision_model_bytes,
        export_parameter_golf_full_precision_model_bytes,
        export_parameter_golf_quantized_model_artifact, promoted_checkpoint_surface_report,
        read_parameter_golf_checkpoint_from_directory,
        restore_parameter_golf_banked_weights_from_safetensors,
        restore_parameter_golf_local_reference_checkpoint,
        restore_parameter_golf_model_from_int8_zlib,
        restore_parameter_golf_model_from_quantized_artifact,
        restore_parameter_golf_model_from_safetensors, run_parameter_golf_promoted_reference_run,
        train_parameter_golf_local_reference,
        train_parameter_golf_local_reference_with_async_checkpoint_writeback,
        train_parameter_golf_local_reference_with_metric_sink,
        write_parameter_golf_promoted_reference_run,
    };
    use psionic_models::{
        ParameterGolfPromotedGenerationTermination, ParameterGolfPromotedProfileKind,
        ParameterGolfPromotedRuntimeBundle, ParameterGolfReferenceModel, TokenizerBoundary,
    };

    #[derive(Clone, Default)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl SharedWriter {
        fn contents(&self) -> String {
            String::from_utf8(
                self.0
                    .lock()
                    .expect("shared writer mutex should not be poisoned")
                    .clone(),
            )
            .expect("shared writer should only contain utf8")
        }
    }

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("shared writer mutex should not be poisoned")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn parameter_golf_local_reference_runner_restores_and_matches_continuous_run()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();

        let mut continuous = ParameterGolfReferenceTrainingRunner::new(&fixture, &config)?;
        let mut restored_source = ParameterGolfReferenceTrainingRunner::new(&fixture, &config)?;

        continuous.step()?;
        continuous.step()?;

        restored_source.step()?;
        let checkpoint = restored_source.latest_checkpoint().clone();
        let mut restored =
            restore_parameter_golf_local_reference_checkpoint(&fixture, &checkpoint)?;
        restored.step()?;

        let continuous_outcome = continuous.into_outcome()?;
        let restored_outcome = restored.into_outcome()?;

        assert_eq!(
            continuous_outcome.trained_model,
            restored_outcome.trained_model
        );
        assert_eq!(
            continuous_outcome.final_validation_eval,
            restored_outcome.final_validation_eval
        );
        assert_eq!(
            continuous_outcome.step_metrics,
            restored_outcome.step_metrics
        );
        assert_eq!(
            continuous_outcome.final_checkpoint.manifest.stable_digest(),
            restored_outcome.final_checkpoint.manifest.stable_digest()
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_local_reference_exports_raw_and_int8_roundtrips() -> Result<(), Box<dyn Error>>
    {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let outcome = train_parameter_golf_local_reference(&fixture, &config)?;

        let restored_raw = restore_parameter_golf_model_from_safetensors(
            &outcome.initial_model,
            outcome.raw_model_artifact.bytes.as_slice(),
        )?;
        assert_eq!(restored_raw, outcome.trained_model);
        assert_eq!(
            outcome.final_validation_eval,
            outcome.raw_roundtrip_validation_eval
        );

        let restored_int8 = restore_parameter_golf_model_from_int8_zlib(
            &outcome.initial_model,
            outcome.int8_zlib_model_artifact.bytes.as_slice(),
        )?;
        let int8_eval = psionic_eval::evaluate_parameter_golf_validation(
            &restored_int8,
            fixture.validation_tokens.as_slice(),
            config.geometry.train_sequence_length,
            config.geometry.local_validation_batch_tokens(),
            &fixture.byte_luts()?,
        )?;
        assert_eq!(int8_eval, outcome.int8_zlib_roundtrip_validation_eval);
        assert!(int8_eval.mean_loss.is_finite());
        assert!(int8_eval.bits_per_byte.is_finite());
        Ok(())
    }

    #[test]
    fn parameter_golf_competitive_quantized_artifact_roundtrips() -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let outcome = train_parameter_golf_local_reference(&fixture, &config)?;
        let artifact_config = ParameterGolfFinalArtifactConfig::competitive_defaults();

        let artifact = export_parameter_golf_quantized_model_artifact(
            &outcome.trained_model,
            &artifact_config,
            "parameter-golf-competitive-artifact-test",
            config.max_steps,
        )?;
        assert_eq!(artifact.artifact_kind, "parameter_golf_model_int6_zstd");

        let restored_model = restore_parameter_golf_model_from_quantized_artifact(
            &outcome.initial_model,
            artifact.bytes.as_slice(),
        )?;
        let restored_eval = psionic_eval::evaluate_parameter_golf_validation(
            &restored_model,
            fixture.validation_tokens.as_slice(),
            config.geometry.train_sequence_length,
            config.geometry.local_validation_batch_tokens(),
            &fixture.byte_luts()?,
        )?;
        assert!(restored_eval.mean_loss.is_finite());
        assert!(restored_eval.bits_per_byte.is_finite());
        Ok(())
    }

    #[test]
    fn parameter_golf_local_reference_async_checkpoint_writeback_restores_sync_equivalently()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let sync_outcome = train_parameter_golf_local_reference(&fixture, &config)?;
        let checkpoint_root = tempdir()?;
        let async_outcome = train_parameter_golf_local_reference_with_async_checkpoint_writeback(
            &fixture,
            &config,
            checkpoint_root.path(),
            AsyncCheckpointWritebackOptions::bounded(1)?,
        )?;

        assert_eq!(async_outcome.trained_model, sync_outcome.trained_model);
        assert_eq!(
            async_outcome.final_validation_eval,
            sync_outcome.final_validation_eval
        );
        assert_eq!(async_outcome.step_metrics, sync_outcome.step_metrics);
        assert_eq!(
            async_outcome.final_checkpoint.manifest.stable_digest(),
            sync_outcome.final_checkpoint.manifest.stable_digest()
        );
        assert_eq!(
            async_outcome.checkpoint_writeback_receipts.len(),
            (config.max_steps + 1) as usize
        );

        let final_receipt = async_outcome
            .checkpoint_writeback_receipts
            .last()
            .expect("final checkpoint receipt should exist");
        let restored_checkpoint = read_parameter_golf_checkpoint_from_directory(
            final_receipt.final_directory.as_path(),
            &async_outcome.final_checkpoint.checkpoint,
        )?;
        assert_eq!(
            restored_checkpoint.manifest_artifact.bytes,
            async_outcome.final_checkpoint.manifest_artifact.bytes
        );
        assert_eq!(
            restored_checkpoint.weights_artifact.bytes,
            async_outcome.final_checkpoint.weights_artifact.bytes
        );
        assert_eq!(
            restored_checkpoint.manifest_artifact.artifact_digest,
            async_outcome
                .final_checkpoint
                .manifest_artifact
                .artifact_digest
        );
        assert_eq!(
            restored_checkpoint.weights_artifact.artifact_digest,
            async_outcome
                .final_checkpoint
                .weights_artifact
                .artifact_digest
        );

        let restored_runner =
            restore_parameter_golf_local_reference_checkpoint(&fixture, &restored_checkpoint)?;
        let restored_outcome = restored_runner.into_outcome()?;
        assert_eq!(restored_outcome.trained_model, sync_outcome.trained_model);
        assert_eq!(
            restored_outcome.final_validation_eval,
            sync_outcome.final_validation_eval
        );
        assert_eq!(
            restored_outcome.final_checkpoint.manifest.stable_digest(),
            sync_outcome.final_checkpoint.manifest.stable_digest()
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_local_reference_async_checkpoint_handoff_beats_sync_stall()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let runner = ParameterGolfReferenceTrainingRunner::new(&fixture, &config)?;
        let sync_root = tempdir()?;
        let async_root = tempdir()?;
        let options = AsyncCheckpointWritebackOptions::bounded(1)?
            .with_test_injected_write_delay(std::time::Duration::from_millis(75));
        let sync_payload =
            checkpoint_async_writeback_payload(runner.latest_checkpoint(), sync_root.path())?;

        let sync_started = Instant::now();
        let _ = write_checkpoint_payload_sync_with_options(&sync_payload, &options)?;
        let sync_elapsed = sync_started.elapsed();

        let async_payload =
            checkpoint_async_writeback_payload(runner.latest_checkpoint(), async_root.path())?;
        let mut worker = AsyncCheckpointWritebackWorker::new(options.clone())?;
        let async_started = Instant::now();
        let ticket = worker.submit(async_payload)?;
        let async_submit_elapsed = async_started.elapsed();
        let receipt = ticket.wait()?;
        let _ = worker.shutdown_flush()?;

        assert!(receipt.final_directory.exists());
        assert!(sync_elapsed >= std::time::Duration::from_millis(60));
        assert!(
            async_submit_elapsed.as_millis() * 5 < sync_elapsed.as_millis(),
            "expected async handoff {:?} to be materially smaller than sync stall {:?}",
            async_submit_elapsed,
            sync_elapsed
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_local_reference_metric_sink_fanout_stays_local_and_deterministic()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let baseline_outcome = train_parameter_golf_local_reference(&fixture, &config)?;
        let progress = SharedWriter::default();
        let structured = SharedWriter::default();
        let collector = LocalTrainMetricCollector::default();
        let jsonl_dir = tempdir()?;
        let jsonl_path = jsonl_dir.path().join("telemetry.jsonl");
        let mut sink = LocalTrainMetricFanout::new(config.run_id.clone());
        sink.add_sink(LocalTrainMetricProgressSink::new(progress.clone()));
        sink.add_sink(LocalTrainMetricStructuredLogSink::new(structured.clone()));
        sink.add_sink(LocalTrainMetricJsonlSink::create(jsonl_path.as_path())?);
        sink.add_sink(collector.clone());

        let sink_outcome =
            train_parameter_golf_local_reference_with_metric_sink(&fixture, &config, &mut sink)?;
        let collected = collector.events();
        let jsonl_lines = std::fs::read_to_string(jsonl_path.as_path())?
            .lines()
            .map(serde_json::from_str::<crate::LocalTrainMetricEvent>)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(sink_outcome, baseline_outcome);
        assert_eq!(collected, jsonl_lines);
        assert_eq!(collected.len(), (config.max_steps as usize) * 3);
        assert!(
            collected
                .iter()
                .any(|event| event.metric_id == "mean_microbatch_loss"
                    && matches!(event.value, LocalTrainMetricValue::F32(_)))
        );
        assert!(progress.contents().contains("mean_microbatch_loss"));
        assert!(structured.contents().starts_with("metric_event {"));
        Ok(())
    }

    #[test]
    fn promoted_parameter_golf_reference_run_proves_full_checkpoint_surface_and_resume()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;

        assert_eq!(
            run.profile_contract.kind,
            ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder
        );
        assert!(run.checkpoint_surface_report.exact_match);
        assert!(run.resume_proof.exact_final_parity);
        assert_eq!(
            run.tokenizer_asset.profile_id,
            config.promoted_profile.profile_id
        );
        assert_eq!(
            run.generation_config.profile_id,
            config.promoted_profile.profile_id
        );
        assert_eq!(
            run.bundle_manifest.profile_id,
            config.promoted_profile.profile_id
        );
        assert_eq!(
            run.summary.final_checkpoint_manifest_digest,
            run.training_outcome
                .final_checkpoint
                .manifest
                .stable_digest()
        );
        assert_eq!(
            run.training_outcome
                .final_checkpoint
                .manifest
                .promoted_profile,
            config.promoted_profile
        );

        let explicit_report = promoted_checkpoint_surface_report(
            &run.profile_contract,
            &run.model_descriptor,
            &run.training_outcome.final_checkpoint.weights_artifact,
        )?;
        assert_eq!(explicit_report, run.checkpoint_surface_report);

        let output_dir = tempdir()?;
        write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;
        assert!(
            output_dir
                .path()
                .join("parameter_golf_promoted_summary.json")
                .exists()
        );
        assert!(
            output_dir
                .path()
                .join("parameter_golf_final_checkpoint.safetensors")
                .exists()
        );
        assert!(
            output_dir
                .path()
                .join("parameter_golf_promoted_resume_proof.json")
                .exists()
        );
        assert!(output_dir.path().join("descriptor.json").exists());
        assert!(output_dir.path().join("model.safetensors").exists());
        assert!(output_dir.path().join("tokenizer.json").exists());
        assert!(output_dir.path().join("generation_config.json").exists());
        assert!(
            output_dir
                .path()
                .join("parameter_golf_promoted_bundle_manifest.json")
                .exists()
        );
        let checked_manifest = check_parameter_golf_promoted_bundle(output_dir.path())?;
        assert_eq!(checked_manifest, run.bundle_manifest);
        Ok(())
    }

    #[test]
    fn xtrain_promoted_parameter_golf_config_expands_the_inferable_budget()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let proof = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let xtrain =
            ParameterGolfReferenceTrainingConfig::xtrain_promoted_general_small_decoder_baseline();

        xtrain.validate()?;
        xtrain
            .promoted_profile
            .validate_local_reference_lane(&fixture)?;
        assert_eq!(xtrain.promoted_profile, proof.promoted_profile);
        assert!(xtrain.max_steps > proof.max_steps);
        assert!(xtrain.geometry.grad_accum_steps < proof.geometry.grad_accum_steps);
        assert_eq!(
            xtrain.geometry.train_sequence_length,
            proof.geometry.train_sequence_length
        );
        assert!(xtrain.finite_difference_epsilon < proof.finite_difference_epsilon);
        assert!(xtrain.hyperparameters.tied_embed_lr > proof.hyperparameters.tied_embed_lr);
        assert!(xtrain.hyperparameters.matrix_lr > proof.hyperparameters.matrix_lr);
        assert!(xtrain.hyperparameters.scalar_lr > proof.hyperparameters.scalar_lr);
        assert_eq!(xtrain.inference_attention_window_tokens, Some(1));
        assert!(xtrain.selected_coordinates.len() > proof.selected_coordinates.len());
        assert!(
            xtrain
                .selected_coordinates
                .iter()
                .any(|coordinate| coordinate.parameter_id == "tok_emb.weight"
                    && coordinate.flat_index >= 512)
        );
        assert!(
            xtrain
                .selected_coordinates
                .iter()
                .any(|coordinate| coordinate.parameter_id == "blocks.0.attn.q_gain")
        );
        assert!(
            xtrain
                .selected_coordinates
                .iter()
                .filter(|coordinate| coordinate.parameter_id == "tok_emb.weight")
                .count()
                >= 32
        );
        Ok(())
    }

    #[test]
    fn strict_pgolf_challenge_profile_refuses_local_reference_lane() -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::strict_pgolf_challenge();
        let error = ParameterGolfReferenceTrainingRunner::new(&fixture, &config).unwrap_err();

        match error {
            ParameterGolfReferenceTrainingError::PromotedProfileRefusal { profile_id, detail } => {
                assert_eq!(
                    profile_id,
                    ParameterGolfPromotedProfileKind::StrictPgolfChallenge.profile_id()
                );
                assert!(detail.contains("challenge SP1024 tokenizer"));
                assert!(detail.contains("FineWeb SP1024 lane"));
                assert!(detail.contains("score-first TTT"));
                assert!(detail.contains("bits-per-byte accounting"));
                assert!(detail.contains("compressed artifact cap"));
            }
            other => panic!("expected promoted profile refusal, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn promoted_parameter_golf_runtime_bundle_loads_publicly_and_restores_trained_model()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
        let output_dir = tempdir()?;
        write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;

        let bundle = ParameterGolfPromotedRuntimeBundle::load_dir(output_dir.path())?;
        let encoded = bundle.tokenizer().encode_with_defaults("abcd efg h");
        let encoded_ids = encoded
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect::<Vec<_>>();

        assert_eq!(
            bundle.manifest().profile_id,
            config.promoted_profile.profile_id
        );
        assert_eq!(
            bundle.generation_config().profile_id,
            config.promoted_profile.profile_id
        );
        assert_eq!(bundle.model(), &run.training_outcome.trained_model);
        assert_eq!(
            bundle.descriptor().stable_digest(),
            run.model_descriptor.stable_digest()
        );
        assert_eq!(encoded_ids, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(bundle.tokenizer().decode(encoded.as_slice()), "abcd efg h");
        Ok(())
    }

    #[test]
    fn promoted_parameter_golf_runtime_bundle_generates_greedy_and_seeded_sample_outputs()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
        let output_dir = tempdir()?;
        write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;
        let bundle = ParameterGolfPromotedRuntimeBundle::load_dir(output_dir.path())?;

        let mut greedy_options = bundle.default_greedy_generation_options();
        greedy_options.max_new_tokens = 4;
        let greedy = bundle.generate_text("abcd", &greedy_options)?;
        assert_eq!(
            greedy.termination,
            ParameterGolfPromotedGenerationTermination::MaxNewTokens
        );
        // Commit 504f63df gated promoted decode to admitted tokenizer ids,
        // so generation now refuses Unused/<reserved_*> ids and emits the
        // best admitted pieces instead.
        assert_eq!(greedy.text, "gbgb");

        let mut sample_options = bundle.default_seeded_sampling_options(42);
        sample_options.max_new_tokens = 4;
        let sample_left = bundle.generate_text("abcd", &sample_options)?;
        let sample_right = bundle.generate_text("abcd", &sample_options)?;
        assert_eq!(sample_left, sample_right);
        assert_eq!(
            sample_left.termination,
            ParameterGolfPromotedGenerationTermination::MaxNewTokens
        );
        assert_eq!(sample_left.text, "gcgc");
        Ok(())
    }

    #[test]
    fn restored_checkpoint_preserves_promoted_profile_policy() -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
        let restored = restore_parameter_golf_local_reference_checkpoint(
            &fixture,
            &run.training_outcome.final_checkpoint,
        )?;

        assert_eq!(restored.config.promoted_profile, config.promoted_profile);
        assert_eq!(
            restored.latest_checkpoint.manifest.promoted_profile,
            config.promoted_profile
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_split_full_precision_safetensors_roundtrip_restores_model()
    -> Result<(), Box<dyn Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let bytes = export_parameter_golf_full_precision_model_bytes(&model)?;
        let restored = restore_parameter_golf_model_from_safetensors(&model, bytes.as_slice())?;
        assert_eq!(restored, model);
        Ok(())
    }

    #[test]
    fn parameter_golf_banked_full_precision_safetensors_roundtrip_restores_model()
    -> Result<(), Box<dyn Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let bytes = export_parameter_golf_banked_full_precision_model_bytes(&model)?;
        let restored = restore_parameter_golf_model_from_safetensors(&model, bytes.as_slice())?;
        assert_eq!(restored, model);
        Ok(())
    }

    #[test]
    fn parameter_golf_banked_full_precision_safetensors_roundtrip_restores_banked_weights()
    -> Result<(), Box<dyn Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let expected = model.banked_weights()?;
        let bytes = export_parameter_golf_banked_full_precision_model_bytes(&model)?;
        let restored =
            restore_parameter_golf_banked_weights_from_safetensors(&model, bytes.as_slice())?;
        assert_eq!(restored.as_ref(), Some(&expected));
        Ok(())
    }
}
