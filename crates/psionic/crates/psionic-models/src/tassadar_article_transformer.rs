use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_core::{DType, Shape, TensorData};
use psionic_runtime::{
    build_tassadar_article_transformer_forward_pass_evidence_bundle,
    tassadar_article_trace_machine_step_schema, TassadarArticleTraceChannelKind,
    TassadarArticleTraceMachineStepSchema, TassadarArticleTransformerCheckpointLineage,
    TassadarArticleTransformerDecodeReceipt, TassadarArticleTransformerForwardPassChannelTrace,
    TassadarArticleTransformerForwardPassEvidenceBundle,
    TassadarArticleTransformerForwardPassRunConfig,
    TassadarArticleTransformerForwardPassTraceArtifact,
    TassadarArticleTransformerModelArtifactBinding, TassadarArticleTransformerReplayReceipt,
    TassadarExecution, TassadarProgram,
};
use psionic_transformer::{
    ActivationKind, AttentionMask, AttentionProbabilityTrace, EncoderDecoderTransformer,
    EncoderDecoderTransformerConfig, EncoderDecoderTransformerError, LayerError, LayerNorm, Linear,
    MultiHeadAttention, PositionwiseFeedForward, TransformerDecoderLayer, TransformerEmbeddings,
    TransformerEncoderLayer, TransformerExecutionMode,
};
use safetensors::{serialize, tensor::TensorView, Dtype as SafeTensorsDType, SafeTensors};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelDescriptor, TassadarArticleTraceDecodeError, TassadarDecodedArticleTraceDomain,
    TassadarTraceTokenizer, TokenId, TokenizerBoundary, WeightArtifactMetadata,
    WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
};

/// Stable architecture classification for the canonical article Transformer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerArchitectureVariant {
    AttentionIsAllYouNeedEncoderDecoder,
}

/// Supported embedding and logits-weight sharing strategies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerEmbeddingStrategy {
    Unshared,
    DecoderInputOutputTied,
    SharedSourceTargetAndOutput,
}

/// Descriptor for the canonical owned article-Transformer model path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerDescriptor {
    pub model: ModelDescriptor,
    pub source_paper_title: String,
    pub source_paper_ref: String,
    pub architecture_variant: TassadarArticleTransformerArchitectureVariant,
    pub paper_faithful: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substitution_justification: Option<String>,
    pub embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
    pub config: EncoderDecoderTransformerConfig,
    pub artifact_binding: TassadarArticleTransformerArtifactBinding,
    pub weights: WeightBundleMetadata,
}

impl TassadarArticleTransformerDescriptor {
    /// Returns a stable descriptor digest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        #[derive(Serialize)]
        struct StableArtifactBinding<'a> {
            artifact_id: &'a str,
            artifact_format: WeightFormat,
            primary_artifact_sha256: &'a str,
            weight_bundle_digest: &'a str,
            tensor_count: usize,
        }

        #[derive(Serialize)]
        struct StableWeightArtifactMetadata<'a> {
            byte_length: u64,
            sha256: &'a str,
            storage: &'a Option<crate::WeightArtifactStorageMetadata>,
        }

        #[derive(Serialize)]
        struct StableWeightBundleMetadata<'a> {
            format: WeightFormat,
            source: WeightSource,
            quantization: psionic_core::QuantizationMode,
            quantization_modes: &'a [psionic_core::QuantizationMode],
            digest: &'a str,
            tensors: &'a [WeightTensorMetadata],
            artifacts: Vec<StableWeightArtifactMetadata<'a>>,
        }

        #[derive(Serialize)]
        struct StableDescriptor<'a> {
            model: &'a ModelDescriptor,
            source_paper_title: &'a str,
            source_paper_ref: &'a str,
            architecture_variant: TassadarArticleTransformerArchitectureVariant,
            paper_faithful: bool,
            substitution_justification: &'a Option<String>,
            embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
            config: &'a EncoderDecoderTransformerConfig,
            artifact_binding: StableArtifactBinding<'a>,
            weights: StableWeightBundleMetadata<'a>,
        }

        let stable_artifacts = self
            .weights
            .artifacts
            .iter()
            .map(|artifact| StableWeightArtifactMetadata {
                byte_length: artifact.byte_length,
                sha256: artifact.sha256.as_str(),
                storage: &artifact.storage,
            })
            .collect::<Vec<_>>();
        let stable_descriptor = StableDescriptor {
            model: &self.model,
            source_paper_title: self.source_paper_title.as_str(),
            source_paper_ref: self.source_paper_ref.as_str(),
            architecture_variant: self.architecture_variant,
            paper_faithful: self.paper_faithful,
            substitution_justification: &self.substitution_justification,
            embedding_strategy: self.embedding_strategy,
            config: &self.config,
            artifact_binding: StableArtifactBinding {
                artifact_id: self.artifact_binding.artifact_id.as_str(),
                artifact_format: self.artifact_binding.artifact_format,
                primary_artifact_sha256: self.artifact_binding.primary_artifact_sha256.as_str(),
                weight_bundle_digest: self.artifact_binding.weight_bundle_digest.as_str(),
                tensor_count: self.artifact_binding.tensor_count,
            },
            weights: StableWeightBundleMetadata {
                format: self.weights.format,
                source: self.weights.source,
                quantization: self.weights.quantization,
                quantization_modes: self.weights.quantization_modes.as_slice(),
                digest: self.weights.digest.as_str(),
                tensors: self.weights.tensors.as_slice(),
                artifacts: stable_artifacts,
            },
        };
        stable_digest(
            b"psionic_tassadar_article_transformer_descriptor|",
            &stable_descriptor,
        )
    }
}

/// Stable model-side artifact identity for one canonical article-Transformer descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerArtifactBinding {
    pub artifact_id: String,
    pub artifact_ref: String,
    pub artifact_format: WeightFormat,
    pub primary_artifact_sha256: String,
    pub weight_bundle_digest: String,
    pub tensor_count: usize,
    pub artifact_identity_digest: String,
}

impl TassadarArticleTransformerArtifactBinding {
    fn new(
        model_id: &str,
        artifact_ref: impl Into<String>,
        artifact_format: WeightFormat,
        primary_artifact_sha256: impl Into<String>,
        weight_bundle_digest: impl Into<String>,
        tensor_count: usize,
    ) -> Self {
        let artifact_ref = artifact_ref.into();
        let primary_artifact_sha256 = primary_artifact_sha256.into();
        let weight_bundle_digest = weight_bundle_digest.into();
        let mut binding = Self {
            artifact_id: format!(
                "tassadar://article_transformer/weights/{model_id}/{weight_bundle_digest}"
            ),
            artifact_ref,
            artifact_format,
            primary_artifact_sha256,
            weight_bundle_digest,
            tensor_count,
            artifact_identity_digest: String::new(),
        };
        binding.artifact_identity_digest = stable_digest(
            b"psionic_tassadar_article_transformer_artifact_binding|",
            &(
                binding.artifact_id.as_str(),
                binding.artifact_ref.as_str(),
                binding.artifact_format.identity_label(),
                binding.primary_artifact_sha256.as_str(),
                binding.weight_bundle_digest.as_str(),
                binding.tensor_count,
            ),
        );
        binding
    }
}

/// One trainable parameter vector exposed by the bounded article route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerParameterVector {
    pub parameter_id: String,
    pub shape: Vec<usize>,
    pub values: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
struct TassadarArticleTransformerArtifactTensorRow {
    name: String,
    shape: Vec<usize>,
    values: Vec<f32>,
}

/// One channel binding between the runtime-owned trace schema and tokenizer forms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTraceChannelBindingRow {
    pub channel_id: String,
    pub channel_kind: TassadarArticleTraceChannelKind,
    pub stable_field_id: String,
    pub token_forms: Vec<String>,
    pub bound: bool,
    pub detail: String,
}

/// Canonical trace-domain binding for the owned article Transformer route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTraceDomainBinding {
    pub binding_id: String,
    pub trace_schema: TassadarArticleTraceMachineStepSchema,
    pub tokenizer_digest: String,
    pub tokenizer_vocab_size: usize,
    pub model_source_vocab_size: usize,
    pub model_target_vocab_size: usize,
    pub model_max_source_positions: usize,
    pub model_max_target_positions: usize,
    pub channel_binding_rows: Vec<TassadarArticleTransformerTraceChannelBindingRow>,
    pub source_vocab_compatible: bool,
    pub target_vocab_compatible: bool,
    pub prompt_trace_boundary_supported: bool,
    pub halt_boundary_supported: bool,
    pub all_required_channels_bound: bool,
    pub summary: String,
    pub binding_digest: String,
}

/// One source/target token batch prepared for the canonical article route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTraceDomainBatch {
    pub source_shape: Vec<usize>,
    pub source_token_ids: Vec<usize>,
    pub target_shape: Vec<usize>,
    pub target_token_ids: Vec<usize>,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub halt_marker: Option<String>,
    pub sequence_digest: String,
}

/// One full encode/decode roundtrip over the canonical article trace domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTraceDomainRoundtrip {
    pub binding: TassadarArticleTransformerTraceDomainBinding,
    pub batch: TassadarArticleTransformerTraceDomainBatch,
    pub decoded_trace: TassadarDecodedArticleTraceDomain,
    pub prompt_boundary_preserved: bool,
    pub halt_marker_preserved: bool,
    pub roundtrip_exact: bool,
}

/// Canonical article-route model wrapper owned by `psionic-models`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformer {
    descriptor: TassadarArticleTransformerDescriptor,
    source_embedding_table: Vec<f32>,
    target_embedding_table: Vec<f32>,
    logits_projection_weight: Vec<f32>,
    logits_projection_bias: Vec<f32>,
    model: EncoderDecoderTransformer,
}

impl TassadarArticleTransformer {
    pub const MODEL_ID: &str = "tassadar-article-transformer-paper-faithful-v0";
    pub const TRACE_BOUND_MODEL_ID: &str = "tassadar-article-transformer-trace-bound-v0";
    pub const TRAINED_TRACE_BOUND_MODEL_ID: &str =
        "tassadar-article-transformer-trace-bound-trained-v0";
    pub const MODEL_FAMILY: &str = "tassadar_article_transformer";
    pub const SOURCE_PAPER_TITLE: &str = "Attention Is All You Need";
    pub const SOURCE_PAPER_REF: &str =
        "~/code/alpha/tassadar/tassadar-research/papers/01-attention-is-all-you-need.pdf";
    pub const MODEL_MODULE_REF: &str = "crates/psionic-models/src/tassadar_article_transformer.rs";
    pub const TOKENIZER_MODULE_REF: &str = "crates/psionic-models/src/tassadar_sequence.rs";
    pub const TRANSFORMER_MODULE_REF: &str = "crates/psionic-transformer/src/encoder_decoder.rs";
    pub const TRACE_SCHEMA_MODULE_REF: &str =
        "crates/psionic-runtime/src/tassadar_article_trace_schema.rs";
    pub const RUNTIME_MODULE_REF: &str =
        "crates/psionic-runtime/src/tassadar_article_transformer_forward_pass.rs";
    pub const SOURCE_EMBEDDING_PARAMETER_ID: &str = "source_embedding_table";
    pub const TARGET_EMBEDDING_PARAMETER_ID: &str = "target_embedding_table";
    pub const SHARED_EMBEDDING_PARAMETER_ID: &str = "shared_embedding_table";
    pub const DECODER_OUTPUT_SHARED_PARAMETER_ID: &str = "decoder_output_shared_table";
    pub const LOGITS_PROJECTION_WEIGHT_PARAMETER_ID: &str = "logits_projection_weight";
    pub const LOGITS_PROJECTION_BIAS_PARAMETER_ID: &str = "logits_projection_bias";
    pub const CANONICAL_DESCRIPTOR_REF: &str =
        "fixtures/tassadar/models/tassadar_article_transformer_paper_faithful_v0_descriptor.json";
    pub const CANONICAL_ARTIFACT_REF: &str =
        "fixtures/tassadar/models/tassadar_article_transformer_paper_faithful_v0.safetensors";
    pub const TRACE_BOUND_DESCRIPTOR_REF: &str =
        "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_v0_descriptor.json";
    pub const TRACE_BOUND_ARTIFACT_REF: &str =
        "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_v0.safetensors";
    pub const TRAINED_TRACE_BOUND_DESCRIPTOR_REF: &str =
        "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_descriptor.json";
    pub const TRAINED_TRACE_BOUND_ARTIFACT_REF: &str =
        "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0.safetensors";
    pub const TRAINED_TRACE_BOUND_LINEAGE_CONTRACT_REF: &str =
        "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_lineage_contract.json";

    /// Returns a small canonical reference config used for closure tests.
    #[must_use]
    pub fn tiny_reference_config() -> EncoderDecoderTransformerConfig {
        EncoderDecoderTransformerConfig {
            source_vocab_size: 8,
            target_vocab_size: 8,
            hidden_size: 4,
            feed_forward_size: 8,
            head_count: 2,
            encoder_layer_count: 2,
            decoder_layer_count: 2,
            max_source_positions: 16,
            max_target_positions: 16,
            dropout_probability_bps: 0,
        }
    }

    /// Builds the canonical paper-faithful article route with one explicit
    /// embedding-sharing strategy.
    pub fn paper_faithful_reference(
        config: EncoderDecoderTransformerConfig,
        embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
    ) -> Result<Self, TassadarArticleTransformerError> {
        let source_embedding_table = seeded_values(
            "article_transformer.source_embedding_table",
            config.source_vocab_size * config.hidden_size,
            0.125,
        );
        let shared_decoder_table = seeded_values(
            "article_transformer.decoder_embedding_table",
            config.target_vocab_size * config.hidden_size,
            0.125,
        );

        let (target_embedding_table, logits_projection_weight) = match embedding_strategy {
            TassadarArticleTransformerEmbeddingStrategy::Unshared => (
                seeded_values(
                    "article_transformer.target_embedding_table",
                    config.target_vocab_size * config.hidden_size,
                    0.125,
                ),
                seeded_values(
                    "article_transformer.logits_projection_weight",
                    config.target_vocab_size * config.hidden_size,
                    0.08,
                ),
            ),
            TassadarArticleTransformerEmbeddingStrategy::DecoderInputOutputTied => {
                (shared_decoder_table.clone(), shared_decoder_table)
            }
            TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput => {
                if config.source_vocab_size != config.target_vocab_size {
                    return Err(TassadarArticleTransformerError::InvalidConfiguration {
                        component: "tassadar_article_transformer.embedding_strategy",
                        message: format!(
                            "shared source/target/output weights require matching vocab sizes, found source_vocab_size={} target_vocab_size={}",
                            config.source_vocab_size, config.target_vocab_size
                        ),
                    });
                }
                (
                    source_embedding_table.clone(),
                    source_embedding_table.clone(),
                )
            }
        };
        let logits_projection_bias = vec![0.0; config.target_vocab_size];
        let model = Self::from_weight_parts(
            config,
            embedding_strategy,
            source_embedding_table,
            target_embedding_table,
            logits_projection_weight,
            logits_projection_bias,
        )?;
        model.with_memory_artifact_binding()
    }

    /// Returns a small canonical paper-faithful route used by closure reports.
    pub fn canonical_reference() -> Result<Self, TassadarArticleTransformerError> {
        Self::load_from_descriptor_path(Self::canonical_reference_descriptor_path())
    }

    /// Returns a bounded trace-domain config whose vocab exactly matches the
    /// canonical Tassadar trace tokenizer.
    #[must_use]
    pub fn trace_domain_reference_config(
        tokenizer: &TassadarTraceTokenizer,
    ) -> EncoderDecoderTransformerConfig {
        let vocab_size = tokenizer.vocabulary().len();
        EncoderDecoderTransformerConfig {
            source_vocab_size: vocab_size,
            target_vocab_size: vocab_size,
            hidden_size: 8,
            feed_forward_size: 16,
            head_count: 2,
            encoder_layer_count: 2,
            decoder_layer_count: 2,
            max_source_positions: 16_384,
            max_target_positions: 16_384,
            dropout_probability_bps: 0,
        }
    }

    /// Returns a trace-bound reference model for the canonical article route.
    pub fn article_trace_domain_reference() -> Result<Self, TassadarArticleTransformerError> {
        Self::load_from_descriptor_path(Self::trace_bound_descriptor_path())
    }

    /// Returns the first produced trace-bound reference model for the canonical
    /// article route.
    pub fn trained_trace_domain_reference() -> Result<Self, TassadarArticleTransformerError> {
        Self::load_from_descriptor_path(Self::trained_trace_bound_descriptor_path())
    }

    fn build_canonical_reference_source() -> Result<Self, TassadarArticleTransformerError> {
        Self::paper_faithful_reference(
            Self::tiny_reference_config(),
            TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput,
        )
    }

    fn build_trace_domain_reference_source() -> Result<Self, TassadarArticleTransformerError> {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = Self::trace_domain_reference_config(&tokenizer);
        let mut model = Self::paper_faithful_reference(
            config,
            TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput,
        )?;
        model.descriptor.model =
            ModelDescriptor::new(Self::TRACE_BOUND_MODEL_ID, Self::MODEL_FAMILY, "v0");
        model = model.with_memory_artifact_binding()?;
        Ok(model)
    }

    #[must_use]
    pub fn canonical_reference_descriptor_path() -> PathBuf {
        repo_root().join(Self::CANONICAL_DESCRIPTOR_REF)
    }

    #[must_use]
    pub fn canonical_reference_artifact_path() -> PathBuf {
        repo_root().join(Self::CANONICAL_ARTIFACT_REF)
    }

    #[must_use]
    pub fn trace_bound_descriptor_path() -> PathBuf {
        repo_root().join(Self::TRACE_BOUND_DESCRIPTOR_REF)
    }

    #[must_use]
    pub fn trace_bound_artifact_path() -> PathBuf {
        repo_root().join(Self::TRACE_BOUND_ARTIFACT_REF)
    }

    #[must_use]
    pub fn trained_trace_bound_descriptor_path() -> PathBuf {
        repo_root().join(Self::TRAINED_TRACE_BOUND_DESCRIPTOR_REF)
    }

    #[must_use]
    pub fn trained_trace_bound_artifact_path() -> PathBuf {
        repo_root().join(Self::TRAINED_TRACE_BOUND_ARTIFACT_REF)
    }

    #[must_use]
    pub fn trained_trace_bound_lineage_contract_path() -> PathBuf {
        repo_root().join(Self::TRAINED_TRACE_BOUND_LINEAGE_CONTRACT_REF)
    }

    /// Returns the public descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &TassadarArticleTransformerDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub fn artifact_binding(&self) -> &TassadarArticleTransformerArtifactBinding {
        &self.descriptor.artifact_binding
    }

    #[must_use]
    pub fn weight_metadata(&self) -> &WeightBundleMetadata {
        &self.descriptor.weights
    }

    pub fn load_from_descriptor_path(
        descriptor_path: impl AsRef<Path>,
    ) -> Result<Self, TassadarArticleTransformerError> {
        let descriptor_path = descriptor_path.as_ref();
        let descriptor_bytes =
            fs::read(descriptor_path).map_err(|error| TassadarArticleTransformerError::Read {
                path: descriptor_path.display().to_string(),
                error,
            })?;
        let descriptor: TassadarArticleTransformerDescriptor =
            serde_json::from_slice(&descriptor_bytes).map_err(|error| {
                TassadarArticleTransformerError::Decode {
                    path: descriptor_path.display().to_string(),
                    error,
                }
            })?;
        let artifact_path = resolve_artifact_path(
            descriptor_path,
            descriptor.artifact_binding.artifact_ref.as_str(),
        )?;
        let artifact_bytes =
            fs::read(&artifact_path).map_err(|error| TassadarArticleTransformerError::Read {
                path: artifact_path.display().to_string(),
                error,
            })?;
        Self::from_descriptor_and_artifact_bytes(descriptor, &artifact_bytes)
    }

    pub fn write_artifact_bundle(
        &self,
        descriptor_path: impl AsRef<Path>,
        artifact_path: impl AsRef<Path>,
    ) -> Result<TassadarArticleTransformerDescriptor, TassadarArticleTransformerError> {
        let descriptor_path = descriptor_path.as_ref();
        let artifact_path = artifact_path.as_ref();
        if let Some(parent) = descriptor_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarArticleTransformerError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        if let Some(parent) = artifact_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarArticleTransformerError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let artifact_ref = relative_artifact_ref(descriptor_path, artifact_path);
        let descriptor = self.descriptor_with_artifact_ref(artifact_ref)?;
        let artifact_bytes = serialize_artifact_rows_to_bytes(&self.artifact_tensor_rows()?)?;
        fs::write(artifact_path, &artifact_bytes).map_err(|error| {
            TassadarArticleTransformerError::Write {
                path: artifact_path.display().to_string(),
                error,
            }
        })?;
        let json = serde_json::to_string_pretty(&descriptor)?;
        fs::write(descriptor_path, format!("{json}\n")).map_err(|error| {
            TassadarArticleTransformerError::Write {
                path: descriptor_path.display().to_string(),
                error,
            }
        })?;
        Ok(descriptor)
    }

    pub fn write_committed_reference_artifacts(
    ) -> Result<Vec<TassadarArticleTransformerDescriptor>, TassadarArticleTransformerError> {
        let canonical = Self::build_canonical_reference_source()?.write_artifact_bundle(
            Self::canonical_reference_descriptor_path(),
            Self::canonical_reference_artifact_path(),
        )?;
        let trace_bound = Self::build_trace_domain_reference_source()?.write_artifact_bundle(
            Self::trace_bound_descriptor_path(),
            Self::trace_bound_artifact_path(),
        )?;
        Ok(vec![canonical, trace_bound])
    }

    fn with_memory_artifact_binding(mut self) -> Result<Self, TassadarArticleTransformerError> {
        self.descriptor = self.descriptor_with_artifact_ref(format!(
            "memory://tassadar/article_transformer/{}.safetensors",
            self.descriptor.model.model_id
        ))?;
        Ok(self)
    }

    fn descriptor_with_artifact_ref(
        &self,
        artifact_ref: impl Into<String>,
    ) -> Result<TassadarArticleTransformerDescriptor, TassadarArticleTransformerError> {
        let artifact_ref = artifact_ref.into();
        let rows = self.artifact_tensor_rows()?;
        let artifact_bytes = serialize_artifact_rows_to_bytes(rows.as_slice())?;
        let artifact_sha256 = hex::encode(Sha256::digest(&artifact_bytes));
        let weights = build_article_transformer_weight_bundle_metadata(
            rows.as_slice(),
            artifact_ref.clone(),
            artifact_sha256.clone(),
            artifact_bytes.len() as u64,
        );
        let artifact_binding = TassadarArticleTransformerArtifactBinding::new(
            self.descriptor.model.model_id.as_str(),
            artifact_ref,
            weights.format,
            artifact_sha256,
            weights.digest.clone(),
            weights.tensors.len(),
        );
        let mut descriptor = self.descriptor.clone();
        descriptor.artifact_binding = artifact_binding;
        descriptor.weights = weights;
        Ok(descriptor)
    }

    fn from_descriptor_and_artifact_bytes(
        descriptor: TassadarArticleTransformerDescriptor,
        artifact_bytes: &[u8],
    ) -> Result<Self, TassadarArticleTransformerError> {
        let tensor_map = load_article_transformer_tensor_map(artifact_bytes)?;
        let actual_weights =
            build_article_transformer_weight_bundle_metadata_from_map(&tensor_map, &descriptor);
        if actual_weights.digest != descriptor.weights.digest
            || actual_weights.tensors != descriptor.weights.tensors
            || actual_weights.artifacts != descriptor.weights.artifacts
            || actual_weights.format != descriptor.weights.format
            || actual_weights.source != descriptor.weights.source
        {
            return Err(TassadarArticleTransformerError::ArtifactMetadataMismatch {
                expected_digest: descriptor.weights.digest.clone(),
                actual_digest: actual_weights.digest,
            });
        }
        let config = descriptor.config.clone();
        let source_embedding_table = require_tensor_values(
            &tensor_map,
            "source_embeddings.token_embedding.weight",
            &[config.source_vocab_size, config.hidden_size],
        )?;
        let target_embedding_table = require_tensor_values(
            &tensor_map,
            "target_embeddings.token_embedding.weight",
            &[config.target_vocab_size, config.hidden_size],
        )?;
        let source_embeddings = TransformerEmbeddings::from_f32_table(
            "tassadar.article_transformer.source_embeddings",
            config.source_vocab_size,
            config.hidden_size,
            config.max_source_positions,
            source_embedding_table.clone(),
            config.dropout_probability(),
        )?;
        let target_embeddings = TransformerEmbeddings::from_f32_table(
            "tassadar.article_transformer.target_embeddings",
            config.target_vocab_size,
            config.hidden_size,
            config.max_target_positions,
            target_embedding_table.clone(),
            config.dropout_probability(),
        )?;
        let encoder_layers = (0..config.encoder_layer_count)
            .map(|layer_index| build_encoder_layer_from_tensors(&config, layer_index, &tensor_map))
            .collect::<Result<Vec<_>, _>>()?;
        let decoder_layers = (0..config.decoder_layer_count)
            .map(|layer_index| build_decoder_layer_from_tensors(&config, layer_index, &tensor_map))
            .collect::<Result<Vec<_>, _>>()?;
        let logits_projection_weight = require_tensor_values(
            &tensor_map,
            "logits_projection.weight",
            &[config.target_vocab_size, config.hidden_size],
        )?;
        let logits_projection_bias = require_tensor_values(
            &tensor_map,
            "logits_projection.bias",
            &[config.target_vocab_size],
        )?;
        let logits_projection = Linear::from_f32_parts(
            "tassadar.article_transformer.logits_projection",
            config.hidden_size,
            config.target_vocab_size,
            logits_projection_weight.clone(),
            Some(logits_projection_bias.clone()),
        )?;
        let model = EncoderDecoderTransformer::from_components(
            config,
            source_embeddings,
            target_embeddings,
            encoder_layers,
            decoder_layers,
            logits_projection,
        )?;
        Ok(Self {
            descriptor,
            source_embedding_table,
            target_embedding_table,
            logits_projection_weight,
            logits_projection_bias,
            model,
        })
    }

    /// Returns the canonical trace-domain binding for the current model.
    #[must_use]
    pub fn trace_domain_binding(&self) -> TassadarArticleTransformerTraceDomainBinding {
        let tokenizer = TassadarTraceTokenizer::new();
        let trace_schema = tassadar_article_trace_machine_step_schema();
        let channel_binding_rows = trace_domain_channel_binding_rows();
        let source_vocab_compatible =
            self.descriptor.config.source_vocab_size >= tokenizer.vocabulary().len();
        let target_vocab_compatible =
            self.descriptor.config.target_vocab_size >= tokenizer.vocabulary().len();
        let prompt_trace_boundary_supported = tokenizer
            .vocabulary()
            .tokens()
            .iter()
            .any(|token| token == "<trace>");
        let halt_boundary_supported = tokenizer
            .vocabulary()
            .tokens()
            .iter()
            .any(|token| token == "<halt>")
            && tokenizer
                .vocabulary()
                .tokens()
                .iter()
                .any(|token| token == "<halt_returned>")
            && tokenizer
                .vocabulary()
                .tokens()
                .iter()
                .any(|token| token == "<halt_fell_off_end>");
        let all_required_channels_bound = channel_binding_rows.iter().all(|row| row.bound);
        let mut binding = TassadarArticleTransformerTraceDomainBinding {
            binding_id: String::from("tassadar.article_transformer.trace_domain_binding.v1"),
            trace_schema,
            tokenizer_digest: tokenizer.stable_digest(),
            tokenizer_vocab_size: tokenizer.vocabulary().len(),
            model_source_vocab_size: self.descriptor.config.source_vocab_size,
            model_target_vocab_size: self.descriptor.config.target_vocab_size,
            model_max_source_positions: self.descriptor.config.max_source_positions,
            model_max_target_positions: self.descriptor.config.max_target_positions,
            channel_binding_rows,
            source_vocab_compatible,
            target_vocab_compatible,
            prompt_trace_boundary_supported,
            halt_boundary_supported,
            all_required_channels_bound,
            summary: String::new(),
            binding_digest: String::new(),
        };
        binding.summary = format!(
            "Article trace-domain binding now records tokenizer_vocab_size={}, model_source_vocab_size={}, model_target_vocab_size={}, channel_binding_rows={}, source_vocab_compatible={}, target_vocab_compatible={}, prompt_trace_boundary_supported={}, halt_boundary_supported={}, and all_required_channels_bound={}.",
            binding.tokenizer_vocab_size,
            binding.model_source_vocab_size,
            binding.model_target_vocab_size,
            binding.channel_binding_rows.len(),
            binding.source_vocab_compatible,
            binding.target_vocab_compatible,
            binding.prompt_trace_boundary_supported,
            binding.halt_boundary_supported,
            binding.all_required_channels_bound,
        );
        binding.binding_digest = stable_digest(
            b"psionic_tassadar_article_transformer_trace_domain_binding|",
            &binding,
        );
        binding
    }

    /// Encodes one typed program and execution pair into the canonical
    /// source/target token split used by the owned article route.
    pub fn encode_article_trace_domain(
        &self,
        program: &TassadarProgram,
        execution: &TassadarExecution,
    ) -> Result<TassadarArticleTransformerTraceDomainBatch, TassadarArticleTransformerError> {
        let batch = self.tokenize_article_trace_domain_unbounded(program, execution)?;
        if batch.source_token_ids.len() > self.descriptor.config.max_source_positions {
            return Err(TassadarArticleTransformerError::InvalidConfiguration {
                component: "tassadar_article_transformer.encode_article_trace_domain",
                message: format!(
                    "prompt token count {} exceeds max_source_positions {}",
                    batch.source_token_ids.len(),
                    self.descriptor.config.max_source_positions
                ),
            });
        }
        if batch.target_token_ids.len() > self.descriptor.config.max_target_positions {
            return Err(TassadarArticleTransformerError::InvalidConfiguration {
                component: "tassadar_article_transformer.encode_article_trace_domain",
                message: format!(
                    "target token count {} exceeds max_target_positions {}",
                    batch.target_token_ids.len(),
                    self.descriptor.config.max_target_positions
                ),
            });
        }
        Ok(batch)
    }

    /// Tokenizes one article trace-domain example without enforcing the current
    /// model-context window.
    pub fn tokenize_article_trace_domain_unbounded(
        &self,
        program: &TassadarProgram,
        execution: &TassadarExecution,
    ) -> Result<TassadarArticleTransformerTraceDomainBatch, TassadarArticleTransformerError> {
        let binding = self.trace_domain_binding();
        if !binding.source_vocab_compatible || !binding.target_vocab_compatible {
            return Err(TassadarArticleTransformerError::InvalidConfiguration {
                component: "tassadar_article_transformer.tokenize_article_trace_domain_unbounded",
                message: format!(
                    "model vocab sizes source={} target={} do not cover tokenizer vocab size {}",
                    binding.model_source_vocab_size,
                    binding.model_target_vocab_size,
                    binding.tokenizer_vocab_size
                ),
            });
        }
        if !binding.prompt_trace_boundary_supported
            || !binding.halt_boundary_supported
            || !binding.all_required_channels_bound
        {
            return Err(TassadarArticleTransformerError::InvalidConfiguration {
                component: "tassadar_article_transformer.tokenize_article_trace_domain_unbounded",
                message: String::from(
                    "trace-domain binding is incomplete for the canonical prompt/trace/halt or channel surface",
                ),
            });
        }
        let tokenizer = TassadarTraceTokenizer::new();
        let tokenized = tokenizer.tokenize_program_and_execution(program, execution);
        let source_tokens = &tokenized.sequence.as_slice()[..tokenized.prompt_token_count];
        let target_tokens = &tokenized.sequence.as_slice()[tokenized.prompt_token_count..];
        Ok(TassadarArticleTransformerTraceDomainBatch {
            source_shape: vec![1, source_tokens.len()],
            source_token_ids: source_tokens
                .iter()
                .map(|token| token.as_u32() as usize)
                .collect(),
            target_shape: vec![1, target_tokens.len()],
            target_token_ids: target_tokens
                .iter()
                .map(|token| token.as_u32() as usize)
                .collect(),
            prompt_token_count: tokenized.prompt_token_count,
            target_token_count: tokenized.target_token_count,
            halt_marker: tokenizer.extract_halt_marker(tokenized.sequence.as_slice()),
            sequence_digest: tokenized.sequence_digest,
        })
    }

    /// Runs one full encode/decode roundtrip over the canonical article trace domain.
    pub fn roundtrip_article_trace_domain(
        &self,
        program: &TassadarProgram,
        execution: &TassadarExecution,
    ) -> Result<TassadarArticleTransformerTraceDomainRoundtrip, TassadarArticleTransformerError>
    {
        let batch = self.encode_article_trace_domain(program, execution)?;
        self.roundtrip_from_batch(program, execution, batch)
    }

    /// Runs one full encode/decode roundtrip over the canonical article trace
    /// domain without enforcing the current model-context window.
    pub fn roundtrip_article_trace_domain_unbounded(
        &self,
        program: &TassadarProgram,
        execution: &TassadarExecution,
    ) -> Result<TassadarArticleTransformerTraceDomainRoundtrip, TassadarArticleTransformerError>
    {
        let batch = self.tokenize_article_trace_domain_unbounded(program, execution)?;
        self.roundtrip_from_batch(program, execution, batch)
    }

    /// Returns the embedding-sharing strategy.
    #[must_use]
    pub const fn embedding_strategy(&self) -> TassadarArticleTransformerEmbeddingStrategy {
        self.descriptor.embedding_strategy
    }

    /// Returns the source embedding table.
    #[must_use]
    pub fn source_embedding_table(&self) -> &[f32] {
        &self.source_embedding_table
    }

    /// Returns the target embedding table.
    #[must_use]
    pub fn target_embedding_table(&self) -> &[f32] {
        &self.target_embedding_table
    }

    /// Returns the logits projection weight.
    #[must_use]
    pub fn logits_projection_weight(&self) -> &[f32] {
        &self.logits_projection_weight
    }

    /// Returns the logits projection bias.
    #[must_use]
    pub fn logits_projection_bias(&self) -> &[f32] {
        &self.logits_projection_bias
    }

    /// Returns the bounded trainable parameter vectors for the current sharing strategy.
    #[must_use]
    pub fn trainable_parameter_vectors(&self) -> Vec<TassadarArticleTransformerParameterVector> {
        let config = &self.descriptor.config;
        let mut parameters = match self.embedding_strategy() {
            TassadarArticleTransformerEmbeddingStrategy::Unshared => vec![
                TassadarArticleTransformerParameterVector {
                    parameter_id: String::from(Self::SOURCE_EMBEDDING_PARAMETER_ID),
                    shape: vec![config.source_vocab_size, config.hidden_size],
                    values: self.source_embedding_table.clone(),
                },
                TassadarArticleTransformerParameterVector {
                    parameter_id: String::from(Self::TARGET_EMBEDDING_PARAMETER_ID),
                    shape: vec![config.target_vocab_size, config.hidden_size],
                    values: self.target_embedding_table.clone(),
                },
                TassadarArticleTransformerParameterVector {
                    parameter_id: String::from(Self::LOGITS_PROJECTION_WEIGHT_PARAMETER_ID),
                    shape: vec![config.target_vocab_size, config.hidden_size],
                    values: self.logits_projection_weight.clone(),
                },
            ],
            TassadarArticleTransformerEmbeddingStrategy::DecoderInputOutputTied => vec![
                TassadarArticleTransformerParameterVector {
                    parameter_id: String::from(Self::SOURCE_EMBEDDING_PARAMETER_ID),
                    shape: vec![config.source_vocab_size, config.hidden_size],
                    values: self.source_embedding_table.clone(),
                },
                TassadarArticleTransformerParameterVector {
                    parameter_id: String::from(Self::DECODER_OUTPUT_SHARED_PARAMETER_ID),
                    shape: vec![config.target_vocab_size, config.hidden_size],
                    values: self.target_embedding_table.clone(),
                },
            ],
            TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput => {
                vec![TassadarArticleTransformerParameterVector {
                    parameter_id: String::from(Self::SHARED_EMBEDDING_PARAMETER_ID),
                    shape: vec![config.source_vocab_size, config.hidden_size],
                    values: self.source_embedding_table.clone(),
                }]
            }
        };
        parameters.push(TassadarArticleTransformerParameterVector {
            parameter_id: String::from(Self::LOGITS_PROJECTION_BIAS_PARAMETER_ID),
            shape: vec![config.target_vocab_size],
            values: self.logits_projection_bias.clone(),
        });
        parameters
    }

    /// Returns the stable digest over the bounded trainable state.
    #[must_use]
    pub fn trainable_parameter_digest(&self) -> String {
        stable_digest(
            b"psionic_tassadar_article_transformer_trainable_parameters|",
            &self.trainable_parameter_vectors(),
        )
    }

    /// Returns one bounded trainable parameter vector by identifier.
    #[must_use]
    pub fn trainable_parameter_vector(
        &self,
        parameter_id: &str,
    ) -> Option<TassadarArticleTransformerParameterVector> {
        self.trainable_parameter_vectors()
            .into_iter()
            .find(|parameter| parameter.parameter_id == parameter_id)
    }

    /// Rebuilds the model with bounded trainable-parameter overrides.
    pub fn with_parameter_overrides(
        &self,
        overrides: &BTreeMap<String, Vec<f32>>,
    ) -> Result<Self, TassadarArticleTransformerError> {
        let mut source_embedding_table = self.source_embedding_table.clone();
        let mut target_embedding_table = self.target_embedding_table.clone();
        let mut logits_projection_weight = self.logits_projection_weight.clone();
        let mut logits_projection_bias = self.logits_projection_bias.clone();

        let apply_override = |parameter_id: &str,
                              expected_len: usize,
                              target: &mut Vec<f32>,
                              overrides: &BTreeMap<String, Vec<f32>>|
         -> Result<(), TassadarArticleTransformerError> {
            if let Some(values) = overrides.get(parameter_id) {
                if values.len() != expected_len {
                    return Err(TassadarArticleTransformerError::InvalidParameterShape {
                        parameter_id: String::from(parameter_id),
                        expected_len,
                        actual_len: values.len(),
                    });
                }
                *target = values.clone();
            }
            Ok(())
        };

        match self.embedding_strategy() {
            TassadarArticleTransformerEmbeddingStrategy::Unshared => {
                apply_override(
                    Self::SOURCE_EMBEDDING_PARAMETER_ID,
                    source_embedding_table.len(),
                    &mut source_embedding_table,
                    overrides,
                )?;
                apply_override(
                    Self::TARGET_EMBEDDING_PARAMETER_ID,
                    target_embedding_table.len(),
                    &mut target_embedding_table,
                    overrides,
                )?;
                apply_override(
                    Self::LOGITS_PROJECTION_WEIGHT_PARAMETER_ID,
                    logits_projection_weight.len(),
                    &mut logits_projection_weight,
                    overrides,
                )?;
            }
            TassadarArticleTransformerEmbeddingStrategy::DecoderInputOutputTied => {
                apply_override(
                    Self::SOURCE_EMBEDDING_PARAMETER_ID,
                    source_embedding_table.len(),
                    &mut source_embedding_table,
                    overrides,
                )?;
                if let Some(values) = overrides.get(Self::DECODER_OUTPUT_SHARED_PARAMETER_ID) {
                    if values.len() != target_embedding_table.len() {
                        return Err(TassadarArticleTransformerError::InvalidParameterShape {
                            parameter_id: String::from(Self::DECODER_OUTPUT_SHARED_PARAMETER_ID),
                            expected_len: target_embedding_table.len(),
                            actual_len: values.len(),
                        });
                    }
                    target_embedding_table = values.clone();
                    logits_projection_weight = values.clone();
                }
            }
            TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput => {
                if let Some(values) = overrides.get(Self::SHARED_EMBEDDING_PARAMETER_ID) {
                    if values.len() != source_embedding_table.len() {
                        return Err(TassadarArticleTransformerError::InvalidParameterShape {
                            parameter_id: String::from(Self::SHARED_EMBEDDING_PARAMETER_ID),
                            expected_len: source_embedding_table.len(),
                            actual_len: values.len(),
                        });
                    }
                    source_embedding_table = values.clone();
                    target_embedding_table = values.clone();
                    logits_projection_weight = values.clone();
                }
            }
        }

        apply_override(
            Self::LOGITS_PROJECTION_BIAS_PARAMETER_ID,
            logits_projection_bias.len(),
            &mut logits_projection_bias,
            overrides,
        )?;

        for parameter_id in overrides.keys() {
            if self
                .trainable_parameter_vector(parameter_id.as_str())
                .is_none()
            {
                return Err(TassadarArticleTransformerError::UnknownParameter {
                    parameter_id: parameter_id.clone(),
                });
            }
        }

        let mut model = Self::from_weight_parts(
            self.descriptor.config.clone(),
            self.embedding_strategy(),
            source_embedding_table,
            target_embedding_table,
            logits_projection_weight,
            logits_projection_bias,
        )?;
        model.descriptor = self.descriptor.clone();
        model.with_memory_artifact_binding()
    }

    /// Rebinds the current weight state to one explicit model identity while
    /// preserving the current architecture and weight-sharing contract.
    pub fn with_model_identity(
        &self,
        model_id: impl Into<String>,
        revision: impl Into<String>,
    ) -> Result<Self, TassadarArticleTransformerError> {
        let mut model = self.clone();
        model.descriptor.model = ModelDescriptor::new(model_id, Self::MODEL_FAMILY, revision);
        model.with_memory_artifact_binding()
    }

    /// Runs a paper-faithful encoder-decoder forward pass.
    pub fn forward(
        &self,
        source_index_shape: Shape,
        source_token_ids: &[usize],
        target_index_shape: Shape,
        target_token_ids: &[usize],
        mode: TransformerExecutionMode,
    ) -> Result<
        psionic_transformer::EncoderDecoderTransformerForwardOutput,
        TassadarArticleTransformerError,
    > {
        Ok(self.model.forward(
            source_index_shape,
            source_token_ids,
            target_index_shape,
            target_token_ids,
            mode,
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn forward_with_masks(
        &self,
        source_index_shape: Shape,
        source_token_ids: &[usize],
        target_index_shape: Shape,
        target_token_ids: &[usize],
        encoder_self_attention_mask: Option<&AttentionMask>,
        decoder_self_attention_mask: Option<&AttentionMask>,
        cross_attention_mask: Option<&AttentionMask>,
        mode: TransformerExecutionMode,
    ) -> Result<
        psionic_transformer::EncoderDecoderTransformerForwardOutput,
        TassadarArticleTransformerError,
    > {
        Ok(self.model.forward_with_masks(
            source_index_shape,
            source_token_ids,
            target_index_shape,
            target_token_ids,
            encoder_self_attention_mask,
            decoder_self_attention_mask,
            cross_attention_mask,
            mode,
        )?)
    }

    /// Returns the digest-bound runtime model artifact identity for the current state.
    #[must_use]
    pub fn model_artifact_binding(&self) -> TassadarArticleTransformerModelArtifactBinding {
        TassadarArticleTransformerModelArtifactBinding::new(
            self.descriptor.model.model_id.clone(),
            self.descriptor.model.family.clone(),
            self.descriptor().stable_digest(),
            self.trainable_parameter_digest(),
            self.artifact_binding().artifact_id.clone(),
            self.weight_metadata().digest.clone(),
            self.artifact_binding().primary_artifact_sha256.clone(),
        )
    }

    /// Runs one canonical forward pass and binds the result into runtime receipts.
    pub fn forward_with_runtime_evidence(
        &self,
        run_id: impl Into<String>,
        request_id: impl Into<String>,
        product_id: impl Into<String>,
        environment_refs: Vec<String>,
        source_index_shape: Shape,
        source_token_ids: &[usize],
        target_index_shape: Shape,
        target_token_ids: &[usize],
        mode: TransformerExecutionMode,
        checkpoint_lineage: Option<TassadarArticleTransformerCheckpointLineage>,
    ) -> Result<TassadarArticleTransformerForwardPassEvidenceBundle, TassadarArticleTransformerError>
    {
        let request_id = request_id.into();
        let run_config = TassadarArticleTransformerForwardPassRunConfig::new(
            run_id,
            request_id.clone(),
            product_id,
            source_index_shape.dims().to_vec(),
            source_token_ids.to_vec(),
            target_index_shape.dims().to_vec(),
            target_token_ids.to_vec(),
            transformer_execution_mode_label(mode),
            environment_refs,
        );
        let forward_output = self.forward(
            source_index_shape.clone(),
            source_token_ids,
            target_index_shape.clone(),
            target_token_ids,
            mode,
        )?;
        let trace_artifact =
            self.trace_artifact_for_forward_output(request_id.as_str(), &forward_output)?;
        let decode_receipt = TassadarArticleTransformerDecodeReceipt::greedy(
            format!(
                "tassadar://article_transformer/decode/{request_id}/{}",
                trace_artifact.trace_digest
            ),
            trace_artifact.logits_digest.clone(),
            trace_artifact.predicted_token_ids.clone(),
        );
        let replay_output = self.forward(
            source_index_shape,
            source_token_ids,
            target_index_shape,
            target_token_ids,
            mode,
        )?;
        let replay_trace_artifact = self
            .trace_artifact_for_forward_output(&format!("{request_id}.replay"), &replay_output)?;
        let replay_receipt = TassadarArticleTransformerReplayReceipt::new(
            format!(
                "tassadar://article_transformer/replay/{request_id}/{}",
                trace_artifact.trace_digest
            ),
            trace_artifact.forward_output_digest.clone(),
            replay_trace_artifact.forward_output_digest,
            trace_artifact.trace_digest.clone(),
            replay_trace_artifact.trace_digest,
        );
        Ok(
            build_tassadar_article_transformer_forward_pass_evidence_bundle(
                format!("tassadar.article_transformer.forward_pass.bundle.{request_id}"),
                Self::MODEL_MODULE_REF,
                Self::TRANSFORMER_MODULE_REF,
                Self::RUNTIME_MODULE_REF,
                self.model_artifact_binding(),
                run_config,
                checkpoint_lineage,
                trace_artifact,
                decode_receipt,
                replay_receipt,
            ),
        )
    }

    fn trace_artifact_for_forward_output(
        &self,
        request_id: &str,
        forward_output: &psionic_transformer::EncoderDecoderTransformerForwardOutput,
    ) -> Result<TassadarArticleTransformerForwardPassTraceArtifact, TassadarArticleTransformerError>
    {
        let predicted_token_ids = predicted_token_ids_from_logits(
            forward_output.logits.dims(),
            &forward_output.logits.data,
        )?;
        let encoder_layer_traces = forward_output
            .encoder_layer_outputs
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                forward_pass_channel_trace(
                    format!("encoder_layer_{layer_index}.self_attention"),
                    "encoder_self_attention",
                    &layer.self_attention_trace,
                )
            })
            .collect::<Vec<_>>();
        let decoder_self_attention_traces = forward_output
            .decoder_layer_outputs
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                forward_pass_channel_trace(
                    format!("decoder_layer_{layer_index}.self_attention"),
                    "decoder_self_attention",
                    &layer.self_attention_trace,
                )
            })
            .collect::<Vec<_>>();
        let decoder_cross_attention_traces = forward_output
            .decoder_layer_outputs
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                forward_pass_channel_trace(
                    format!("decoder_layer_{layer_index}.cross_attention"),
                    "decoder_cross_attention",
                    &layer.cross_attention_trace,
                )
            })
            .collect::<Vec<_>>();
        Ok(TassadarArticleTransformerForwardPassTraceArtifact::new(
            format!(
                "tassadar://article_transformer/trace/{request_id}/{}",
                self.descriptor().stable_digest()
            ),
            stable_digest(
                b"psionic_tassadar_article_transformer_forward_output|",
                forward_output,
            ),
            stable_digest(
                b"psionic_tassadar_article_transformer_encoder_hidden_state|",
                &forward_output.encoder_hidden_state,
            ),
            stable_digest(
                b"psionic_tassadar_article_transformer_decoder_hidden_state|",
                &forward_output.decoder_hidden_state,
            ),
            stable_digest(
                b"psionic_tassadar_article_transformer_logits|",
                &forward_output.logits,
            ),
            predicted_token_ids,
            encoder_layer_traces,
            decoder_self_attention_traces,
            decoder_cross_attention_traces,
            Self::MODEL_MODULE_REF,
            Self::TRANSFORMER_MODULE_REF,
        ))
    }

    fn roundtrip_from_batch(
        &self,
        program: &TassadarProgram,
        execution: &TassadarExecution,
        batch: TassadarArticleTransformerTraceDomainBatch,
    ) -> Result<TassadarArticleTransformerTraceDomainRoundtrip, TassadarArticleTransformerError>
    {
        let binding = self.trace_domain_binding();
        let tokenizer = TassadarTraceTokenizer::new();
        let prompt_tokens = usize_slice_to_token_ids(&batch.source_token_ids);
        let target_tokens = usize_slice_to_token_ids(&batch.target_token_ids);
        let tokenized =
            tokenizer.compose_prompt_and_target_sequence(&prompt_tokens, &target_tokens);
        let decoded_trace = tokenizer.decode_article_trace_domain(&tokenized)?;
        let reconstructed_program = decoded_trace
            .materialize_program(program.program_id.clone(), program.profile_id.clone());
        let reconstructed_execution = decoded_trace.materialize_execution(
            execution.program_id.clone(),
            execution.profile_id.clone(),
            execution.runner_id.clone(),
            execution.trace_abi.clone(),
        );
        Ok(TassadarArticleTransformerTraceDomainRoundtrip {
            binding,
            batch,
            prompt_boundary_preserved: decoded_trace.prompt_token_count
                == reconstructed_prompt_token_count(program, execution),
            halt_marker_preserved: decoded_trace.halt_reason == execution.halt_reason,
            roundtrip_exact: decoded_trace.sequence_digest
                == tokenizer
                    .tokenize_program_and_execution(program, execution)
                    .sequence_digest
                && reconstructed_program == *program
                && reconstructed_execution == *execution,
            decoded_trace,
        })
    }

    fn artifact_tensor_rows(
        &self,
    ) -> Result<Vec<TassadarArticleTransformerArtifactTensorRow>, TassadarArticleTransformerError>
    {
        let config = self.model.config();
        let mut rows = vec![
            artifact_tensor_row(
                "source_embeddings.token_embedding.weight",
                vec![config.source_vocab_size, config.hidden_size],
                self.model
                    .source_embeddings()
                    .token_embedding_table_f32()?
                    .to_vec(),
            ),
            artifact_tensor_row(
                "target_embeddings.token_embedding.weight",
                vec![config.target_vocab_size, config.hidden_size],
                self.model
                    .target_embeddings()
                    .token_embedding_table_f32()?
                    .to_vec(),
            ),
        ];
        for (layer_index, layer) in self.model.encoder_layers().iter().enumerate() {
            append_attention_tensor_rows(
                &mut rows,
                &format!("encoder_layers.{layer_index}.self_attention"),
                layer.self_attention(),
            )?;
            append_layer_norm_tensor_rows(
                &mut rows,
                &format!("encoder_layers.{layer_index}.self_attention_norm"),
                layer.self_attention_norm(),
            )?;
            append_feed_forward_tensor_rows(
                &mut rows,
                &format!("encoder_layers.{layer_index}.feed_forward"),
                layer.feed_forward(),
            )?;
            append_layer_norm_tensor_rows(
                &mut rows,
                &format!("encoder_layers.{layer_index}.feed_forward_norm"),
                layer.feed_forward_norm(),
            )?;
        }
        for (layer_index, layer) in self.model.decoder_layers().iter().enumerate() {
            append_attention_tensor_rows(
                &mut rows,
                &format!("decoder_layers.{layer_index}.self_attention"),
                layer.self_attention(),
            )?;
            append_layer_norm_tensor_rows(
                &mut rows,
                &format!("decoder_layers.{layer_index}.self_attention_norm"),
                layer.self_attention_norm(),
            )?;
            append_attention_tensor_rows(
                &mut rows,
                &format!("decoder_layers.{layer_index}.cross_attention"),
                layer.cross_attention(),
            )?;
            append_layer_norm_tensor_rows(
                &mut rows,
                &format!("decoder_layers.{layer_index}.cross_attention_norm"),
                layer.cross_attention_norm(),
            )?;
            append_feed_forward_tensor_rows(
                &mut rows,
                &format!("decoder_layers.{layer_index}.feed_forward"),
                layer.feed_forward(),
            )?;
            append_layer_norm_tensor_rows(
                &mut rows,
                &format!("decoder_layers.{layer_index}.feed_forward_norm"),
                layer.feed_forward_norm(),
            )?;
        }
        rows.push(artifact_tensor_row(
            "logits_projection.weight",
            vec![config.target_vocab_size, config.hidden_size],
            self.model.logits_projection().weight_f32()?.to_vec(),
        ));
        rows.push(artifact_tensor_row(
            "logits_projection.bias",
            vec![config.target_vocab_size],
            self.model
                .logits_projection()
                .bias_f32()?
                .ok_or_else(|| TassadarArticleTransformerError::MissingLinearBias {
                    path: String::from("logits_projection.bias"),
                })?
                .to_vec(),
        ));
        Ok(rows)
    }

    fn from_weight_parts(
        config: EncoderDecoderTransformerConfig,
        embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
        source_embedding_table: Vec<f32>,
        target_embedding_table: Vec<f32>,
        logits_projection_weight: Vec<f32>,
        logits_projection_bias: Vec<f32>,
    ) -> Result<Self, TassadarArticleTransformerError> {
        let dropout_probability = config.dropout_probability();
        let source_embeddings = TransformerEmbeddings::from_f32_table(
            "tassadar.article_transformer.source_embeddings",
            config.source_vocab_size,
            config.hidden_size,
            config.max_source_positions,
            source_embedding_table.clone(),
            dropout_probability,
        )?;
        let target_embeddings = TransformerEmbeddings::from_f32_table(
            "tassadar.article_transformer.target_embeddings",
            config.target_vocab_size,
            config.hidden_size,
            config.max_target_positions,
            target_embedding_table.clone(),
            dropout_probability,
        )?;
        let encoder_layers = (0..config.encoder_layer_count)
            .map(|layer_index| build_encoder_layer(&config, layer_index))
            .collect::<Result<Vec<_>, _>>()?;
        let decoder_layers = (0..config.decoder_layer_count)
            .map(|layer_index| build_decoder_layer(&config, layer_index))
            .collect::<Result<Vec<_>, _>>()?;
        let logits_projection = Linear::from_f32_parts(
            "tassadar.article_transformer.logits_projection",
            config.hidden_size,
            config.target_vocab_size,
            logits_projection_weight.clone(),
            Some(logits_projection_bias.clone()),
        )?;
        let model = EncoderDecoderTransformer::from_components(
            config.clone(),
            source_embeddings,
            target_embeddings,
            encoder_layers,
            decoder_layers,
            logits_projection,
        )?;
        let descriptor = TassadarArticleTransformerDescriptor {
            model: ModelDescriptor::new(Self::MODEL_ID, Self::MODEL_FAMILY, "v0"),
            source_paper_title: String::from(Self::SOURCE_PAPER_TITLE),
            source_paper_ref: String::from(Self::SOURCE_PAPER_REF),
            architecture_variant:
                TassadarArticleTransformerArchitectureVariant::AttentionIsAllYouNeedEncoderDecoder,
            paper_faithful: true,
            substitution_justification: None,
            embedding_strategy,
            config,
            artifact_binding: placeholder_artifact_binding(Self::MODEL_ID),
            weights: placeholder_weight_bundle_metadata(),
        };
        Ok(Self {
            descriptor,
            source_embedding_table,
            target_embedding_table,
            logits_projection_weight,
            logits_projection_bias,
            model,
        })
    }
}

/// Article-route wrapper construction or forward-pass failure.
#[derive(Debug, Error)]
pub enum TassadarArticleTransformerError {
    #[error(transparent)]
    EncoderDecoder(#[from] EncoderDecoderTransformerError),
    #[error(transparent)]
    Block(#[from] psionic_transformer::TransformerBlockError),
    #[error(transparent)]
    Layer(#[from] LayerError),
    #[error(transparent)]
    TraceDecode(#[from] TassadarArticleTraceDecodeError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to parse {format} artifact: {message}")]
    ArtifactFormat { format: String, message: String },
    #[error("component `{component}` invalid configuration: {message}")]
    InvalidConfiguration {
        component: &'static str,
        message: String,
    },
    #[error("linear tensor `{path}` unexpectedly omitted bias")]
    MissingLinearBias { path: String },
    #[error("artifact tensor `{name}` is missing")]
    MissingArtifactTensor { name: String },
    #[error("artifact tensor `{name}` expected shape {expected:?}, found {actual:?}")]
    ArtifactTensorShapeMismatch {
        name: String,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    #[error("artifact tensor `{name}` used unsupported dtype `{dtype}`")]
    UnsupportedArtifactTensorDType { name: String, dtype: String },
    #[error(
        "descriptor weight metadata digest `{expected_digest}` did not match loaded artifact digest `{actual_digest}`"
    )]
    ArtifactMetadataMismatch {
        expected_digest: String,
        actual_digest: String,
    },
    #[error("artifact ref `{artifact_ref}` is not file-backed")]
    NonFileArtifactRef { artifact_ref: String },
    #[error("unknown bounded trainable parameter `{parameter_id}`")]
    UnknownParameter { parameter_id: String },
    #[error(
        "bounded trainable parameter `{parameter_id}` expected length {expected_len}, found {actual_len}"
    )]
    InvalidParameterShape {
        parameter_id: String,
        expected_len: usize,
        actual_len: usize,
    },
}

fn build_encoder_layer(
    config: &EncoderDecoderTransformerConfig,
    layer_index: usize,
) -> Result<TransformerEncoderLayer, TassadarArticleTransformerError> {
    let module_id = format!("tassadar.article_transformer.encoder_layers.{layer_index}");
    Ok(TransformerEncoderLayer::from_components(
        build_attention(
            format!("{module_id}.self_attention"),
            config.hidden_size,
            config.head_count,
        )?,
        LayerNorm::new(
            format!("{module_id}.self_attention_norm"),
            config.hidden_size,
            1e-5,
        )?,
        build_feed_forward(
            format!("{module_id}.feed_forward"),
            config.hidden_size,
            config.feed_forward_size,
        )?,
        LayerNorm::new(
            format!("{module_id}.feed_forward_norm"),
            config.hidden_size,
            1e-5,
        )?,
    )?)
}

fn build_decoder_layer(
    config: &EncoderDecoderTransformerConfig,
    layer_index: usize,
) -> Result<TransformerDecoderLayer, TassadarArticleTransformerError> {
    let module_id = format!("tassadar.article_transformer.decoder_layers.{layer_index}");
    Ok(TransformerDecoderLayer::from_components(
        build_attention(
            format!("{module_id}.self_attention"),
            config.hidden_size,
            config.head_count,
        )?,
        LayerNorm::new(
            format!("{module_id}.self_attention_norm"),
            config.hidden_size,
            1e-5,
        )?,
        build_attention(
            format!("{module_id}.cross_attention"),
            config.hidden_size,
            config.head_count,
        )?,
        LayerNorm::new(
            format!("{module_id}.cross_attention_norm"),
            config.hidden_size,
            1e-5,
        )?,
        build_feed_forward(
            format!("{module_id}.feed_forward"),
            config.hidden_size,
            config.feed_forward_size,
        )?,
        LayerNorm::new(
            format!("{module_id}.feed_forward_norm"),
            config.hidden_size,
            1e-5,
        )?,
    )?)
}

fn build_attention(
    module_id: String,
    hidden_size: usize,
    head_count: usize,
) -> Result<MultiHeadAttention, TassadarArticleTransformerError> {
    let weight_len = hidden_size * hidden_size;
    Ok(MultiHeadAttention::from_f32_parts(
        module_id.clone(),
        hidden_size,
        head_count,
        seeded_values(&format!("{module_id}.query_projection"), weight_len, 0.06),
        Some(seeded_values(
            &format!("{module_id}.query_bias"),
            hidden_size,
            0.01,
        )),
        seeded_values(&format!("{module_id}.key_projection"), weight_len, 0.06),
        Some(seeded_values(
            &format!("{module_id}.key_bias"),
            hidden_size,
            0.01,
        )),
        seeded_values(&format!("{module_id}.value_projection"), weight_len, 0.06),
        Some(seeded_values(
            &format!("{module_id}.value_bias"),
            hidden_size,
            0.01,
        )),
        seeded_values(&format!("{module_id}.output_projection"), weight_len, 0.06),
        Some(seeded_values(
            &format!("{module_id}.output_bias"),
            hidden_size,
            0.01,
        )),
        0.0,
    )?)
}

fn build_feed_forward(
    module_id: String,
    hidden_size: usize,
    feed_forward_size: usize,
) -> Result<PositionwiseFeedForward, TassadarArticleTransformerError> {
    Ok(PositionwiseFeedForward::from_f32_parts(
        module_id.clone(),
        hidden_size,
        feed_forward_size,
        ActivationKind::Relu,
        seeded_values(
            &format!("{module_id}.feed_forward_input"),
            hidden_size * feed_forward_size,
            0.05,
        ),
        Some(seeded_values(
            &format!("{module_id}.feed_forward_input_bias"),
            feed_forward_size,
            0.01,
        )),
        seeded_values(
            &format!("{module_id}.feed_forward_output"),
            hidden_size * feed_forward_size,
            0.05,
        ),
        Some(seeded_values(
            &format!("{module_id}.feed_forward_output_bias"),
            hidden_size,
            0.01,
        )),
        0.0,
    )?)
}

fn forward_pass_channel_trace(
    channel_id: String,
    channel_kind: &str,
    trace: &AttentionProbabilityTrace,
) -> TassadarArticleTransformerForwardPassChannelTrace {
    let tensor_shape = trace.tensor_spec().shape().dims().to_vec();
    TassadarArticleTransformerForwardPassChannelTrace::new(
        channel_id,
        channel_kind,
        tensor_shape.clone(),
        stable_digest(
            b"psionic_tassadar_article_transformer_attention_probability_trace|",
            trace,
        ),
        tensor_shape.into_iter().product(),
    )
}

fn predicted_token_ids_from_logits(
    dims: &[usize],
    data: &TensorData,
) -> Result<Vec<usize>, TassadarArticleTransformerError> {
    if dims.len() != 3 {
        return Err(TassadarArticleTransformerError::InvalidConfiguration {
            component: "tassadar_article_transformer.forward_with_runtime_evidence",
            message: format!("expected logits rank 3, found shape {dims:?}"),
        });
    }
    let batch_size = dims[0];
    let target_len = dims[1];
    let vocab_size = dims[2];
    if batch_size == 0 || target_len == 0 || vocab_size == 0 {
        return Err(TassadarArticleTransformerError::InvalidConfiguration {
            component: "tassadar_article_transformer.forward_with_runtime_evidence",
            message: format!("logits dimensions must be non-zero, found shape {dims:?}"),
        });
    }
    let values = match data {
        TensorData::F32(values) => values.as_slice(),
        other => {
            return Err(TassadarArticleTransformerError::InvalidConfiguration {
                component: "tassadar_article_transformer.forward_with_runtime_evidence",
                message: format!("expected dense f32 logits, found {other:?}"),
            });
        }
    };
    let expected_len = batch_size * target_len * vocab_size;
    if values.len() != expected_len {
        return Err(TassadarArticleTransformerError::InvalidConfiguration {
            component: "tassadar_article_transformer.forward_with_runtime_evidence",
            message: format!(
                "logits value count {} does not match shape {dims:?} (expected {expected_len})",
                values.len()
            ),
        });
    }
    let mut predictions = Vec::with_capacity(batch_size * target_len);
    for position_index in 0..(batch_size * target_len) {
        let offset = position_index * vocab_size;
        let mut best_index = 0usize;
        let mut best_value = values[offset];
        for vocab_index in 1..vocab_size {
            let candidate = values[offset + vocab_index];
            if candidate > best_value {
                best_index = vocab_index;
                best_value = candidate;
            }
        }
        predictions.push(best_index);
    }
    Ok(predictions)
}

fn transformer_execution_mode_label(mode: TransformerExecutionMode) -> String {
    match mode {
        TransformerExecutionMode::Eval => String::from("eval"),
        TransformerExecutionMode::Train { seed } => format!("train_seed_{seed}"),
    }
}

fn trace_domain_channel_binding_rows() -> Vec<TassadarArticleTransformerTraceChannelBindingRow> {
    vec![
        trace_channel_row(
            "prompt.locals",
            TassadarArticleTraceChannelKind::PromptScalar,
            "locals",
            &["<locals>"],
            "the canonical article wrapper binds the runtime-owned local-count prompt scalar to the shared tokenizer field token",
        ),
        trace_channel_row(
            "prompt.memory_slots",
            TassadarArticleTraceChannelKind::PromptScalar,
            "memory_slots",
            &["<memory_slots>"],
            "the canonical article wrapper binds the runtime-owned memory-slot prompt scalar to the shared tokenizer field token",
        ),
        trace_channel_row(
            "prompt.initial_memory",
            TassadarArticleTraceChannelKind::MemoryChannel,
            "initial_memory",
            &["<initial_memory>", "<list>"],
            "the canonical article wrapper binds the runtime-owned initial-memory channel to the tokenizer list prefix",
        ),
        trace_channel_row(
            "prompt.instructions",
            TassadarArticleTraceChannelKind::PromptInstructionStream,
            "instruction_stream",
            &[
                "<op_i32_const>",
                "<op_local_get>",
                "<op_local_set>",
                "<op_i32_add>",
                "<op_i32_sub>",
                "<op_i32_mul>",
                "<op_i32_lt>",
                "<op_i32_load>",
                "<op_i32_store>",
                "<op_br_if>",
                "<op_output>",
                "<op_return>",
            ],
            "the canonical article wrapper binds the runtime-owned instruction stream to the shared opcode-token family",
        ),
        trace_channel_row(
            "step.step_index",
            TassadarArticleTraceChannelKind::StepScalar,
            "step_index",
            &["<step>", "<step_index>"],
            "each append-only trace step begins with the shared step-index field token",
        ),
        trace_channel_row(
            "step.pc",
            TassadarArticleTraceChannelKind::StepScalar,
            "pc",
            &["<pc>"],
            "the current pc scalar binds to the shared pc field token",
        ),
        trace_channel_row(
            "step.next_pc",
            TassadarArticleTraceChannelKind::StepScalar,
            "next_pc",
            &["<next_pc>"],
            "the next-pc scalar binds to the shared next-pc field token",
        ),
        trace_channel_row(
            "step.instruction",
            TassadarArticleTraceChannelKind::StepInstruction,
            "instruction",
            &[
                "<op_i32_const>",
                "<op_local_get>",
                "<op_local_set>",
                "<op_i32_add>",
                "<op_i32_sub>",
                "<op_i32_mul>",
                "<op_i32_lt>",
                "<op_i32_load>",
                "<op_i32_store>",
                "<op_br_if>",
                "<op_output>",
                "<op_return>",
            ],
            "the realized step instruction binds to the same shared opcode-token family as the prompt instruction stream",
        ),
        trace_channel_row(
            "step.event",
            TassadarArticleTraceChannelKind::StepEvent,
            "event",
            &[
                "<event_const_push>",
                "<event_local_get>",
                "<event_local_set>",
                "<event_binary_add>",
                "<event_binary_sub>",
                "<event_binary_mul>",
                "<event_binary_lt>",
                "<event_load>",
                "<event_store>",
                "<event_branch>",
                "<event_output>",
                "<event_return>",
            ],
            "the realized machine event binds to the shared event-token family",
        ),
        trace_channel_row(
            "step.stack_before",
            TassadarArticleTraceChannelKind::StackChannel,
            "stack_before",
            &["<stack_before>", "<list>"],
            "the pre-step operand stack binds to the shared stack-before list token",
        ),
        trace_channel_row(
            "step.stack_after",
            TassadarArticleTraceChannelKind::StackChannel,
            "stack_after",
            &["<stack_after>", "<list>"],
            "the post-step operand stack binds to the shared stack-after list token",
        ),
        trace_channel_row(
            "step.locals_after",
            TassadarArticleTraceChannelKind::LocalsChannel,
            "locals_after",
            &["<locals_after>", "<list>"],
            "the post-step locals channel binds to the shared locals-after list token",
        ),
        trace_channel_row(
            "step.memory_after",
            TassadarArticleTraceChannelKind::MemoryChannel,
            "memory_after",
            &["<memory_after>", "<list>"],
            "the post-step memory channel binds to the shared memory-after list token",
        ),
        trace_channel_row(
            "terminal.halt_reason",
            TassadarArticleTraceChannelKind::HaltMarker,
            "halt_reason",
            &["<halt>", "<halt_returned>", "<halt_fell_off_end>", "<eos>"],
            "the trace suffix terminates through the shared halt markers before EOS",
        ),
    ]
}

fn trace_channel_row(
    channel_id: &str,
    channel_kind: TassadarArticleTraceChannelKind,
    stable_field_id: &str,
    token_forms: &[&str],
    detail: &str,
) -> TassadarArticleTransformerTraceChannelBindingRow {
    TassadarArticleTransformerTraceChannelBindingRow {
        channel_id: String::from(channel_id),
        channel_kind,
        stable_field_id: String::from(stable_field_id),
        token_forms: token_forms
            .iter()
            .map(|token| String::from(*token))
            .collect(),
        bound: true,
        detail: String::from(detail),
    }
}

fn usize_slice_to_token_ids(tokens: &[usize]) -> Vec<TokenId> {
    tokens
        .iter()
        .map(|token| TokenId(*token as u32))
        .collect::<Vec<_>>()
}

fn reconstructed_prompt_token_count(
    program: &TassadarProgram,
    execution: &TassadarExecution,
) -> usize {
    TassadarTraceTokenizer::new()
        .tokenize_program_and_execution(program, execution)
        .prompt_token_count
}

fn seeded_values(label: &str, len: usize, scale: f32) -> Vec<f32> {
    (0..len)
        .map(|index| {
            let mut hasher = Sha256::new();
            hasher.update(b"psionic_tassadar_article_transformer_seed|");
            hasher.update(label.as_bytes());
            hasher.update(index.to_le_bytes());
            let digest = hasher.finalize();
            let sample = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
            let centered = (sample as f32 / u32::MAX as f32) * 2.0 - 1.0;
            centered * scale
        })
        .collect()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-models should live under <repo>/crates/psionic-models")
        .to_path_buf()
}

fn placeholder_artifact_binding(model_id: &str) -> TassadarArticleTransformerArtifactBinding {
    TassadarArticleTransformerArtifactBinding::new(
        model_id,
        "pending://tassadar/article_transformer/unbound.safetensors",
        WeightFormat::SafeTensors,
        "pending",
        "pending",
        0,
    )
}

fn placeholder_weight_bundle_metadata() -> WeightBundleMetadata {
    WeightBundleMetadata {
        format: WeightFormat::SafeTensors,
        source: WeightSource::ExternalArtifact,
        quantization: psionic_core::QuantizationMode::None,
        quantization_modes: Vec::new(),
        digest: String::from("pending"),
        tensors: Vec::new(),
        artifacts: Vec::new(),
    }
}

fn artifact_tensor_row(
    name: &str,
    shape: Vec<usize>,
    values: Vec<f32>,
) -> TassadarArticleTransformerArtifactTensorRow {
    TassadarArticleTransformerArtifactTensorRow {
        name: String::from(name),
        shape,
        values,
    }
}

fn append_attention_tensor_rows(
    rows: &mut Vec<TassadarArticleTransformerArtifactTensorRow>,
    prefix: &str,
    attention: &MultiHeadAttention,
) -> Result<(), TassadarArticleTransformerError> {
    let hidden_size = attention.hidden_size();
    rows.push(artifact_tensor_row(
        &format!("{prefix}.query_projection.weight"),
        vec![hidden_size, hidden_size],
        attention.query_weight_f32()?.to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.query_projection.bias"),
        vec![hidden_size],
        attention
            .query_bias_f32()?
            .ok_or_else(|| TassadarArticleTransformerError::MissingLinearBias {
                path: format!("{prefix}.query_projection.bias"),
            })?
            .to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.key_projection.weight"),
        vec![hidden_size, hidden_size],
        attention.key_weight_f32()?.to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.key_projection.bias"),
        vec![hidden_size],
        attention
            .key_bias_f32()?
            .ok_or_else(|| TassadarArticleTransformerError::MissingLinearBias {
                path: format!("{prefix}.key_projection.bias"),
            })?
            .to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.value_projection.weight"),
        vec![hidden_size, hidden_size],
        attention.value_weight_f32()?.to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.value_projection.bias"),
        vec![hidden_size],
        attention
            .value_bias_f32()?
            .ok_or_else(|| TassadarArticleTransformerError::MissingLinearBias {
                path: format!("{prefix}.value_projection.bias"),
            })?
            .to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.output_projection.weight"),
        vec![hidden_size, hidden_size],
        attention.output_weight_f32()?.to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.output_projection.bias"),
        vec![hidden_size],
        attention
            .output_bias_f32()?
            .ok_or_else(|| TassadarArticleTransformerError::MissingLinearBias {
                path: format!("{prefix}.output_projection.bias"),
            })?
            .to_vec(),
    ));
    Ok(())
}

fn append_feed_forward_tensor_rows(
    rows: &mut Vec<TassadarArticleTransformerArtifactTensorRow>,
    prefix: &str,
    feed_forward: &PositionwiseFeedForward,
) -> Result<(), TassadarArticleTransformerError> {
    rows.push(artifact_tensor_row(
        &format!("{prefix}.input_projection.weight"),
        vec![feed_forward.intermediate_size(), feed_forward.hidden_size()],
        feed_forward.input_weight_f32()?.to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.input_projection.bias"),
        vec![feed_forward.intermediate_size()],
        feed_forward
            .input_bias_f32()?
            .ok_or_else(|| TassadarArticleTransformerError::MissingLinearBias {
                path: format!("{prefix}.input_projection.bias"),
            })?
            .to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.output_projection.weight"),
        vec![feed_forward.hidden_size(), feed_forward.intermediate_size()],
        feed_forward.output_weight_f32()?.to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.output_projection.bias"),
        vec![feed_forward.hidden_size()],
        feed_forward
            .output_bias_f32()?
            .ok_or_else(|| TassadarArticleTransformerError::MissingLinearBias {
                path: format!("{prefix}.output_projection.bias"),
            })?
            .to_vec(),
    ));
    Ok(())
}

fn append_layer_norm_tensor_rows(
    rows: &mut Vec<TassadarArticleTransformerArtifactTensorRow>,
    prefix: &str,
    layer_norm: &LayerNorm,
) -> Result<(), TassadarArticleTransformerError> {
    rows.push(artifact_tensor_row(
        &format!("{prefix}.weight"),
        vec![layer_norm.feature_size()],
        layer_norm.weight_f32()?.to_vec(),
    ));
    rows.push(artifact_tensor_row(
        &format!("{prefix}.bias"),
        vec![layer_norm.feature_size()],
        layer_norm.bias_f32()?.to_vec(),
    ));
    Ok(())
}

fn build_attention_from_tensors(
    module_id: String,
    tensor_prefix: &str,
    hidden_size: usize,
    head_count: usize,
    tensor_map: &BTreeMap<String, TassadarArticleTransformerArtifactTensorRow>,
) -> Result<MultiHeadAttention, TassadarArticleTransformerError> {
    MultiHeadAttention::from_f32_parts(
        module_id,
        hidden_size,
        head_count,
        require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.query_projection.weight"),
            &[hidden_size, hidden_size],
        )?,
        Some(require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.query_projection.bias"),
            &[hidden_size],
        )?),
        require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.key_projection.weight"),
            &[hidden_size, hidden_size],
        )?,
        Some(require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.key_projection.bias"),
            &[hidden_size],
        )?),
        require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.value_projection.weight"),
            &[hidden_size, hidden_size],
        )?,
        Some(require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.value_projection.bias"),
            &[hidden_size],
        )?),
        require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.output_projection.weight"),
            &[hidden_size, hidden_size],
        )?,
        Some(require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.output_projection.bias"),
            &[hidden_size],
        )?),
        0.0,
    )
    .map_err(Into::into)
}

fn build_feed_forward_from_tensors(
    module_id: String,
    tensor_prefix: &str,
    hidden_size: usize,
    feed_forward_size: usize,
    tensor_map: &BTreeMap<String, TassadarArticleTransformerArtifactTensorRow>,
) -> Result<PositionwiseFeedForward, TassadarArticleTransformerError> {
    PositionwiseFeedForward::from_f32_parts(
        module_id,
        hidden_size,
        feed_forward_size,
        ActivationKind::Relu,
        require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.input_projection.weight"),
            &[feed_forward_size, hidden_size],
        )?,
        Some(require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.input_projection.bias"),
            &[feed_forward_size],
        )?),
        require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.output_projection.weight"),
            &[hidden_size, feed_forward_size],
        )?,
        Some(require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.output_projection.bias"),
            &[hidden_size],
        )?),
        0.0,
    )
    .map_err(Into::into)
}

fn build_layer_norm_from_tensors(
    module_id: String,
    tensor_prefix: &str,
    feature_size: usize,
    tensor_map: &BTreeMap<String, TassadarArticleTransformerArtifactTensorRow>,
) -> Result<LayerNorm, TassadarArticleTransformerError> {
    LayerNorm::from_f32_parts(
        module_id,
        feature_size,
        1e-5,
        require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.weight"),
            &[feature_size],
        )?,
        require_tensor_values(
            tensor_map,
            &format!("{tensor_prefix}.bias"),
            &[feature_size],
        )?,
    )
    .map_err(Into::into)
}

fn build_encoder_layer_from_tensors(
    config: &EncoderDecoderTransformerConfig,
    layer_index: usize,
    tensor_map: &BTreeMap<String, TassadarArticleTransformerArtifactTensorRow>,
) -> Result<TransformerEncoderLayer, TassadarArticleTransformerError> {
    let module_id = format!("tassadar.article_transformer.encoder_layers.{layer_index}");
    let tensor_prefix = format!("encoder_layers.{layer_index}");
    TransformerEncoderLayer::from_components(
        build_attention_from_tensors(
            format!("{module_id}.self_attention"),
            &format!("{tensor_prefix}.self_attention"),
            config.hidden_size,
            config.head_count,
            tensor_map,
        )?,
        build_layer_norm_from_tensors(
            format!("{module_id}.self_attention_norm"),
            &format!("{tensor_prefix}.self_attention_norm"),
            config.hidden_size,
            tensor_map,
        )?,
        build_feed_forward_from_tensors(
            format!("{module_id}.feed_forward"),
            &format!("{tensor_prefix}.feed_forward"),
            config.hidden_size,
            config.feed_forward_size,
            tensor_map,
        )?,
        build_layer_norm_from_tensors(
            format!("{module_id}.feed_forward_norm"),
            &format!("{tensor_prefix}.feed_forward_norm"),
            config.hidden_size,
            tensor_map,
        )?,
    )
    .map_err(Into::into)
}

fn build_decoder_layer_from_tensors(
    config: &EncoderDecoderTransformerConfig,
    layer_index: usize,
    tensor_map: &BTreeMap<String, TassadarArticleTransformerArtifactTensorRow>,
) -> Result<TransformerDecoderLayer, TassadarArticleTransformerError> {
    let module_id = format!("tassadar.article_transformer.decoder_layers.{layer_index}");
    let tensor_prefix = format!("decoder_layers.{layer_index}");
    TransformerDecoderLayer::from_components(
        build_attention_from_tensors(
            format!("{module_id}.self_attention"),
            &format!("{tensor_prefix}.self_attention"),
            config.hidden_size,
            config.head_count,
            tensor_map,
        )?,
        build_layer_norm_from_tensors(
            format!("{module_id}.self_attention_norm"),
            &format!("{tensor_prefix}.self_attention_norm"),
            config.hidden_size,
            tensor_map,
        )?,
        build_attention_from_tensors(
            format!("{module_id}.cross_attention"),
            &format!("{tensor_prefix}.cross_attention"),
            config.hidden_size,
            config.head_count,
            tensor_map,
        )?,
        build_layer_norm_from_tensors(
            format!("{module_id}.cross_attention_norm"),
            &format!("{tensor_prefix}.cross_attention_norm"),
            config.hidden_size,
            tensor_map,
        )?,
        build_feed_forward_from_tensors(
            format!("{module_id}.feed_forward"),
            &format!("{tensor_prefix}.feed_forward"),
            config.hidden_size,
            config.feed_forward_size,
            tensor_map,
        )?,
        build_layer_norm_from_tensors(
            format!("{module_id}.feed_forward_norm"),
            &format!("{tensor_prefix}.feed_forward_norm"),
            config.hidden_size,
            tensor_map,
        )?,
    )
    .map_err(Into::into)
}

fn serialize_artifact_rows_to_bytes(
    rows: &[TassadarArticleTransformerArtifactTensorRow],
) -> Result<Vec<u8>, TassadarArticleTransformerError> {
    let mut ordered = rows.to_vec();
    ordered.sort_by(|left, right| left.name.cmp(&right.name));
    let mut tensors = Vec::with_capacity(ordered.len());
    for row in &ordered {
        let byte_data = encode_f32_le_bytes(row.values.as_slice()).into_boxed_slice();
        let leaked: &'static [u8] = Box::leak(byte_data);
        let view =
            TensorView::new(SafeTensorsDType::F32, row.shape.clone(), leaked).map_err(|error| {
                TassadarArticleTransformerError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                }
            })?;
        tensors.push((row.name.clone(), view));
    }
    serialize(tensors, None).map_err(|error| TassadarArticleTransformerError::ArtifactFormat {
        format: String::from("safetensors"),
        message: error.to_string(),
    })
}

fn load_article_transformer_tensor_map(
    artifact_bytes: &[u8],
) -> Result<
    BTreeMap<String, TassadarArticleTransformerArtifactTensorRow>,
    TassadarArticleTransformerError,
> {
    let tensors = SafeTensors::deserialize(artifact_bytes).map_err(|error| {
        TassadarArticleTransformerError::ArtifactFormat {
            format: String::from("safetensors"),
            message: error.to_string(),
        }
    })?;
    let mut names = tensors.names();
    names.sort_unstable();
    let mut tensor_map = BTreeMap::new();
    for name in names {
        let tensor = tensors.tensor(name).map_err(|error| {
            TassadarArticleTransformerError::ArtifactFormat {
                format: String::from("safetensors"),
                message: error.to_string(),
            }
        })?;
        if tensor.dtype() != SafeTensorsDType::F32 {
            return Err(
                TassadarArticleTransformerError::UnsupportedArtifactTensorDType {
                    name: name.to_string(),
                    dtype: tensor.dtype().to_string(),
                },
            );
        }
        tensor_map.insert(
            name.to_string(),
            artifact_tensor_row(
                name,
                tensor.shape().to_vec(),
                decode_f32_tensor_bytes(name, tensor.data())?,
            ),
        );
    }
    Ok(tensor_map)
}

fn require_tensor_values(
    tensor_map: &BTreeMap<String, TassadarArticleTransformerArtifactTensorRow>,
    name: &str,
    expected_shape: &[usize],
) -> Result<Vec<f32>, TassadarArticleTransformerError> {
    let row = tensor_map.get(name).ok_or_else(|| {
        TassadarArticleTransformerError::MissingArtifactTensor {
            name: String::from(name),
        }
    })?;
    if row.shape != expected_shape {
        return Err(
            TassadarArticleTransformerError::ArtifactTensorShapeMismatch {
                name: String::from(name),
                expected: expected_shape.to_vec(),
                actual: row.shape.clone(),
            },
        );
    }
    Ok(row.values.clone())
}

fn build_article_transformer_weight_bundle_metadata(
    rows: &[TassadarArticleTransformerArtifactTensorRow],
    artifact_ref: String,
    artifact_sha256: String,
    artifact_byte_length: u64,
) -> WeightBundleMetadata {
    let mut ordered = rows.to_vec();
    ordered.sort_by(|left, right| left.name.cmp(&right.name));
    let tensors = ordered
        .iter()
        .map(|row| {
            WeightTensorMetadata::new(row.name.clone(), Shape::new(row.shape.clone()), DType::F32)
        })
        .collect::<Vec<_>>();
    let mut hasher = Sha256::new();
    for (metadata, row) in tensors.iter().zip(ordered.iter()) {
        digest_tensor_values(&mut hasher, metadata, row.values.as_slice());
    }
    WeightBundleMetadata {
        format: WeightFormat::SafeTensors,
        source: WeightSource::ExternalArtifact,
        quantization: psionic_core::QuantizationMode::None,
        quantization_modes: Vec::new(),
        digest: hex::encode(hasher.finalize()),
        tensors,
        artifacts: vec![WeightArtifactMetadata::new(
            artifact_ref,
            artifact_byte_length,
            artifact_sha256,
        )],
    }
}

fn build_article_transformer_weight_bundle_metadata_from_map(
    tensor_map: &BTreeMap<String, TassadarArticleTransformerArtifactTensorRow>,
    descriptor: &TassadarArticleTransformerDescriptor,
) -> WeightBundleMetadata {
    let rows = tensor_map.values().cloned().collect::<Vec<_>>();
    let artifact_byte_length = descriptor
        .weights
        .artifacts
        .first()
        .map(|artifact| artifact.byte_length)
        .unwrap_or_default();
    build_article_transformer_weight_bundle_metadata(
        rows.as_slice(),
        descriptor.artifact_binding.artifact_ref.clone(),
        descriptor.artifact_binding.primary_artifact_sha256.clone(),
        artifact_byte_length,
    )
}

fn resolve_artifact_path(
    descriptor_path: &Path,
    artifact_ref: &str,
) -> Result<PathBuf, TassadarArticleTransformerError> {
    if artifact_ref.starts_with("memory://") {
        return Err(TassadarArticleTransformerError::NonFileArtifactRef {
            artifact_ref: String::from(artifact_ref),
        });
    }
    let artifact_path = PathBuf::from(artifact_ref);
    if artifact_path.is_absolute() {
        Ok(artifact_path)
    } else {
        Ok(descriptor_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(artifact_path))
    }
}

fn relative_artifact_ref(descriptor_path: &Path, artifact_path: &Path) -> String {
    if let Some(parent) = descriptor_path.parent() {
        if let Ok(relative) = artifact_path.strip_prefix(parent) {
            return relative.display().to_string();
        }
    }
    artifact_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(String::from)
        .unwrap_or_else(|| artifact_path.display().to_string())
}

fn encode_f32_le_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn decode_f32_tensor_bytes(
    name: &str,
    bytes: &[u8],
) -> Result<Vec<f32>, TassadarArticleTransformerError> {
    if bytes.len() % 4 != 0 {
        return Err(TassadarArticleTransformerError::ArtifactFormat {
            format: String::from("safetensors"),
            message: format!(
                "tensor `{name}` byte length {} is not divisible by 4",
                bytes.len()
            ),
        });
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn digest_tensor_values(hasher: &mut Sha256, metadata: &WeightTensorMetadata, values: &[f32]) {
    hasher.update(metadata.name.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.dtype).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.quantization).as_bytes());
    hasher.update(b"|");
    for dimension in metadata.shape.dims() {
        hasher.update(dimension.to_string().as_bytes());
        hasher.update(b",");
    }
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_bits().to_be_bytes());
    }
    hasher.update(b"\n");
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        TassadarArticleTransformer, TassadarArticleTransformerArchitectureVariant,
        TassadarArticleTransformerEmbeddingStrategy, TassadarArticleTransformerError,
    };
    use crate::{WeightFormat, WeightSource};
    use psionic_core::Shape;
    use psionic_runtime::{
        tassadar_article_class_corpus, TassadarArticleTransformerCheckpointLineage,
        TassadarCpuReferenceRunner, TrainingCheckpointReference,
    };
    use psionic_transformer::TransformerExecutionMode;

    #[test]
    fn article_transformer_descriptor_is_paper_faithful(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::canonical_reference()?;
        let descriptor = model.descriptor();

        assert_eq!(
            descriptor.model.model_id,
            TassadarArticleTransformer::MODEL_ID
        );
        assert_eq!(
            descriptor.architecture_variant,
            TassadarArticleTransformerArchitectureVariant::AttentionIsAllYouNeedEncoderDecoder
        );
        assert!(descriptor.paper_faithful);
        assert!(descriptor.substitution_justification.is_none());
        assert_eq!(
            descriptor.source_paper_title,
            TassadarArticleTransformer::SOURCE_PAPER_TITLE
        );
        Ok(())
    }

    #[test]
    fn article_transformer_descriptor_is_artifact_backed(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::canonical_reference()?;
        let descriptor = model.descriptor();

        assert_eq!(descriptor.weights.format, WeightFormat::SafeTensors);
        assert_eq!(descriptor.weights.source, WeightSource::ExternalArtifact);
        assert!(!descriptor.weights.artifacts.is_empty());
        assert!(!descriptor.weights.tensors.is_empty());
        assert_eq!(
            descriptor.artifact_binding.weight_bundle_digest,
            descriptor.weights.digest
        );
        assert_eq!(
            descriptor.artifact_binding.tensor_count,
            descriptor.weights.tensors.len()
        );
        assert_eq!(
            descriptor.artifact_binding.primary_artifact_sha256,
            descriptor.weights.artifacts[0].sha256
        );
        Ok(())
    }

    #[test]
    fn article_transformer_artifact_bundle_roundtrips(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::canonical_reference()?;
        let directory = tempfile::tempdir().expect("tempdir");
        let descriptor_path = directory.path().join("article_transformer_descriptor.json");
        let artifact_path = directory
            .path()
            .join("article_transformer_weights.safetensors");

        let written_descriptor = model.write_artifact_bundle(&descriptor_path, &artifact_path)?;
        let reloaded = TassadarArticleTransformer::load_from_descriptor_path(&descriptor_path)?;
        let output = reloaded.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 3]),
            &[0, 1, 2],
            TransformerExecutionMode::Eval,
        )?;

        assert_eq!(written_descriptor, *reloaded.descriptor());
        assert_eq!(
            model.descriptor().stable_digest(),
            reloaded.descriptor().stable_digest()
        );
        assert_eq!(
            model.model_artifact_binding().artifact_digest,
            reloaded.model_artifact_binding().artifact_digest
        );
        assert_eq!(output.logits.dims(), &[1, 3, 8]);
        Ok(())
    }

    #[test]
    fn article_transformer_supports_tied_and_unshared_embedding_options(
    ) -> Result<(), TassadarArticleTransformerError> {
        let config = TassadarArticleTransformer::tiny_reference_config();
        let unshared = TassadarArticleTransformer::paper_faithful_reference(
            config.clone(),
            TassadarArticleTransformerEmbeddingStrategy::Unshared,
        )?;
        let tied = TassadarArticleTransformer::paper_faithful_reference(
            config.clone(),
            TassadarArticleTransformerEmbeddingStrategy::DecoderInputOutputTied,
        )?;
        let shared = TassadarArticleTransformer::paper_faithful_reference(
            config,
            TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput,
        )?;

        assert_ne!(
            unshared.target_embedding_table(),
            unshared.logits_projection_weight()
        );
        assert_eq!(
            tied.target_embedding_table(),
            tied.logits_projection_weight()
        );
        assert_eq!(
            shared.source_embedding_table(),
            shared.target_embedding_table()
        );
        assert_eq!(
            shared.target_embedding_table(),
            shared.logits_projection_weight()
        );
        Ok(())
    }

    #[test]
    fn article_transformer_forward_emits_encoder_decoder_logits(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::canonical_reference()?;
        let output = model.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 3]),
            &[0, 1, 2],
            TransformerExecutionMode::Eval,
        )?;

        assert_eq!(output.encoder_hidden_state.dims(), &[1, 2, 4]);
        assert_eq!(output.decoder_hidden_state.dims(), &[1, 3, 4]);
        assert_eq!(output.logits.dims(), &[1, 3, 8]);
        assert_eq!(output.encoder_layer_outputs.len(), 2);
        assert_eq!(output.decoder_layer_outputs.len(), 2);
        Ok(())
    }

    #[test]
    fn article_transformer_forward_runtime_evidence_is_replay_stable(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::canonical_reference()?;
        let evidence = model.forward_with_runtime_evidence(
            "run-1",
            "request-1",
            "psionic.article_transformer.forward_pass",
            vec![String::from("fixtures://tassadar/article_transformer")],
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 3]),
            &[0, 1, 2],
            TransformerExecutionMode::Eval,
            Some(TassadarArticleTransformerCheckpointLineage {
                checkpoint: TrainingCheckpointReference::new(
                    "train.tassadar.article_transformer",
                    "checkpoint-stream",
                    "checkpoint-manifest-digest",
                    "checkpoint-object-digest",
                    "psionic.local.cpu_reference",
                    0,
                    "cluster.local.cpu_reference",
                    "topology.cpu_reference",
                    42,
                )
                .with_checkpoint_ref("checkpoint-ref")
                .with_step(7)
                .with_durable_at_ms(42),
                parent_checkpoint_ref: Some(String::from("parent-checkpoint-ref")),
                parent_manifest_digest: Some(String::from("parent-manifest-digest")),
            }),
        )?;

        assert!(evidence.replay_receipt.deterministic_match);
        assert_eq!(evidence.proof_bundle.failure_reason, None);
        assert_eq!(
            evidence.model_module_ref,
            TassadarArticleTransformer::MODEL_MODULE_REF
        );
        assert_eq!(
            evidence.transformer_module_ref,
            TassadarArticleTransformer::TRANSFORMER_MODULE_REF
        );
        assert_eq!(
            evidence.runtime_module_ref,
            TassadarArticleTransformer::RUNTIME_MODULE_REF
        );
        assert_eq!(evidence.trace_artifact.encoder_layer_traces.len(), 2);
        assert_eq!(
            evidence.trace_artifact.decoder_self_attention_traces.len(),
            2
        );
        assert_eq!(
            evidence.trace_artifact.decoder_cross_attention_traces.len(),
            2
        );
        Ok(())
    }

    #[test]
    fn article_transformer_trace_bound_reference_matches_canonical_tokenizer_vocab(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::article_trace_domain_reference()?;
        let binding = model.trace_domain_binding();

        assert_eq!(
            binding.model_source_vocab_size,
            binding.tokenizer_vocab_size
        );
        assert_eq!(
            binding.model_target_vocab_size,
            binding.tokenizer_vocab_size
        );
        assert!(binding.source_vocab_compatible);
        assert!(binding.target_vocab_compatible);
        assert!(binding.prompt_trace_boundary_supported);
        assert!(binding.halt_boundary_supported);
        assert!(binding.all_required_channels_bound);
        assert_eq!(binding.channel_binding_rows.len(), 14);
        Ok(())
    }

    #[test]
    fn article_transformer_trace_bound_reference_roundtrips_article_trace_domain(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::article_trace_domain_reference()?;
        let case = tassadar_article_class_corpus()
            .into_iter()
            .next()
            .expect("article corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.program)
            .expect("runner")
            .execute(&case.program)
            .expect("execution");
        let roundtrip = model.roundtrip_article_trace_domain(&case.program, &execution)?;

        assert_eq!(roundtrip.batch.source_shape[0], 1);
        assert_eq!(roundtrip.batch.target_shape[0], 1);
        assert_eq!(
            roundtrip.batch.source_shape[1],
            roundtrip.batch.prompt_token_count
        );
        assert_eq!(
            roundtrip.batch.target_shape[1],
            roundtrip.batch.target_token_count
        );
        assert_eq!(
            roundtrip.batch.halt_marker.as_deref(),
            Some("<halt_returned>")
        );
        assert!(roundtrip.prompt_boundary_preserved);
        assert!(roundtrip.halt_marker_preserved);
        assert!(roundtrip.roundtrip_exact);
        Ok(())
    }

    #[test]
    fn trace_bound_parameter_overrides_preserve_model_identity(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::article_trace_domain_reference()?;
        let mut shared = model
            .trainable_parameter_vector(TassadarArticleTransformer::SHARED_EMBEDDING_PARAMETER_ID)
            .expect("shared parameter");
        shared.values[0] += 0.125;
        let updated = model.with_parameter_overrides(&BTreeMap::from([(
            String::from(TassadarArticleTransformer::SHARED_EMBEDDING_PARAMETER_ID),
            shared.values,
        )]))?;

        assert_eq!(
            updated.descriptor().model.model_id,
            TassadarArticleTransformer::TRACE_BOUND_MODEL_ID
        );
        assert!(updated
            .artifact_binding()
            .artifact_ref
            .contains(TassadarArticleTransformer::TRACE_BOUND_MODEL_ID));
        Ok(())
    }

    #[test]
    fn article_transformer_supports_rebinding_to_trained_trace_bound_identity(
    ) -> Result<(), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::article_trace_domain_reference()?;
        let trained = model.with_model_identity(
            TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID,
            "v0",
        )?;

        assert_eq!(
            trained.descriptor().model.model_id,
            TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        );
        assert!(trained
            .artifact_binding()
            .artifact_ref
            .contains(TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID));
        Ok(())
    }
}
