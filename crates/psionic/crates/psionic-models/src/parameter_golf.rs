use std::collections::BTreeMap;

use psionic_core::{DType, QuantizationMode, Shape, StableF32};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelDescriptor, WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
};

/// Stable family label for the Parameter Golf decoder lane.
pub const PARAMETER_GOLF_MODEL_FAMILY: &str = "parameter_golf_decoder";
/// Stable promoted profile id for the first general Psion-owned small decoder
/// built on the PGOLF core family.
pub const PARAMETER_GOLF_PROMOTED_GENERAL_PROFILE_ID: &str = "psion_small_decoder_pgolf_core_v0";
/// Stable promoted profile id for the strict challenge-bound PGOLF overlay.
pub const PARAMETER_GOLF_PROMOTED_CHALLENGE_PROFILE_ID: &str = "parameter_golf_challenge_sp1024_v0";
/// Stable fixture model id for the public `9x512` baseline shape.
pub const PARAMETER_GOLF_BASELINE_MODEL_ID: &str = "parameter-golf-sp1024-9x512";
/// Stable fixture revision for the current public baseline as of 2026-03-18.
pub const PARAMETER_GOLF_BASELINE_REVISION: &str = "public-2026-03-18";
/// The current public baseline vocab size.
pub const PARAMETER_GOLF_BASELINE_VOCAB_SIZE: usize = 1024;
/// The current public baseline layer count.
pub const PARAMETER_GOLF_BASELINE_NUM_LAYERS: usize = 9;
/// The current public baseline hidden width.
pub const PARAMETER_GOLF_BASELINE_MODEL_DIM: usize = 512;
/// The current public baseline query head count.
pub const PARAMETER_GOLF_BASELINE_NUM_HEADS: usize = 8;
/// The current public baseline KV head count.
pub const PARAMETER_GOLF_BASELINE_NUM_KV_HEADS: usize = 4;
/// The current public baseline MLP expansion multiplier.
pub const PARAMETER_GOLF_BASELINE_MLP_MULT: usize = 2;
/// The current public baseline bigram-hash vocab size.
pub const PARAMETER_GOLF_BASELINE_BIGRAM_VOCAB_SIZE: usize = 0;
/// The current public baseline bigram embedding width.
pub const PARAMETER_GOLF_BASELINE_BIGRAM_DIM: usize = 128;
/// The current public baseline context length.
pub const PARAMETER_GOLF_BASELINE_MAX_CONTEXT: usize = 1024;
/// The current public baseline tied-embedding posture.
pub const PARAMETER_GOLF_BASELINE_TIE_EMBEDDINGS: bool = true;
/// The current public baseline tied-embedding init std.
pub const PARAMETER_GOLF_BASELINE_TIED_EMBED_INIT_STD: f32 = 0.005;
/// The current public baseline tanh logit softcap.
pub const PARAMETER_GOLF_BASELINE_LOGIT_SOFTCAP: f32 = 30.0;
/// The current public baseline RoPE base.
pub const PARAMETER_GOLF_BASELINE_ROPE_BASE: f32 = 10_000.0;
/// The current public baseline q-gain init.
pub const PARAMETER_GOLF_BASELINE_QK_GAIN_INIT: f32 = 1.5;
/// The current public baseline MLP activation.
pub const PARAMETER_GOLF_BASELINE_MLP_ACTIVATION: ParameterGolfMlpActivation =
    ParameterGolfMlpActivation::ReluSquared;
/// Stable Parameter Banking tensor id for the combined query/output bank.
pub const PARAMETER_GOLF_QO_BANK_NAME: &str = "qo_bank";
/// Stable Parameter Banking tensor id for the combined key/value bank.
pub const PARAMETER_GOLF_KV_BANK_NAME: &str = "kv_bank";
/// Stable Parameter Banking tensor id for the MLP up-projection bank.
pub const PARAMETER_GOLF_MLP_UP_BANK_NAME: &str = "mlp_up_bank";
/// Stable Parameter Banking tensor id for the MLP down-projection bank.
pub const PARAMETER_GOLF_MLP_DOWN_BANK_NAME: &str = "mlp_down_bank";
/// Ordered upstream-style matrix bank tensor ids used by the competitive score path.
pub const PARAMETER_GOLF_MATRIX_BANK_NAMES: &[&str] = &[
    PARAMETER_GOLF_KV_BANK_NAME,
    PARAMETER_GOLF_MLP_DOWN_BANK_NAME,
    PARAMETER_GOLF_MLP_UP_BANK_NAME,
    PARAMETER_GOLF_QO_BANK_NAME,
];
/// The effective epsilon used when the public PyTorch path leaves RMSNorm epsilon unset.
pub const PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON: f32 = f32::EPSILON;

/// Frozen promoted profile kind for the PGOLF-shaped compact-decoder family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedProfileKind {
    /// General Psion-owned small decoder profile. Challenge-only requirements are
    /// intentionally absent.
    GeneralPsionSmallDecoder,
    /// Strict challenge overlay preserving the public PGOLF tokenizer, data,
    /// evaluation, and artifact rules.
    StrictPgolfChallenge,
}

impl ParameterGolfPromotedProfileKind {
    /// Returns the stable profile id for this promoted profile kind.
    #[must_use]
    pub const fn profile_id(self) -> &'static str {
        match self {
            Self::GeneralPsionSmallDecoder => PARAMETER_GOLF_PROMOTED_GENERAL_PROFILE_ID,
            Self::StrictPgolfChallenge => PARAMETER_GOLF_PROMOTED_CHALLENGE_PROFILE_ID,
        }
    }
}

/// Machine-readable shared capability contract for the promoted PGOLF-shaped
/// decoder family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSharedCapabilityContract {
    /// Tied token embedding and LM-head support is part of the shared family.
    pub tied_embeddings: bool,
    /// Grouped-query attention with explicit KV-head count is part of the family.
    pub grouped_query_attention: bool,
    /// RoPE is part of the family contract.
    pub rope: bool,
    /// Partial RoPE is allowed in the shared family.
    pub partial_rope: bool,
    /// Learned skip or U-Net-like skip weights are part of the family.
    pub skip_weights: bool,
    /// BigramHash-style local context augmentation is an admitted optional feature.
    pub optional_bigram_hash: bool,
    /// Late-layer shared value embeddings are an admitted optional feature.
    pub optional_value_embeddings: bool,
    /// XSA on deep layers is an admitted optional feature.
    pub optional_xsa: bool,
    /// ReLU-squared remains a valid activation posture.
    pub relu_squared: bool,
    /// LeakyReLU(0.5)^2 is an admitted activation posture.
    pub leaky_relu_squared_point_five: bool,
    /// Parameter-banked weight surfaces are part of the admitted family.
    pub parameter_banking: bool,
    /// Quantized small-model export is part of the admitted family.
    pub quantized_export: bool,
}

impl ParameterGolfSharedCapabilityContract {
    /// Returns the frozen shared capability contract for the promoted family.
    #[must_use]
    pub const fn promoted_defaults() -> Self {
        Self {
            tied_embeddings: true,
            grouped_query_attention: true,
            rope: true,
            partial_rope: true,
            skip_weights: true,
            optional_bigram_hash: true,
            optional_value_embeddings: true,
            optional_xsa: true,
            relu_squared: true,
            leaky_relu_squared_point_five: true,
            parameter_banking: true,
            quantized_export: true,
        }
    }
}

/// Machine-readable challenge-only overlay contract for one promoted profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfChallengeOverlayContract {
    /// Exact public SP1024 tokenizer is required.
    pub exact_sp1024_tokenizer: bool,
    /// Exact public FineWeb challenge data lane is required.
    pub exact_fineweb_challenge_data: bool,
    /// Exact compressed artifact cap is required.
    pub exact_16_mib_compressed_artifact_cap: bool,
    /// Legal score-first TTT is part of the overlay.
    pub score_first_ttt: bool,
    /// Tokenizer-agnostic BPB accounting is part of the overlay.
    pub contest_bpb_accounting: bool,
}

impl ParameterGolfChallengeOverlayContract {
    /// Returns the general Psion small-decoder overlay: none of the contest-only
    /// rules are required.
    #[must_use]
    pub const fn general_psion_small_decoder() -> Self {
        Self {
            exact_sp1024_tokenizer: false,
            exact_fineweb_challenge_data: false,
            exact_16_mib_compressed_artifact_cap: false,
            score_first_ttt: false,
            contest_bpb_accounting: false,
        }
    }

    /// Returns the strict PGOLF challenge overlay.
    #[must_use]
    pub const fn strict_pgolf_challenge() -> Self {
        Self {
            exact_sp1024_tokenizer: true,
            exact_fineweb_challenge_data: true,
            exact_16_mib_compressed_artifact_cap: true,
            score_first_ttt: true,
            contest_bpb_accounting: true,
        }
    }
}

/// Frozen promoted family contract for the first serious PGOLF-shaped small
/// decoder family inside Psionic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedProfileContract {
    /// Stable promoted profile id.
    pub profile_id: String,
    /// General profile versus strict challenge overlay.
    pub kind: ParameterGolfPromotedProfileKind,
    /// Stable shared family id.
    pub family_id: String,
    /// Stable baseline model id for the first shipping shape.
    pub baseline_model_id: String,
    /// Stable baseline revision for the profile contract.
    pub baseline_revision: String,
    /// Frozen baseline config for the first shipping shape.
    pub baseline_config: ParameterGolfConfig,
    /// Shared capability contract preserved across both profiles.
    pub shared_capabilities: ParameterGolfSharedCapabilityContract,
    /// Challenge-only overlay contract.
    pub challenge_overlay: ParameterGolfChallengeOverlayContract,
}

impl ParameterGolfPromotedProfileContract {
    /// Returns the promoted general Psion small-decoder profile.
    #[must_use]
    pub fn general_psion_small_decoder_v0() -> Self {
        Self {
            profile_id: String::from(
                ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder.profile_id(),
            ),
            kind: ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder,
            family_id: String::from(PARAMETER_GOLF_MODEL_FAMILY),
            baseline_model_id: String::from(PARAMETER_GOLF_BASELINE_MODEL_ID),
            baseline_revision: String::from(PARAMETER_GOLF_BASELINE_REVISION),
            baseline_config: ParameterGolfConfig::baseline_sp1024_9x512(),
            shared_capabilities: ParameterGolfSharedCapabilityContract::promoted_defaults(),
            challenge_overlay: ParameterGolfChallengeOverlayContract::general_psion_small_decoder(),
        }
    }

    /// Returns the strict challenge-bound PGOLF overlay profile.
    #[must_use]
    pub fn strict_pgolf_challenge_v0() -> Self {
        Self {
            profile_id: String::from(
                ParameterGolfPromotedProfileKind::StrictPgolfChallenge.profile_id(),
            ),
            kind: ParameterGolfPromotedProfileKind::StrictPgolfChallenge,
            family_id: String::from(PARAMETER_GOLF_MODEL_FAMILY),
            baseline_model_id: String::from(PARAMETER_GOLF_BASELINE_MODEL_ID),
            baseline_revision: String::from(PARAMETER_GOLF_BASELINE_REVISION),
            baseline_config: ParameterGolfConfig::baseline_sp1024_9x512(),
            shared_capabilities: ParameterGolfSharedCapabilityContract::promoted_defaults(),
            challenge_overlay: ParameterGolfChallengeOverlayContract::strict_pgolf_challenge(),
        }
    }
}

/// MLP activation posture for one Parameter Golf decoder-family instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParameterGolfMlpActivation {
    /// Standard ReLU-squared activation.
    ReluSquared,
    /// Leaky-ReLU with one fixed negative slope followed by squaring.
    LeakyReluSquared {
        /// Negative slope applied to values below zero before squaring.
        negative_slope: StableF32,
    },
}

/// Layerwise RMSNorm output scaling posture for one Parameter Golf decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfLayerNormScale {
    /// No extra layerwise norm-output scaling.
    None,
    /// Scale each block-local RMSNorm output by `1 / sqrt(layer_index + 1)`.
    InverseSqrtLayerIndexPlusOne,
}

impl ParameterGolfLayerNormScale {
    /// Returns the resolved multiplicative factor for one zero-based layer index.
    #[must_use]
    pub fn factor(self, layer_index: usize) -> f32 {
        match self {
            Self::None => 1.0,
            Self::InverseSqrtLayerIndexPlusOne => 1.0 / ((layer_index + 1) as f32).sqrt(),
        }
    }
}

impl Default for ParameterGolfLayerNormScale {
    fn default() -> Self {
        Self::None
    }
}

fn parameter_golf_layer_norm_scale_is_none(scale: &ParameterGolfLayerNormScale) -> bool {
    matches!(scale, ParameterGolfLayerNormScale::None)
}

const fn parameter_golf_xsa_last_n_is_zero(value: &usize) -> bool {
    *value == 0
}

impl ParameterGolfMlpActivation {
    /// Competitive public score-path posture from the current top local record surface.
    #[must_use]
    pub const fn leaky_relu_squared_point_five() -> Self {
        Self::LeakyReluSquared {
            negative_slope: StableF32::from_f32(0.5),
        }
    }
}

impl Default for ParameterGolfMlpActivation {
    fn default() -> Self {
        PARAMETER_GOLF_BASELINE_MLP_ACTIVATION
    }
}

const fn parameter_golf_default_bigram_vocab_size() -> usize {
    PARAMETER_GOLF_BASELINE_BIGRAM_VOCAB_SIZE
}

const fn parameter_golf_default_bigram_dim() -> usize {
    PARAMETER_GOLF_BASELINE_BIGRAM_DIM
}

const fn parameter_golf_default_ve_dim() -> usize {
    128
}

fn parameter_golf_ve_layer_indices_are_empty(value: &[usize]) -> bool {
    value.is_empty()
}

/// Configuration for one Parameter Golf decoder-family instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfConfig {
    /// Vocabulary size for the tokenizer and tied LM head.
    pub vocab_size: usize,
    /// Total layer count across encoder and decoder halves.
    pub num_layers: usize,
    /// Hidden width.
    pub model_dim: usize,
    /// Number of query heads.
    pub num_heads: usize,
    /// Number of KV heads.
    pub num_kv_heads: usize,
    /// MLP expansion multiplier.
    pub mlp_mult: usize,
    /// Hashed bigram feature vocab size. Zero disables the feature.
    #[serde(default = "parameter_golf_default_bigram_vocab_size")]
    pub bigram_vocab_size: usize,
    /// Bigram embedding width before the optional projection.
    #[serde(default = "parameter_golf_default_bigram_dim")]
    pub bigram_dim: usize,
    /// Shared value-embedding width before the optional projection to `kv_dim`.
    #[serde(default = "parameter_golf_default_ve_dim")]
    pub ve_dim: usize,
    /// Zero-based layer indices that receive the shared late-layer value embedding.
    #[serde(
        default,
        skip_serializing_if = "parameter_golf_ve_layer_indices_are_empty"
    )]
    pub ve_layer_indices: Vec<usize>,
    /// MLP activation family.
    #[serde(default)]
    pub mlp_activation: ParameterGolfMlpActivation,
    /// Maximum context length.
    pub max_context: usize,
    /// Whether token embeddings are tied to the LM head.
    pub tie_embeddings: bool,
    /// Initialization std used by the tied-embedding baseline.
    pub tied_embed_init_std: f32,
    /// Tanh logit softcap.
    pub logit_softcap: f32,
    /// Rotary frequency base.
    pub rope_base: f32,
    /// Optional rotary sub-dimension. `None` means the full head width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rope_rotary_dim: Option<usize>,
    /// Optional layerwise scaling on block-local RMSNorm outputs.
    #[serde(
        default,
        skip_serializing_if = "parameter_golf_layer_norm_scale_is_none"
    )]
    pub layer_norm_scale: ParameterGolfLayerNormScale,
    /// Apply XSA-style self-value orthogonalization on the deepest `xsa_last_n` layers.
    #[serde(default, skip_serializing_if = "parameter_golf_xsa_last_n_is_zero")]
    pub xsa_last_n: usize,
    /// Initial learned q-gain value.
    pub qk_gain_init: f32,
}

impl ParameterGolfConfig {
    /// Returns the current public `SP-1024` `9x512` baseline config.
    #[must_use]
    pub fn baseline_sp1024_9x512() -> Self {
        Self {
            vocab_size: PARAMETER_GOLF_BASELINE_VOCAB_SIZE,
            num_layers: PARAMETER_GOLF_BASELINE_NUM_LAYERS,
            model_dim: PARAMETER_GOLF_BASELINE_MODEL_DIM,
            num_heads: PARAMETER_GOLF_BASELINE_NUM_HEADS,
            num_kv_heads: PARAMETER_GOLF_BASELINE_NUM_KV_HEADS,
            mlp_mult: PARAMETER_GOLF_BASELINE_MLP_MULT,
            bigram_vocab_size: PARAMETER_GOLF_BASELINE_BIGRAM_VOCAB_SIZE,
            bigram_dim: PARAMETER_GOLF_BASELINE_BIGRAM_DIM,
            ve_dim: parameter_golf_default_ve_dim(),
            ve_layer_indices: Vec::new(),
            mlp_activation: PARAMETER_GOLF_BASELINE_MLP_ACTIVATION,
            max_context: PARAMETER_GOLF_BASELINE_MAX_CONTEXT,
            tie_embeddings: PARAMETER_GOLF_BASELINE_TIE_EMBEDDINGS,
            tied_embed_init_std: PARAMETER_GOLF_BASELINE_TIED_EMBED_INIT_STD,
            logit_softcap: PARAMETER_GOLF_BASELINE_LOGIT_SOFTCAP,
            rope_base: PARAMETER_GOLF_BASELINE_ROPE_BASE,
            rope_rotary_dim: None,
            layer_norm_scale: ParameterGolfLayerNormScale::None,
            xsa_last_n: 0,
            qk_gain_init: PARAMETER_GOLF_BASELINE_QK_GAIN_INIT,
        }
    }

    /// Returns the first HOMEGOLF competitive variant built from already
    /// admitted PGOLF family surfaces.
    #[must_use]
    pub fn competitive_homegolf_v1() -> Self {
        Self {
            num_layers: 10,
            mlp_mult: 3,
            bigram_vocab_size: 2_048,
            bigram_dim: 128,
            ve_dim: 128,
            ve_layer_indices: vec![7, 8, 9],
            mlp_activation: ParameterGolfMlpActivation::leaky_relu_squared_point_five(),
            rope_rotary_dim: Some(32),
            layer_norm_scale: ParameterGolfLayerNormScale::InverseSqrtLayerIndexPlusOne,
            xsa_last_n: 2,
            ..Self::baseline_sp1024_9x512()
        }
    }

    /// Validates the config against the public family invariants.
    pub fn validate(&self) -> Result<(), ParameterGolfConfigError> {
        if self.vocab_size == 0 {
            return Err(ParameterGolfConfigError::VocabSizeMustBePositive);
        }
        if self.num_layers == 0 {
            return Err(ParameterGolfConfigError::NumLayersMustBePositive);
        }
        if self.model_dim == 0 {
            return Err(ParameterGolfConfigError::ModelDimMustBePositive);
        }
        if self.num_heads == 0 {
            return Err(ParameterGolfConfigError::NumHeadsMustBePositive);
        }
        if self.num_kv_heads == 0 {
            return Err(ParameterGolfConfigError::NumKvHeadsMustBePositive);
        }
        if !self.num_heads.is_multiple_of(self.num_kv_heads) {
            return Err(
                ParameterGolfConfigError::NumHeadsMustBeDivisibleByNumKvHeads {
                    num_heads: self.num_heads,
                    num_kv_heads: self.num_kv_heads,
                },
            );
        }
        if !self.model_dim.is_multiple_of(self.num_heads) {
            return Err(
                ParameterGolfConfigError::ModelDimMustBeDivisibleByNumHeads {
                    model_dim: self.model_dim,
                    num_heads: self.num_heads,
                },
            );
        }
        let head_dim = self.model_dim / self.num_heads;
        if !head_dim.is_multiple_of(2) {
            return Err(ParameterGolfConfigError::HeadDimMustBeEven { head_dim });
        }
        let rope_rotary_dim = self.rope_rotary_dim.unwrap_or(head_dim);
        if rope_rotary_dim == 0 {
            return Err(ParameterGolfConfigError::RopeRotaryDimMustBePositive);
        }
        if rope_rotary_dim > head_dim {
            return Err(
                ParameterGolfConfigError::RopeRotaryDimMustNotExceedHeadDim {
                    rope_rotary_dim,
                    head_dim,
                },
            );
        }
        if !rope_rotary_dim.is_multiple_of(2) {
            return Err(ParameterGolfConfigError::RopeRotaryDimMustBeEven { rope_rotary_dim });
        }
        if self.mlp_mult == 0 {
            return Err(ParameterGolfConfigError::MlpMultMustBePositive);
        }
        if self.bigram_vocab_size == 1 {
            return Err(ParameterGolfConfigError::BigramVocabSizeMustBeZeroOrAtLeastTwo);
        }
        if self.bigram_vocab_size > 0 && self.bigram_dim == 0 {
            return Err(ParameterGolfConfigError::BigramDimMustBePositiveWhenEnabled);
        }
        if !self.ve_layer_indices.is_empty() && self.ve_dim == 0 {
            return Err(ParameterGolfConfigError::VeDimMustBePositiveWhenEnabled);
        }
        for (position, layer_index) in self.ve_layer_indices.iter().copied().enumerate() {
            if layer_index >= self.num_layers {
                return Err(ParameterGolfConfigError::VeLayerIndexOutOfRange {
                    layer_index,
                    num_layers: self.num_layers,
                });
            }
            if position > 0 {
                let previous = self.ve_layer_indices[position - 1];
                if layer_index <= previous {
                    return Err(
                        ParameterGolfConfigError::VeLayerIndicesMustBeStrictlyIncreasing {
                            previous,
                            current: layer_index,
                        },
                    );
                }
            }
        }
        if self.xsa_last_n > self.num_layers {
            return Err(ParameterGolfConfigError::XsaLastNMustNotExceedNumLayers {
                xsa_last_n: self.xsa_last_n,
                num_layers: self.num_layers,
            });
        }
        if let ParameterGolfMlpActivation::LeakyReluSquared { negative_slope } = self.mlp_activation
        {
            let negative_slope = negative_slope.to_f32();
            if !negative_slope.is_finite() || negative_slope < 0.0 {
                return Err(
                    ParameterGolfConfigError::MlpActivationNegativeSlopeMustBeFinite {
                        value: negative_slope,
                    },
                );
            }
        }
        if self.max_context == 0 {
            return Err(ParameterGolfConfigError::MaxContextMustBePositive);
        }
        if !self.tied_embed_init_std.is_finite() || self.tied_embed_init_std < 0.0 {
            return Err(
                ParameterGolfConfigError::TiedEmbedInitStdMustBeNonNegative {
                    value: self.tied_embed_init_std,
                },
            );
        }
        if !self.logit_softcap.is_finite() || self.logit_softcap <= 0.0 {
            return Err(ParameterGolfConfigError::LogitSoftcapMustBePositive {
                value: self.logit_softcap,
            });
        }
        if !self.rope_base.is_finite() || self.rope_base <= 0.0 {
            return Err(ParameterGolfConfigError::RopeBaseMustBePositive {
                value: self.rope_base,
            });
        }
        if !self.qk_gain_init.is_finite() {
            return Err(ParameterGolfConfigError::QkGainInitMustBeFinite {
                value: self.qk_gain_init,
            });
        }
        let _ = self.parameter_facts()?;
        Ok(())
    }

    /// Returns the encoder-half layer count.
    #[must_use]
    pub const fn num_encoder_layers(&self) -> usize {
        self.num_layers / 2
    }

    /// Returns the decoder-half layer count.
    #[must_use]
    pub const fn num_decoder_layers(&self) -> usize {
        self.num_layers - self.num_encoder_layers()
    }

    /// Returns the number of learned skip-weight rows.
    #[must_use]
    pub fn num_skip_weights(&self) -> usize {
        self.num_encoder_layers().min(self.num_decoder_layers())
    }

    /// Returns the per-head width.
    pub fn head_dim(&self) -> Result<usize, ParameterGolfConfigError> {
        self.validate()?;
        Ok(self.model_dim / self.num_heads)
    }

    /// Returns the total KV width.
    pub fn kv_dim(&self) -> Result<usize, ParameterGolfConfigError> {
        Ok(self.num_kv_heads * self.head_dim()?)
    }

    /// Returns the effective rotary sub-dimension per head.
    pub fn effective_rope_rotary_dim(&self) -> Result<usize, ParameterGolfConfigError> {
        Ok(self.rope_rotary_dim.unwrap_or(self.head_dim()?))
    }

    /// Returns the MLP hidden width.
    pub fn mlp_hidden_dim(&self) -> Result<usize, ParameterGolfConfigError> {
        self.model_dim.checked_mul(self.mlp_mult).ok_or(
            ParameterGolfConfigError::ParameterCountOverflow {
                component: String::from("mlp_hidden_dim"),
            },
        )
    }

    /// Returns one deterministic hashed-bigram id surface for the supplied
    /// token batch, or `None` when the feature is disabled.
    pub fn bigram_hash_batch(
        &self,
        input_ids: &[Vec<u32>],
    ) -> Result<Option<Vec<Vec<u32>>>, ParameterGolfExecutionError> {
        let _ = validate_token_batch(input_ids, self.vocab_size)?;
        if self.bigram_vocab_size == 0 {
            return Ok(None);
        }
        let mod_base = (self.bigram_vocab_size - 1) as u64;
        let mut hashed = Vec::with_capacity(input_ids.len());
        for row in input_ids {
            let mut hashed_row = vec![mod_base as u32; row.len()];
            for position in 1..row.len() {
                hashed_row[position] = (((36_313_u64 * row[position] as u64)
                    ^ (27_191_u64 * row[position - 1] as u64))
                    % mod_base) as u32;
            }
            hashed.push(hashed_row);
        }
        Ok(Some(hashed))
    }

    /// Returns the resolved multiplicative scale applied to block-local
    /// RMSNorm outputs for one zero-based layer index.
    #[must_use]
    pub fn layer_norm_scale_factor(&self, layer_index: usize) -> f32 {
        self.layer_norm_scale.factor(layer_index)
    }

    /// Returns the slot inside `ve_layer_indices` for one layer, when present.
    #[must_use]
    pub fn ve_layer_slot(&self, layer_index: usize) -> Option<usize> {
        self.ve_layer_indices
            .iter()
            .position(|candidate| *candidate == layer_index)
    }

    /// Returns whether the supplied zero-based layer index uses the XSA score path.
    #[must_use]
    pub fn xsa_applies_to_layer(&self, layer_index: usize) -> bool {
        self.xsa_last_n > 0 && layer_index >= self.num_layers.saturating_sub(self.xsa_last_n)
    }

    /// Returns challenge-specific parameter accounting facts for the family.
    pub fn parameter_facts(&self) -> Result<ParameterGolfParameterFacts, ParameterGolfConfigError> {
        let head_dim = self.model_dim / self.num_heads;
        let kv_dim = self.num_kv_heads.checked_mul(head_dim).ok_or(
            ParameterGolfConfigError::ParameterCountOverflow {
                component: String::from("kv_dim"),
            },
        )?;
        let token_embedding =
            checked_mul_usize(self.vocab_size, self.model_dim, "token_embedding")?;
        let bigram_embedding = if self.bigram_vocab_size > 0 {
            checked_mul_usize(self.bigram_vocab_size, self.bigram_dim, "bigram_embedding")?
        } else {
            0
        };
        let bigram_projection = if self.bigram_vocab_size > 0 && self.bigram_dim != self.model_dim {
            checked_mul_usize(self.model_dim, self.bigram_dim, "bigram_projection")?
        } else {
            0
        };
        let bigram_scale = usize::from(self.bigram_vocab_size > 0);
        let ve_embedding = if self.ve_layer_indices.is_empty() {
            0
        } else {
            checked_mul_usize(self.vocab_size, self.ve_dim, "ve_embedding")?
        };
        let ve_projection = if !self.ve_layer_indices.is_empty() && self.ve_dim != kv_dim {
            checked_mul_usize(kv_dim, self.ve_dim, "ve_projection")?
        } else {
            0
        };
        let ve_scale = usize::from(!self.ve_layer_indices.is_empty());
        let ve_layer_scales = self.ve_layer_indices.len();
        let q_proj = checked_mul_usize(self.model_dim, self.model_dim, "q_proj")?;
        let k_proj = checked_mul_usize(kv_dim, self.model_dim, "k_proj")?;
        let v_proj = checked_mul_usize(kv_dim, self.model_dim, "v_proj")?;
        let out_proj = checked_mul_usize(self.model_dim, self.model_dim, "out_proj")?;
        let q_gain = self.num_heads;
        let per_block_attention = checked_add_usize(
            checked_add_usize(
                checked_add_usize(q_proj, k_proj, "per_block_attention")?,
                v_proj,
                "per_block_attention",
            )?,
            out_proj,
            "per_block_attention",
        )?
        .checked_add(q_gain)
        .ok_or(ParameterGolfConfigError::ParameterCountOverflow {
            component: String::from("per_block_attention"),
        })?;
        let mlp_hidden_dim = self.model_dim.checked_mul(self.mlp_mult).ok_or(
            ParameterGolfConfigError::ParameterCountOverflow {
                component: String::from("mlp_hidden_dim"),
            },
        )?;
        let fc = checked_mul_usize(mlp_hidden_dim, self.model_dim, "mlp.fc")?;
        let proj = checked_mul_usize(self.model_dim, mlp_hidden_dim, "mlp.proj")?;
        let per_block_feed_forward = checked_add_usize(fc, proj, "per_block_feed_forward")?;
        let per_block_control = checked_add_usize(
            checked_add_usize(self.model_dim, self.model_dim, "per_block_control")?,
            checked_mul_usize(2, self.model_dim, "resid_mix")?,
            "per_block_control",
        )?;
        let skip_weights =
            checked_mul_usize(self.num_skip_weights(), self.model_dim, "skip_weights")?;
        let lm_head = if self.tie_embeddings {
            0
        } else {
            checked_mul_usize(self.vocab_size, self.model_dim, "lm_head")?
        };
        let per_block_total = checked_add_usize(
            checked_add_usize(
                per_block_attention,
                per_block_feed_forward,
                "per_block_total",
            )?,
            per_block_control,
            "per_block_total",
        )?;
        let total_parameters = checked_add_usize(
            checked_add_usize(
                checked_add_usize(
                    checked_add_usize(token_embedding, bigram_embedding, "total_parameters")?,
                    checked_add_usize(bigram_projection, bigram_scale, "total_parameters")?,
                    "total_parameters",
                )?,
                checked_add_usize(
                    checked_add_usize(ve_embedding, ve_projection, "total_parameters")?,
                    checked_add_usize(ve_scale, ve_layer_scales, "total_parameters")?,
                    "total_parameters",
                )?,
                "total_parameters",
            )?,
            checked_add_usize(
                checked_mul_usize(self.num_layers, per_block_total, "blocks_total")?,
                checked_add_usize(skip_weights, lm_head, "total_parameters")?,
                "total_parameters",
            )?,
            "total_parameters",
        )?;
        Ok(ParameterGolfParameterFacts {
            token_embedding,
            bigram_embedding,
            bigram_projection,
            bigram_scale,
            ve_embedding,
            ve_projection,
            ve_scale,
            ve_layer_scales,
            skip_weights,
            per_block_attention,
            per_block_feed_forward,
            per_block_control,
            lm_head,
            total_parameters,
        })
    }
}

/// Validation failure for one Parameter Golf config.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ParameterGolfConfigError {
    /// `vocab_size` must be strictly positive.
    #[error("vocab_size must be positive")]
    VocabSizeMustBePositive,
    /// `num_layers` must be strictly positive.
    #[error("num_layers must be positive")]
    NumLayersMustBePositive,
    /// `model_dim` must be strictly positive.
    #[error("model_dim must be positive")]
    ModelDimMustBePositive,
    /// `num_heads` must be strictly positive.
    #[error("num_heads must be positive")]
    NumHeadsMustBePositive,
    /// `num_kv_heads` must be strictly positive.
    #[error("num_kv_heads must be positive")]
    NumKvHeadsMustBePositive,
    /// Query heads must divide evenly across KV heads.
    #[error("num_heads ({num_heads}) must be divisible by num_kv_heads ({num_kv_heads})")]
    NumHeadsMustBeDivisibleByNumKvHeads {
        /// Invalid query head count.
        num_heads: usize,
        /// Invalid KV head count.
        num_kv_heads: usize,
    },
    /// The hidden width must divide evenly across heads.
    #[error("model_dim ({model_dim}) must be divisible by num_heads ({num_heads})")]
    ModelDimMustBeDivisibleByNumHeads {
        /// Invalid hidden width.
        model_dim: usize,
        /// Invalid query head count.
        num_heads: usize,
    },
    /// RoPE requires an even head dimension.
    #[error("head_dim must be even for RoPE, got {head_dim}")]
    HeadDimMustBeEven {
        /// Invalid head dimension.
        head_dim: usize,
    },
    /// The rotary sub-dimension must be positive when provided.
    #[error("rope_rotary_dim must be positive when provided")]
    RopeRotaryDimMustBePositive,
    /// The rotary sub-dimension must fit within the head width.
    #[error("rope_rotary_dim ({rope_rotary_dim}) must not exceed head_dim ({head_dim})")]
    RopeRotaryDimMustNotExceedHeadDim {
        /// Invalid rotary sub-dimension.
        rope_rotary_dim: usize,
        /// Resolved head width.
        head_dim: usize,
    },
    /// The rotary sub-dimension must stay even.
    #[error("rope_rotary_dim must be even for RoPE, got {rope_rotary_dim}")]
    RopeRotaryDimMustBeEven {
        /// Invalid rotary sub-dimension.
        rope_rotary_dim: usize,
    },
    /// `mlp_mult` must be strictly positive.
    #[error("mlp_mult must be positive")]
    MlpMultMustBePositive,
    /// `bigram_vocab_size` must be zero or at least two.
    #[error("bigram_vocab_size must be zero or at least two")]
    BigramVocabSizeMustBeZeroOrAtLeastTwo,
    /// `bigram_dim` must be strictly positive when the feature is enabled.
    #[error("bigram_dim must be positive when bigram_vocab_size > 0")]
    BigramDimMustBePositiveWhenEnabled,
    /// `ve_dim` must be strictly positive when VE is enabled.
    #[error("ve_dim must be positive when ve_layer_indices is non-empty")]
    VeDimMustBePositiveWhenEnabled,
    /// VE layer indices must remain in bounds.
    #[error("ve layer index {layer_index} must be less than num_layers ({num_layers})")]
    VeLayerIndexOutOfRange {
        /// Invalid VE layer index.
        layer_index: usize,
        /// Total layer count.
        num_layers: usize,
    },
    /// VE layer indices must stay sorted and unique.
    #[error("ve_layer_indices must be strictly increasing, got {previous} then {current}")]
    VeLayerIndicesMustBeStrictlyIncreasing {
        /// Previous layer index.
        previous: usize,
        /// Current layer index.
        current: usize,
    },
    /// `xsa_last_n` must stay within the layer count.
    #[error("xsa_last_n ({xsa_last_n}) must not exceed num_layers ({num_layers})")]
    XsaLastNMustNotExceedNumLayers {
        /// Invalid XSA layer count.
        xsa_last_n: usize,
        /// Total layer count.
        num_layers: usize,
    },
    /// The leaky-ReLU negative slope must be finite and non-negative.
    #[error("mlp_activation negative_slope must be finite and non-negative, got {value}")]
    MlpActivationNegativeSlopeMustBeFinite {
        /// Invalid negative slope.
        value: f32,
    },
    /// `max_context` must be strictly positive.
    #[error("max_context must be positive")]
    MaxContextMustBePositive,
    /// The tied-embedding init std must be finite and non-negative.
    #[error("tied_embed_init_std must be finite and non-negative, got {value}")]
    TiedEmbedInitStdMustBeNonNegative {
        /// Invalid std.
        value: f32,
    },
    /// The tanh softcap must be finite and strictly positive.
    #[error("logit_softcap must be positive, got {value}")]
    LogitSoftcapMustBePositive {
        /// Invalid softcap.
        value: f32,
    },
    /// The RoPE base must be finite and strictly positive.
    #[error("rope_base must be positive, got {value}")]
    RopeBaseMustBePositive {
        /// Invalid base.
        value: f32,
    },
    /// The initial q-gain must be finite.
    #[error("qk_gain_init must be finite, got {value}")]
    QkGainInitMustBeFinite {
        /// Invalid q-gain init.
        value: f32,
    },
    /// One derived shape or parameter count overflowed `usize`.
    #[error("parameter golf config component `{component}` overflowed usize")]
    ParameterCountOverflow {
        /// Overflowing component label.
        component: String,
    },
}

/// Challenge-specific parameter accounting for one Parameter Golf config.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfParameterFacts {
    /// Parameters in the token embedding matrix.
    pub token_embedding: usize,
    /// Parameters in the optional bigram embedding table.
    pub bigram_embedding: usize,
    /// Parameters in the optional bigram projection matrix.
    pub bigram_projection: usize,
    /// Parameters in the optional bigram scalar scale.
    pub bigram_scale: usize,
    /// Parameters in the optional shared VE embedding table.
    pub ve_embedding: usize,
    /// Parameters in the optional shared VE projection matrix.
    pub ve_projection: usize,
    /// Parameters in the optional shared VE scalar scale.
    pub ve_scale: usize,
    /// Parameters in the optional per-layer VE scales.
    pub ve_layer_scales: usize,
    /// Parameters in the learned skip-weight table.
    pub skip_weights: usize,
    /// Per-block attention parameters, including q-gain.
    pub per_block_attention: usize,
    /// Per-block MLP matrix parameters.
    pub per_block_feed_forward: usize,
    /// Per-block control tensors: `attn_scale`, `mlp_scale`, and `resid_mix`.
    pub per_block_control: usize,
    /// Untied LM-head parameters, or zero when embeddings are tied.
    pub lm_head: usize,
    /// Total parameter count for the config.
    pub total_parameters: usize,
}

/// Deterministic reference initializer used by the frozen `train_gpt.py` parity fixture.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDeterministicInitializer {
    /// Modulus applied to the flat parameter ramp.
    pub modulus: u32,
    /// Center offset subtracted after the modulus.
    pub centered_offset: i32,
    /// Stride applied per flat element.
    pub stride: u32,
    /// Final divisor converting integer ramps into small `f32` values.
    pub scale_divisor: f32,
}

impl Default for ParameterGolfDeterministicInitializer {
    fn default() -> Self {
        Self {
            modulus: 257,
            centered_offset: 128,
            stride: 17,
            scale_divisor: 2048.0,
        }
    }
}

impl ParameterGolfDeterministicInitializer {
    fn values_for(self, tensor_name: &str, element_count: usize) -> Vec<f32> {
        let name_seed = tensor_name
            .bytes()
            .enumerate()
            .fold(0_u32, |acc, (index, byte)| {
                acc.wrapping_add((index as u32 + 1) * u32::from(byte))
            });
        (0..element_count)
            .map(|index| {
                let raw = name_seed.wrapping_add(self.stride.wrapping_mul(index as u32));
                let centered = (raw % self.modulus) as i32 - self.centered_offset;
                centered as f32 / self.scale_divisor
            })
            .collect()
    }
}

/// Simple CPU-reference 3D tensor used by the Parameter Golf reference model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfTensor3 {
    shape: [usize; 3],
    values: Vec<f32>,
}

impl ParameterGolfTensor3 {
    /// Creates one tensor from a logical shape and row-major values.
    pub fn new(shape: [usize; 3], values: Vec<f32>) -> Result<Self, ParameterGolfTensorError> {
        let expected = shape.iter().copied().product::<usize>();
        if values.len() != expected {
            return Err(ParameterGolfTensorError::InvalidValueCount {
                shape,
                actual: values.len(),
                expected,
            });
        }
        Ok(Self { shape, values })
    }

    /// Creates one zero tensor.
    #[must_use]
    pub fn zeros(shape: [usize; 3]) -> Self {
        let len = shape.iter().copied().product::<usize>();
        Self {
            shape,
            values: vec![0.0; len],
        }
    }

    /// Creates one tensor from nested `[batch][sequence][width]` values.
    pub fn from_nested(values: Vec<Vec<Vec<f32>>>) -> Result<Self, ParameterGolfTensorError> {
        if values.is_empty() {
            return Err(ParameterGolfTensorError::EmptyBatch);
        }
        let sequence_length = values.first().map(Vec::len).unwrap_or(0);
        if sequence_length == 0 {
            return Err(ParameterGolfTensorError::EmptySequence);
        }
        let width = values
            .first()
            .and_then(|batch| batch.first())
            .map(Vec::len)
            .unwrap_or(0);
        if width == 0 {
            return Err(ParameterGolfTensorError::EmptyWidth);
        }
        let mut flat = Vec::with_capacity(values.len() * sequence_length * width);
        for (batch_index, batch) in values.iter().enumerate() {
            if batch.len() != sequence_length {
                return Err(ParameterGolfTensorError::RaggedSequence {
                    batch_index,
                    expected: sequence_length,
                    actual: batch.len(),
                });
            }
            for (position_index, row) in batch.iter().enumerate() {
                if row.len() != width {
                    return Err(ParameterGolfTensorError::RaggedWidth {
                        batch_index,
                        position_index,
                        expected: width,
                        actual: row.len(),
                    });
                }
                flat.extend_from_slice(row);
            }
        }
        Self::new([values.len(), sequence_length, width], flat)
    }

    /// Returns the logical shape.
    #[must_use]
    pub const fn shape(&self) -> [usize; 3] {
        self.shape
    }

    /// Returns the batch size.
    #[must_use]
    pub const fn batch_size(&self) -> usize {
        self.shape[0]
    }

    /// Returns the sequence length.
    #[must_use]
    pub const fn sequence_length(&self) -> usize {
        self.shape[1]
    }

    /// Returns the feature width.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.shape[2]
    }

    /// Returns the flat row-major values.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Returns the maximum absolute difference against another tensor.
    pub fn max_abs_diff(&self, other: &Self) -> Result<f32, ParameterGolfTensorError> {
        if self.shape != other.shape {
            return Err(ParameterGolfTensorError::ShapeMismatch {
                left: self.shape,
                right: other.shape,
            });
        }
        Ok(self
            .values
            .iter()
            .zip(other.values.iter())
            .map(|(left, right)| (left - right).abs())
            .fold(0.0_f32, f32::max))
    }

    fn get(&self, batch: usize, position: usize, feature: usize) -> f32 {
        self.values[self.index(batch, position, feature)]
    }

    fn set(&mut self, batch: usize, position: usize, feature: usize, value: f32) {
        let index = self.index(batch, position, feature);
        self.values[index] = value;
    }

    fn index(&self, batch: usize, position: usize, feature: usize) -> usize {
        (batch * self.sequence_length() + position) * self.width() + feature
    }
}

/// Tensor construction failure for the Parameter Golf reference model.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ParameterGolfTensorError {
    /// The flat value count did not match the declared shape.
    #[error("invalid value count {actual} for shape {shape:?}; expected {expected}")]
    InvalidValueCount {
        /// Declared shape.
        shape: [usize; 3],
        /// Actual value count.
        actual: usize,
        /// Expected value count.
        expected: usize,
    },
    /// Nested construction requires a non-empty batch.
    #[error("nested tensor input must contain at least one batch element")]
    EmptyBatch,
    /// Nested construction requires a non-empty sequence.
    #[error("nested tensor input must contain at least one sequence position")]
    EmptySequence,
    /// Nested construction requires a non-zero feature width.
    #[error("nested tensor input must contain at least one feature value")]
    EmptyWidth,
    /// Nested construction saw a ragged sequence length.
    #[error(
        "nested tensor input is ragged at batch {batch_index}: expected sequence length {expected}, got {actual}"
    )]
    RaggedSequence {
        /// Batch index that diverged.
        batch_index: usize,
        /// Expected sequence length.
        expected: usize,
        /// Actual sequence length.
        actual: usize,
    },
    /// Nested construction saw a ragged width.
    #[error(
        "nested tensor input is ragged at batch {batch_index}, position {position_index}: expected width {expected}, got {actual}"
    )]
    RaggedWidth {
        /// Batch index that diverged.
        batch_index: usize,
        /// Sequence position that diverged.
        position_index: usize,
        /// Expected width.
        expected: usize,
        /// Actual width.
        actual: usize,
    },
    /// Max-abs-diff requires identical shapes.
    #[error("tensor shapes do not match: left {left:?}, right {right:?}")]
    ShapeMismatch {
        /// Left-hand shape.
        left: [usize; 3],
        /// Right-hand shape.
        right: [usize; 3],
    },
}

/// Stable descriptor for one Parameter Golf family instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfModelDescriptor {
    /// Shared model identity.
    pub model: ModelDescriptor,
    /// Stable family config.
    pub config: ParameterGolfConfig,
    /// Stable weight metadata for the bound bundle.
    pub weights: WeightBundleMetadata,
}

impl ParameterGolfModelDescriptor {
    /// Returns a stable digest over the descriptor payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_parameter_golf_model_descriptor|", self)
    }
}

/// One dense linear tensor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfLinearWeights {
    /// Flat row-major weight matrix in `[out_features, in_features]` order.
    pub weight: Vec<f32>,
    /// Number of output rows.
    pub out_features: usize,
    /// Number of input columns.
    pub in_features: usize,
}

impl ParameterGolfLinearWeights {
    fn from_initializer(
        tensor_name: &str,
        out_features: usize,
        in_features: usize,
        initializer: ParameterGolfDeterministicInitializer,
    ) -> Self {
        Self {
            weight: initializer.values_for(tensor_name, out_features.saturating_mul(in_features)),
            out_features,
            in_features,
        }
    }
}

/// One contiguous 3D matrix bank in `[bank_len, out_features, in_features]` order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfLinearBankWeights {
    /// Flat row-major bank values in `[bank_len, out_features, in_features]` order.
    pub weight: Vec<f32>,
    /// Number of bank slices.
    pub bank_len: usize,
    /// Number of output rows per bank slice.
    pub out_features: usize,
    /// Number of input columns per bank slice.
    pub in_features: usize,
}

impl ParameterGolfLinearBankWeights {
    fn from_linears(linears: &[&ParameterGolfLinearWeights]) -> Self {
        let first = linears
            .first()
            .expect("parameter golf banks require at least one slice");
        let mut weight = Vec::with_capacity(
            linears.len() * first.out_features.saturating_mul(first.in_features),
        );
        for linear in linears {
            weight.extend_from_slice(linear.weight.as_slice());
        }
        Self {
            weight,
            bank_len: linears.len(),
            out_features: first.out_features,
            in_features: first.in_features,
        }
    }

    fn validate(
        &self,
        bank_name: &str,
        expected_bank_len: usize,
    ) -> Result<(), ParameterGolfModelError> {
        if self.bank_len != expected_bank_len {
            return Err(ParameterGolfModelError::BankLengthMismatch {
                name: String::from(bank_name),
                actual: self.bank_len,
                expected: expected_bank_len,
            });
        }
        let expected = self
            .bank_len
            .checked_mul(self.out_features)
            .and_then(|value| value.checked_mul(self.in_features))
            .ok_or_else(|| ParameterGolfModelError::InvalidVectorLength {
                name: String::from(bank_name),
                actual: self.weight.len(),
                expected: usize::MAX,
            })?;
        if self.weight.len() != expected {
            return Err(ParameterGolfModelError::InvalidVectorLength {
                name: String::from(bank_name),
                actual: self.weight.len(),
                expected,
            });
        }
        Ok(())
    }

    fn shape(&self) -> Shape {
        Shape::new(vec![self.bank_len, self.out_features, self.in_features])
    }

    fn linear_at(
        &self,
        bank_name: &str,
        expected_bank_len: usize,
        index: usize,
    ) -> Result<ParameterGolfLinearWeights, ParameterGolfModelError> {
        self.validate(bank_name, expected_bank_len)?;
        let stride = self.out_features.saturating_mul(self.in_features);
        let start = index.saturating_mul(stride);
        let end = start.saturating_add(stride);
        Ok(ParameterGolfLinearWeights {
            weight: self.weight[start..end].to_vec(),
            out_features: self.out_features,
            in_features: self.in_features,
        })
    }
}

/// Attention weights for one Parameter Golf block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfAttentionWeights {
    /// Query projection.
    pub q_proj: ParameterGolfLinearWeights,
    /// Key projection.
    pub k_proj: ParameterGolfLinearWeights,
    /// Value projection.
    pub v_proj: ParameterGolfLinearWeights,
    /// Output projection.
    pub out_proj: ParameterGolfLinearWeights,
    /// Learned q-gain, one scalar per query head.
    pub q_gain: Vec<f32>,
}

/// MLP weights for one Parameter Golf block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfMlpWeights {
    /// Input projection.
    pub fc: ParameterGolfLinearWeights,
    /// Output projection.
    pub proj: ParameterGolfLinearWeights,
}

/// Full weights for one Parameter Golf block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBlockWeights {
    /// Attention weights.
    pub attention: ParameterGolfAttentionWeights,
    /// MLP weights.
    pub mlp: ParameterGolfMlpWeights,
    /// Learned attention residual scale.
    pub attn_scale: Vec<f32>,
    /// Learned MLP residual scale.
    pub mlp_scale: Vec<f32>,
    /// Learned residual mixing coefficients in `[2, model_dim]` order.
    pub resid_mix: Vec<f32>,
}

/// Control tensors kept separate from the contiguous matrix banks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBlockControlWeights {
    /// Learned q-gain, one scalar per query head.
    pub q_gain: Vec<f32>,
    /// Learned attention residual scale.
    pub attn_scale: Vec<f32>,
    /// Learned MLP residual scale.
    pub mlp_scale: Vec<f32>,
    /// Learned residual mixing coefficients in `[2, model_dim]` order.
    pub resid_mix: Vec<f32>,
}

/// Optional hashed-bigram feature tensors kept outside the block stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBigramHashWeights {
    /// Bigram embedding matrix in `[bigram_vocab_size, bigram_dim]` order.
    pub embedding: Vec<f32>,
    /// Optional projection in `[model_dim, bigram_dim]` order.
    pub proj: Option<ParameterGolfLinearWeights>,
    /// Learned scalar multiplier.
    pub scale: Vec<f32>,
}

/// Optional late-layer shared value-embedding tensors kept outside the block stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfValueEmbeddingWeights {
    /// Shared embedding matrix in `[vocab_size, ve_dim]` order.
    pub embedding: Vec<f32>,
    /// Optional projection in `[kv_dim, ve_dim]` order.
    pub proj: Option<ParameterGolfLinearWeights>,
    /// Learned shared scalar multiplier.
    pub scale: Vec<f32>,
    /// Learned per-layer scalar multipliers aligned with `ve_layer_indices`.
    pub layer_scales: Vec<f32>,
}

/// Full logical weight bundle for one Parameter Golf family instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfWeights {
    /// Token embedding matrix in `[vocab_size, model_dim]` order.
    pub token_embedding: Vec<f32>,
    /// Optional hashed bigram feature tensors.
    pub bigram: Option<ParameterGolfBigramHashWeights>,
    /// Optional shared late-layer value-embedding tensors.
    pub value_embedding: Option<ParameterGolfValueEmbeddingWeights>,
    /// Learned skip-weight table in `[num_skip_weights, model_dim]` order.
    pub skip_weights: Vec<f32>,
    /// Ordered block weights.
    pub blocks: Vec<ParameterGolfBlockWeights>,
    /// Optional untied LM head in `[vocab_size, model_dim]` order.
    pub lm_head: Option<ParameterGolfLinearWeights>,
}

/// Parameter-banked matrix surface matching the upstream competitive PGOLF lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBankedWeights {
    /// Token embedding matrix in `[vocab_size, model_dim]` order.
    pub token_embedding: Vec<f32>,
    /// Optional hashed bigram feature tensors that stay outside the matrix banks.
    pub bigram: Option<ParameterGolfBigramHashWeights>,
    /// Optional shared late-layer value-embedding tensors that stay outside the matrix banks.
    pub value_embedding: Option<ParameterGolfValueEmbeddingWeights>,
    /// Learned skip-weight table in `[num_skip_weights, model_dim]` order.
    pub skip_weights: Vec<f32>,
    /// Combined query/output bank in `[2 * num_layers, model_dim, model_dim]` order.
    pub qo_bank: ParameterGolfLinearBankWeights,
    /// Combined key/value bank in `[2 * num_layers, kv_dim, model_dim]` order.
    pub kv_bank: ParameterGolfLinearBankWeights,
    /// Combined MLP up-projection bank in `[num_layers, mlp_hidden_dim, model_dim]` order.
    pub mlp_up_bank: ParameterGolfLinearBankWeights,
    /// Combined MLP down-projection bank in `[num_layers, model_dim, mlp_hidden_dim]` order.
    pub mlp_down_bank: ParameterGolfLinearBankWeights,
    /// Per-block control tensors that remain separate from the banks.
    pub block_controls: Vec<ParameterGolfBlockControlWeights>,
    /// Optional untied LM head in `[vocab_size, model_dim]` order.
    pub lm_head: Option<ParameterGolfLinearWeights>,
}

/// One named Parameter Golf parameter tensor with stable shape metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfParameterVector {
    /// Stable tensor name.
    pub parameter_id: String,
    /// Logical tensor shape.
    pub shape: Shape,
    /// Flat dense values.
    pub values: Vec<f32>,
}

impl ParameterGolfWeights {
    /// Builds the deterministic reference bundle used by the frozen parity fixture.
    pub fn from_initializer(
        config: &ParameterGolfConfig,
        initializer: ParameterGolfDeterministicInitializer,
    ) -> Result<Self, ParameterGolfModelError> {
        config.validate()?;
        let kv_dim = config.kv_dim()?;
        let mlp_hidden_dim = config.mlp_hidden_dim()?;
        let blocks = (0..config.num_layers)
            .map(|layer_index| ParameterGolfBlockWeights {
                attention: ParameterGolfAttentionWeights {
                    q_proj: ParameterGolfLinearWeights::from_initializer(
                        format!("blocks.{layer_index}.attn.c_q.weight").as_str(),
                        config.model_dim,
                        config.model_dim,
                        initializer,
                    ),
                    k_proj: ParameterGolfLinearWeights::from_initializer(
                        format!("blocks.{layer_index}.attn.c_k.weight").as_str(),
                        kv_dim,
                        config.model_dim,
                        initializer,
                    ),
                    v_proj: ParameterGolfLinearWeights::from_initializer(
                        format!("blocks.{layer_index}.attn.c_v.weight").as_str(),
                        kv_dim,
                        config.model_dim,
                        initializer,
                    ),
                    out_proj: ParameterGolfLinearWeights::from_initializer(
                        format!("blocks.{layer_index}.attn.proj.weight").as_str(),
                        config.model_dim,
                        config.model_dim,
                        initializer,
                    ),
                    q_gain: initializer.values_for(
                        format!("blocks.{layer_index}.attn.q_gain").as_str(),
                        config.num_heads,
                    ),
                },
                mlp: ParameterGolfMlpWeights {
                    fc: ParameterGolfLinearWeights::from_initializer(
                        format!("blocks.{layer_index}.mlp.fc.weight").as_str(),
                        mlp_hidden_dim,
                        config.model_dim,
                        initializer,
                    ),
                    proj: ParameterGolfLinearWeights::from_initializer(
                        format!("blocks.{layer_index}.mlp.proj.weight").as_str(),
                        config.model_dim,
                        mlp_hidden_dim,
                        initializer,
                    ),
                },
                attn_scale: initializer.values_for(
                    format!("blocks.{layer_index}.attn_scale").as_str(),
                    config.model_dim,
                ),
                mlp_scale: initializer.values_for(
                    format!("blocks.{layer_index}.mlp_scale").as_str(),
                    config.model_dim,
                ),
                resid_mix: initializer.values_for(
                    format!("blocks.{layer_index}.resid_mix").as_str(),
                    2 * config.model_dim,
                ),
            })
            .collect();
        let bigram = if config.bigram_vocab_size > 0 {
            Some(ParameterGolfBigramHashWeights {
                embedding: vec![0.0; config.bigram_vocab_size * config.bigram_dim],
                proj: (config.bigram_dim != config.model_dim).then(|| ParameterGolfLinearWeights {
                    out_features: config.model_dim,
                    in_features: config.bigram_dim,
                    weight: vec![0.0; config.model_dim * config.bigram_dim],
                }),
                scale: vec![0.05],
            })
        } else {
            None
        };
        let value_embedding = if config.ve_layer_indices.is_empty() {
            None
        } else {
            Some(ParameterGolfValueEmbeddingWeights {
                embedding: initializer
                    .values_for("ve_shared.embed.weight", config.vocab_size * config.ve_dim),
                proj: (config.ve_dim != kv_dim).then(|| {
                    ParameterGolfLinearWeights::from_initializer(
                        "ve_shared.proj.weight",
                        kv_dim,
                        config.ve_dim,
                        initializer,
                    )
                }),
                scale: vec![0.1],
                layer_scales: vec![1.0; config.ve_layer_indices.len()],
            })
        };
        let lm_head = (!config.tie_embeddings).then(|| {
            ParameterGolfLinearWeights::from_initializer(
                "lm_head.weight",
                config.vocab_size,
                config.model_dim,
                initializer,
            )
        });
        Ok(Self {
            token_embedding: initializer
                .values_for("tok_emb.weight", config.vocab_size * config.model_dim),
            bigram,
            value_embedding,
            skip_weights: initializer
                .values_for("skip_weights", config.num_skip_weights() * config.model_dim),
            blocks,
            lm_head,
        })
    }

    fn bundle_metadata(&self, config: &ParameterGolfConfig) -> WeightBundleMetadata {
        let mut entries = self.tensor_entries(config);
        entries.sort_by(|(left, _), (right, _)| left.name.cmp(&right.name));
        let mut hasher = Sha256::new();
        for (metadata, values) in &entries {
            digest_tensor_values(&mut hasher, metadata, values);
        }
        WeightBundleMetadata {
            format: WeightFormat::ProgrammaticFixture,
            source: WeightSource::Fixture,
            quantization: QuantizationMode::None,
            quantization_modes: Vec::new(),
            digest: hex::encode(hasher.finalize()),
            tensors: entries.into_iter().map(|(metadata, _)| metadata).collect(),
            artifacts: Vec::new(),
        }
    }

    /// Returns the upstream-style banked matrix surface for the current split bundle.
    pub fn banked(
        &self,
        config: &ParameterGolfConfig,
    ) -> Result<ParameterGolfBankedWeights, ParameterGolfModelError> {
        config.validate()?;
        validate_weights(config, self)?;
        let qo_slices = self
            .blocks
            .iter()
            .map(|block| &block.attention.q_proj)
            .chain(self.blocks.iter().map(|block| &block.attention.out_proj))
            .collect::<Vec<_>>();
        let kv_slices = self
            .blocks
            .iter()
            .map(|block| &block.attention.k_proj)
            .chain(self.blocks.iter().map(|block| &block.attention.v_proj))
            .collect::<Vec<_>>();
        let mlp_up_slices = self
            .blocks
            .iter()
            .map(|block| &block.mlp.fc)
            .collect::<Vec<_>>();
        let mlp_down_slices = self
            .blocks
            .iter()
            .map(|block| &block.mlp.proj)
            .collect::<Vec<_>>();
        Ok(ParameterGolfBankedWeights {
            token_embedding: self.token_embedding.clone(),
            bigram: self.bigram.clone(),
            value_embedding: self.value_embedding.clone(),
            skip_weights: self.skip_weights.clone(),
            qo_bank: ParameterGolfLinearBankWeights::from_linears(qo_slices.as_slice()),
            kv_bank: ParameterGolfLinearBankWeights::from_linears(kv_slices.as_slice()),
            mlp_up_bank: ParameterGolfLinearBankWeights::from_linears(mlp_up_slices.as_slice()),
            mlp_down_bank: ParameterGolfLinearBankWeights::from_linears(mlp_down_slices.as_slice()),
            block_controls: self
                .blocks
                .iter()
                .map(|block| ParameterGolfBlockControlWeights {
                    q_gain: block.attention.q_gain.clone(),
                    attn_scale: block.attn_scale.clone(),
                    mlp_scale: block.mlp_scale.clone(),
                    resid_mix: block.resid_mix.clone(),
                })
                .collect(),
            lm_head: self.lm_head.clone(),
        })
    }

    fn tensor_entries<'a>(
        &'a self,
        config: &ParameterGolfConfig,
    ) -> Vec<(WeightTensorMetadata, &'a [f32])> {
        let mut entries = vec![(
            WeightTensorMetadata::new(
                "tok_emb.weight",
                Shape::new(vec![config.vocab_size, config.model_dim]),
                DType::F32,
            ),
            self.token_embedding.as_slice(),
        )];
        if let Some(bigram) = &self.bigram {
            entries.push((
                WeightTensorMetadata::new(
                    "bigram.embed.weight",
                    Shape::new(vec![config.bigram_vocab_size, config.bigram_dim]),
                    DType::F32,
                ),
                bigram.embedding.as_slice(),
            ));
            if let Some(proj) = &bigram.proj {
                entries.push((
                    WeightTensorMetadata::new(
                        "bigram.proj.weight",
                        Shape::new(vec![proj.out_features, proj.in_features]),
                        DType::F32,
                    ),
                    proj.weight.as_slice(),
                ));
            }
            entries.push((
                WeightTensorMetadata::new("bigram.scale", Shape::new(vec![1]), DType::F32),
                bigram.scale.as_slice(),
            ));
        }
        if let Some(value_embedding) = &self.value_embedding {
            entries.push((
                WeightTensorMetadata::new(
                    "ve_shared.embed.weight",
                    Shape::new(vec![config.vocab_size, config.ve_dim]),
                    DType::F32,
                ),
                value_embedding.embedding.as_slice(),
            ));
            if let Some(proj) = &value_embedding.proj {
                entries.push((
                    WeightTensorMetadata::new(
                        "ve_shared.proj.weight",
                        Shape::new(vec![proj.out_features, proj.in_features]),
                        DType::F32,
                    ),
                    proj.weight.as_slice(),
                ));
            }
            entries.push((
                WeightTensorMetadata::new("ve_shared.scale", Shape::new(vec![1]), DType::F32),
                value_embedding.scale.as_slice(),
            ));
            for (slot, layer_scale) in value_embedding.layer_scales.iter().enumerate() {
                entries.push((
                    WeightTensorMetadata::new(
                        format!("ve_layer_scales.{slot}"),
                        Shape::new(vec![1]),
                        DType::F32,
                    ),
                    std::slice::from_ref(layer_scale),
                ));
            }
        }
        entries.push((
            WeightTensorMetadata::new(
                "skip_weights",
                Shape::new(vec![config.num_skip_weights(), config.model_dim]),
                DType::F32,
            ),
            self.skip_weights.as_slice(),
        ));
        for (layer_index, block) in self.blocks.iter().enumerate() {
            entries.extend_from_slice(&[
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.attn.c_q.weight"),
                        Shape::new(vec![
                            block.attention.q_proj.out_features,
                            block.attention.q_proj.in_features,
                        ]),
                        DType::F32,
                    ),
                    block.attention.q_proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.attn.c_k.weight"),
                        Shape::new(vec![
                            block.attention.k_proj.out_features,
                            block.attention.k_proj.in_features,
                        ]),
                        DType::F32,
                    ),
                    block.attention.k_proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.attn.c_v.weight"),
                        Shape::new(vec![
                            block.attention.v_proj.out_features,
                            block.attention.v_proj.in_features,
                        ]),
                        DType::F32,
                    ),
                    block.attention.v_proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.attn.proj.weight"),
                        Shape::new(vec![
                            block.attention.out_proj.out_features,
                            block.attention.out_proj.in_features,
                        ]),
                        DType::F32,
                    ),
                    block.attention.out_proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.attn.q_gain"),
                        Shape::new(vec![config.num_heads]),
                        DType::F32,
                    ),
                    block.attention.q_gain.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.mlp.fc.weight"),
                        Shape::new(vec![block.mlp.fc.out_features, block.mlp.fc.in_features]),
                        DType::F32,
                    ),
                    block.mlp.fc.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.mlp.proj.weight"),
                        Shape::new(vec![
                            block.mlp.proj.out_features,
                            block.mlp.proj.in_features,
                        ]),
                        DType::F32,
                    ),
                    block.mlp.proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.attn_scale"),
                        Shape::new(vec![config.model_dim]),
                        DType::F32,
                    ),
                    block.attn_scale.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.mlp_scale"),
                        Shape::new(vec![config.model_dim]),
                        DType::F32,
                    ),
                    block.mlp_scale.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.resid_mix"),
                        Shape::new(vec![2, config.model_dim]),
                        DType::F32,
                    ),
                    block.resid_mix.as_slice(),
                ),
            ]);
        }
        if let Some(lm_head) = &self.lm_head {
            entries.push((
                WeightTensorMetadata::new(
                    "lm_head.weight",
                    Shape::new(vec![lm_head.out_features, lm_head.in_features]),
                    DType::F32,
                ),
                lm_head.weight.as_slice(),
            ));
        }
        entries
    }

    /// Returns all named parameter tensors in stable name order.
    #[must_use]
    pub fn parameter_vectors(
        &self,
        config: &ParameterGolfConfig,
    ) -> Vec<ParameterGolfParameterVector> {
        let mut vectors = self
            .tensor_entries(config)
            .into_iter()
            .map(|(metadata, values)| ParameterGolfParameterVector {
                parameter_id: metadata.name,
                shape: metadata.shape,
                values: values.to_vec(),
            })
            .collect::<Vec<_>>();
        vectors.sort_by(|left, right| left.parameter_id.cmp(&right.parameter_id));
        vectors
    }

    /// Returns one named parameter tensor when it exists.
    #[must_use]
    pub fn parameter_vector(
        &self,
        config: &ParameterGolfConfig,
        parameter_id: &str,
    ) -> Option<ParameterGolfParameterVector> {
        self.parameter_vectors(config)
            .into_iter()
            .find(|parameter| parameter.parameter_id == parameter_id)
    }

    /// Returns a rebuilt bundle with the supplied named tensor overrides.
    pub fn with_parameter_overrides(
        &self,
        config: &ParameterGolfConfig,
        overrides: &BTreeMap<String, Vec<f32>>,
    ) -> Result<Self, ParameterGolfModelError> {
        let mut updated = self.clone();
        for (parameter_id, values) in overrides {
            apply_parameter_override(&mut updated, parameter_id.as_str(), values.as_slice())?;
        }
        validate_weights(config, &updated)?;
        Ok(updated)
    }
}

impl ParameterGolfBankedWeights {
    /// Returns a rebuilt banked bundle with the supplied named tensor overrides.
    pub fn with_parameter_overrides(
        &self,
        config: &ParameterGolfConfig,
        overrides: &BTreeMap<String, Vec<f32>>,
    ) -> Result<Self, ParameterGolfModelError> {
        let mut updated = self.clone();
        for (parameter_id, values) in overrides {
            apply_banked_parameter_override(
                &mut updated,
                parameter_id.as_str(),
                values.as_slice(),
            )?;
        }
        let _ = updated.to_split(config)?;
        Ok(updated)
    }

    /// Rebuilds the split logical bundle from the banked matrix surface.
    pub fn to_split(
        &self,
        config: &ParameterGolfConfig,
    ) -> Result<ParameterGolfWeights, ParameterGolfModelError> {
        config.validate()?;
        if self.block_controls.len() != config.num_layers {
            return Err(ParameterGolfModelError::LayerCountMismatch {
                actual: self.block_controls.len(),
                expected: config.num_layers,
            });
        }
        self.qo_bank
            .validate(PARAMETER_GOLF_QO_BANK_NAME, 2 * config.num_layers)?;
        self.kv_bank
            .validate(PARAMETER_GOLF_KV_BANK_NAME, 2 * config.num_layers)?;
        self.mlp_up_bank
            .validate(PARAMETER_GOLF_MLP_UP_BANK_NAME, config.num_layers)?;
        self.mlp_down_bank
            .validate(PARAMETER_GOLF_MLP_DOWN_BANK_NAME, config.num_layers)?;

        let mut blocks = Vec::with_capacity(config.num_layers);
        for layer_index in 0..config.num_layers {
            let controls = &self.block_controls[layer_index];
            blocks.push(ParameterGolfBlockWeights {
                attention: ParameterGolfAttentionWeights {
                    q_proj: self.qo_bank.linear_at(
                        PARAMETER_GOLF_QO_BANK_NAME,
                        2 * config.num_layers,
                        layer_index,
                    )?,
                    k_proj: self.kv_bank.linear_at(
                        PARAMETER_GOLF_KV_BANK_NAME,
                        2 * config.num_layers,
                        layer_index,
                    )?,
                    v_proj: self.kv_bank.linear_at(
                        PARAMETER_GOLF_KV_BANK_NAME,
                        2 * config.num_layers,
                        config.num_layers + layer_index,
                    )?,
                    out_proj: self.qo_bank.linear_at(
                        PARAMETER_GOLF_QO_BANK_NAME,
                        2 * config.num_layers,
                        config.num_layers + layer_index,
                    )?,
                    q_gain: controls.q_gain.clone(),
                },
                mlp: ParameterGolfMlpWeights {
                    fc: self.mlp_up_bank.linear_at(
                        PARAMETER_GOLF_MLP_UP_BANK_NAME,
                        config.num_layers,
                        layer_index,
                    )?,
                    proj: self.mlp_down_bank.linear_at(
                        PARAMETER_GOLF_MLP_DOWN_BANK_NAME,
                        config.num_layers,
                        layer_index,
                    )?,
                },
                attn_scale: controls.attn_scale.clone(),
                mlp_scale: controls.mlp_scale.clone(),
                resid_mix: controls.resid_mix.clone(),
            });
        }

        let weights = ParameterGolfWeights {
            token_embedding: self.token_embedding.clone(),
            bigram: self.bigram.clone(),
            value_embedding: self.value_embedding.clone(),
            skip_weights: self.skip_weights.clone(),
            blocks,
            lm_head: self.lm_head.clone(),
        };
        validate_weights(config, &weights)?;
        Ok(weights)
    }

    fn bundle_metadata(&self, config: &ParameterGolfConfig) -> WeightBundleMetadata {
        let mut entries = self.tensor_entries(config);
        entries.sort_by(|(left, _), (right, _)| left.name.cmp(&right.name));
        let mut hasher = Sha256::new();
        for (metadata, values) in &entries {
            digest_tensor_values(&mut hasher, metadata, values);
        }
        WeightBundleMetadata {
            format: WeightFormat::ProgrammaticFixture,
            source: WeightSource::Fixture,
            quantization: QuantizationMode::None,
            quantization_modes: Vec::new(),
            digest: hex::encode(hasher.finalize()),
            tensors: entries.into_iter().map(|(metadata, _)| metadata).collect(),
            artifacts: Vec::new(),
        }
    }

    fn tensor_entries<'a>(
        &'a self,
        config: &ParameterGolfConfig,
    ) -> Vec<(WeightTensorMetadata, &'a [f32])> {
        let mut entries = vec![
            (
                WeightTensorMetadata::new(
                    "tok_emb.weight",
                    Shape::new(vec![config.vocab_size, config.model_dim]),
                    DType::F32,
                ),
                self.token_embedding.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    PARAMETER_GOLF_QO_BANK_NAME,
                    self.qo_bank.shape(),
                    DType::F32,
                ),
                self.qo_bank.weight.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    PARAMETER_GOLF_KV_BANK_NAME,
                    self.kv_bank.shape(),
                    DType::F32,
                ),
                self.kv_bank.weight.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    PARAMETER_GOLF_MLP_UP_BANK_NAME,
                    self.mlp_up_bank.shape(),
                    DType::F32,
                ),
                self.mlp_up_bank.weight.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    PARAMETER_GOLF_MLP_DOWN_BANK_NAME,
                    self.mlp_down_bank.shape(),
                    DType::F32,
                ),
                self.mlp_down_bank.weight.as_slice(),
            ),
        ];
        if let Some(bigram) = &self.bigram {
            entries.push((
                WeightTensorMetadata::new(
                    "bigram.embed.weight",
                    Shape::new(vec![config.bigram_vocab_size, config.bigram_dim]),
                    DType::F32,
                ),
                bigram.embedding.as_slice(),
            ));
            if let Some(proj) = &bigram.proj {
                entries.push((
                    WeightTensorMetadata::new(
                        "bigram.proj.weight",
                        Shape::new(vec![proj.out_features, proj.in_features]),
                        DType::F32,
                    ),
                    proj.weight.as_slice(),
                ));
            }
            entries.push((
                WeightTensorMetadata::new("bigram.scale", Shape::new(vec![1]), DType::F32),
                bigram.scale.as_slice(),
            ));
        }
        if let Some(value_embedding) = &self.value_embedding {
            entries.push((
                WeightTensorMetadata::new(
                    "ve_shared.embed.weight",
                    Shape::new(vec![config.vocab_size, config.ve_dim]),
                    DType::F32,
                ),
                value_embedding.embedding.as_slice(),
            ));
            if let Some(proj) = &value_embedding.proj {
                entries.push((
                    WeightTensorMetadata::new(
                        "ve_shared.proj.weight",
                        Shape::new(vec![proj.out_features, proj.in_features]),
                        DType::F32,
                    ),
                    proj.weight.as_slice(),
                ));
            }
            entries.push((
                WeightTensorMetadata::new("ve_shared.scale", Shape::new(vec![1]), DType::F32),
                value_embedding.scale.as_slice(),
            ));
            for (slot, layer_scale) in value_embedding.layer_scales.iter().enumerate() {
                entries.push((
                    WeightTensorMetadata::new(
                        format!("ve_layer_scales.{slot}"),
                        Shape::new(vec![1]),
                        DType::F32,
                    ),
                    std::slice::from_ref(layer_scale),
                ));
            }
        }
        entries.push((
            WeightTensorMetadata::new(
                "skip_weights",
                Shape::new(vec![config.num_skip_weights(), config.model_dim]),
                DType::F32,
            ),
            self.skip_weights.as_slice(),
        ));
        for (layer_index, controls) in self.block_controls.iter().enumerate() {
            entries.extend_from_slice(&[
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.attn.q_gain"),
                        Shape::new(vec![config.num_heads]),
                        DType::F32,
                    ),
                    controls.q_gain.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.attn_scale"),
                        Shape::new(vec![config.model_dim]),
                        DType::F32,
                    ),
                    controls.attn_scale.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.mlp_scale"),
                        Shape::new(vec![config.model_dim]),
                        DType::F32,
                    ),
                    controls.mlp_scale.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("blocks.{layer_index}.resid_mix"),
                        Shape::new(vec![2, config.model_dim]),
                        DType::F32,
                    ),
                    controls.resid_mix.as_slice(),
                ),
            ]);
        }
        if let Some(lm_head) = &self.lm_head {
            entries.push((
                WeightTensorMetadata::new(
                    "lm_head.weight",
                    Shape::new(vec![lm_head.out_features, lm_head.in_features]),
                    DType::F32,
                ),
                lm_head.weight.as_slice(),
            ));
        }
        entries
    }

    /// Returns all named tensors in stable name order for the banked surface.
    #[must_use]
    pub fn parameter_vectors(
        &self,
        config: &ParameterGolfConfig,
    ) -> Vec<ParameterGolfParameterVector> {
        let mut vectors = self
            .tensor_entries(config)
            .into_iter()
            .map(|(metadata, values)| ParameterGolfParameterVector {
                parameter_id: metadata.name,
                shape: metadata.shape,
                values: values.to_vec(),
            })
            .collect::<Vec<_>>();
        vectors.sort_by(|left, right| left.parameter_id.cmp(&right.parameter_id));
        vectors
    }

    /// Returns the weight metadata surface for the banked representation.
    #[must_use]
    pub fn weight_bundle_metadata(&self, config: &ParameterGolfConfig) -> WeightBundleMetadata {
        self.bundle_metadata(config)
    }
}

/// CPU-reference build failure for the Parameter Golf family.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ParameterGolfModelError {
    /// Config validation failed.
    #[error(transparent)]
    Config(#[from] ParameterGolfConfigError),
    /// The block count did not match the config.
    #[error("weight layer count {actual} does not match layer count {expected}")]
    LayerCountMismatch {
        /// Actual block count.
        actual: usize,
        /// Expected block count.
        expected: usize,
    },
    /// One flat vector did not match the expected element count.
    #[error("weight tensor `{name}` has length {actual}; expected {expected}")]
    InvalidVectorLength {
        /// Tensor name.
        name: String,
        /// Actual element count.
        actual: usize,
        /// Expected element count.
        expected: usize,
    },
    /// Tied-embedding configs must not also carry an untied LM head.
    #[error("lm_head must be absent when tie_embeddings=true")]
    UnexpectedLmHead,
    /// Untied configs require an LM head.
    #[error("lm_head is required when tie_embeddings=false")]
    MissingLmHead,
    /// Bigram-disabled configs must not carry bigram tensors.
    #[error("bigram tensors must be absent when bigram_vocab_size=0")]
    UnexpectedBigram,
    /// Bigram-enabled configs require bigram tensors.
    #[error("bigram tensors are required when bigram_vocab_size>0")]
    MissingBigram,
    /// The bigram projection must be absent when `bigram_dim == model_dim`.
    #[error("bigram projection must be absent when bigram_dim == model_dim")]
    UnexpectedBigramProjection,
    /// The bigram projection is required when `bigram_dim != model_dim`.
    #[error("bigram projection is required when bigram_dim != model_dim")]
    MissingBigramProjection,
    /// VE-disabled configs must not carry value-embedding tensors.
    #[error("value embedding tensors must be absent when ve_layer_indices is empty")]
    UnexpectedValueEmbedding,
    /// VE-enabled configs require value-embedding tensors.
    #[error("value embedding tensors are required when ve_layer_indices is non-empty")]
    MissingValueEmbedding,
    /// The VE projection must be absent when `ve_dim == kv_dim`.
    #[error("value embedding projection must be absent when ve_dim == kv_dim")]
    UnexpectedValueEmbeddingProjection,
    /// The VE projection is required when `ve_dim != kv_dim`.
    #[error("value embedding projection is required when ve_dim != kv_dim")]
    MissingValueEmbeddingProjection,
    /// One bank carried the wrong number of slices.
    #[error("weight bank `{name}` has bank_len {actual}; expected {expected}")]
    BankLengthMismatch {
        /// Bank tensor name.
        name: String,
        /// Actual bank slice count.
        actual: usize,
        /// Expected bank slice count.
        expected: usize,
    },
    /// A named parameter override targeted an unknown tensor.
    #[error("unknown parameter golf tensor `{parameter_id}`")]
    UnknownParameter {
        /// Unknown tensor name.
        parameter_id: String,
    },
}

/// CPU-reference execution failure for the Parameter Golf family.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ParameterGolfExecutionError {
    /// The caller supplied no sequences.
    #[error("input batch must not be empty")]
    EmptyBatch,
    /// The caller supplied an empty sequence.
    #[error("input sequence length must be positive")]
    EmptySequence,
    /// The caller supplied a ragged batch.
    #[error(
        "input batch is ragged: expected sequence length {expected}, batch {batch_index} had length {actual}"
    )]
    RaggedBatch {
        /// Batch index that diverged.
        batch_index: usize,
        /// Expected sequence length.
        expected: usize,
        /// Actual sequence length.
        actual: usize,
    },
    /// One token id exceeded the configured vocabulary.
    #[error("token id {token_id} is outside vocab size {vocab_size}")]
    TokenOutOfRange {
        /// Invalid token id.
        token_id: u32,
        /// Configured vocabulary size.
        vocab_size: usize,
    },
    /// The target batch shape did not match the input shape.
    #[error(
        "target shape mismatch: expected [{expected_batch}, {expected_sequence}], found [{actual_batch}, {actual_sequence}]"
    )]
    TargetShapeMismatch {
        /// Expected batch size.
        expected_batch: usize,
        /// Expected sequence length.
        expected_sequence: usize,
        /// Actual batch size.
        actual_batch: usize,
        /// Actual sequence length.
        actual_sequence: usize,
    },
    /// The requested causal attention window must be strictly positive.
    #[error("attention_window_size must be positive, got {attention_window_size}")]
    InvalidAttentionWindowSize {
        /// Invalid causal attention window.
        attention_window_size: usize,
    },
}

/// CPU-reference Parameter Golf model used for frozen parity fixtures and future trainer integration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfReferenceModel {
    descriptor: ParameterGolfModelDescriptor,
    weights: ParameterGolfWeights,
}

impl ParameterGolfReferenceModel {
    /// Creates one CPU-reference family instance from explicit weights.
    pub fn new(
        model: ModelDescriptor,
        config: ParameterGolfConfig,
        weights: ParameterGolfWeights,
    ) -> Result<Self, ParameterGolfModelError> {
        config.validate()?;
        validate_weights(&config, &weights)?;
        let descriptor = ParameterGolfModelDescriptor {
            model,
            weights: weights.bundle_metadata(&config),
            config,
        };
        Ok(Self {
            descriptor,
            weights,
        })
    }

    /// Creates the deterministic reference baseline used by the committed parity fixture.
    pub fn baseline_fixture(
        initializer: ParameterGolfDeterministicInitializer,
    ) -> Result<Self, ParameterGolfModelError> {
        let config = ParameterGolfConfig::baseline_sp1024_9x512();
        let weights = ParameterGolfWeights::from_initializer(&config, initializer)?;
        Self::new(
            ModelDescriptor::new(
                PARAMETER_GOLF_BASELINE_MODEL_ID,
                PARAMETER_GOLF_MODEL_FAMILY,
                PARAMETER_GOLF_BASELINE_REVISION,
            ),
            config,
            weights,
        )
    }

    /// Returns the stable descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &ParameterGolfModelDescriptor {
        &self.descriptor
    }

    /// Returns the logical weight bundle.
    #[must_use]
    pub fn weights(&self) -> &ParameterGolfWeights {
        &self.weights
    }

    /// Creates one execution-only model view that reuses the current
    /// descriptor and swaps in a compatible weight bundle without recomputing
    /// descriptor digests.
    ///
    /// This is intended for ephemeral runtime paths such as finite-difference
    /// loss probes where the caller already knows the override preserved the
    /// current model shape contract and does not need fresh persisted metadata.
    #[must_use]
    pub fn with_execution_weights_unchecked(&self, weights: ParameterGolfWeights) -> Self {
        Self {
            descriptor: self.descriptor.clone(),
            weights,
        }
    }

    /// Returns the upstream-style banked matrix surface for the current weights.
    pub fn banked_weights(&self) -> Result<ParameterGolfBankedWeights, ParameterGolfModelError> {
        self.weights.banked(&self.descriptor.config)
    }

    /// Returns the stable descriptor for the upstream-style banked matrix surface.
    pub fn banked_descriptor(
        &self,
    ) -> Result<ParameterGolfModelDescriptor, ParameterGolfModelError> {
        Ok(ParameterGolfModelDescriptor {
            model: self.descriptor.model.clone(),
            config: self.descriptor.config.clone(),
            weights: self
                .banked_weights()?
                .weight_bundle_metadata(&self.descriptor.config),
        })
    }

    /// Returns the union of the split and banked parameter surfaces in stable name order.
    pub fn all_parameter_vectors(
        &self,
    ) -> Result<Vec<ParameterGolfParameterVector>, ParameterGolfModelError> {
        let config = &self.descriptor.config;
        let mut vectors = self
            .weights
            .parameter_vectors(config)
            .into_iter()
            .map(|vector| (vector.parameter_id.clone(), vector))
            .collect::<BTreeMap<_, _>>();
        for vector in self.banked_weights()?.parameter_vectors(config) {
            vectors.insert(vector.parameter_id.clone(), vector);
        }
        Ok(vectors.into_values().collect())
    }

    /// Returns the final normalized hidden states before the LM head.
    pub fn forward_hidden(
        &self,
        input_ids: &[Vec<u32>],
    ) -> Result<ParameterGolfTensor3, ParameterGolfExecutionError> {
        self.forward_hidden_with_attention_window_impl(input_ids, None)
    }

    fn forward_hidden_with_attention_window_impl(
        &self,
        input_ids: &[Vec<u32>],
        attention_window_size: Option<usize>,
    ) -> Result<ParameterGolfTensor3, ParameterGolfExecutionError> {
        let (batch_size, sequence_length) =
            validate_token_batch(input_ids, self.descriptor.config.vocab_size)?;
        let attention_window_size = match attention_window_size {
            Some(0) => {
                return Err(ParameterGolfExecutionError::InvalidAttentionWindowSize {
                    attention_window_size: 0,
                });
            }
            Some(attention_window_size) => Some(attention_window_size.min(sequence_length)),
            None => None,
        };
        let config = &self.descriptor.config;
        let mut x = embedding_forward(
            &self.weights.token_embedding,
            config.vocab_size,
            config.model_dim,
            input_ids,
            batch_size,
            sequence_length,
        );
        if let Some(bigram) = &self.weights.bigram {
            let bigram = bigram_forward(bigram, config, input_ids, batch_size, sequence_length)?;
            add_in_place(&mut x, &bigram);
        }
        let value_embedding_base = if let Some(value_embedding) = &self.weights.value_embedding {
            Some(value_embedding_forward(
                value_embedding,
                config,
                input_ids,
                batch_size,
                sequence_length,
            )?)
        } else {
            None
        };
        x = rms_norm_last_dim(&x, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON);
        let x0 = x.clone();
        let mut skips = Vec::new();
        for layer_index in 0..config.num_encoder_layers() {
            let layer_value_embedding = value_embedding_for_layer(
                value_embedding_base.as_ref(),
                self.weights.value_embedding.as_ref(),
                config,
                layer_index,
            );
            x = block_forward(
                &self.weights.blocks[layer_index],
                &x,
                &x0,
                config,
                layer_index,
                layer_value_embedding.as_ref(),
                attention_window_size,
            );
            skips.push(x.clone());
        }
        for decoder_index in 0..config.num_decoder_layers() {
            if let Some(skip) = skips.pop() {
                add_scaled_in_place(
                    &mut x,
                    &skip,
                    skip_weight_row(&self.weights.skip_weights, config.model_dim, decoder_index),
                );
            }
            let layer_index = config.num_encoder_layers() + decoder_index;
            let layer_value_embedding = value_embedding_for_layer(
                value_embedding_base.as_ref(),
                self.weights.value_embedding.as_ref(),
                config,
                layer_index,
            );
            x = block_forward(
                &self.weights.blocks[layer_index],
                &x,
                &x0,
                config,
                layer_index,
                layer_value_embedding.as_ref(),
                attention_window_size,
            );
        }
        Ok(rms_norm_last_dim(
            &x,
            PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON,
        ))
    }

    /// Computes logits with the current tanh softcap.
    pub fn forward_logits(
        &self,
        input_ids: &[Vec<u32>],
    ) -> Result<ParameterGolfTensor3, ParameterGolfExecutionError> {
        let hidden = self.forward_hidden(input_ids)?;
        self.hidden_to_logits(hidden)
    }

    /// Computes logits with one bounded causal attention window.
    pub fn forward_logits_with_attention_window(
        &self,
        input_ids: &[Vec<u32>],
        attention_window_size: usize,
    ) -> Result<ParameterGolfTensor3, ParameterGolfExecutionError> {
        let hidden =
            self.forward_hidden_with_attention_window_impl(input_ids, Some(attention_window_size))?;
        self.hidden_to_logits(hidden)
    }

    fn hidden_to_logits(
        &self,
        hidden: ParameterGolfTensor3,
    ) -> Result<ParameterGolfTensor3, ParameterGolfExecutionError> {
        let logits_proj = if self.descriptor.config.tie_embeddings {
            linear_forward_with_weight(
                &hidden,
                self.weights.token_embedding.as_slice(),
                self.descriptor.config.vocab_size,
                self.descriptor.config.model_dim,
            )
        } else {
            let lm_head = self
                .weights
                .lm_head
                .as_ref()
                .expect("validated untied model must carry lm_head");
            linear_forward(lm_head, &hidden)
        };
        Ok(softcap_logits(
            logits_proj,
            self.descriptor.config.logit_softcap,
        ))
    }

    /// Computes mean cross-entropy loss over the flattened `[batch, seq]` targets.
    pub fn loss(
        &self,
        input_ids: &[Vec<u32>],
        target_ids: &[Vec<u32>],
    ) -> Result<f32, ParameterGolfExecutionError> {
        self.loss_with_attention_window_impl(input_ids, target_ids, None)
    }

    /// Computes mean cross-entropy loss with one bounded causal attention window.
    pub fn loss_with_attention_window(
        &self,
        input_ids: &[Vec<u32>],
        target_ids: &[Vec<u32>],
        attention_window_size: usize,
    ) -> Result<f32, ParameterGolfExecutionError> {
        self.loss_with_attention_window_impl(input_ids, target_ids, Some(attention_window_size))
    }

    fn loss_with_attention_window_impl(
        &self,
        input_ids: &[Vec<u32>],
        target_ids: &[Vec<u32>],
        attention_window_size: Option<usize>,
    ) -> Result<f32, ParameterGolfExecutionError> {
        let expected_batch = input_ids.len();
        let expected_sequence = input_ids.first().map(Vec::len).unwrap_or(0);
        if target_ids.len() != expected_batch
            || target_ids.first().map(Vec::len).unwrap_or(0) != expected_sequence
        {
            return Err(ParameterGolfExecutionError::TargetShapeMismatch {
                expected_batch,
                expected_sequence,
                actual_batch: target_ids.len(),
                actual_sequence: target_ids.first().map(Vec::len).unwrap_or(0),
            });
        }
        validate_token_batch(target_ids, self.descriptor.config.vocab_size)?;
        let logits = match attention_window_size {
            Some(attention_window_size) => {
                self.forward_logits_with_attention_window(input_ids, attention_window_size)?
            }
            None => self.forward_logits(input_ids)?,
        };
        Ok(mean_cross_entropy(&logits, target_ids))
    }
}

fn validate_weights(
    config: &ParameterGolfConfig,
    weights: &ParameterGolfWeights,
) -> Result<(), ParameterGolfModelError> {
    if weights.blocks.len() != config.num_layers {
        return Err(ParameterGolfModelError::LayerCountMismatch {
            actual: weights.blocks.len(),
            expected: config.num_layers,
        });
    }
    validate_vector_length(
        "tok_emb.weight",
        weights.token_embedding.as_slice(),
        config.vocab_size * config.model_dim,
    )?;
    if config.bigram_vocab_size == 0 {
        if weights.bigram.is_some() {
            return Err(ParameterGolfModelError::UnexpectedBigram);
        }
    } else {
        let Some(bigram) = &weights.bigram else {
            return Err(ParameterGolfModelError::MissingBigram);
        };
        validate_vector_length(
            "bigram.embed.weight",
            bigram.embedding.as_slice(),
            config.bigram_vocab_size * config.bigram_dim,
        )?;
        if config.bigram_dim == config.model_dim {
            if bigram.proj.is_some() {
                return Err(ParameterGolfModelError::UnexpectedBigramProjection);
            }
        } else {
            let Some(proj) = &bigram.proj else {
                return Err(ParameterGolfModelError::MissingBigramProjection);
            };
            validate_linear(
                "bigram.proj.weight",
                proj,
                config.model_dim,
                config.bigram_dim,
            )?;
        }
        validate_vector_length("bigram.scale", bigram.scale.as_slice(), 1)?;
    }
    let kv_dim = config.kv_dim()?;
    if config.ve_layer_indices.is_empty() {
        if weights.value_embedding.is_some() {
            return Err(ParameterGolfModelError::UnexpectedValueEmbedding);
        }
    } else {
        let Some(value_embedding) = &weights.value_embedding else {
            return Err(ParameterGolfModelError::MissingValueEmbedding);
        };
        validate_vector_length(
            "ve_shared.embed.weight",
            value_embedding.embedding.as_slice(),
            config.vocab_size * config.ve_dim,
        )?;
        if config.ve_dim == kv_dim {
            if value_embedding.proj.is_some() {
                return Err(ParameterGolfModelError::UnexpectedValueEmbeddingProjection);
            }
        } else {
            let Some(proj) = &value_embedding.proj else {
                return Err(ParameterGolfModelError::MissingValueEmbeddingProjection);
            };
            validate_linear("ve_shared.proj.weight", proj, kv_dim, config.ve_dim)?;
        }
        validate_vector_length("ve_shared.scale", value_embedding.scale.as_slice(), 1)?;
        validate_vector_length(
            "ve_layer_scales",
            value_embedding.layer_scales.as_slice(),
            config.ve_layer_indices.len(),
        )?;
    }
    validate_vector_length(
        "skip_weights",
        weights.skip_weights.as_slice(),
        config.num_skip_weights() * config.model_dim,
    )?;
    let mlp_hidden_dim = config.mlp_hidden_dim()?;
    for (layer_index, block) in weights.blocks.iter().enumerate() {
        validate_linear(
            format!("blocks.{layer_index}.attn.c_q.weight").as_str(),
            &block.attention.q_proj,
            config.model_dim,
            config.model_dim,
        )?;
        validate_linear(
            format!("blocks.{layer_index}.attn.c_k.weight").as_str(),
            &block.attention.k_proj,
            kv_dim,
            config.model_dim,
        )?;
        validate_linear(
            format!("blocks.{layer_index}.attn.c_v.weight").as_str(),
            &block.attention.v_proj,
            kv_dim,
            config.model_dim,
        )?;
        validate_linear(
            format!("blocks.{layer_index}.attn.proj.weight").as_str(),
            &block.attention.out_proj,
            config.model_dim,
            config.model_dim,
        )?;
        validate_vector_length(
            format!("blocks.{layer_index}.attn.q_gain").as_str(),
            block.attention.q_gain.as_slice(),
            config.num_heads,
        )?;
        validate_linear(
            format!("blocks.{layer_index}.mlp.fc.weight").as_str(),
            &block.mlp.fc,
            mlp_hidden_dim,
            config.model_dim,
        )?;
        validate_linear(
            format!("blocks.{layer_index}.mlp.proj.weight").as_str(),
            &block.mlp.proj,
            config.model_dim,
            mlp_hidden_dim,
        )?;
        validate_vector_length(
            format!("blocks.{layer_index}.attn_scale").as_str(),
            block.attn_scale.as_slice(),
            config.model_dim,
        )?;
        validate_vector_length(
            format!("blocks.{layer_index}.mlp_scale").as_str(),
            block.mlp_scale.as_slice(),
            config.model_dim,
        )?;
        validate_vector_length(
            format!("blocks.{layer_index}.resid_mix").as_str(),
            block.resid_mix.as_slice(),
            2 * config.model_dim,
        )?;
    }
    if config.tie_embeddings {
        if weights.lm_head.is_some() {
            return Err(ParameterGolfModelError::UnexpectedLmHead);
        }
    } else {
        let Some(lm_head) = &weights.lm_head else {
            return Err(ParameterGolfModelError::MissingLmHead);
        };
        validate_linear(
            "lm_head.weight",
            lm_head,
            config.vocab_size,
            config.model_dim,
        )?;
    }
    Ok(())
}

fn validate_linear(
    name: &str,
    linear: &ParameterGolfLinearWeights,
    out_features: usize,
    in_features: usize,
) -> Result<(), ParameterGolfModelError> {
    if linear.out_features != out_features || linear.in_features != in_features {
        return Err(ParameterGolfModelError::InvalidVectorLength {
            name: name.to_string(),
            actual: linear.out_features.saturating_mul(linear.in_features),
            expected: out_features.saturating_mul(in_features),
        });
    }
    validate_vector_length(
        name,
        linear.weight.as_slice(),
        out_features.saturating_mul(in_features),
    )
}

fn validate_vector_length(
    name: &str,
    values: &[f32],
    expected: usize,
) -> Result<(), ParameterGolfModelError> {
    if values.len() != expected {
        return Err(ParameterGolfModelError::InvalidVectorLength {
            name: name.to_string(),
            actual: values.len(),
            expected,
        });
    }
    Ok(())
}

fn apply_parameter_override(
    bundle: &mut ParameterGolfWeights,
    parameter_id: &str,
    values: &[f32],
) -> Result<(), ParameterGolfModelError> {
    if parameter_id == "tok_emb.weight" {
        assign_parameter_values(&mut bundle.token_embedding, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "bigram.embed.weight" {
        let Some(bigram) = &mut bundle.bigram else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut bigram.embedding, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "bigram.proj.weight" {
        let Some(bigram) = &mut bundle.bigram else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(proj) = &mut bigram.proj else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut proj.weight, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "bigram.scale" {
        let Some(bigram) = &mut bundle.bigram else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut bigram.scale, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "ve_shared.embed.weight" {
        let Some(value_embedding) = &mut bundle.value_embedding else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut value_embedding.embedding, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "ve_shared.proj.weight" {
        let Some(value_embedding) = &mut bundle.value_embedding else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(proj) = &mut value_embedding.proj else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut proj.weight, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "ve_shared.scale" {
        let Some(value_embedding) = &mut bundle.value_embedding else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut value_embedding.scale, parameter_id, values)?;
        return Ok(());
    }
    if let Some(slot_text) = parameter_id.strip_prefix("ve_layer_scales.") {
        let Some(value_embedding) = &mut bundle.value_embedding else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Ok(slot) = slot_text.parse::<usize>() else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(layer_scale) = value_embedding.layer_scales.get_mut(slot) else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let mut slot_buffer = vec![*layer_scale];
        assign_parameter_values(&mut slot_buffer, parameter_id, values)?;
        *layer_scale = slot_buffer[0];
        return Ok(());
    }
    if parameter_id == "skip_weights" {
        assign_parameter_values(&mut bundle.skip_weights, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "lm_head.weight" {
        let Some(lm_head) = &mut bundle.lm_head else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut lm_head.weight, parameter_id, values)?;
        return Ok(());
    }
    if let Some(rest) = parameter_id.strip_prefix("blocks.") {
        let Some((layer_text, tail)) = rest.split_once('.') else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Ok(layer_index) = layer_text.parse::<usize>() else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(block) = bundle.blocks.get_mut(layer_index) else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        match tail {
            "attn.c_q.weight" => {
                assign_parameter_values(&mut block.attention.q_proj.weight, parameter_id, values)?
            }
            "attn.c_k.weight" => {
                assign_parameter_values(&mut block.attention.k_proj.weight, parameter_id, values)?
            }
            "attn.c_v.weight" => {
                assign_parameter_values(&mut block.attention.v_proj.weight, parameter_id, values)?
            }
            "attn.proj.weight" => {
                assign_parameter_values(&mut block.attention.out_proj.weight, parameter_id, values)?
            }
            "attn.q_gain" => {
                assign_parameter_values(&mut block.attention.q_gain, parameter_id, values)?
            }
            "mlp.fc.weight" => {
                assign_parameter_values(&mut block.mlp.fc.weight, parameter_id, values)?
            }
            "mlp.proj.weight" => {
                assign_parameter_values(&mut block.mlp.proj.weight, parameter_id, values)?
            }
            "attn_scale" => assign_parameter_values(&mut block.attn_scale, parameter_id, values)?,
            "mlp_scale" => assign_parameter_values(&mut block.mlp_scale, parameter_id, values)?,
            "resid_mix" => assign_parameter_values(&mut block.resid_mix, parameter_id, values)?,
            _ => {
                return Err(ParameterGolfModelError::UnknownParameter {
                    parameter_id: String::from(parameter_id),
                });
            }
        }
        return Ok(());
    }
    Err(ParameterGolfModelError::UnknownParameter {
        parameter_id: String::from(parameter_id),
    })
}

fn apply_banked_parameter_override(
    bundle: &mut ParameterGolfBankedWeights,
    parameter_id: &str,
    values: &[f32],
) -> Result<(), ParameterGolfModelError> {
    if parameter_id == "tok_emb.weight" {
        assign_parameter_values(&mut bundle.token_embedding, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "bigram.embed.weight" {
        let Some(bigram) = &mut bundle.bigram else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut bigram.embedding, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "bigram.proj.weight" {
        let Some(bigram) = &mut bundle.bigram else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(proj) = &mut bigram.proj else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut proj.weight, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "bigram.scale" {
        let Some(bigram) = &mut bundle.bigram else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut bigram.scale, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "ve_shared.embed.weight" {
        let Some(value_embedding) = &mut bundle.value_embedding else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut value_embedding.embedding, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "ve_shared.proj.weight" {
        let Some(value_embedding) = &mut bundle.value_embedding else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(proj) = &mut value_embedding.proj else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut proj.weight, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "ve_shared.scale" {
        let Some(value_embedding) = &mut bundle.value_embedding else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut value_embedding.scale, parameter_id, values)?;
        return Ok(());
    }
    if let Some(slot_text) = parameter_id.strip_prefix("ve_layer_scales.") {
        let Some(value_embedding) = &mut bundle.value_embedding else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Ok(slot) = slot_text.parse::<usize>() else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(layer_scale) = value_embedding.layer_scales.get_mut(slot) else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let mut slot_buffer = vec![*layer_scale];
        assign_parameter_values(&mut slot_buffer, parameter_id, values)?;
        *layer_scale = slot_buffer[0];
        return Ok(());
    }
    if parameter_id == "skip_weights" {
        assign_parameter_values(&mut bundle.skip_weights, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == PARAMETER_GOLF_QO_BANK_NAME {
        assign_parameter_values(&mut bundle.qo_bank.weight, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == PARAMETER_GOLF_KV_BANK_NAME {
        assign_parameter_values(&mut bundle.kv_bank.weight, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == PARAMETER_GOLF_MLP_UP_BANK_NAME {
        assign_parameter_values(&mut bundle.mlp_up_bank.weight, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == PARAMETER_GOLF_MLP_DOWN_BANK_NAME {
        assign_parameter_values(&mut bundle.mlp_down_bank.weight, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "lm_head.weight" {
        let Some(lm_head) = &mut bundle.lm_head else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        assign_parameter_values(&mut lm_head.weight, parameter_id, values)?;
        return Ok(());
    }
    if let Some(rest) = parameter_id.strip_prefix("blocks.") {
        let Some((layer_text, tail)) = rest.split_once('.') else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Ok(layer_index) = layer_text.parse::<usize>() else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(block_controls) = bundle.block_controls.get_mut(layer_index) else {
            return Err(ParameterGolfModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        match tail {
            "attn.q_gain" => {
                assign_parameter_values(&mut block_controls.q_gain, parameter_id, values)?
            }
            "attn_scale" => {
                assign_parameter_values(&mut block_controls.attn_scale, parameter_id, values)?
            }
            "mlp_scale" => {
                assign_parameter_values(&mut block_controls.mlp_scale, parameter_id, values)?
            }
            "resid_mix" => {
                assign_parameter_values(&mut block_controls.resid_mix, parameter_id, values)?
            }
            _ => {
                return Err(ParameterGolfModelError::UnknownParameter {
                    parameter_id: String::from(parameter_id),
                });
            }
        }
        return Ok(());
    }
    Err(ParameterGolfModelError::UnknownParameter {
        parameter_id: String::from(parameter_id),
    })
}

fn assign_parameter_values(
    target: &mut Vec<f32>,
    parameter_id: &str,
    values: &[f32],
) -> Result<(), ParameterGolfModelError> {
    validate_vector_length(parameter_id, values, target.len())?;
    target.clear();
    target.extend_from_slice(values);
    Ok(())
}

fn validate_token_batch(
    input_ids: &[Vec<u32>],
    vocab_size: usize,
) -> Result<(usize, usize), ParameterGolfExecutionError> {
    if input_ids.is_empty() {
        return Err(ParameterGolfExecutionError::EmptyBatch);
    }
    let sequence_length = input_ids.first().map(Vec::len).unwrap_or(0);
    if sequence_length == 0 {
        return Err(ParameterGolfExecutionError::EmptySequence);
    }
    for (batch_index, row) in input_ids.iter().enumerate() {
        if row.len() != sequence_length {
            return Err(ParameterGolfExecutionError::RaggedBatch {
                batch_index,
                expected: sequence_length,
                actual: row.len(),
            });
        }
        for &token_id in row {
            if token_id as usize >= vocab_size {
                return Err(ParameterGolfExecutionError::TokenOutOfRange {
                    token_id,
                    vocab_size,
                });
            }
        }
    }
    Ok((input_ids.len(), sequence_length))
}

fn embedding_forward(
    embedding: &[f32],
    vocab_size: usize,
    model_dim: usize,
    input_ids: &[Vec<u32>],
    batch_size: usize,
    sequence_length: usize,
) -> ParameterGolfTensor3 {
    let mut output = ParameterGolfTensor3::zeros([batch_size, sequence_length, model_dim]);
    for (batch_index, row) in input_ids.iter().enumerate() {
        for (position_index, &token_id) in row.iter().enumerate() {
            debug_assert!((token_id as usize) < vocab_size);
            let source_offset = token_id as usize * model_dim;
            for feature in 0..model_dim {
                output.set(
                    batch_index,
                    position_index,
                    feature,
                    embedding[source_offset + feature],
                );
            }
        }
    }
    output
}

fn bigram_forward(
    bigram: &ParameterGolfBigramHashWeights,
    config: &ParameterGolfConfig,
    input_ids: &[Vec<u32>],
    batch_size: usize,
    sequence_length: usize,
) -> Result<ParameterGolfTensor3, ParameterGolfExecutionError> {
    let hashed = config
        .bigram_hash_batch(input_ids)?
        .expect("validated bigram-enabled config must materialize hashes");
    let mut output = embedding_forward(
        bigram.embedding.as_slice(),
        config.bigram_vocab_size,
        config.bigram_dim,
        hashed.as_slice(),
        batch_size,
        sequence_length,
    );
    if let Some(proj) = &bigram.proj {
        output = linear_forward(proj, &output);
    }
    for value in &mut output.values {
        *value *= bigram.scale[0];
    }
    Ok(output)
}

fn value_embedding_forward(
    value_embedding: &ParameterGolfValueEmbeddingWeights,
    config: &ParameterGolfConfig,
    input_ids: &[Vec<u32>],
    batch_size: usize,
    sequence_length: usize,
) -> Result<ParameterGolfTensor3, ParameterGolfExecutionError> {
    let kv_dim = config
        .kv_dim()
        .expect("validated value-embedding config must produce kv_dim");
    let mut output = embedding_forward(
        value_embedding.embedding.as_slice(),
        config.vocab_size,
        config.ve_dim,
        input_ids,
        batch_size,
        sequence_length,
    );
    if let Some(proj) = &value_embedding.proj {
        output = linear_forward_with_weight(&output, proj.weight.as_slice(), kv_dim, config.ve_dim);
    }
    for value in &mut output.values {
        *value *= value_embedding.scale[0];
    }
    Ok(output)
}

fn value_embedding_for_layer(
    value_embedding_base: Option<&ParameterGolfTensor3>,
    value_embedding: Option<&ParameterGolfValueEmbeddingWeights>,
    config: &ParameterGolfConfig,
    layer_index: usize,
) -> Option<ParameterGolfTensor3> {
    let base = value_embedding_base?;
    let slot = config.ve_layer_slot(layer_index)?;
    let value_embedding = value_embedding?;
    let mut output = base.clone();
    scale_tensor3_in_place(&mut output, value_embedding.layer_scales[slot]);
    Some(output)
}

fn rms_norm_last_dim(input: &ParameterGolfTensor3, epsilon: f32) -> ParameterGolfTensor3 {
    let mut output = ParameterGolfTensor3::zeros(input.shape());
    let width = input.width();
    for batch in 0..input.batch_size() {
        for position in 0..input.sequence_length() {
            let mut mean_square = 0.0_f32;
            for feature in 0..width {
                let value = input.get(batch, position, feature);
                mean_square += value * value;
            }
            mean_square /= width as f32;
            let scale = 1.0_f32 / (mean_square + epsilon).sqrt();
            for feature in 0..width {
                output.set(
                    batch,
                    position,
                    feature,
                    input.get(batch, position, feature) * scale,
                );
            }
        }
    }
    output
}

fn blend_with_source(
    current: &ParameterGolfTensor3,
    source: &ParameterGolfTensor3,
    mix: &[f32],
) -> ParameterGolfTensor3 {
    let width = current.width();
    let mut output = ParameterGolfTensor3::zeros(current.shape());
    for batch in 0..current.batch_size() {
        for position in 0..current.sequence_length() {
            for feature in 0..width {
                let mixed = mix[feature] * current.get(batch, position, feature)
                    + mix[width + feature] * source.get(batch, position, feature);
                output.set(batch, position, feature, mixed);
            }
        }
    }
    output
}

fn add_in_place(target: &mut ParameterGolfTensor3, delta: &ParameterGolfTensor3) {
    for batch in 0..target.batch_size() {
        for position in 0..target.sequence_length() {
            for feature in 0..target.width() {
                let value =
                    target.get(batch, position, feature) + delta.get(batch, position, feature);
                target.set(batch, position, feature, value);
            }
        }
    }
}

fn add_scaled_in_place(
    target: &mut ParameterGolfTensor3,
    delta: &ParameterGolfTensor3,
    scale: &[f32],
) {
    for batch in 0..target.batch_size() {
        for position in 0..target.sequence_length() {
            for feature in 0..target.width() {
                let value = target.get(batch, position, feature)
                    + scale[feature] * delta.get(batch, position, feature);
                target.set(batch, position, feature, value);
            }
        }
    }
}

fn scale_tensor3_in_place(target: &mut ParameterGolfTensor3, scale: f32) {
    if scale == 1.0 {
        return;
    }
    for value in &mut target.values {
        *value *= scale;
    }
}

fn block_forward(
    block: &ParameterGolfBlockWeights,
    x: &ParameterGolfTensor3,
    x0: &ParameterGolfTensor3,
    config: &ParameterGolfConfig,
    layer_index: usize,
    value_embedding: Option<&ParameterGolfTensor3>,
    attention_window_size: Option<usize>,
) -> ParameterGolfTensor3 {
    let mixed = blend_with_source(x, x0, block.resid_mix.as_slice());
    let mut normed_for_attention =
        rms_norm_last_dim(&mixed, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON);
    scale_tensor3_in_place(
        &mut normed_for_attention,
        config.layer_norm_scale_factor(layer_index),
    );
    let attention = attention_forward(
        &block.attention,
        &normed_for_attention,
        config,
        layer_index,
        value_embedding,
        attention_window_size,
    );
    let mut x = mixed;
    add_scaled_in_place(&mut x, &attention, block.attn_scale.as_slice());
    let mut normed_for_mlp = rms_norm_last_dim(&x, PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON);
    scale_tensor3_in_place(
        &mut normed_for_mlp,
        config.layer_norm_scale_factor(layer_index),
    );
    let mlp = mlp_forward(&block.mlp, &normed_for_mlp, config);
    add_scaled_in_place(&mut x, &mlp, block.mlp_scale.as_slice());
    x
}

fn linear_forward(
    linear: &ParameterGolfLinearWeights,
    input: &ParameterGolfTensor3,
) -> ParameterGolfTensor3 {
    linear_forward_with_weight(
        input,
        linear.weight.as_slice(),
        linear.out_features,
        linear.in_features,
    )
}

fn linear_forward_with_weight(
    input: &ParameterGolfTensor3,
    weight: &[f32],
    out_features: usize,
    in_features: usize,
) -> ParameterGolfTensor3 {
    debug_assert_eq!(input.width(), in_features);
    let mut output =
        ParameterGolfTensor3::zeros([input.batch_size(), input.sequence_length(), out_features]);
    for batch in 0..input.batch_size() {
        for position in 0..input.sequence_length() {
            for out_feature in 0..out_features {
                let mut sum = 0.0_f32;
                let row_offset = out_feature * in_features;
                for in_feature in 0..in_features {
                    sum += input.get(batch, position, in_feature) * weight[row_offset + in_feature];
                }
                output.set(batch, position, out_feature, sum);
            }
        }
    }
    output
}

fn mlp_forward(
    mlp: &ParameterGolfMlpWeights,
    input: &ParameterGolfTensor3,
    config: &ParameterGolfConfig,
) -> ParameterGolfTensor3 {
    let mut hidden = linear_forward(&mlp.fc, input);
    for value in &mut hidden.values {
        let activated = match config.mlp_activation {
            ParameterGolfMlpActivation::ReluSquared => value.max(0.0),
            ParameterGolfMlpActivation::LeakyReluSquared { negative_slope } => {
                if *value >= 0.0 {
                    *value
                } else {
                    *value * negative_slope.to_f32()
                }
            }
        };
        *value = activated * activated;
    }
    linear_forward(&mlp.proj, &hidden)
}

fn attention_forward(
    attention: &ParameterGolfAttentionWeights,
    input: &ParameterGolfTensor3,
    config: &ParameterGolfConfig,
    layer_index: usize,
    value_embedding: Option<&ParameterGolfTensor3>,
    attention_window_size: Option<usize>,
) -> ParameterGolfTensor3 {
    let batch_size = input.batch_size();
    let sequence_length = input.sequence_length();
    let head_dim = config.model_dim / config.num_heads;
    let kv_dim = config.num_kv_heads * head_dim;
    if attention_window_size == Some(1) && sequence_length == 1 {
        return attention_forward_single_token_window_one(
            attention,
            input,
            config,
            layer_index,
            value_embedding,
        );
    }
    let q_proj = linear_forward(&attention.q_proj, input);
    let k_proj = linear_forward(&attention.k_proj, input);
    let mut v_proj = linear_forward(&attention.v_proj, input);
    if let Some(value_embedding) = value_embedding {
        add_in_place(&mut v_proj, value_embedding);
    }

    let mut q = vec![0.0_f32; batch_size * config.num_heads * sequence_length * head_dim];
    let mut k = vec![0.0_f32; batch_size * config.num_kv_heads * sequence_length * head_dim];
    let mut v = vec![0.0_f32; batch_size * config.num_kv_heads * sequence_length * head_dim];

    for batch in 0..batch_size {
        for position in 0..sequence_length {
            for head in 0..config.num_heads {
                for feature in 0..head_dim {
                    q[tensor4_index(
                        batch_size,
                        config.num_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    )] = q_proj.get(batch, position, head * head_dim + feature);
                }
            }
            for head in 0..config.num_kv_heads {
                for feature in 0..head_dim {
                    let offset = head * head_dim + feature;
                    k[tensor4_index(
                        batch_size,
                        config.num_kv_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    )] = k_proj.get(batch, position, offset);
                    v[tensor4_index(
                        batch_size,
                        config.num_kv_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    )] = v_proj.get(batch, position, offset);
                }
            }
        }
    }

    rms_norm_heads_in_place(
        q.as_mut_slice(),
        batch_size,
        config.num_heads,
        sequence_length,
        head_dim,
        PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON,
    );
    rms_norm_heads_in_place(
        k.as_mut_slice(),
        batch_size,
        config.num_kv_heads,
        sequence_length,
        head_dim,
        PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON,
    );
    let rope_rotary_dim = config
        .effective_rope_rotary_dim()
        .expect("validated parameter golf config should resolve rope_rotary_dim");
    apply_rope_in_place(
        q.as_mut_slice(),
        batch_size,
        config.num_heads,
        sequence_length,
        head_dim,
        config.rope_base,
        rope_rotary_dim,
    );
    apply_rope_in_place(
        k.as_mut_slice(),
        batch_size,
        config.num_kv_heads,
        sequence_length,
        head_dim,
        config.rope_base,
        rope_rotary_dim,
    );
    for batch in 0..batch_size {
        for head in 0..config.num_heads {
            for position in 0..sequence_length {
                for feature in 0..head_dim {
                    let index = tensor4_index(
                        batch_size,
                        config.num_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    );
                    q[index] *= attention.q_gain[head];
                }
            }
        }
    }

    let group_size = config.num_heads / config.num_kv_heads;
    let scale = 1.0_f32 / (head_dim as f32).sqrt();
    let mut attended = vec![0.0_f32; batch_size * config.num_heads * sequence_length * head_dim];
    let mut scores = Vec::new();
    let mut weights = Vec::new();
    for batch in 0..batch_size {
        for head in 0..config.num_heads {
            let kv_head = head / group_size;
            for target_position in 0..sequence_length {
                let source_start = attention_window_size
                    .map(|attention_window_size| {
                        target_position
                            .saturating_add(1)
                            .saturating_sub(attention_window_size)
                    })
                    .unwrap_or(0);
                scores.clear();
                let mut max_score = f32::NEG_INFINITY;
                for source_position in source_start..=target_position {
                    let mut dot = 0.0_f32;
                    for feature in 0..head_dim {
                        let q_index = tensor4_index(
                            batch_size,
                            config.num_heads,
                            sequence_length,
                            head_dim,
                            batch,
                            head,
                            target_position,
                            feature,
                        );
                        let k_index = tensor4_index(
                            batch_size,
                            config.num_kv_heads,
                            sequence_length,
                            head_dim,
                            batch,
                            kv_head,
                            source_position,
                            feature,
                        );
                        dot += q[q_index] * k[k_index];
                    }
                    let score = dot * scale;
                    max_score = max_score.max(score);
                    scores.push(score);
                }
                weights.clear();
                let mut denom = 0.0_f32;
                for score in &scores {
                    let weight = (*score - max_score).exp();
                    denom += weight;
                    weights.push(weight);
                }
                for weight in &mut weights {
                    *weight /= denom;
                }
                for feature in 0..head_dim {
                    let mut value = 0.0_f32;
                    for (source_offset, weight) in weights.iter().enumerate() {
                        let source_position = source_start + source_offset;
                        let v_index = tensor4_index(
                            batch_size,
                            config.num_kv_heads,
                            sequence_length,
                            head_dim,
                            batch,
                            kv_head,
                            source_position,
                            feature,
                        );
                        value += *weight * v[v_index];
                    }
                    let out_index = tensor4_index(
                        batch_size,
                        config.num_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        target_position,
                        feature,
                    );
                    attended[out_index] = value;
                }
            }
        }
    }

    if config.xsa_applies_to_layer(layer_index) {
        apply_xsa_to_attended_in_place(
            attended.as_mut_slice(),
            v.as_slice(),
            batch_size,
            config.num_heads,
            config.num_kv_heads,
            sequence_length,
            head_dim,
            PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON,
        );
    }

    let mut merged = ParameterGolfTensor3::zeros([batch_size, sequence_length, config.model_dim]);
    for batch in 0..batch_size {
        for position in 0..sequence_length {
            for head in 0..config.num_heads {
                for feature in 0..head_dim {
                    merged.set(
                        batch,
                        position,
                        head * head_dim + feature,
                        attended[tensor4_index(
                            batch_size,
                            config.num_heads,
                            sequence_length,
                            head_dim,
                            batch,
                            head,
                            position,
                            feature,
                        )],
                    );
                }
            }
        }
    }
    debug_assert_eq!(kv_dim, attention.k_proj.out_features);
    linear_forward(&attention.out_proj, &merged)
}

fn attention_forward_single_token_window_one(
    attention: &ParameterGolfAttentionWeights,
    input: &ParameterGolfTensor3,
    config: &ParameterGolfConfig,
    layer_index: usize,
    value_embedding: Option<&ParameterGolfTensor3>,
) -> ParameterGolfTensor3 {
    let batch_size = input.batch_size();
    let sequence_length = input.sequence_length();
    debug_assert_eq!(sequence_length, 1);
    let head_dim = config.model_dim / config.num_heads;
    let kv_dim = config.num_kv_heads * head_dim;
    let mut v_proj = linear_forward(&attention.v_proj, input);
    if let Some(value_embedding) = value_embedding {
        add_in_place(&mut v_proj, value_embedding);
    }

    let group_size = config.num_heads / config.num_kv_heads;
    let mut value_heads =
        vec![0.0_f32; batch_size * config.num_kv_heads * sequence_length * head_dim];
    let mut attended = vec![0.0_f32; batch_size * config.num_heads * sequence_length * head_dim];
    for batch in 0..batch_size {
        for kv_head in 0..config.num_kv_heads {
            for feature in 0..head_dim {
                let value = v_proj.get(batch, 0, kv_head * head_dim + feature);
                value_heads[tensor4_index(
                    batch_size,
                    config.num_kv_heads,
                    sequence_length,
                    head_dim,
                    batch,
                    kv_head,
                    0,
                    feature,
                )] = value;
                for group_head in 0..group_size {
                    let head = kv_head * group_size + group_head;
                    attended[tensor4_index(
                        batch_size,
                        config.num_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        0,
                        feature,
                    )] = value;
                }
            }
        }
    }

    if config.xsa_applies_to_layer(layer_index) {
        apply_xsa_to_attended_in_place(
            attended.as_mut_slice(),
            value_heads.as_slice(),
            batch_size,
            config.num_heads,
            config.num_kv_heads,
            sequence_length,
            head_dim,
            PARAMETER_GOLF_DEFAULT_RMS_NORM_EPSILON,
        );
    }

    let mut merged = ParameterGolfTensor3::zeros([batch_size, sequence_length, config.model_dim]);
    for batch in 0..batch_size {
        for head in 0..config.num_heads {
            for feature in 0..head_dim {
                merged.set(
                    batch,
                    0,
                    head * head_dim + feature,
                    attended[tensor4_index(
                        batch_size,
                        config.num_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        0,
                        feature,
                    )],
                );
            }
        }
    }
    debug_assert_eq!(kv_dim, attention.k_proj.out_features);
    linear_forward(&attention.out_proj, &merged)
}

fn apply_xsa_to_attended_in_place(
    attended: &mut [f32],
    value_heads: &[f32],
    batch_size: usize,
    num_heads: usize,
    num_kv_heads: usize,
    sequence_length: usize,
    head_dim: usize,
    epsilon: f32,
) {
    let group_size = num_heads / num_kv_heads;
    let mut normalized = vec![0.0_f32; head_dim];
    for batch in 0..batch_size {
        for kv_head in 0..num_kv_heads {
            for position in 0..sequence_length {
                let mut sum_square = 0.0_f32;
                for feature in 0..head_dim {
                    let value = value_heads[tensor4_index(
                        batch_size,
                        num_kv_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        kv_head,
                        position,
                        feature,
                    )];
                    sum_square += value * value;
                }
                let inv_norm = (sum_square + head_dim as f32 * epsilon).sqrt().recip();
                for feature in 0..head_dim {
                    normalized[feature] = value_heads[tensor4_index(
                        batch_size,
                        num_kv_heads,
                        sequence_length,
                        head_dim,
                        batch,
                        kv_head,
                        position,
                        feature,
                    )] * inv_norm;
                }
                for group_head in 0..group_size {
                    let head = kv_head * group_size + group_head;
                    let mut dot = 0.0_f32;
                    for (feature, normalized_value) in normalized.iter().enumerate() {
                        dot += attended[tensor4_index(
                            batch_size,
                            num_heads,
                            sequence_length,
                            head_dim,
                            batch,
                            head,
                            position,
                            feature,
                        )] * *normalized_value;
                    }
                    for (feature, normalized_value) in normalized.iter().enumerate() {
                        let index = tensor4_index(
                            batch_size,
                            num_heads,
                            sequence_length,
                            head_dim,
                            batch,
                            head,
                            position,
                            feature,
                        );
                        attended[index] -= dot * *normalized_value;
                    }
                }
            }
        }
    }
}

fn tensor4_index(
    _batch_size: usize,
    head_count: usize,
    sequence_length: usize,
    head_dim: usize,
    batch: usize,
    head: usize,
    position: usize,
    feature: usize,
) -> usize {
    ((batch * head_count + head) * sequence_length + position) * head_dim + feature
}

fn rms_norm_heads_in_place(
    values: &mut [f32],
    batch_size: usize,
    head_count: usize,
    sequence_length: usize,
    head_dim: usize,
    epsilon: f32,
) {
    for batch in 0..batch_size {
        for head in 0..head_count {
            for position in 0..sequence_length {
                let mut mean_square = 0.0_f32;
                for feature in 0..head_dim {
                    let index = tensor4_index(
                        batch_size,
                        head_count,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    );
                    mean_square += values[index] * values[index];
                }
                mean_square /= head_dim as f32;
                let scale = 1.0_f32 / (mean_square + epsilon).sqrt();
                for feature in 0..head_dim {
                    let index = tensor4_index(
                        batch_size,
                        head_count,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    );
                    values[index] *= scale;
                }
            }
        }
    }
}

fn apply_rope_in_place(
    values: &mut [f32],
    batch_size: usize,
    head_count: usize,
    sequence_length: usize,
    head_dim: usize,
    rope_base: f32,
    rope_rotary_dim: usize,
) {
    let half = rope_rotary_dim / 2;
    let mut cos = vec![0.0_f32; sequence_length * half];
    let mut sin = vec![0.0_f32; sequence_length * half];
    for position in 0..sequence_length {
        for feature in 0..half {
            let exponent = (2 * feature) as f32 / rope_rotary_dim as f32;
            let inv_freq = 1.0_f32 / rope_base.powf(exponent);
            let angle = position as f32 * inv_freq;
            cos[position * half + feature] = angle.cos();
            sin[position * half + feature] = angle.sin();
        }
    }
    for batch in 0..batch_size {
        for head in 0..head_count {
            for position in 0..sequence_length {
                let mut rotated = vec![0.0_f32; head_dim];
                for feature in 0..head_dim {
                    rotated[feature] = values[tensor4_index(
                        batch_size,
                        head_count,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    )];
                }
                for feature in 0..half {
                    let x1_index = tensor4_index(
                        batch_size,
                        head_count,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    );
                    let x2_index = tensor4_index(
                        batch_size,
                        head_count,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        half + feature,
                    );
                    let c = cos[position * half + feature];
                    let s = sin[position * half + feature];
                    let x1 = values[x1_index];
                    let x2 = values[x2_index];
                    rotated[feature] = x1 * c + x2 * s;
                    rotated[half + feature] = x1 * (-s) + x2 * c;
                }
                for feature in 0..head_dim {
                    let index = tensor4_index(
                        batch_size,
                        head_count,
                        sequence_length,
                        head_dim,
                        batch,
                        head,
                        position,
                        feature,
                    );
                    values[index] = rotated[feature];
                }
            }
        }
    }
}

fn skip_weight_row(skip_weights: &[f32], model_dim: usize, row_index: usize) -> &[f32] {
    let start = row_index * model_dim;
    &skip_weights[start..start + model_dim]
}

fn softcap_logits(mut logits: ParameterGolfTensor3, logit_softcap: f32) -> ParameterGolfTensor3 {
    for value in &mut logits.values {
        *value = logit_softcap * (*value / logit_softcap).tanh();
    }
    logits
}

fn mean_cross_entropy(logits: &ParameterGolfTensor3, target_ids: &[Vec<u32>]) -> f32 {
    let mut total = 0.0_f32;
    let vocab_size = logits.width();
    for batch in 0..logits.batch_size() {
        for position in 0..logits.sequence_length() {
            let target = target_ids[batch][position] as usize;
            debug_assert!(target < vocab_size);
            let mut max_logit = f32::NEG_INFINITY;
            for vocab in 0..vocab_size {
                max_logit = max_logit.max(logits.get(batch, position, vocab));
            }
            let mut exp_sum = 0.0_f32;
            for vocab in 0..vocab_size {
                exp_sum += (logits.get(batch, position, vocab) - max_logit).exp();
            }
            let logsumexp = max_logit + exp_sum.ln();
            total += logsumexp - logits.get(batch, position, target);
        }
    }
    total / (logits.batch_size() * logits.sequence_length()) as f32
}

fn checked_mul_usize(
    left: usize,
    right: usize,
    component: &str,
) -> Result<usize, ParameterGolfConfigError> {
    left.checked_mul(right)
        .ok_or(ParameterGolfConfigError::ParameterCountOverflow {
            component: component.to_string(),
        })
}

fn checked_add_usize(
    left: usize,
    right: usize,
    component: &str,
) -> Result<usize, ParameterGolfConfigError> {
    left.checked_add(right)
        .ok_or(ParameterGolfConfigError::ParameterCountOverflow {
            component: component.to_string(),
        })
}

fn digest_tensor_values(hasher: &mut Sha256, metadata: &WeightTensorMetadata, values: &[f32]) {
    hasher.update(metadata.name.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.dtype).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.quantization).as_bytes());
    hasher.update(b"|");
    for dimension in metadata.shape.dims() {
        hasher.update(dimension.to_be_bytes());
    }
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_le_bytes());
    }
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
    use std::{fs, path::Path};

    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct BaselineFixture {
        config: ParameterGolfConfig,
        initializer: ParameterGolfDeterministicInitializer,
        input_ids: Vec<Vec<u32>>,
        target_ids: Vec<Vec<u32>>,
        expected_parameter_count: usize,
        expected_tensor_shapes: Vec<FixtureTensorShape>,
        expected_logits: FixtureTensor3,
        expected_loss: f32,
    }

    #[derive(Deserialize)]
    struct FixtureTensorShape {
        name: String,
        shape: Vec<usize>,
        numel: usize,
    }

    #[derive(Deserialize)]
    struct FixtureTensor3 {
        shape: [usize; 3],
        values: Vec<f32>,
    }

    fn load_baseline_fixture() -> BaselineFixture {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/parameter_golf/models/parameter_golf_baseline_model_fixture.json",
        );
        serde_json::from_slice(&fs::read(path).expect("fixture should exist"))
            .expect("fixture should deserialize")
    }

    #[test]
    fn baseline_config_matches_public_parameter_count_and_skip_layout() {
        let config = ParameterGolfConfig::baseline_sp1024_9x512();
        config.validate().expect("baseline config should validate");
        let facts = config.parameter_facts().expect("facts should compute");
        assert_eq!(config.num_encoder_layers(), 4);
        assert_eq!(config.num_decoder_layers(), 5);
        assert_eq!(config.num_skip_weights(), 4);
        assert_eq!(config.head_dim().expect("head dim should compute"), 64);
        assert_eq!(config.kv_dim().expect("kv dim should compute"), 256);
        assert_eq!(
            config
                .mlp_hidden_dim()
                .expect("mlp hidden dim should compute"),
            1024
        );
        assert_eq!(facts.token_embedding, 524_288);
        assert_eq!(facts.bigram_embedding, 0);
        assert_eq!(facts.bigram_projection, 0);
        assert_eq!(facts.bigram_scale, 0);
        assert_eq!(facts.ve_embedding, 0);
        assert_eq!(facts.ve_projection, 0);
        assert_eq!(facts.ve_scale, 0);
        assert_eq!(facts.ve_layer_scales, 0);
        assert_eq!(facts.skip_weights, 2_048);
        assert_eq!(facts.per_block_attention, 786_440);
        assert_eq!(facts.per_block_feed_forward, 1_048_576);
        assert_eq!(facts.per_block_control, 2_048);
        assert_eq!(facts.lm_head, 0);
        assert_eq!(facts.total_parameters, 17_059_912);
    }

    #[test]
    fn config_rejects_invalid_head_geometry_and_softcap() {
        let err = ParameterGolfConfig {
            num_kv_heads: 3,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        }
        .validate()
        .expect_err("invalid kv geometry should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::NumHeadsMustBeDivisibleByNumKvHeads { .. }
        ));

        let err = ParameterGolfConfig {
            logit_softcap: 0.0,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        }
        .validate()
        .expect_err("non-positive softcap should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::LogitSoftcapMustBePositive { .. }
        ));
    }

    #[test]
    fn config_rejects_negative_leaky_relu_squared_slope() {
        let err = ParameterGolfConfig {
            mlp_activation: ParameterGolfMlpActivation::LeakyReluSquared {
                negative_slope: StableF32::from_f32(-0.25),
            },
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        }
        .validate()
        .expect_err("negative leaky slope should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::MlpActivationNegativeSlopeMustBeFinite { .. }
        ));
    }

    #[test]
    fn config_rejects_invalid_bigram_shape() {
        let err = ParameterGolfConfig {
            bigram_vocab_size: 1,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        }
        .validate()
        .expect_err("size-one bigram vocab should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::BigramVocabSizeMustBeZeroOrAtLeastTwo
        ));

        let err = ParameterGolfConfig {
            bigram_vocab_size: 1536,
            bigram_dim: 0,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        }
        .validate()
        .expect_err("zero-width enabled bigram should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::BigramDimMustBePositiveWhenEnabled
        ));
    }

    #[test]
    fn config_rejects_invalid_value_embedding_shape() {
        let err = ParameterGolfConfig {
            ve_dim: 0,
            ve_layer_indices: vec![6],
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        }
        .validate()
        .expect_err("zero-width value embedding should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::VeDimMustBePositiveWhenEnabled
        ));

        let err = ParameterGolfConfig {
            ve_layer_indices: vec![6, 6],
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        }
        .validate()
        .expect_err("duplicate value-embedding layer slots should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::VeLayerIndicesMustBeStrictlyIncreasing {
                previous: 6,
                current: 6,
            }
        ));
    }

    #[test]
    fn baseline_fixture_deserializes_without_explicit_optional_scorepath_fields() {
        let fixture = load_baseline_fixture();
        assert_eq!(fixture.config.bigram_vocab_size, 0);
        assert_eq!(
            fixture.config.bigram_dim,
            PARAMETER_GOLF_BASELINE_BIGRAM_DIM
        );
        assert_eq!(
            fixture.config.mlp_activation,
            ParameterGolfMlpActivation::ReluSquared
        );
        assert_eq!(fixture.config.rope_rotary_dim, None);
        assert_eq!(
            fixture.config.layer_norm_scale,
            ParameterGolfLayerNormScale::None
        );
        assert_eq!(fixture.config.xsa_last_n, 0);
        assert_eq!(fixture.config.ve_dim, 128);
        assert!(fixture.config.ve_layer_indices.is_empty());
    }

    #[test]
    fn competitive_homegolf_v1_enables_competitive_family_surfaces() {
        let config = ParameterGolfConfig::competitive_homegolf_v1();
        config
            .validate()
            .expect("competitive config should validate");
        assert_eq!(config.num_layers, 10);
        assert_eq!(config.mlp_mult, 3);
        assert_eq!(config.bigram_vocab_size, 2_048);
        assert_eq!(
            config.mlp_activation,
            ParameterGolfMlpActivation::leaky_relu_squared_point_five()
        );
        assert_eq!(config.rope_rotary_dim, Some(32));
        assert_eq!(
            config.layer_norm_scale,
            ParameterGolfLayerNormScale::InverseSqrtLayerIndexPlusOne
        );
        assert_eq!(config.xsa_last_n, 2);
        assert_eq!(config.ve_layer_indices, vec![7, 8, 9]);
    }

    #[test]
    fn bigram_hash_batch_matches_public_reference_formula() {
        let config = ParameterGolfConfig {
            bigram_vocab_size: 1536,
            bigram_dim: 128,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        let hashed = config
            .bigram_hash_batch(&[vec![11_u32, 7, 99, 7]])
            .expect("hashing should succeed")
            .expect("bigram should be enabled");
        let mod_base = (config.bigram_vocab_size - 1) as u32;
        assert_eq!(hashed[0][0], mod_base);
        assert_eq!(
            hashed[0][1],
            (((36_313_u64 * 7_u64) ^ (27_191_u64 * 11_u64)) % mod_base as u64) as u32
        );
        assert_eq!(
            hashed[0][2],
            (((36_313_u64 * 99_u64) ^ (27_191_u64 * 7_u64)) % mod_base as u64) as u32
        );
        assert_eq!(
            hashed[0][3],
            (((36_313_u64 * 7_u64) ^ (27_191_u64 * 99_u64)) % mod_base as u64) as u32
        );
    }

    #[test]
    fn config_rejects_odd_rope_rotary_dim() {
        let config = ParameterGolfConfig {
            rope_rotary_dim: Some(15),
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        let err = config
            .validate()
            .expect_err("odd rope rotary dim should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::RopeRotaryDimMustBeEven {
                rope_rotary_dim: 15
            }
        ));
    }

    #[test]
    fn inverse_sqrt_layer_norm_scale_factor_matches_expected_schedule() {
        let config = ParameterGolfConfig {
            layer_norm_scale: ParameterGolfLayerNormScale::InverseSqrtLayerIndexPlusOne,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        assert_eq!(config.layer_norm_scale_factor(0), 1.0);
        assert!((config.layer_norm_scale_factor(3) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn config_rejects_xsa_last_n_larger_than_layer_count() {
        let config = ParameterGolfConfig {
            xsa_last_n: 10,
            ..ParameterGolfConfig::baseline_sp1024_9x512()
        };
        let err = config
            .validate()
            .expect_err("xsa_last_n past num_layers should be refused");
        assert!(matches!(
            err,
            ParameterGolfConfigError::XsaLastNMustNotExceedNumLayers {
                xsa_last_n: 10,
                num_layers: 9,
            }
        ));
    }

    #[test]
    fn leaky_relu_squared_point_five_changes_reference_logits() {
        let initializer = ParameterGolfDeterministicInitializer::default();
        let input_ids = vec![vec![0_u32, 1, 2, 3]];
        let baseline_config = ParameterGolfConfig::baseline_sp1024_9x512();
        let baseline = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-test-baseline",
                PARAMETER_GOLF_MODEL_FAMILY,
                "v1",
            ),
            baseline_config.clone(),
            ParameterGolfWeights::from_initializer(&baseline_config, initializer)
                .expect("baseline weights should build"),
        )
        .expect("baseline model should build");
        let leaky_config = ParameterGolfConfig {
            mlp_activation: ParameterGolfMlpActivation::leaky_relu_squared_point_five(),
            ..baseline_config
        };
        let leaky = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-test-leaky",
                PARAMETER_GOLF_MODEL_FAMILY,
                "v1",
            ),
            leaky_config.clone(),
            ParameterGolfWeights::from_initializer(&leaky_config, initializer)
                .expect("leaky weights should build"),
        )
        .expect("leaky model should build");

        let baseline_logits = baseline
            .forward_logits(&input_ids)
            .expect("baseline logits should materialize");
        let leaky_logits = leaky
            .forward_logits(&input_ids)
            .expect("leaky logits should materialize");
        assert_ne!(baseline_logits.values, leaky_logits.values);
    }

    #[test]
    fn partial_rope_and_layer_norm_scale_change_reference_logits() {
        let input_ids = vec![vec![0_u32, 1, 2, 3]];
        let baseline = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture should build");
        let scorepath_config = ParameterGolfConfig {
            rope_rotary_dim: Some(16),
            layer_norm_scale: ParameterGolfLayerNormScale::InverseSqrtLayerIndexPlusOne,
            ..baseline.descriptor().config.clone()
        };
        let scorepath = ParameterGolfReferenceModel::new(
            baseline.descriptor().model.clone(),
            scorepath_config,
            baseline.weights().clone(),
        )
        .expect("scorepath variant should build");
        let baseline_logits = baseline
            .forward_logits(input_ids.as_slice())
            .expect("baseline logits should execute");
        let scorepath_logits = scorepath
            .forward_logits(input_ids.as_slice())
            .expect("scorepath logits should execute");
        assert!(
            baseline_logits
                .max_abs_diff(&scorepath_logits)
                .unwrap_or_default()
                > 1e-4,
            "partial rope plus layerwise LN scaling should change reference logits"
        );
    }

    #[test]
    fn xsa_changes_reference_logits() {
        let input_ids = vec![vec![0_u32, 1, 2, 3]];
        let baseline = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture should build");
        let xsa_config = ParameterGolfConfig {
            xsa_last_n: 2,
            ..baseline.descriptor().config.clone()
        };
        let xsa = ParameterGolfReferenceModel::new(
            baseline.descriptor().model.clone(),
            xsa_config,
            baseline.weights().clone(),
        )
        .expect("xsa variant should build");
        let baseline_logits = baseline
            .forward_logits(input_ids.as_slice())
            .expect("baseline logits should execute");
        let xsa_logits = xsa
            .forward_logits(input_ids.as_slice())
            .expect("xsa logits should execute");
        assert!(
            baseline_logits
                .max_abs_diff(&xsa_logits)
                .unwrap_or_default()
                > 1e-4,
            "xsa should change reference logits"
        );
    }

    #[test]
    fn value_embeddings_change_reference_logits_when_weights_are_nonzero() {
        let initializer = ParameterGolfDeterministicInitializer::default();
        let input_ids = vec![vec![0_u32, 1, 2, 3]];
        let baseline_config = ParameterGolfConfig::baseline_sp1024_9x512();
        let baseline = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-test-baseline",
                PARAMETER_GOLF_MODEL_FAMILY,
                "v1",
            ),
            baseline_config.clone(),
            ParameterGolfWeights::from_initializer(&baseline_config, initializer)
                .expect("baseline weights should build"),
        )
        .expect("baseline model should build");
        let ve_config = ParameterGolfConfig {
            ve_dim: 128,
            ve_layer_indices: vec![6, 7, 8],
            ..baseline_config
        };
        let mut ve_weights = ParameterGolfWeights::from_initializer(&ve_config, initializer)
            .expect("value-embedding weights should build");
        let value_embedding = ve_weights
            .value_embedding
            .as_mut()
            .expect("value-embedding weights should exist");
        for token_id in &input_ids[0] {
            let row_offset = *token_id as usize * ve_config.ve_dim;
            for feature in 0..ve_config.ve_dim {
                value_embedding.embedding[row_offset + feature] = 0.01 * (feature as f32 + 1.0);
            }
        }
        if let Some(proj) = &mut value_embedding.proj {
            for value in &mut proj.weight {
                *value = 0.001;
            }
        }
        value_embedding.scale[0] = 0.2;
        value_embedding.layer_scales = vec![0.5, 0.75, 1.0];
        let ve_model = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-test-value-embedding",
                PARAMETER_GOLF_MODEL_FAMILY,
                "v1",
            ),
            ve_config,
            ve_weights,
        )
        .expect("value-embedding model should build");

        let baseline_logits = baseline
            .forward_logits(&input_ids)
            .expect("baseline logits should materialize");
        let ve_logits = ve_model
            .forward_logits(&input_ids)
            .expect("value-embedding logits should materialize");
        assert!(
            baseline_logits.max_abs_diff(&ve_logits).unwrap_or_default() > 1e-4,
            "value embeddings should change reference logits"
        );
    }

    #[test]
    fn bigram_hash_changes_reference_logits_when_weights_are_nonzero() {
        let initializer = ParameterGolfDeterministicInitializer::default();
        let input_ids = vec![vec![0_u32, 1, 2, 3]];
        let baseline_config = ParameterGolfConfig::baseline_sp1024_9x512();
        let baseline = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-test-baseline",
                PARAMETER_GOLF_MODEL_FAMILY,
                "v1",
            ),
            baseline_config.clone(),
            ParameterGolfWeights::from_initializer(&baseline_config, initializer)
                .expect("baseline weights should build"),
        )
        .expect("baseline model should build");
        let bigram_config = ParameterGolfConfig {
            bigram_vocab_size: 1536,
            bigram_dim: 128,
            ..baseline_config
        };
        let mut bigram_weights =
            ParameterGolfWeights::from_initializer(&bigram_config, initializer)
                .expect("bigram weights should build");
        let hashed = bigram_config
            .bigram_hash_batch(&input_ids)
            .expect("hashing should succeed")
            .expect("bigram should be enabled");
        let bigram = bigram_weights
            .bigram
            .as_mut()
            .expect("bigram weights should exist");
        for token_id in &hashed[0] {
            let row_offset = *token_id as usize * bigram_config.bigram_dim;
            for feature in 0..bigram_config.bigram_dim {
                bigram.embedding[row_offset + feature] = 0.01 * (feature as f32 + 1.0);
            }
        }
        if let Some(proj) = &mut bigram.proj {
            for value in &mut proj.weight {
                *value = 0.001;
            }
        }
        bigram.scale[0] = 0.2;
        let bigram_model = ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-test-bigram",
                PARAMETER_GOLF_MODEL_FAMILY,
                "v1",
            ),
            bigram_config,
            bigram_weights,
        )
        .expect("bigram model should build");

        let baseline_logits = baseline
            .forward_logits(&input_ids)
            .expect("baseline logits should materialize");
        let bigram_logits = bigram_model
            .forward_logits(&input_ids)
            .expect("bigram logits should materialize");
        assert_ne!(baseline_logits.values, bigram_logits.values);
    }

    #[test]
    fn deterministic_baseline_bundle_matches_public_tensor_layout_and_parity_fixture() {
        let fixture = load_baseline_fixture();
        let model = ParameterGolfReferenceModel::baseline_fixture(fixture.initializer)
            .expect("baseline fixture model should build");
        assert_eq!(model.descriptor().config, fixture.config);
        assert_eq!(
            model
                .descriptor()
                .config
                .parameter_facts()
                .expect("parameter facts should compute")
                .total_parameters,
            fixture.expected_parameter_count
        );
        assert_eq!(
            model.descriptor().weights.tensors.len(),
            fixture.expected_tensor_shapes.len()
        );
        for (actual, expected) in model
            .descriptor()
            .weights
            .tensors
            .iter()
            .zip(fixture.expected_tensor_shapes.iter())
        {
            assert_eq!(actual.name, expected.name);
            assert_eq!(actual.shape.dims(), expected.shape.as_slice());
            assert_eq!(actual.element_count(), expected.numel);
        }
    }

    #[test]
    fn banked_weights_roundtrip_back_to_split_bundle() {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture model should build");
        let banked = model
            .banked_weights()
            .expect("banked weights should materialize");
        let restored = banked
            .to_split(&model.descriptor().config)
            .expect("banked weights should restore the split bundle");
        assert_eq!(restored, *model.weights());
    }

    #[test]
    fn banked_weights_publish_upstream_bank_tensor_names_and_shapes() {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture model should build");
        let banked = model
            .banked_weights()
            .expect("banked weights should materialize");
        let vectors = banked.parameter_vectors(&model.descriptor().config);
        let qo_bank = vectors
            .iter()
            .find(|vector| vector.parameter_id == PARAMETER_GOLF_QO_BANK_NAME)
            .expect("qo bank should exist");
        let kv_bank = vectors
            .iter()
            .find(|vector| vector.parameter_id == PARAMETER_GOLF_KV_BANK_NAME)
            .expect("kv bank should exist");
        let mlp_up_bank = vectors
            .iter()
            .find(|vector| vector.parameter_id == PARAMETER_GOLF_MLP_UP_BANK_NAME)
            .expect("mlp up bank should exist");
        let mlp_down_bank = vectors
            .iter()
            .find(|vector| vector.parameter_id == PARAMETER_GOLF_MLP_DOWN_BANK_NAME)
            .expect("mlp down bank should exist");
        assert_eq!(qo_bank.shape.dims(), &[18, 512, 512]);
        assert_eq!(kv_bank.shape.dims(), &[18, 256, 512]);
        assert_eq!(mlp_up_bank.shape.dims(), &[9, 1024, 512]);
        assert_eq!(mlp_down_bank.shape.dims(), &[9, 512, 1024]);
    }

    #[test]
    fn banked_weights_accept_parameter_overrides_and_roundtrip_to_split() {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture model should build");
        let mut overrides = BTreeMap::new();
        let mut qo_bank = model
            .banked_weights()
            .expect("banked weights should materialize")
            .parameter_vectors(&model.descriptor().config)
            .into_iter()
            .find(|vector| vector.parameter_id == PARAMETER_GOLF_QO_BANK_NAME)
            .expect("qo bank should exist")
            .values;
        qo_bank[0] += 0.25;
        overrides.insert(String::from(PARAMETER_GOLF_QO_BANK_NAME), qo_bank.clone());

        let updated = model
            .banked_weights()
            .expect("banked weights should materialize")
            .with_parameter_overrides(&model.descriptor().config, &overrides)
            .expect("banked overrides should apply");
        let restored = updated
            .to_split(&model.descriptor().config)
            .expect("banked weights should restore the split bundle");
        assert_eq!(updated.qo_bank.weight, qo_bank);
        assert_eq!(restored.blocks[0].attention.q_proj.weight[0], qo_bank[0]);
    }

    #[test]
    fn execution_weight_override_clone_matches_validated_reference_loss() {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture model should build");
        let mut overrides = BTreeMap::new();
        let mut token_embedding = model
            .weights()
            .parameter_vectors(&model.descriptor().config)
            .into_iter()
            .find(|vector| vector.parameter_id == "tok_emb.weight")
            .expect("token embedding should exist")
            .values;
        token_embedding[0] += 0.125;
        overrides.insert(String::from("tok_emb.weight"), token_embedding);

        let overridden = model
            .weights()
            .with_parameter_overrides(&model.descriptor().config, &overrides)
            .expect("weight override should apply");
        let validated = ParameterGolfReferenceModel::new(
            model.descriptor().model.clone(),
            model.descriptor().config.clone(),
            overridden.clone(),
        )
        .expect("validated override model should build");
        let execution_only = model.with_execution_weights_unchecked(overridden);
        let input_ids = vec![vec![1_u32, 2, 3, 4]];
        let target_ids = vec![vec![2_u32, 3, 4, 5]];

        assert_eq!(
            execution_only
                .loss(input_ids.as_slice(), target_ids.as_slice())
                .expect("execution-only loss should execute"),
            validated
                .loss(input_ids.as_slice(), target_ids.as_slice())
                .expect("validated loss should execute"),
        );
    }

    #[test]
    fn reference_model_all_parameter_vectors_include_split_and_banked_surfaces() {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture model should build");
        let vectors = model
            .all_parameter_vectors()
            .expect("combined parameter vectors should materialize");
        assert!(vectors
            .iter()
            .any(|vector| vector.parameter_id == "blocks.0.attn.c_q.weight"));
        assert!(vectors
            .iter()
            .any(|vector| vector.parameter_id == PARAMETER_GOLF_QO_BANK_NAME));
        assert!(vectors
            .iter()
            .any(|vector| vector.parameter_id == PARAMETER_GOLF_KV_BANK_NAME));
    }

    #[test]
    fn deterministic_baseline_fixture_matches_train_gpt_logits_and_loss() {
        let fixture = load_baseline_fixture();
        let model = ParameterGolfReferenceModel::baseline_fixture(fixture.initializer)
            .expect("baseline fixture model should build");
        let logits = model
            .forward_logits(fixture.input_ids.as_slice())
            .expect("logits should compute");
        let expected_logits = ParameterGolfTensor3::new(
            fixture.expected_logits.shape,
            fixture.expected_logits.values,
        )
        .expect("expected logits tensor should build");
        let max_abs_diff = logits
            .max_abs_diff(&expected_logits)
            .expect("logit shapes should match");
        assert!(
            max_abs_diff < 5e-5,
            "max logit drift {max_abs_diff} exceeded tolerance"
        );
        let loss = model
            .loss(fixture.input_ids.as_slice(), fixture.target_ids.as_slice())
            .expect("loss should compute");
        assert!(
            (loss - fixture.expected_loss).abs() < 5e-6,
            "loss drift {} exceeded tolerance",
            (loss - fixture.expected_loss).abs()
        );
    }

    #[test]
    fn dense_attention_window_matches_default_forward_path() {
        let fixture = load_baseline_fixture();
        let model = ParameterGolfReferenceModel::baseline_fixture(fixture.initializer)
            .expect("baseline fixture model should build");
        let seq_len = fixture.input_ids.first().map(Vec::len).unwrap_or(0);
        let dense_logits = model
            .forward_logits(fixture.input_ids.as_slice())
            .expect("dense logits should compute");
        let windowed_logits = model
            .forward_logits_with_attention_window(fixture.input_ids.as_slice(), seq_len)
            .expect("seq_len window logits should compute");
        let max_abs_diff = dense_logits
            .max_abs_diff(&windowed_logits)
            .expect("shapes should match");
        assert!(
            max_abs_diff < 1e-6,
            "unexpected dense/windowed drift: {max_abs_diff}"
        );
    }

    #[test]
    fn windowed_attention_rejects_zero_window_size() {
        let fixture = load_baseline_fixture();
        let model = ParameterGolfReferenceModel::baseline_fixture(fixture.initializer)
            .expect("baseline fixture model should build");
        let error = model
            .forward_logits_with_attention_window(fixture.input_ids.as_slice(), 0)
            .expect_err("zero window size should be refused");
        assert!(matches!(
            error,
            ParameterGolfExecutionError::InvalidAttentionWindowSize {
                attention_window_size: 0
            }
        ));
    }

    #[test]
    fn single_token_window_one_matches_dense_forward_path() {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture model should build");
        let input_ids = vec![vec![1_u32]];
        let dense_logits = model
            .forward_logits(input_ids.as_slice())
            .expect("dense logits should compute");
        let windowed_logits = model
            .forward_logits_with_attention_window(input_ids.as_slice(), 1)
            .expect("single-token window logits should compute");
        let max_abs_diff = dense_logits
            .max_abs_diff(&windowed_logits)
            .expect("single-token shapes should match");
        assert!(
            max_abs_diff < 1e-6,
            "unexpected single-token dense/windowed drift: {max_abs_diff}"
        );
    }

    #[test]
    fn single_token_window_one_matches_dense_forward_path_with_xsa() {
        let baseline = ParameterGolfReferenceModel::baseline_fixture(Default::default())
            .expect("baseline fixture should build");
        let model = ParameterGolfReferenceModel::new(
            baseline.descriptor().model.clone(),
            ParameterGolfConfig {
                xsa_last_n: 2,
                ..baseline.descriptor().config.clone()
            },
            baseline.weights().clone(),
        )
        .expect("xsa variant should build");
        let input_ids = vec![vec![1_u32]];
        let dense_logits = model
            .forward_logits(input_ids.as_slice())
            .expect("dense xsa logits should compute");
        let windowed_logits = model
            .forward_logits_with_attention_window(input_ids.as_slice(), 1)
            .expect("single-token xsa window logits should compute");
        let max_abs_diff = dense_logits
            .max_abs_diff(&windowed_logits)
            .expect("single-token xsa shapes should match");
        assert!(
            max_abs_diff < 1e-6,
            "unexpected single-token xsa dense/windowed drift: {max_abs_diff}"
        );
    }

    #[test]
    fn promoted_profile_contracts_keep_shared_family_and_split_challenge_overlay() {
        let general = ParameterGolfPromotedProfileContract::general_psion_small_decoder_v0();
        let challenge = ParameterGolfPromotedProfileContract::strict_pgolf_challenge_v0();

        assert_eq!(general.family_id, PARAMETER_GOLF_MODEL_FAMILY);
        assert_eq!(challenge.family_id, PARAMETER_GOLF_MODEL_FAMILY);
        assert_eq!(general.baseline_model_id, PARAMETER_GOLF_BASELINE_MODEL_ID);
        assert_eq!(
            challenge.baseline_model_id,
            PARAMETER_GOLF_BASELINE_MODEL_ID
        );
        assert_eq!(general.baseline_config, challenge.baseline_config);
        assert_eq!(general.shared_capabilities, challenge.shared_capabilities);

        assert!(!general.challenge_overlay.exact_sp1024_tokenizer);
        assert!(!general.challenge_overlay.exact_fineweb_challenge_data);
        assert!(
            !general
                .challenge_overlay
                .exact_16_mib_compressed_artifact_cap
        );
        assert!(!general.challenge_overlay.score_first_ttt);
        assert!(!general.challenge_overlay.contest_bpb_accounting);

        assert!(challenge.challenge_overlay.exact_sp1024_tokenizer);
        assert!(challenge.challenge_overlay.exact_fineweb_challenge_data);
        assert!(
            challenge
                .challenge_overlay
                .exact_16_mib_compressed_artifact_cap
        );
        assert!(challenge.challenge_overlay.score_first_ttt);
        assert!(challenge.challenge_overlay.contest_bpb_accounting);
    }
}
