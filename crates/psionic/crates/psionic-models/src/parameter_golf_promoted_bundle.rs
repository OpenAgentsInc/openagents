use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use half::f16;
use psionic_runtime::{SamplingPolicy, SamplingStrategy, TokenSampler};
use safetensors::{Dtype as SafeTensorsDType, SafeTensors};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelDescriptor, ParameterGolfBankedWeights, ParameterGolfDeterministicInitializer,
    ParameterGolfExecutionError, ParameterGolfModelDescriptor, ParameterGolfModelError,
    ParameterGolfPromotedProfileContract, ParameterGolfPromotedProfileKind,
    ParameterGolfReferenceModel, ParameterGolfWeights, TokenId, TokenSequence, TokenVocabulary,
    TokenizerBoundary,
};

/// Stable schema version for the promoted PGOLF tokenizer asset.
pub const PARAMETER_GOLF_PROMOTED_TOKENIZER_ASSET_SCHEMA_VERSION: &str =
    "psionic.parameter_golf_promoted_tokenizer_asset.v1";
/// Stable schema version for the promoted PGOLF generation config.
pub const PARAMETER_GOLF_PROMOTED_GENERATION_CONFIG_SCHEMA_VERSION: &str =
    "psionic.parameter_golf_promoted_generation_config.v1";
/// Stable schema version for the promoted PGOLF bundle manifest.
pub const PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.parameter_golf_promoted_bundle_manifest.v1";
/// Canonical manifest filename for one promoted PGOLF runtime bundle.
pub const PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_FILENAME: &str =
    "parameter_golf_promoted_bundle_manifest.json";

const PARAMETER_GOLF_WEIGHT_SURFACE_KEY: &str = "psionic.parameter_golf.weight_surface";
const PARAMETER_GOLF_SPLIT_WEIGHT_SURFACE: &str = "split_f32_v1";
const PARAMETER_GOLF_BANKED_WEIGHT_SURFACE: &str = "banked_f32_v1";

/// Runtime-loadable tokenizer asset format admitted by the promoted PGOLF bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedTokenizerAssetFormat {
    /// One JSON piece table carrying the full runtime vocabulary.
    SentencePiecePieceTableJson,
}

/// Tokenizer family admitted by the promoted PGOLF bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedTokenizerFamily {
    /// SentencePiece-style tokenization.
    SentencePiece,
}

/// Token role admitted by the promoted PGOLF runtime tokenizer asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedTokenizerTokenKind {
    Normal,
    Byte,
    Control,
    Unknown,
    Unused,
}

/// One ordered tokenizer piece inside the promoted PGOLF runtime asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedTokenizerToken {
    /// Stable token id.
    pub token_id: u32,
    /// Stable token piece string.
    pub piece: String,
    /// Runtime token role.
    pub kind: ParameterGolfPromotedTokenizerTokenKind,
}

/// Runtime-loadable tokenizer asset emitted beside one promoted PGOLF model bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedTokenizerAsset {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Stable tokenizer identifier.
    pub tokenizer_id: String,
    /// Stable tokenizer revision or version.
    pub tokenizer_version: String,
    /// Tokenizer family.
    pub family: ParameterGolfPromotedTokenizerFamily,
    /// Runtime asset format.
    pub asset_format: ParameterGolfPromotedTokenizerAssetFormat,
    /// Vocabulary size for the emitted runtime tokenizer.
    pub vocab_size: u32,
    /// Whether BOS should be injected by default.
    pub add_bos: bool,
    /// Whether EOS should be injected by default.
    pub add_eos: bool,
    /// Optional BOS token id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bos_token_id: Option<u32>,
    /// Ordered EOS token ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eos_token_ids: Vec<u32>,
    /// Optional PAD token id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad_token_id: Option<u32>,
    /// Optional unknown-token id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown_token_id: Option<u32>,
    /// Full runtime vocabulary in stable token-id order.
    pub pieces: Vec<ParameterGolfPromotedTokenizerToken>,
    /// Stable digest over the tokenizer contract itself.
    pub tokenizer_digest: String,
    /// Stable digest over the full asset payload.
    pub asset_digest: String,
    /// Human-readable detail for operators and audits.
    pub detail: String,
}

impl ParameterGolfPromotedTokenizerAsset {
    /// Returns the stable digest over the logical tokenizer contract.
    #[must_use]
    pub fn tokenizer_contract_digest(&self) -> String {
        stable_digest(
            b"psionic_parameter_golf_promoted_tokenizer_contract|",
            &(
                self.profile_id.as_str(),
                self.tokenizer_id.as_str(),
                self.tokenizer_version.as_str(),
                self.family,
                self.vocab_size,
                self.add_bos,
                self.add_eos,
                self.bos_token_id,
                self.eos_token_ids.as_slice(),
                self.pad_token_id,
                self.unknown_token_id,
                self.pieces.as_slice(),
            ),
        )
    }

    /// Returns the stable digest over the full asset payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.asset_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_promoted_tokenizer_asset|",
            &canonical,
        )
    }

    /// Validates the emitted tokenizer asset.
    pub fn validate(&self) -> Result<(), ParameterGolfPromotedBundleError> {
        require_exact_schema(
            self.schema_version.as_str(),
            PARAMETER_GOLF_PROMOTED_TOKENIZER_ASSET_SCHEMA_VERSION,
            "tokenizer_asset.schema_version",
        )?;
        require_nonempty(self.profile_id.as_str(), "tokenizer_asset.profile_id")?;
        require_nonempty(self.tokenizer_id.as_str(), "tokenizer_asset.tokenizer_id")?;
        require_nonempty(
            self.tokenizer_version.as_str(),
            "tokenizer_asset.tokenizer_version",
        )?;
        require_nonempty(
            self.tokenizer_digest.as_str(),
            "tokenizer_asset.tokenizer_digest",
        )?;
        require_nonempty(self.asset_digest.as_str(), "tokenizer_asset.asset_digest")?;
        require_nonempty(self.detail.as_str(), "tokenizer_asset.detail")?;
        if self.vocab_size == 0 {
            return Err(ParameterGolfPromotedBundleError::InvalidValue {
                field: String::from("tokenizer_asset.vocab_size"),
                detail: String::from("vocab_size must be positive"),
            });
        }
        if self.pieces.len() != self.vocab_size as usize {
            return Err(ParameterGolfPromotedBundleError::InvalidValue {
                field: String::from("tokenizer_asset.pieces"),
                detail: format!(
                    "piece count {} must exactly match vocab size {}",
                    self.pieces.len(),
                    self.vocab_size
                ),
            });
        }
        for (expected_id, piece) in self.pieces.iter().enumerate() {
            if piece.token_id != expected_id as u32 {
                return Err(ParameterGolfPromotedBundleError::InvalidValue {
                    field: String::from("tokenizer_asset.pieces"),
                    detail: format!(
                        "piece at position {} carried token id {}",
                        expected_id, piece.token_id
                    ),
                });
            }
        }
        for (field, token_id) in [
            ("tokenizer_asset.bos_token_id", self.bos_token_id),
            ("tokenizer_asset.pad_token_id", self.pad_token_id),
            ("tokenizer_asset.unknown_token_id", self.unknown_token_id),
        ] {
            if let Some(token_id) = token_id {
                if token_id >= self.vocab_size {
                    return Err(ParameterGolfPromotedBundleError::InvalidValue {
                        field: String::from(field),
                        detail: format!(
                            "token id {} exceeds vocab size {}",
                            token_id, self.vocab_size
                        ),
                    });
                }
            }
        }
        for token_id in &self.eos_token_ids {
            if *token_id >= self.vocab_size {
                return Err(ParameterGolfPromotedBundleError::InvalidValue {
                    field: String::from("tokenizer_asset.eos_token_ids"),
                    detail: format!(
                        "token id {} exceeds vocab size {}",
                        token_id, self.vocab_size
                    ),
                });
            }
        }
        if self.tokenizer_digest != self.tokenizer_contract_digest() {
            return Err(ParameterGolfPromotedBundleError::DigestMismatch {
                field: String::from("tokenizer_asset.tokenizer_digest"),
                expected: self.tokenizer_contract_digest(),
                actual: self.tokenizer_digest.clone(),
            });
        }
        if self.asset_digest != self.stable_digest() {
            return Err(ParameterGolfPromotedBundleError::DigestMismatch {
                field: String::from("tokenizer_asset.asset_digest"),
                expected: self.stable_digest(),
                actual: self.asset_digest.clone(),
            });
        }
        Ok(())
    }
}

/// Default generation config emitted with one promoted PGOLF model bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedGenerationConfig {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Stable prompt-format identifier.
    pub prompt_format: String,
    /// Maximum supported context length.
    pub max_context: usize,
    /// Default max new-token budget.
    pub default_max_new_tokens: usize,
    /// Default sampling mode label.
    pub default_sampling_mode: String,
    /// Default temperature for seeded sampling paths.
    pub default_temperature: f32,
    /// Optional top-k cap for seeded sampling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_top_k: Option<usize>,
    /// Optional bounded trailing attention window admitted by the bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounded_attention_window_tokens: Option<usize>,
    /// Whether the runtime should stop on EOS when one is configured.
    pub stop_on_eos: bool,
    /// Stable digest over the config payload.
    pub config_digest: String,
    /// Human-readable detail for operators and audits.
    pub detail: String,
}

impl ParameterGolfPromotedGenerationConfig {
    /// Returns the stable digest over the config payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.config_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_promoted_generation_config|",
            &canonical,
        )
    }

    /// Validates the emitted generation config.
    pub fn validate(&self) -> Result<(), ParameterGolfPromotedBundleError> {
        require_exact_schema(
            self.schema_version.as_str(),
            PARAMETER_GOLF_PROMOTED_GENERATION_CONFIG_SCHEMA_VERSION,
            "generation_config.schema_version",
        )?;
        require_nonempty(self.profile_id.as_str(), "generation_config.profile_id")?;
        require_nonempty(
            self.prompt_format.as_str(),
            "generation_config.prompt_format",
        )?;
        require_nonempty(
            self.default_sampling_mode.as_str(),
            "generation_config.default_sampling_mode",
        )?;
        require_nonempty(
            self.config_digest.as_str(),
            "generation_config.config_digest",
        )?;
        require_nonempty(self.detail.as_str(), "generation_config.detail")?;
        if self.max_context == 0 {
            return Err(ParameterGolfPromotedBundleError::InvalidValue {
                field: String::from("generation_config.max_context"),
                detail: String::from("max_context must be positive"),
            });
        }
        if self.default_max_new_tokens == 0 {
            return Err(ParameterGolfPromotedBundleError::InvalidValue {
                field: String::from("generation_config.default_max_new_tokens"),
                detail: String::from("default_max_new_tokens must be positive"),
            });
        }
        if !self.default_temperature.is_finite() || self.default_temperature < 0.0 {
            return Err(ParameterGolfPromotedBundleError::InvalidValue {
                field: String::from("generation_config.default_temperature"),
                detail: String::from("default_temperature must be finite and non-negative"),
            });
        }
        if let Some(top_k) = self.default_top_k {
            if top_k == 0 {
                return Err(ParameterGolfPromotedBundleError::InvalidValue {
                    field: String::from("generation_config.default_top_k"),
                    detail: String::from("default_top_k must be positive when present"),
                });
            }
        }
        if let Some(window_tokens) = self.bounded_attention_window_tokens {
            if window_tokens == 0 || window_tokens > self.max_context {
                return Err(ParameterGolfPromotedBundleError::InvalidValue {
                    field: String::from("generation_config.bounded_attention_window_tokens"),
                    detail: format!(
                        "bounded_attention_window_tokens must be in 1..={} when present",
                        self.max_context
                    ),
                });
            }
        }
        if self.config_digest != self.stable_digest() {
            return Err(ParameterGolfPromotedBundleError::DigestMismatch {
                field: String::from("generation_config.config_digest"),
                expected: self.stable_digest(),
                actual: self.config_digest.clone(),
            });
        }
        Ok(())
    }
}

/// One file artifact referenced by the promoted PGOLF bundle manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedBundleArtifactRef {
    /// Relative file path inside the bundle directory.
    pub relative_path: String,
    /// Raw SHA-256 over the referenced file bytes.
    pub sha256: String,
}

impl ParameterGolfPromotedBundleArtifactRef {
    fn validate(&self, field: &str) -> Result<(), ParameterGolfPromotedBundleError> {
        require_relative_path(self.relative_path.as_str(), field)?;
        require_nonempty(self.sha256.as_str(), &format!("{field}.sha256"))?;
        Ok(())
    }
}

/// File inventory for the promoted PGOLF model bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedBundleArtifacts {
    pub descriptor: ParameterGolfPromotedBundleArtifactRef,
    pub model: ParameterGolfPromotedBundleArtifactRef,
    pub tokenizer_asset: ParameterGolfPromotedBundleArtifactRef,
    pub generation_config: ParameterGolfPromotedBundleArtifactRef,
    pub profile_contract: ParameterGolfPromotedBundleArtifactRef,
    pub training_config: ParameterGolfPromotedBundleArtifactRef,
    pub summary: ParameterGolfPromotedBundleArtifactRef,
    pub checkpoint_manifest: ParameterGolfPromotedBundleArtifactRef,
    pub checkpoint_surface_report: ParameterGolfPromotedBundleArtifactRef,
    pub resume_proof: ParameterGolfPromotedBundleArtifactRef,
}

impl ParameterGolfPromotedBundleArtifacts {
    fn validate(&self) -> Result<(), ParameterGolfPromotedBundleError> {
        self.descriptor
            .validate("bundle_manifest.artifacts.descriptor")?;
        self.model.validate("bundle_manifest.artifacts.model")?;
        self.tokenizer_asset
            .validate("bundle_manifest.artifacts.tokenizer_asset")?;
        self.generation_config
            .validate("bundle_manifest.artifacts.generation_config")?;
        self.profile_contract
            .validate("bundle_manifest.artifacts.profile_contract")?;
        self.training_config
            .validate("bundle_manifest.artifacts.training_config")?;
        self.summary.validate("bundle_manifest.artifacts.summary")?;
        self.checkpoint_manifest
            .validate("bundle_manifest.artifacts.checkpoint_manifest")?;
        self.checkpoint_surface_report
            .validate("bundle_manifest.artifacts.checkpoint_surface_report")?;
        self.resume_proof
            .validate("bundle_manifest.artifacts.resume_proof")?;
        Ok(())
    }
}

/// Training lineage and provenance carried by the promoted PGOLF bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedBundleLineage {
    /// Stable promoted run id.
    pub run_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Stable final checkpoint ref.
    pub final_checkpoint_ref: String,
    /// Stable final checkpoint manifest digest.
    pub final_checkpoint_manifest_digest: String,
    /// Stable emitted checkpoint artifact digest.
    pub checkpoint_artifact_digest: String,
    /// Stable emitted descriptor digest.
    pub descriptor_digest: String,
    /// Stable training dataset digest.
    pub training_dataset_digest: String,
    /// Stable validation dataset digest.
    pub validation_dataset_digest: String,
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Stable promoted profile kind label.
    pub profile_kind: String,
    /// Human-readable lineage detail.
    pub detail: String,
}

impl ParameterGolfPromotedBundleLineage {
    fn validate(&self) -> Result<(), ParameterGolfPromotedBundleError> {
        require_nonempty(self.run_id.as_str(), "bundle_manifest.lineage.run_id")?;
        require_nonempty(
            self.checkpoint_family.as_str(),
            "bundle_manifest.lineage.checkpoint_family",
        )?;
        require_nonempty(
            self.final_checkpoint_ref.as_str(),
            "bundle_manifest.lineage.final_checkpoint_ref",
        )?;
        require_nonempty(
            self.final_checkpoint_manifest_digest.as_str(),
            "bundle_manifest.lineage.final_checkpoint_manifest_digest",
        )?;
        require_nonempty(
            self.checkpoint_artifact_digest.as_str(),
            "bundle_manifest.lineage.checkpoint_artifact_digest",
        )?;
        require_nonempty(
            self.descriptor_digest.as_str(),
            "bundle_manifest.lineage.descriptor_digest",
        )?;
        require_nonempty(
            self.training_dataset_digest.as_str(),
            "bundle_manifest.lineage.training_dataset_digest",
        )?;
        require_nonempty(
            self.validation_dataset_digest.as_str(),
            "bundle_manifest.lineage.validation_dataset_digest",
        )?;
        require_nonempty(
            self.profile_id.as_str(),
            "bundle_manifest.lineage.profile_id",
        )?;
        require_nonempty(
            self.profile_kind.as_str(),
            "bundle_manifest.lineage.profile_kind",
        )?;
        require_nonempty(self.detail.as_str(), "bundle_manifest.lineage.detail")?;
        Ok(())
    }
}

/// Canonical promoted PGOLF bundle manifest emitted by training and consumed by
/// later runtime or serve loaders.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedBundleManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable promoted family id.
    pub family_id: String,
    /// Stable model family.
    pub model_family: String,
    /// Stable model id.
    pub model_id: String,
    /// Stable model revision.
    pub model_revision: String,
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Stable promoted profile kind label.
    pub profile_kind: String,
    /// File inventory for the bundle.
    pub artifacts: ParameterGolfPromotedBundleArtifacts,
    /// Training lineage and provenance.
    pub lineage: ParameterGolfPromotedBundleLineage,
    /// Human-readable manifest detail.
    pub detail: String,
    /// Stable digest over the bundle manifest payload.
    pub bundle_digest: String,
}

impl ParameterGolfPromotedBundleManifest {
    /// Returns the stable digest over the bundle manifest payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.bundle_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_promoted_bundle_manifest|",
            &canonical,
        )
    }

    /// Validates the promoted PGOLF bundle manifest.
    pub fn validate(&self) -> Result<(), ParameterGolfPromotedBundleError> {
        require_exact_schema(
            self.schema_version.as_str(),
            PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_SCHEMA_VERSION,
            "bundle_manifest.schema_version",
        )?;
        require_nonempty(self.bundle_id.as_str(), "bundle_manifest.bundle_id")?;
        require_nonempty(self.family_id.as_str(), "bundle_manifest.family_id")?;
        require_nonempty(self.model_family.as_str(), "bundle_manifest.model_family")?;
        require_nonempty(self.model_id.as_str(), "bundle_manifest.model_id")?;
        require_nonempty(
            self.model_revision.as_str(),
            "bundle_manifest.model_revision",
        )?;
        require_nonempty(self.profile_id.as_str(), "bundle_manifest.profile_id")?;
        require_nonempty(self.profile_kind.as_str(), "bundle_manifest.profile_kind")?;
        self.artifacts.validate()?;
        self.lineage.validate()?;
        require_nonempty(self.detail.as_str(), "bundle_manifest.detail")?;
        require_nonempty(self.bundle_digest.as_str(), "bundle_manifest.bundle_digest")?;
        if self.bundle_digest != self.stable_digest() {
            return Err(ParameterGolfPromotedBundleError::DigestMismatch {
                field: String::from("bundle_manifest.bundle_digest"),
                expected: self.stable_digest(),
                actual: self.bundle_digest.clone(),
            });
        }
        Ok(())
    }
}

/// Validation error for the promoted PGOLF bundle contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ParameterGolfPromotedBundleError {
    #[error("promoted PGOLF bundle field `{field}` is missing")]
    MissingField { field: String },
    #[error("promoted PGOLF bundle field `{field}` expected schema `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error(
        "promoted PGOLF bundle digest mismatch for `{field}`: expected `{expected}`, found `{actual}`"
    )]
    DigestMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("promoted PGOLF bundle field `{field}` is invalid: {detail}")]
    InvalidValue { field: String, detail: String },
}

/// Public runtime load failure for the promoted PGOLF bundle surface.
#[derive(Debug, Error)]
pub enum ParameterGolfPromotedRuntimeLoadError {
    #[error("failed to read promoted PGOLF bundle artifact `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse promoted PGOLF bundle artifact `{path}` as {context}: {source}")]
    Json {
        path: PathBuf,
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Bundle(#[from] ParameterGolfPromotedBundleError),
    #[error(transparent)]
    Model(#[from] ParameterGolfModelError),
    #[error("invalid promoted PGOLF runtime bundle: {detail}")]
    InvalidBundle { detail: String },
    #[error(
        "promoted PGOLF runtime bundle field `{field}` mismatched: expected `{expected}`, found `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("promoted PGOLF runtime safetensors load failed for `{context}`: {message}")]
    SafeTensors {
        context: &'static str,
        message: String,
    },
    #[error("promoted PGOLF runtime tensor `{parameter_id}` was missing from the model artifact")]
    MissingArtifactTensor { parameter_id: String },
    #[error(
        "promoted PGOLF runtime tensor `{parameter_id}` had shape {actual:?}; expected {expected:?}"
    )]
    ArtifactTensorShape {
        parameter_id: String,
        actual: Vec<usize>,
        expected: Vec<usize>,
    },
    #[error("promoted PGOLF runtime tensor decode failed for `{parameter_id}`: {detail}")]
    TensorDecode {
        parameter_id: String,
        detail: String,
    },
}

/// Public runtime tokenizer bound to one promoted PGOLF bundle asset.
#[derive(Clone, Debug)]
pub struct ParameterGolfPromotedRuntimeTokenizer {
    asset: ParameterGolfPromotedTokenizerAsset,
    vocabulary: TokenVocabulary,
    lookup: BTreeMap<String, TokenId>,
    eos_token_ids: Vec<TokenId>,
    generation_allowed_token_ids: Vec<u32>,
    generation_disallowed_token_ids: Vec<u32>,
}

impl ParameterGolfPromotedRuntimeTokenizer {
    /// Builds the runtime tokenizer directly from one emitted tokenizer asset.
    pub fn from_asset(
        asset: ParameterGolfPromotedTokenizerAsset,
    ) -> Result<Self, ParameterGolfPromotedRuntimeLoadError> {
        asset.validate()?;
        if asset.family != ParameterGolfPromotedTokenizerFamily::SentencePiece {
            return Err(ParameterGolfPromotedRuntimeLoadError::InvalidBundle {
                detail: format!("unsupported promoted tokenizer family `{:?}`", asset.family),
            });
        }
        if asset.asset_format
            != ParameterGolfPromotedTokenizerAssetFormat::SentencePiecePieceTableJson
        {
            return Err(ParameterGolfPromotedRuntimeLoadError::InvalidBundle {
                detail: format!(
                    "unsupported promoted tokenizer asset format `{:?}`",
                    asset.asset_format
                ),
            });
        }

        let mut lookup = BTreeMap::new();
        for piece in &asset.pieces {
            if let Some(previous) = lookup.insert(piece.piece.clone(), TokenId(piece.token_id)) {
                return Err(ParameterGolfPromotedRuntimeLoadError::InvalidBundle {
                    detail: format!(
                        "duplicate tokenizer piece `{}` mapped to ids {} and {}",
                        piece.piece,
                        previous.as_u32(),
                        piece.token_id
                    ),
                });
            }
        }

        let fallback = asset.unknown_token_id.unwrap_or(0);
        let vocabulary = TokenVocabulary::new(
            asset
                .pieces
                .iter()
                .map(|piece| piece.piece.clone())
                .collect(),
            TokenId(asset.pad_token_id.unwrap_or(fallback)),
            TokenId(asset.bos_token_id.unwrap_or(fallback)),
            TokenId(asset.eos_token_ids.first().copied().unwrap_or(fallback)),
            TokenId(fallback),
        );
        let eos_token_ids = asset
            .eos_token_ids
            .iter()
            .copied()
            .map(TokenId)
            .collect::<Vec<_>>();
        let generation_disallowed_token_ids = asset
            .pieces
            .iter()
            .filter_map(|piece| match piece.kind {
                ParameterGolfPromotedTokenizerTokenKind::Normal
                | ParameterGolfPromotedTokenizerTokenKind::Byte => None,
                ParameterGolfPromotedTokenizerTokenKind::Control => {
                    if asset.eos_token_ids.contains(&piece.token_id) {
                        None
                    } else {
                        Some(piece.token_id)
                    }
                }
                ParameterGolfPromotedTokenizerTokenKind::Unknown
                | ParameterGolfPromotedTokenizerTokenKind::Unused => Some(piece.token_id),
            })
            .collect::<Vec<_>>();
        let generation_allowed_token_ids = asset
            .pieces
            .iter()
            .filter_map(|piece| match piece.kind {
                ParameterGolfPromotedTokenizerTokenKind::Normal
                | ParameterGolfPromotedTokenizerTokenKind::Byte => Some(piece.token_id),
                ParameterGolfPromotedTokenizerTokenKind::Control => asset
                    .eos_token_ids
                    .contains(&piece.token_id)
                    .then_some(piece.token_id),
                ParameterGolfPromotedTokenizerTokenKind::Unknown
                | ParameterGolfPromotedTokenizerTokenKind::Unused => None,
            })
            .collect::<Vec<_>>();
        Ok(Self {
            asset,
            vocabulary,
            lookup,
            eos_token_ids,
            generation_allowed_token_ids,
            generation_disallowed_token_ids,
        })
    }

    /// Returns the emitted tokenizer asset.
    #[must_use]
    pub fn asset(&self) -> &ParameterGolfPromotedTokenizerAsset {
        &self.asset
    }

    /// Returns token ids that inference should refuse by default.
    #[must_use]
    pub fn generation_disallowed_token_ids(&self) -> &[u32] {
        self.generation_disallowed_token_ids.as_slice()
    }

    /// Returns token ids that inference may emit by default.
    #[must_use]
    pub fn generation_allowed_token_ids(&self) -> &[u32] {
        self.generation_allowed_token_ids.as_slice()
    }

    /// Encodes text with explicit BOS/EOS injection.
    #[must_use]
    pub fn encode_with_special_tokens(
        &self,
        text: &str,
        add_bos: bool,
        add_eos: bool,
    ) -> TokenSequence {
        let mut tokens = Vec::new();
        if add_bos {
            if let Some(token_id) = self.asset.bos_token_id {
                tokens.push(TokenId(token_id));
            }
        }
        for word in text.split_whitespace() {
            self.encode_word(word, &mut tokens);
        }
        if add_eos {
            if let Some(token_id) = self.asset.eos_token_ids.first().copied() {
                tokens.push(TokenId(token_id));
            }
        }
        TokenSequence::new(tokens)
    }

    /// Encodes text using the tokenizer defaults emitted by the training bundle.
    #[must_use]
    pub fn encode_with_defaults(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, self.asset.add_bos, self.asset.add_eos)
    }

    /// Returns whether one token is admitted as EOS by the emitted asset.
    #[must_use]
    pub fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.eos_token_ids.contains(&token)
    }

    fn encode_word(&self, word: &str, output: &mut Vec<TokenId>) {
        if word.is_empty() {
            return;
        }
        let mut remaining = word;
        let mut first_piece = true;
        while !remaining.is_empty() {
            let mut matched = None;
            for end in remaining
                .char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(remaining.len()))
                .rev()
            {
                if end == 0 {
                    continue;
                }
                let candidate = &remaining[..end];
                if first_piece {
                    let boundary_candidate = format!("▁{candidate}");
                    if let Some(token_id) = self.lookup.get(boundary_candidate.as_str()) {
                        matched = Some((end, *token_id));
                        break;
                    }
                }
                if let Some(token_id) = self.lookup.get(candidate) {
                    matched = Some((end, *token_id));
                    break;
                }
            }
            match matched {
                Some((matched_len, token_id)) => {
                    output.push(token_id);
                    remaining = &remaining[matched_len..];
                    first_piece = false;
                }
                None => {
                    output.push(self.vocabulary.unknown_id());
                    break;
                }
            }
        }
    }
}

impl TokenizerBoundary for ParameterGolfPromotedRuntimeTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, false, false)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        let mut output = String::new();
        for token in tokens {
            if is_runtime_special_token(&self.asset, self.eos_token_ids.as_slice(), *token) {
                continue;
            }
            let Some(piece) = self.vocabulary.token(*token) else {
                continue;
            };
            if let Some(boundary_stripped) = piece.strip_prefix('▁') {
                if !output.is_empty() {
                    output.push(' ');
                }
                output.push_str(boundary_stripped);
                continue;
            }
            output.push_str(piece);
        }
        output
    }

    fn append_decoded_token(&self, text: &mut String, token: TokenId) {
        if is_runtime_special_token(&self.asset, self.eos_token_ids.as_slice(), token) {
            return;
        }
        let Some(piece) = self.vocabulary.token(token) else {
            return;
        };
        if let Some(boundary_stripped) = piece.strip_prefix('▁') {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(boundary_stripped);
            return;
        }
        text.push_str(piece);
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        &self.vocabulary
    }
}

/// Public runtime bundle containing the emitted promoted PGOLF artifacts.
#[derive(Clone, Debug)]
pub struct ParameterGolfPromotedRuntimeBundle {
    bundle_root: PathBuf,
    manifest: ParameterGolfPromotedBundleManifest,
    profile_contract: ParameterGolfPromotedProfileContract,
    descriptor: ParameterGolfModelDescriptor,
    tokenizer: ParameterGolfPromotedRuntimeTokenizer,
    generation_config: ParameterGolfPromotedGenerationConfig,
    model: ParameterGolfReferenceModel,
}

impl ParameterGolfPromotedRuntimeBundle {
    /// Loads one promoted PGOLF runtime bundle from a directory.
    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, ParameterGolfPromotedRuntimeLoadError> {
        let bundle_root = path.as_ref().to_path_buf();
        let manifest_path = bundle_root.join(PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_FILENAME);
        let manifest: ParameterGolfPromotedBundleManifest =
            read_json_file(manifest_path.as_path(), "promoted bundle manifest")?;
        manifest.validate()?;

        let profile_contract: ParameterGolfPromotedProfileContract = read_json_artifact(
            bundle_root.as_path(),
            &manifest.artifacts.profile_contract,
            "promoted profile contract",
        )?;
        let descriptor: ParameterGolfModelDescriptor = read_json_artifact(
            bundle_root.as_path(),
            &manifest.artifacts.descriptor,
            "promoted model descriptor",
        )?;
        let tokenizer_asset: ParameterGolfPromotedTokenizerAsset = read_json_artifact(
            bundle_root.as_path(),
            &manifest.artifacts.tokenizer_asset,
            "promoted tokenizer asset",
        )?;
        let generation_config: ParameterGolfPromotedGenerationConfig = read_json_artifact(
            bundle_root.as_path(),
            &manifest.artifacts.generation_config,
            "promoted generation config",
        )?;
        generation_config.validate()?;

        let model_bytes = read_bytes_artifact(
            bundle_root.as_path(),
            &manifest.artifacts.model,
            "promoted model weights",
        )?;
        let tokenizer = ParameterGolfPromotedRuntimeTokenizer::from_asset(tokenizer_asset)?;

        validate_loaded_bundle_relationships(
            &manifest,
            &profile_contract,
            &descriptor,
            tokenizer.asset(),
            &generation_config,
        )?;

        let model = restore_parameter_golf_model_from_safetensors(&descriptor, &model_bytes)?;
        if model.descriptor().stable_digest() != descriptor.stable_digest() {
            return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
                field: String::from("descriptor.digest"),
                expected: descriptor.stable_digest(),
                actual: model.descriptor().stable_digest(),
            });
        }

        Ok(Self {
            bundle_root,
            manifest,
            profile_contract,
            descriptor,
            tokenizer,
            generation_config,
            model,
        })
    }

    /// Returns the bundle root directory.
    #[must_use]
    pub fn bundle_root(&self) -> &Path {
        self.bundle_root.as_path()
    }

    /// Returns the canonical promoted bundle manifest.
    #[must_use]
    pub fn manifest(&self) -> &ParameterGolfPromotedBundleManifest {
        &self.manifest
    }

    /// Returns the promoted profile contract.
    #[must_use]
    pub fn profile_contract(&self) -> &ParameterGolfPromotedProfileContract {
        &self.profile_contract
    }

    /// Returns the model descriptor bound to the runtime bundle.
    #[must_use]
    pub fn descriptor(&self) -> &ParameterGolfModelDescriptor {
        &self.descriptor
    }

    /// Returns the runtime tokenizer bound to the bundle.
    #[must_use]
    pub fn tokenizer(&self) -> &ParameterGolfPromotedRuntimeTokenizer {
        &self.tokenizer
    }

    /// Returns the default generation config emitted by training.
    #[must_use]
    pub fn generation_config(&self) -> &ParameterGolfPromotedGenerationConfig {
        &self.generation_config
    }

    /// Returns the restored runtime model object.
    #[must_use]
    pub fn model(&self) -> &ParameterGolfReferenceModel {
        &self.model
    }

    /// Returns greedy generation defaults for this promoted bundle.
    #[must_use]
    pub fn default_greedy_generation_options(&self) -> ParameterGolfPromotedGenerationOptions {
        ParameterGolfPromotedGenerationOptions {
            max_new_tokens: self.generation_config.default_max_new_tokens,
            stop_on_eos: self.generation_config.stop_on_eos,
            sampling_policy: SamplingPolicy {
                strategy: SamplingStrategy::Greedy,
                temperature: None,
                top_k: None,
                top_p: None,
                min_p: None,
                typical_p: None,
                mirostat: None,
                mirostat_tau: None,
                mirostat_eta: None,
                repeat_penalty: None,
                repeat_last_n: None,
                presence_penalty: None,
                frequency_penalty: None,
                seed: None,
            },
        }
    }

    /// Returns seeded sampling defaults for this promoted bundle.
    #[must_use]
    pub fn default_seeded_sampling_options(
        &self,
        seed: u64,
    ) -> ParameterGolfPromotedGenerationOptions {
        let temperature = if self.generation_config.default_temperature <= 1e-6 {
            0.8
        } else {
            self.generation_config.default_temperature
        };
        ParameterGolfPromotedGenerationOptions {
            max_new_tokens: self.generation_config.default_max_new_tokens,
            stop_on_eos: self.generation_config.stop_on_eos,
            sampling_policy: SamplingPolicy {
                strategy: SamplingStrategy::Sample,
                temperature: Some(temperature),
                top_k: self.generation_config.default_top_k,
                top_p: None,
                min_p: None,
                typical_p: None,
                mirostat: None,
                mirostat_tau: None,
                mirostat_eta: None,
                repeat_penalty: None,
                repeat_last_n: None,
                presence_penalty: None,
                frequency_penalty: None,
                seed: Some(seed),
            },
        }
    }

    /// Generates text locally from one plain-text prompt through the emitted
    /// tokenizer asset and restored promoted PGOLF runtime model.
    pub fn generate_text(
        &self,
        prompt: &str,
        options: &ParameterGolfPromotedGenerationOptions,
    ) -> Result<ParameterGolfPromotedGenerationOutput, ParameterGolfPromotedGenerationError> {
        let prompt_tokens = self.tokenizer.encode_with_defaults(prompt);
        self.generate_tokens(prompt_tokens, options)
    }

    /// Generates text locally from one pre-tokenized prompt.
    pub fn generate_tokens(
        &self,
        prompt_tokens: TokenSequence,
        options: &ParameterGolfPromotedGenerationOptions,
    ) -> Result<ParameterGolfPromotedGenerationOutput, ParameterGolfPromotedGenerationError> {
        options.validate()?;
        if prompt_tokens.is_empty() {
            return Err(ParameterGolfPromotedGenerationError::EmptyPrompt);
        }
        let max_context = self.generation_config.max_context;
        if prompt_tokens.len() > max_context {
            return Err(ParameterGolfPromotedGenerationError::PromptTooLong {
                prompt_tokens: prompt_tokens.len(),
                max_context,
            });
        }

        let total_history_capacity = prompt_tokens
            .len()
            .saturating_add(options.max_new_tokens)
            .min(max_context);
        let mut history = Vec::with_capacity(total_history_capacity);
        history.extend(prompt_tokens.as_slice().iter().map(|token| token.as_u32()));
        let bounded_history_capacity = self
            .generation_config
            .bounded_attention_window_tokens
            .unwrap_or(max_context)
            .min(max_context);
        let mut bounded_history = Vec::with_capacity(bounded_history_capacity);
        let mut generated_tokens = Vec::with_capacity(options.max_new_tokens);
        let mut sampler = TokenSampler::new(&options.sampling_policy);
        let generation_allowed_token_ids = self.tokenizer.generation_allowed_token_ids();
        let termination = loop {
            if generated_tokens.len() >= options.max_new_tokens {
                break ParameterGolfPromotedGenerationTermination::MaxNewTokens;
            }
            if history.len() >= max_context {
                break ParameterGolfPromotedGenerationTermination::ContextLimit;
            }
            let logits = if let Some(window_tokens) =
                self.generation_config.bounded_attention_window_tokens
            {
                let start = history.len().saturating_sub(window_tokens);
                bounded_history.clear();
                bounded_history.extend_from_slice(&history[start..]);
                self.model.forward_logits_with_attention_window(
                    std::slice::from_ref(&bounded_history),
                    bounded_history.len(),
                )?
            } else {
                self.model.forward_logits(std::slice::from_ref(&history))?
            };
            let width = logits.width();
            let sequence_length = logits.sequence_length();
            let last_row_start = sequence_length
                .checked_sub(1)
                .ok_or(ParameterGolfPromotedGenerationError::MissingLogits)?
                .saturating_mul(width);
            let last_logits = logits
                .values()
                .get(last_row_start..last_row_start.saturating_add(width))
                .ok_or(ParameterGolfPromotedGenerationError::MissingLogits)?;
            let next_token = sampler
                .select_next_token_with_allowed(
                    last_logits,
                    history.as_slice(),
                    generation_allowed_token_ids,
                )
                .ok_or(ParameterGolfPromotedGenerationError::MissingLogits)?;
            let next_token = TokenId(next_token);
            history.push(next_token.as_u32());
            generated_tokens.push(next_token);
            if options.stop_on_eos && self.tokenizer.is_end_of_sequence(next_token) {
                break ParameterGolfPromotedGenerationTermination::EndOfSequence;
            }
        };

        let all_tokens = prompt_tokens
            .as_slice()
            .iter()
            .copied()
            .chain(generated_tokens.iter().copied())
            .collect::<Vec<_>>();
        let generated_sequence = TokenSequence::new(generated_tokens);
        Ok(ParameterGolfPromotedGenerationOutput {
            prompt_tokens,
            generated_tokens: generated_sequence.clone(),
            all_tokens: TokenSequence::new(all_tokens),
            text: self.tokenizer.decode(generated_sequence.as_slice()),
            termination,
        })
    }
}

/// Local decode options for one promoted PGOLF runtime generation call.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterGolfPromotedGenerationOptions {
    /// Maximum number of new tokens to emit.
    pub max_new_tokens: usize,
    /// Whether generation should stop when one emitted EOS token appears.
    pub stop_on_eos: bool,
    /// Runtime token-sampling policy.
    pub sampling_policy: SamplingPolicy,
}

impl ParameterGolfPromotedGenerationOptions {
    /// Creates greedy generation options with the requested budget.
    #[must_use]
    pub fn greedy(max_new_tokens: usize) -> Self {
        Self {
            max_new_tokens,
            stop_on_eos: true,
            sampling_policy: SamplingPolicy {
                strategy: SamplingStrategy::Greedy,
                temperature: None,
                top_k: None,
                top_p: None,
                min_p: None,
                typical_p: None,
                mirostat: None,
                mirostat_tau: None,
                mirostat_eta: None,
                repeat_penalty: None,
                repeat_last_n: None,
                presence_penalty: None,
                frequency_penalty: None,
                seed: None,
            },
        }
    }

    /// Creates seeded sampling options with the requested budget.
    #[must_use]
    pub fn sample(max_new_tokens: usize, seed: u64) -> Self {
        Self {
            max_new_tokens,
            stop_on_eos: true,
            sampling_policy: SamplingPolicy {
                strategy: SamplingStrategy::Sample,
                temperature: None,
                top_k: None,
                top_p: None,
                min_p: None,
                typical_p: None,
                mirostat: None,
                mirostat_tau: None,
                mirostat_eta: None,
                repeat_penalty: None,
                repeat_last_n: None,
                presence_penalty: None,
                frequency_penalty: None,
                seed: Some(seed),
            },
        }
    }

    /// Overrides the runtime sampling policy.
    #[must_use]
    pub fn with_sampling_policy(mut self, sampling_policy: SamplingPolicy) -> Self {
        self.sampling_policy = sampling_policy;
        self
    }

    /// Overrides EOS stopping behavior.
    #[must_use]
    pub fn with_stop_on_eos(mut self, stop_on_eos: bool) -> Self {
        self.stop_on_eos = stop_on_eos;
        self
    }

    fn validate(&self) -> Result<(), ParameterGolfPromotedGenerationError> {
        if self.max_new_tokens == 0 {
            return Err(ParameterGolfPromotedGenerationError::InvalidMaxNewTokens);
        }
        Ok(())
    }
}

/// Local termination reason for one promoted PGOLF generation call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterGolfPromotedGenerationTermination {
    /// Generation stopped because the requested output budget was exhausted.
    MaxNewTokens,
    /// Generation stopped because one emitted token matched the tokenizer EOS set.
    EndOfSequence,
    /// Generation stopped because the runtime context budget was exhausted.
    ContextLimit,
}

/// Public generation output for one promoted PGOLF runtime inference call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterGolfPromotedGenerationOutput {
    /// Prompt tokenization fed into the runtime model.
    pub prompt_tokens: TokenSequence,
    /// Newly generated tokens.
    pub generated_tokens: TokenSequence,
    /// Prompt plus generated tokens in one contiguous sequence.
    pub all_tokens: TokenSequence,
    /// Decoded generated text only.
    pub text: String,
    /// Why generation terminated.
    pub termination: ParameterGolfPromotedGenerationTermination,
}

/// Local generation failure for the promoted PGOLF runtime path.
#[derive(Debug, Error)]
pub enum ParameterGolfPromotedGenerationError {
    #[error("promoted PGOLF generation requires a non-empty prompt")]
    EmptyPrompt,
    #[error("promoted PGOLF generation max_new_tokens must be positive")]
    InvalidMaxNewTokens,
    #[error(
        "promoted PGOLF prompt carried {prompt_tokens} tokens but max_context is {max_context}"
    )]
    PromptTooLong {
        prompt_tokens: usize,
        max_context: usize,
    },
    #[error("promoted PGOLF generation could not recover one next-token logit row")]
    MissingLogits,
    #[error(transparent)]
    Model(#[from] ParameterGolfExecutionError),
}

fn require_nonempty(value: &str, field: &str) -> Result<(), ParameterGolfPromotedBundleError> {
    if value.trim().is_empty() {
        return Err(ParameterGolfPromotedBundleError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn require_exact_schema(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), ParameterGolfPromotedBundleError> {
    require_nonempty(actual, field)?;
    if actual != expected {
        return Err(ParameterGolfPromotedBundleError::SchemaVersionMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn require_relative_path(path: &str, field: &str) -> Result<(), ParameterGolfPromotedBundleError> {
    require_nonempty(path, field)?;
    if path.starts_with('/') || path.split('/').any(|component| component == "..") {
        return Err(ParameterGolfPromotedBundleError::InvalidValue {
            field: String::from(field),
            detail: format!("path `{path}` must stay relative to the bundle directory"),
        });
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("promoted PGOLF bundle value should serialize"));
    hex::encode(hasher.finalize())
}

fn read_json_file<T: for<'de> Deserialize<'de>>(
    path: &Path,
    context: &'static str,
) -> Result<T, ParameterGolfPromotedRuntimeLoadError> {
    let bytes = fs::read(path).map_err(|source| ParameterGolfPromotedRuntimeLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(bytes.as_slice()).map_err(|source| {
        ParameterGolfPromotedRuntimeLoadError::Json {
            path: path.to_path_buf(),
            context,
            source,
        }
    })
}

fn read_json_artifact<T: for<'de> Deserialize<'de>>(
    bundle_root: &Path,
    artifact: &ParameterGolfPromotedBundleArtifactRef,
    context: &'static str,
) -> Result<T, ParameterGolfPromotedRuntimeLoadError> {
    let path = bundle_root.join(artifact.relative_path.as_str());
    let bytes =
        fs::read(path.as_path()).map_err(|source| ParameterGolfPromotedRuntimeLoadError::Io {
            path: path.clone(),
            source,
        })?;
    validate_artifact_sha(path.as_path(), bytes.as_slice(), artifact.sha256.as_str())?;
    serde_json::from_slice(bytes.as_slice()).map_err(|source| {
        ParameterGolfPromotedRuntimeLoadError::Json {
            path,
            context,
            source,
        }
    })
}

fn read_bytes_artifact(
    bundle_root: &Path,
    artifact: &ParameterGolfPromotedBundleArtifactRef,
    _context: &'static str,
) -> Result<Vec<u8>, ParameterGolfPromotedRuntimeLoadError> {
    let path = bundle_root.join(artifact.relative_path.as_str());
    let bytes =
        fs::read(path.as_path()).map_err(|source| ParameterGolfPromotedRuntimeLoadError::Io {
            path: path.clone(),
            source,
        })?;
    validate_artifact_sha(path.as_path(), bytes.as_slice(), artifact.sha256.as_str())?;
    Ok(bytes)
}

fn validate_artifact_sha(
    path: &Path,
    bytes: &[u8],
    expected_sha256: &str,
) -> Result<(), ParameterGolfPromotedRuntimeLoadError> {
    let actual = sha256_hex(bytes);
    if actual != expected_sha256 {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: format!("artifact.sha256:{}", path.display()),
            expected: String::from(expected_sha256),
            actual,
        });
    }
    Ok(())
}

fn validate_loaded_bundle_relationships(
    manifest: &ParameterGolfPromotedBundleManifest,
    profile_contract: &ParameterGolfPromotedProfileContract,
    descriptor: &ParameterGolfModelDescriptor,
    tokenizer_asset: &ParameterGolfPromotedTokenizerAsset,
    generation_config: &ParameterGolfPromotedGenerationConfig,
) -> Result<(), ParameterGolfPromotedRuntimeLoadError> {
    if manifest.profile_id != manifest.lineage.profile_id {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("manifest.profile_id"),
            expected: manifest.lineage.profile_id.clone(),
            actual: manifest.profile_id.clone(),
        });
    }
    if manifest.profile_kind != manifest.lineage.profile_kind {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("manifest.profile_kind"),
            expected: manifest.lineage.profile_kind.clone(),
            actual: manifest.profile_kind.clone(),
        });
    }
    if descriptor.model.family != manifest.model_family {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("descriptor.model.family"),
            expected: manifest.model_family.clone(),
            actual: descriptor.model.family.clone(),
        });
    }
    if descriptor.model.model_id != manifest.model_id {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("descriptor.model.model_id"),
            expected: manifest.model_id.clone(),
            actual: descriptor.model.model_id.clone(),
        });
    }
    if descriptor.model.revision != manifest.model_revision {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("descriptor.model.revision"),
            expected: manifest.model_revision.clone(),
            actual: descriptor.model.revision.clone(),
        });
    }
    if descriptor.stable_digest() != manifest.lineage.descriptor_digest {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("descriptor.digest"),
            expected: manifest.lineage.descriptor_digest.clone(),
            actual: descriptor.stable_digest(),
        });
    }
    if profile_contract.profile_id != manifest.profile_id {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("profile_contract.profile_id"),
            expected: manifest.profile_id.clone(),
            actual: profile_contract.profile_id.clone(),
        });
    }
    if profile_contract.family_id != manifest.family_id {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("profile_contract.family_id"),
            expected: manifest.family_id.clone(),
            actual: profile_contract.family_id.clone(),
        });
    }
    let expected_profile_kind = promoted_profile_kind_label(profile_contract.kind);
    if manifest.profile_kind != expected_profile_kind {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("manifest.profile_kind"),
            expected: expected_profile_kind,
            actual: manifest.profile_kind.clone(),
        });
    }
    if tokenizer_asset.profile_id != manifest.profile_id {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("tokenizer_asset.profile_id"),
            expected: manifest.profile_id.clone(),
            actual: tokenizer_asset.profile_id.clone(),
        });
    }
    if generation_config.profile_id != manifest.profile_id {
        return Err(ParameterGolfPromotedRuntimeLoadError::FieldMismatch {
            field: String::from("generation_config.profile_id"),
            expected: manifest.profile_id.clone(),
            actual: generation_config.profile_id.clone(),
        });
    }
    if descriptor.config != profile_contract.baseline_config {
        return Err(ParameterGolfPromotedRuntimeLoadError::InvalidBundle {
            detail: String::from(
                "descriptor.config drifted from the promoted profile baseline contract",
            ),
        });
    }
    Ok(())
}

fn promoted_profile_kind_label(kind: ParameterGolfPromotedProfileKind) -> String {
    match kind {
        ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder => {
            String::from("general_psion_small_decoder")
        }
        ParameterGolfPromotedProfileKind::StrictPgolfChallenge => {
            String::from("strict_pgolf_challenge")
        }
    }
}

fn restore_parameter_golf_model_from_safetensors(
    descriptor: &ParameterGolfModelDescriptor,
    weights_bytes: &[u8],
) -> Result<ParameterGolfReferenceModel, ParameterGolfPromotedRuntimeLoadError> {
    let baseline_model = ParameterGolfReferenceModel::new(
        ModelDescriptor::new(
            descriptor.model.model_id.clone(),
            descriptor.model.family.clone(),
            descriptor.model.revision.clone(),
        ),
        descriptor.config.clone(),
        ParameterGolfWeights::from_initializer(
            &descriptor.config,
            ParameterGolfDeterministicInitializer::default(),
        )?,
    )?;
    let (_, metadata) = SafeTensors::read_metadata(weights_bytes).map_err(|error| {
        ParameterGolfPromotedRuntimeLoadError::SafeTensors {
            context: "parameter golf promoted runtime metadata",
            message: error.to_string(),
        }
    })?;
    let safetensors = SafeTensors::deserialize(weights_bytes).map_err(|error| {
        ParameterGolfPromotedRuntimeLoadError::SafeTensors {
            context: "parameter golf promoted runtime deserialize",
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
            restore_parameter_golf_model_from_split_safetensors(&baseline_model, &safetensors)
        }
        PARAMETER_GOLF_BANKED_WEIGHT_SURFACE => {
            restore_parameter_golf_model_from_banked_safetensors(&baseline_model, &safetensors)
        }
        other => Err(ParameterGolfPromotedRuntimeLoadError::InvalidBundle {
            detail: format!("unsupported parameter golf weight surface `{other}`"),
        }),
    }
}

fn restore_parameter_golf_model_from_split_safetensors(
    baseline_model: &ParameterGolfReferenceModel,
    safetensors: &SafeTensors<'_>,
) -> Result<ParameterGolfReferenceModel, ParameterGolfPromotedRuntimeLoadError> {
    let mut overrides = BTreeMap::new();
    for parameter in baseline_model
        .weights()
        .parameter_vectors(&baseline_model.descriptor().config)
    {
        let tensor = safetensors
            .tensor(parameter.parameter_id.as_str())
            .map_err(
                |_| ParameterGolfPromotedRuntimeLoadError::MissingArtifactTensor {
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

fn restore_parameter_golf_model_from_banked_safetensors(
    baseline_model: &ParameterGolfReferenceModel,
    safetensors: &SafeTensors<'_>,
) -> Result<ParameterGolfReferenceModel, ParameterGolfPromotedRuntimeLoadError> {
    let weights =
        restore_parameter_golf_banked_weights_from_banked_safetensors(baseline_model, safetensors)?
            .to_split(&baseline_model.descriptor().config)?;
    Ok(ParameterGolfReferenceModel::new(
        baseline_model.descriptor().model.clone(),
        baseline_model.descriptor().config.clone(),
        weights,
    )?)
}

fn restore_parameter_golf_banked_weights_from_banked_safetensors(
    baseline_model: &ParameterGolfReferenceModel,
    safetensors: &SafeTensors<'_>,
) -> Result<ParameterGolfBankedWeights, ParameterGolfPromotedRuntimeLoadError> {
    let config = &baseline_model.descriptor().config;
    let banked_weights = baseline_model.banked_weights()?;
    let mut overrides = BTreeMap::new();
    for parameter in banked_weights.parameter_vectors(config) {
        let tensor = safetensors
            .tensor(parameter.parameter_id.as_str())
            .map_err(
                |_| ParameterGolfPromotedRuntimeLoadError::MissingArtifactTensor {
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

fn validate_tensor_shape(
    parameter_id: &str,
    actual: &[usize],
    expected: &[usize],
) -> Result<(), ParameterGolfPromotedRuntimeLoadError> {
    if actual != expected {
        return Err(ParameterGolfPromotedRuntimeLoadError::ArtifactTensorShape {
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
) -> Result<Vec<f32>, ParameterGolfPromotedRuntimeLoadError> {
    let expected_len = shape.iter().product::<usize>();
    match dtype {
        SafeTensorsDType::F32 => decode_f32_bytes(parameter_id, bytes, expected_len),
        SafeTensorsDType::F16 => decode_f16_bytes(parameter_id, bytes, expected_len),
        other => Err(ParameterGolfPromotedRuntimeLoadError::TensorDecode {
            parameter_id: String::from(parameter_id),
            detail: format!("unsupported dtype `{other}`"),
        }),
    }
}

fn decode_f32_bytes(
    parameter_id: &str,
    bytes: &[u8],
    expected_len: usize,
) -> Result<Vec<f32>, ParameterGolfPromotedRuntimeLoadError> {
    if bytes.len() != expected_len.saturating_mul(4) {
        return Err(ParameterGolfPromotedRuntimeLoadError::TensorDecode {
            parameter_id: String::from(parameter_id),
            detail: format!(
                "tensor had {} bytes; expected {}",
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
) -> Result<Vec<f32>, ParameterGolfPromotedRuntimeLoadError> {
    if bytes.len() != expected_len.saturating_mul(2) {
        return Err(ParameterGolfPromotedRuntimeLoadError::TensorDecode {
            parameter_id: String::from(parameter_id),
            detail: format!(
                "tensor had {} bytes; expected {}",
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

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn is_runtime_special_token(
    asset: &ParameterGolfPromotedTokenizerAsset,
    eos_token_ids: &[TokenId],
    token: TokenId,
) -> bool {
    Some(token.as_u32()) == asset.pad_token_id
        || Some(token.as_u32()) == asset.bos_token_id
        || eos_token_ids.contains(&token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promoted_runtime_tokenizer_refuses_unknown_non_eos_control_and_unused_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut asset = ParameterGolfPromotedTokenizerAsset {
            schema_version: String::from(PARAMETER_GOLF_PROMOTED_TOKENIZER_ASSET_SCHEMA_VERSION),
            profile_id: String::from("pgolf.test"),
            tokenizer_id: String::from("pgolf.test.tokenizer"),
            tokenizer_version: String::from("v1"),
            family: ParameterGolfPromotedTokenizerFamily::SentencePiece,
            asset_format: ParameterGolfPromotedTokenizerAssetFormat::SentencePiecePieceTableJson,
            vocab_size: 5,
            add_bos: false,
            add_eos: false,
            bos_token_id: Some(1),
            eos_token_ids: vec![2],
            pad_token_id: None,
            unknown_token_id: Some(0),
            pieces: vec![
                ParameterGolfPromotedTokenizerToken {
                    token_id: 0,
                    piece: String::from("<unk>"),
                    kind: ParameterGolfPromotedTokenizerTokenKind::Unknown,
                },
                ParameterGolfPromotedTokenizerToken {
                    token_id: 1,
                    piece: String::from("<s>"),
                    kind: ParameterGolfPromotedTokenizerTokenKind::Control,
                },
                ParameterGolfPromotedTokenizerToken {
                    token_id: 2,
                    piece: String::from("</s>"),
                    kind: ParameterGolfPromotedTokenizerTokenKind::Control,
                },
                ParameterGolfPromotedTokenizerToken {
                    token_id: 3,
                    piece: String::from("a"),
                    kind: ParameterGolfPromotedTokenizerTokenKind::Normal,
                },
                ParameterGolfPromotedTokenizerToken {
                    token_id: 4,
                    piece: String::from("<reserved_0004>"),
                    kind: ParameterGolfPromotedTokenizerTokenKind::Unused,
                },
            ],
            tokenizer_digest: String::new(),
            asset_digest: String::new(),
            detail: String::from("test tokenizer asset"),
        };
        asset.tokenizer_digest = asset.tokenizer_contract_digest();
        asset.asset_digest = asset.stable_digest();

        let tokenizer = ParameterGolfPromotedRuntimeTokenizer::from_asset(asset)?;

        assert_eq!(tokenizer.generation_allowed_token_ids(), &[2, 3]);
        assert_eq!(tokenizer.generation_disallowed_token_ids(), &[0, 1, 4]);
        Ok(())
    }
}
