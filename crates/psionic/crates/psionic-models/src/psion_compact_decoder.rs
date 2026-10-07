use psionic_core::{DType, Shape};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ActivationFunction, DecoderAttentionConfig, DecoderBlockConfig, DecoderConfig,
    DecoderFeedForwardConfig, DecoderModelDescriptor, ModelDescriptor, WeightBundleMetadata,
    WeightFormat, WeightSource, WeightTensorMetadata,
};

/// Stable schema version for the first Psion compact-decoder descriptor.
pub const PSION_COMPACT_DECODER_DESCRIPTOR_SCHEMA_VERSION: &str =
    "psion.compact_decoder_descriptor.v1";
/// Stable family label for the first Psion compact decoder.
pub const PSION_COMPACT_DECODER_FAMILY: &str = "psion_decoder";
/// Stable checkpoint artifact file name used across the first Psion decoder family.
pub const PSION_COMPACT_DECODER_CHECKPOINT_FILE_NAME: &str = "model.safetensors";
/// Stable descriptor file name used across the first Psion decoder family.
pub const PSION_COMPACT_DECODER_DESCRIPTOR_FILE_NAME: &str = "descriptor.json";
/// Stable state-dict namespace prefix used across the first Psion decoder family.
pub const PSION_COMPACT_DECODER_TENSOR_PREFIX: &str = "decoder";
/// Stable export-format identifier for the first Psion decoder family.
pub const PSION_COMPACT_DECODER_EXPORT_FORMAT_ID: &str =
    "psionic.decoder_state_dict.safetensors.v1";

/// Size anchor admitted by the first Psion compact decoder family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCompactDecoderSizeAnchor {
    /// Pilot-class compact decoder for the first bounded training lane.
    Pilot32m,
    /// First more serious internal anchor without claiming larger closure.
    Internal128m,
}

impl PsionCompactDecoderSizeAnchor {
    #[must_use]
    pub const fn default_model_id(self) -> &'static str {
        match self {
            Self::Pilot32m => "psion-compact-decoder-pilot-v1",
            Self::Internal128m => "psion-compact-decoder-internal-v1",
        }
    }
}

/// Position-encoding strategy frozen by the first Psion compact decoder family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCompactDecoderPositionEncoding {
    /// Learned absolute position embeddings.
    LearnedAbsoluteEmbeddings,
}

/// Normalization strategy frozen by the first Psion compact decoder family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCompactDecoderNormKind {
    /// Per-channel layer norm with gamma and beta parameters.
    LayerNorm,
}

/// Weight-tying strategy frozen by the first Psion compact decoder family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCompactDecoderWeightTying {
    /// Input token embeddings are reused for the LM-head projection.
    InputEmbeddingAndLmHeadTied,
}

/// Tokenizer family label admitted by the first Psion compact decoder family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCompactDecoderTokenizerFamily {
    /// SentencePiece-style tokenizer family.
    SentencePiece,
    /// GPT-style BPE tokenizer family.
    BytePairEncoding,
    /// WordPiece tokenizer family.
    WordPiece,
    /// Generic unigram tokenizer family.
    Unigram,
}

impl PsionCompactDecoderTokenizerFamily {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SentencePiece => "sentence_piece",
            Self::BytePairEncoding => "byte_pair_encoding",
            Self::WordPiece => "word_piece",
            Self::Unigram => "unigram",
        }
    }
}

/// Explicit tokenizer binding for one Psion compact decoder descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCompactDecoderTokenizerBinding {
    /// Stable tokenizer identifier.
    pub tokenizer_id: String,
    /// Stable tokenizer version.
    pub tokenizer_version: String,
    /// Tokenizer family admitted by the decoder family.
    pub tokenizer_family: PsionCompactDecoderTokenizerFamily,
    /// Stable tokenizer digest.
    pub tokenizer_digest: String,
    /// Stable vocabulary size bound into the model config.
    pub vocab_size: usize,
    /// Optional digest over special-token inventory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special_tokens_digest: Option<String>,
    /// Optional digest over prompt-template metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_digest: Option<String>,
}

impl PsionCompactDecoderTokenizerBinding {
    /// Returns a stable digest over the tokenizer binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(
            b"psion_compact_decoder_tokenizer_binding|",
            &(self, self.tokenizer_family.label()),
        )
    }
}

/// Stable checkpoint naming and tensor-layout contract for one Psion compact decoder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCompactDecoderCheckpointContract {
    /// Checkpoint format used across the family.
    pub checkpoint_format: WeightFormat,
    /// Stable checkpoint file name.
    pub checkpoint_file_name: String,
    /// Stable tensor namespace prefix.
    pub state_dict_namespace: String,
    /// Ordered tensor metadata expected in checkpoints for the descriptor.
    pub tensor_layout: Vec<WeightTensorMetadata>,
    /// Stable digest over the tensor layout.
    pub tensor_layout_digest: String,
}

/// Stable export naming contract for one Psion compact decoder descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCompactDecoderExportContract {
    /// Stable descriptor file name.
    pub descriptor_file_name: String,
    /// Stable checkpoint file name.
    pub checkpoint_file_name: String,
    /// Stable export-format identifier.
    pub export_format_id: String,
    /// Expected checkpoint format for the export.
    pub checkpoint_format: WeightFormat,
    /// Stable digest over the checkpoint tensor layout.
    pub tensor_layout_digest: String,
}

/// Stable model-family descriptor for one Psion compact decoder size anchor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCompactDecoderDescriptor {
    /// Stable schema version.
    pub schema_version: String,
    /// Shared model identity.
    pub model: ModelDescriptor,
    /// Admitted size anchor for the descriptor.
    pub size_anchor: PsionCompactDecoderSizeAnchor,
    /// Position-encoding strategy.
    pub position_encoding: PsionCompactDecoderPositionEncoding,
    /// Normalization strategy.
    pub norm_kind: PsionCompactDecoderNormKind,
    /// Weight-tying strategy.
    pub weight_tying: PsionCompactDecoderWeightTying,
    /// Explicit decoder architecture config.
    pub config: DecoderConfig,
    /// Explicit tokenizer binding.
    pub tokenizer_binding: PsionCompactDecoderTokenizerBinding,
    /// Stable checkpoint naming and tensor-layout contract.
    pub checkpoint_contract: PsionCompactDecoderCheckpointContract,
    /// Stable export naming contract.
    pub export_contract: PsionCompactDecoderExportContract,
    /// Estimated parameter count implied by the tensor layout.
    pub parameter_count_estimate: u64,
}

impl PsionCompactDecoderDescriptor {
    /// Builds a descriptor for one admitted size anchor and explicit context length.
    pub fn new(
        size_anchor: PsionCompactDecoderSizeAnchor,
        revision: impl Into<String>,
        max_context_tokens: usize,
        tokenizer_binding: PsionCompactDecoderTokenizerBinding,
    ) -> Result<Self, PsionCompactDecoderError> {
        validate_tokenizer_binding(&tokenizer_binding)?;
        let revision = revision.into();
        ensure_nonempty(revision.as_str(), "descriptor.model.revision")?;
        let config = config_for_anchor(
            size_anchor,
            tokenizer_binding.vocab_size,
            max_context_tokens,
        )?;
        let checkpoint_contract = build_checkpoint_contract(&config)?;
        let export_contract = build_export_contract(&checkpoint_contract)?;
        let descriptor = Self {
            schema_version: String::from(PSION_COMPACT_DECODER_DESCRIPTOR_SCHEMA_VERSION),
            model: ModelDescriptor::new(
                size_anchor.default_model_id(),
                PSION_COMPACT_DECODER_FAMILY,
                revision,
            ),
            size_anchor,
            position_encoding: PsionCompactDecoderPositionEncoding::LearnedAbsoluteEmbeddings,
            norm_kind: PsionCompactDecoderNormKind::LayerNorm,
            weight_tying: PsionCompactDecoderWeightTying::InputEmbeddingAndLmHeadTied,
            config,
            tokenizer_binding,
            parameter_count_estimate: checkpoint_contract
                .tensor_layout
                .iter()
                .map(|tensor| u64::try_from(tensor.element_count()).expect("tensor count fits u64"))
                .sum(),
            checkpoint_contract,
            export_contract,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Returns a stable digest over the descriptor.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(
            b"psion_compact_decoder_descriptor|",
            &(self, self.tokenizer_binding.tokenizer_family.label()),
        )
    }

    /// Validates the descriptor for later checkpoint, eval, and serving use.
    pub fn validate(&self) -> Result<(), PsionCompactDecoderError> {
        ensure_nonempty(self.schema_version.as_str(), "descriptor.schema_version")?;
        if self.schema_version != PSION_COMPACT_DECODER_DESCRIPTOR_SCHEMA_VERSION {
            return Err(PsionCompactDecoderError::SchemaVersionMismatch {
                expected: String::from(PSION_COMPACT_DECODER_DESCRIPTOR_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.model.model_id.as_str(), "descriptor.model.model_id")?;
        check_string_match(
            self.model.family.as_str(),
            PSION_COMPACT_DECODER_FAMILY,
            "descriptor.model.family",
        )?;
        ensure_nonempty(self.model.revision.as_str(), "descriptor.model.revision")?;
        validate_tokenizer_binding(&self.tokenizer_binding)?;
        validate_decoder_config(&self.config)?;
        if self.tokenizer_binding.vocab_size != self.config.vocab_size {
            return Err(PsionCompactDecoderError::TokenizerVocabMismatch {
                expected: self.config.vocab_size,
                actual: self.tokenizer_binding.vocab_size,
            });
        }
        let expected_config = config_for_anchor(
            self.size_anchor,
            self.tokenizer_binding.vocab_size,
            self.config.max_context,
        )?;
        if self.config != expected_config {
            return Err(PsionCompactDecoderError::ConfigDriftFromSizeAnchor {
                size_anchor: self.size_anchor,
            });
        }
        let expected_checkpoint_contract = build_checkpoint_contract(&self.config)?;
        if self.checkpoint_contract != expected_checkpoint_contract {
            return Err(PsionCompactDecoderError::CheckpointContractMismatch);
        }
        let expected_export_contract = build_export_contract(&expected_checkpoint_contract)?;
        if self.export_contract != expected_export_contract {
            return Err(PsionCompactDecoderError::ExportContractMismatch);
        }
        let expected_parameter_count = expected_checkpoint_contract
            .tensor_layout
            .iter()
            .map(|tensor| u64::try_from(tensor.element_count()).expect("tensor count fits u64"))
            .sum::<u64>();
        if self.parameter_count_estimate != expected_parameter_count {
            return Err(PsionCompactDecoderError::ParameterCountMismatch {
                expected: expected_parameter_count,
                actual: self.parameter_count_estimate,
            });
        }
        Ok(())
    }

    /// Bridges the Psion-specific descriptor into the generic decoder model descriptor.
    pub fn to_decoder_model_descriptor(
        &self,
        weights: WeightBundleMetadata,
    ) -> Result<DecoderModelDescriptor, PsionCompactDecoderError> {
        self.validate()?;
        if weights.format != self.checkpoint_contract.checkpoint_format {
            return Err(PsionCompactDecoderError::CheckpointFormatMismatch {
                expected: self.checkpoint_contract.checkpoint_format,
                actual: weights.format,
            });
        }
        if weights.source != WeightSource::ExternalArtifact {
            return Err(PsionCompactDecoderError::WeightSourceMismatch {
                expected: WeightSource::ExternalArtifact,
                actual: weights.source,
            });
        }
        if weights.tensors != self.checkpoint_contract.tensor_layout {
            return Err(PsionCompactDecoderError::TensorLayoutMismatch);
        }
        Ok(DecoderModelDescriptor::new(
            self.model.clone(),
            self.config.clone(),
            self.tokenizer_binding.tokenizer_family.label(),
            weights,
        ))
    }
}

/// Error returned by the Psion compact-decoder contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionCompactDecoderError {
    /// One required field was missing or empty.
    #[error("Psion compact decoder field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// The descriptor schema version drifted from the expected contract.
    #[error("Psion compact decoder expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// One string field drifted from the expected value.
    #[error("Psion compact decoder field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// One size-anchor config no longer matches the frozen family contract.
    #[error("Psion compact decoder config drifted from the `{size_anchor:?}` size anchor")]
    ConfigDriftFromSizeAnchor {
        /// Size anchor that drifted.
        size_anchor: PsionCompactDecoderSizeAnchor,
    },
    /// One decoder config field was invalid.
    #[error("Psion compact decoder config field `{field}` is invalid: {detail}")]
    InvalidConfig {
        /// Field name.
        field: String,
        /// Machine-readable detail.
        detail: String,
    },
    /// Tokenizer vocab size did not match the model config vocab size.
    #[error("Psion compact decoder tokenizer vocab size mismatch: expected `{expected}`, found `{actual}`")]
    TokenizerVocabMismatch {
        /// Expected vocab size.
        expected: usize,
        /// Actual vocab size.
        actual: usize,
    },
    /// The rebuilt checkpoint contract drifted from the descriptor.
    #[error("Psion compact decoder checkpoint contract drifted from the descriptor")]
    CheckpointContractMismatch,
    /// The rebuilt export contract drifted from the descriptor.
    #[error("Psion compact decoder export contract drifted from the descriptor")]
    ExportContractMismatch,
    /// The parameter-count estimate drifted from the tensor layout.
    #[error(
        "Psion compact decoder parameter count mismatch: expected `{expected}`, found `{actual}`"
    )]
    ParameterCountMismatch {
        /// Expected parameter count.
        expected: u64,
        /// Actual parameter count.
        actual: u64,
    },
    /// The supplied weight bundle format did not match the checkpoint contract.
    #[error("Psion compact decoder expected checkpoint format `{expected:?}`, found `{actual:?}`")]
    CheckpointFormatMismatch {
        /// Expected checkpoint format.
        expected: WeightFormat,
        /// Actual checkpoint format.
        actual: WeightFormat,
    },
    /// The supplied weight source did not match the generic bridge contract.
    #[error("Psion compact decoder expected weight source `{expected:?}`, found `{actual:?}`")]
    WeightSourceMismatch {
        /// Expected source posture.
        expected: WeightSource,
        /// Actual source posture.
        actual: WeightSource,
    },
    /// The supplied weight bundle tensor layout drifted from the checkpoint contract.
    #[error(
        "Psion compact decoder weight bundle tensor layout drifted from the checkpoint contract"
    )]
    TensorLayoutMismatch,
}

fn config_for_anchor(
    size_anchor: PsionCompactDecoderSizeAnchor,
    vocab_size: usize,
    max_context_tokens: usize,
) -> Result<DecoderConfig, PsionCompactDecoderError> {
    if max_context_tokens == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.max_context"),
            detail: String::from("max_context must be greater than zero"),
        });
    }
    let (hidden_size, layer_count, head_count, intermediate_size) = match size_anchor {
        PsionCompactDecoderSizeAnchor::Pilot32m => (384, 8, 6, 1536),
        PsionCompactDecoderSizeAnchor::Internal128m => (768, 12, 12, 3072),
    };
    let config = DecoderConfig {
        hidden_size,
        layer_count,
        vocab_size,
        max_context: max_context_tokens,
        block: DecoderBlockConfig {
            attention: DecoderAttentionConfig {
                head_count,
                kv_head_count: head_count,
                head_dim: hidden_size / head_count,
                rotary_dim: hidden_size / head_count,
            },
            feed_forward: DecoderFeedForwardConfig {
                intermediate_size,
                activation: ActivationFunction::Silu,
            },
        },
    };
    validate_decoder_config(&config)?;
    Ok(config)
}

fn build_checkpoint_contract(
    config: &DecoderConfig,
) -> Result<PsionCompactDecoderCheckpointContract, PsionCompactDecoderError> {
    let tensor_layout = build_tensor_layout(config)?;
    Ok(PsionCompactDecoderCheckpointContract {
        checkpoint_format: WeightFormat::SafeTensors,
        checkpoint_file_name: String::from(PSION_COMPACT_DECODER_CHECKPOINT_FILE_NAME),
        state_dict_namespace: String::from(PSION_COMPACT_DECODER_TENSOR_PREFIX),
        tensor_layout_digest: digest_tensor_layout(tensor_layout.as_slice()),
        tensor_layout,
    })
}

fn build_export_contract(
    checkpoint_contract: &PsionCompactDecoderCheckpointContract,
) -> Result<PsionCompactDecoderExportContract, PsionCompactDecoderError> {
    ensure_nonempty(
        checkpoint_contract.checkpoint_file_name.as_str(),
        "checkpoint_contract.checkpoint_file_name",
    )?;
    ensure_nonempty(
        checkpoint_contract.state_dict_namespace.as_str(),
        "checkpoint_contract.state_dict_namespace",
    )?;
    Ok(PsionCompactDecoderExportContract {
        descriptor_file_name: String::from(PSION_COMPACT_DECODER_DESCRIPTOR_FILE_NAME),
        checkpoint_file_name: checkpoint_contract.checkpoint_file_name.clone(),
        export_format_id: String::from(PSION_COMPACT_DECODER_EXPORT_FORMAT_ID),
        checkpoint_format: checkpoint_contract.checkpoint_format,
        tensor_layout_digest: checkpoint_contract.tensor_layout_digest.clone(),
    })
}

fn build_tensor_layout(
    config: &DecoderConfig,
) -> Result<Vec<WeightTensorMetadata>, PsionCompactDecoderError> {
    validate_decoder_config(config)?;
    let hidden_size = config.hidden_size;
    let intermediate_size = config.block.feed_forward.intermediate_size;
    let vocab_size = config.vocab_size;
    let max_context = config.max_context;
    let mut tensors = vec![
        WeightTensorMetadata::new(
            "decoder.embed_tokens.weight",
            Shape::new(vec![vocab_size, hidden_size]),
            DType::F32,
        ),
        WeightTensorMetadata::new(
            "decoder.embed_positions.weight",
            Shape::new(vec![max_context, hidden_size]),
            DType::F32,
        ),
    ];
    for layer_index in 0..config.layer_count {
        let prefix = format!("decoder.layers.{layer_index}");
        tensors.extend([
            WeightTensorMetadata::new(
                format!("{prefix}.attention_norm.gamma"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention_norm.beta"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention.q_proj.weight"),
                Shape::new(vec![hidden_size, hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention.q_proj.bias"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention.k_proj.weight"),
                Shape::new(vec![hidden_size, hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention.k_proj.bias"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention.v_proj.weight"),
                Shape::new(vec![hidden_size, hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention.v_proj.bias"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention.o_proj.weight"),
                Shape::new(vec![hidden_size, hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.attention.o_proj.bias"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.feed_forward_norm.gamma"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.feed_forward_norm.beta"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.feed_forward.gate_proj.weight"),
                Shape::new(vec![hidden_size, intermediate_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.feed_forward.gate_proj.bias"),
                Shape::new(vec![intermediate_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.feed_forward.up_proj.weight"),
                Shape::new(vec![hidden_size, intermediate_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.feed_forward.up_proj.bias"),
                Shape::new(vec![intermediate_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.feed_forward.down_proj.weight"),
                Shape::new(vec![intermediate_size, hidden_size]),
                DType::F32,
            ),
            WeightTensorMetadata::new(
                format!("{prefix}.feed_forward.down_proj.bias"),
                Shape::new(vec![hidden_size]),
                DType::F32,
            ),
        ]);
    }
    tensors.extend([
        WeightTensorMetadata::new(
            "decoder.final_norm.gamma",
            Shape::new(vec![hidden_size]),
            DType::F32,
        ),
        WeightTensorMetadata::new(
            "decoder.final_norm.beta",
            Shape::new(vec![hidden_size]),
            DType::F32,
        ),
        WeightTensorMetadata::new("lm_head.bias", Shape::new(vec![vocab_size]), DType::F32),
    ]);
    Ok(tensors)
}

fn validate_decoder_config(config: &DecoderConfig) -> Result<(), PsionCompactDecoderError> {
    if config.hidden_size == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.hidden_size"),
            detail: String::from("hidden_size must be greater than zero"),
        });
    }
    if config.layer_count == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.layer_count"),
            detail: String::from("layer_count must be greater than zero"),
        });
    }
    if config.vocab_size == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.vocab_size"),
            detail: String::from("vocab_size must be greater than zero"),
        });
    }
    if config.max_context == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.max_context"),
            detail: String::from("max_context must be greater than zero"),
        });
    }
    if config.block.attention.head_count == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.block.attention.head_count"),
            detail: String::from("head_count must be greater than zero"),
        });
    }
    if config.block.attention.kv_head_count == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.block.attention.kv_head_count"),
            detail: String::from("kv_head_count must be greater than zero"),
        });
    }
    if config.block.attention.head_dim == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.block.attention.head_dim"),
            detail: String::from("head_dim must be greater than zero"),
        });
    }
    if config.block.attention.rotary_dim == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.block.attention.rotary_dim"),
            detail: String::from("rotary_dim must be greater than zero"),
        });
    }
    if config.hidden_size != config.block.attention.head_count * config.block.attention.head_dim {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.hidden_size"),
            detail: String::from("hidden_size must equal head_count * head_dim"),
        });
    }
    if config.block.attention.kv_head_count > config.block.attention.head_count {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.block.attention.kv_head_count"),
            detail: String::from("kv_head_count must not exceed head_count"),
        });
    }
    if !config
        .block
        .attention
        .head_count
        .is_multiple_of(config.block.attention.kv_head_count)
    {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.block.attention.kv_head_count"),
            detail: String::from("head_count must be a multiple of kv_head_count"),
        });
    }
    if config.block.feed_forward.intermediate_size == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("config.block.feed_forward.intermediate_size"),
            detail: String::from("intermediate_size must be greater than zero"),
        });
    }
    Ok(())
}

fn validate_tokenizer_binding(
    tokenizer_binding: &PsionCompactDecoderTokenizerBinding,
) -> Result<(), PsionCompactDecoderError> {
    ensure_nonempty(
        tokenizer_binding.tokenizer_id.as_str(),
        "descriptor.tokenizer_binding.tokenizer_id",
    )?;
    ensure_nonempty(
        tokenizer_binding.tokenizer_version.as_str(),
        "descriptor.tokenizer_binding.tokenizer_version",
    )?;
    ensure_nonempty(
        tokenizer_binding.tokenizer_digest.as_str(),
        "descriptor.tokenizer_binding.tokenizer_digest",
    )?;
    if tokenizer_binding.vocab_size == 0 {
        return Err(PsionCompactDecoderError::InvalidConfig {
            field: String::from("descriptor.tokenizer_binding.vocab_size"),
            detail: String::from("vocab_size must be greater than zero"),
        });
    }
    Ok(())
}

fn digest_tensor_layout(layout: &[WeightTensorMetadata]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_compact_decoder_tensor_layout|");
    for tensor in layout {
        digest_tensor_metadata(&mut hasher, tensor);
    }
    hex::encode(hasher.finalize())
}

fn digest_tensor_metadata(hasher: &mut Sha256, metadata: &WeightTensorMetadata) {
    hasher.update(metadata.name.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.dtype).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.quantization).as_bytes());
    hasher.update(b"|");
    for dim in metadata.shape.dims() {
        hasher.update(dim.to_string().as_bytes());
        hasher.update(b",");
    }
    if let Some(layout) = metadata.quantized_layout {
        hasher.update(b"|");
        hasher.update(format!("{layout:?}").as_bytes());
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("Psion compact decoder value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded.as_slice());
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionCompactDecoderError> {
    if value.trim().is_empty() {
        return Err(PsionCompactDecoderError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionCompactDecoderError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionCompactDecoderError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenizer_binding() -> PsionCompactDecoderTokenizerBinding {
        PsionCompactDecoderTokenizerBinding {
            tokenizer_id: String::from("psion_sentencepiece_seed"),
            tokenizer_version: String::from("v1"),
            tokenizer_family: PsionCompactDecoderTokenizerFamily::SentencePiece,
            tokenizer_digest: String::from("sha256:psion_sentencepiece_seed_tokenizer_digest_v1"),
            vocab_size: 32_768,
            special_tokens_digest: Some(String::from(
                "sha256:psion_sentencepiece_seed_added_tokens_digest_v1",
            )),
            template_digest: Some(String::from(
                "sha256:psion_sentencepiece_seed_config_digest_v1",
            )),
        }
    }

    fn pilot_descriptor() -> PsionCompactDecoderDescriptor {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/models/psion_compact_decoder_pilot_descriptor_v1.json"
        ))
        .expect("pilot descriptor fixture should parse")
    }

    fn internal_descriptor() -> PsionCompactDecoderDescriptor {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json"
        ))
        .expect("internal descriptor fixture should parse")
    }

    fn matching_weights(descriptor: &PsionCompactDecoderDescriptor) -> WeightBundleMetadata {
        WeightBundleMetadata {
            format: WeightFormat::SafeTensors,
            source: WeightSource::ExternalArtifact,
            quantization: psionic_core::QuantizationMode::None,
            quantization_modes: Vec::new(),
            digest: String::from("sha256:psion_compact_decoder_placeholder_weight_bundle_v1"),
            tensors: descriptor.checkpoint_contract.tensor_layout.clone(),
            artifacts: Vec::new(),
        }
    }

    #[test]
    fn pilot_descriptor_fixture_validates_and_keeps_context_explicit() {
        let descriptor = pilot_descriptor();
        descriptor
            .validate()
            .expect("pilot descriptor should validate");
        assert_eq!(descriptor.model.family, PSION_COMPACT_DECODER_FAMILY);
        assert_eq!(descriptor.config.max_context, 4096);
        assert_eq!(
            descriptor.tokenizer_binding.vocab_size,
            descriptor.config.vocab_size
        );
    }

    #[test]
    fn internal_anchor_scales_shapes_but_keeps_export_naming_stable() {
        let pilot = pilot_descriptor();
        let internal = internal_descriptor();
        pilot.validate().expect("pilot descriptor should validate");
        internal
            .validate()
            .expect("internal descriptor should validate");
        assert_eq!(
            pilot.checkpoint_contract.checkpoint_file_name,
            internal.checkpoint_contract.checkpoint_file_name
        );
        assert_eq!(
            pilot.export_contract.descriptor_file_name,
            internal.export_contract.descriptor_file_name
        );
        assert!(internal.parameter_count_estimate > pilot.parameter_count_estimate);
        assert!(
            internal.checkpoint_contract.tensor_layout.len()
                > pilot.checkpoint_contract.tensor_layout.len()
        );
    }

    #[test]
    fn tokenizer_vocab_must_match_decoder_config() {
        let mut descriptor = pilot_descriptor();
        descriptor.tokenizer_binding.vocab_size = 16_384;
        let error = descriptor
            .validate()
            .expect_err("vocab mismatch should be rejected");
        assert!(matches!(
            error,
            PsionCompactDecoderError::TokenizerVocabMismatch { .. }
        ));
    }

    #[test]
    fn decoder_descriptor_bridge_requires_matching_tensor_layout() {
        let descriptor = PsionCompactDecoderDescriptor::new(
            PsionCompactDecoderSizeAnchor::Pilot32m,
            "v1",
            4096,
            tokenizer_binding(),
        )
        .expect("descriptor should build");
        let mut weights = matching_weights(&descriptor);
        weights.tensors.pop();
        let error = descriptor
            .to_decoder_model_descriptor(weights)
            .expect_err("drifted tensor layout should be rejected");
        assert!(matches!(
            error,
            PsionCompactDecoderError::TensorLayoutMismatch
        ));
    }
}
