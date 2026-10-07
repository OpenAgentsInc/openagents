use std::collections::BTreeMap;

use psionic_core::{DType, QuantizationMode, Shape};
pub use psionic_runtime::{
    AttnResDiagnosticsSnapshot, AttnResSublayerKind, AttnResSublayerSnapshot,
};
use psionic_transformer::{
    AttnResBlockState, AttnResConfig, AttnResConfigError, AttnResExecutionError, AttnResTensor3,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelDescriptor, TokenId, TokenSequence, WeightArtifactMetadata, WeightBundleMetadata,
    WeightFormat, WeightSource, WeightTensorMetadata,
};

/// Stable family label for Attention Residual models.
pub const ATTN_RES_MODEL_FAMILY: &str = "attnres";

/// One next-token supervision sample for bounded AttnRes train/eval flows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttnResNextTokenSample {
    /// Stable sample identifier.
    pub sample_id: String,
    /// Ordered input token prefix.
    pub input_tokens: TokenSequence,
    /// Target token the model should predict at the last position.
    pub target_token: TokenId,
}

impl AttnResNextTokenSample {
    /// Creates one next-token supervision sample.
    #[must_use]
    pub fn new(
        sample_id: impl Into<String>,
        input_tokens: TokenSequence,
        target_token: TokenId,
    ) -> Self {
        Self {
            sample_id: sample_id.into(),
            input_tokens,
            target_token,
        }
    }
}

/// Stable descriptor for one AttnRes family instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResModelDescriptor {
    /// Shared model identity.
    pub model: ModelDescriptor,
    /// Stable model config.
    pub config: AttnResConfig,
    /// Weight metadata for the bound bundle.
    pub weights: WeightBundleMetadata,
}

impl AttnResModelDescriptor {
    /// Returns a stable digest over the descriptor payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_attnres_model_descriptor|", self)
    }
}

/// CPU-reference model build failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AttnResModelError {
    /// Configuration validation failed.
    #[error(transparent)]
    Config(#[from] AttnResConfigError),
    /// Layer count did not match the configuration.
    #[error("weight layer count {actual} does not match transformer layer count {expected}")]
    LayerCountMismatch {
        /// Actual layer count in the bundle.
        actual: usize,
        /// Expected layer count from the config.
        expected: usize,
    },
    /// One flat vector did not match the expected element count.
    #[error("weight tensor `{name}` has length {actual}; expected {expected}")]
    InvalidVectorLength {
        /// Logical tensor name.
        name: String,
        /// Actual element count.
        actual: usize,
        /// Expected element count.
        expected: usize,
    },
    /// A named parameter override targeted an unknown parameter.
    #[error("unknown attnres parameter `{parameter_id}`")]
    UnknownParameter {
        /// Stable parameter identifier.
        parameter_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct AttnResOpWeights {
    pseudo_query: Vec<f32>,
    norm_gamma: Vec<f32>,
}

impl AttnResOpWeights {
    fn new(d_model: usize) -> Self {
        Self {
            pseudo_query: vec![0.0; d_model],
            norm_gamma: vec![1.0; d_model],
        }
    }

    fn validate(&self, name: &str, d_model: usize) -> Result<(), AttnResModelError> {
        validate_vector_length(format!("{name}.pseudo_query"), &self.pseudo_query, d_model)?;
        validate_vector_length(format!("{name}.norm_gamma"), &self.norm_gamma, d_model)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct AttnResLinearWeights {
    input_dim: usize,
    output_dim: usize,
    weight: Vec<f32>,
    bias: Vec<f32>,
}

impl AttnResLinearWeights {
    fn seeded(label: &str, input_dim: usize, output_dim: usize, scale: f32) -> Self {
        Self {
            input_dim,
            output_dim,
            weight: seeded_values(&format!("{label}.weight"), input_dim * output_dim, scale),
            bias: vec![0.0; output_dim],
        }
    }

    fn validate(&self, name: &str) -> Result<(), AttnResModelError> {
        validate_vector_length(
            format!("{name}.weight"),
            &self.weight,
            self.input_dim * self.output_dim,
        )?;
        validate_vector_length(format!("{name}.bias"), &self.bias, self.output_dim)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct AttnResAttentionWeights {
    q_proj: AttnResLinearWeights,
    k_proj: AttnResLinearWeights,
    v_proj: AttnResLinearWeights,
    o_proj: AttnResLinearWeights,
}

impl AttnResAttentionWeights {
    fn seeded(label: &str, d_model: usize) -> Self {
        Self {
            q_proj: AttnResLinearWeights::seeded(
                &format!("{label}.q_proj"),
                d_model,
                d_model,
                0.08,
            ),
            k_proj: AttnResLinearWeights::seeded(
                &format!("{label}.k_proj"),
                d_model,
                d_model,
                0.08,
            ),
            v_proj: AttnResLinearWeights::seeded(
                &format!("{label}.v_proj"),
                d_model,
                d_model,
                0.08,
            ),
            o_proj: AttnResLinearWeights::seeded(
                &format!("{label}.o_proj"),
                d_model,
                d_model,
                0.08,
            ),
        }
    }

    fn validate(&self, name: &str) -> Result<(), AttnResModelError> {
        self.q_proj.validate(&format!("{name}.q_proj"))?;
        self.k_proj.validate(&format!("{name}.k_proj"))?;
        self.v_proj.validate(&format!("{name}.v_proj"))?;
        self.o_proj.validate(&format!("{name}.o_proj"))?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct AttnResFeedForwardWeights {
    linear1: AttnResLinearWeights,
    linear2: AttnResLinearWeights,
}

impl AttnResFeedForwardWeights {
    fn seeded(label: &str, d_model: usize, d_ff: usize) -> Self {
        Self {
            linear1: AttnResLinearWeights::seeded(&format!("{label}.linear1"), d_model, d_ff, 0.07),
            linear2: AttnResLinearWeights::seeded(&format!("{label}.linear2"), d_ff, d_model, 0.07),
        }
    }

    fn validate(&self, name: &str) -> Result<(), AttnResModelError> {
        self.linear1.validate(&format!("{name}.linear1"))?;
        self.linear2.validate(&format!("{name}.linear2"))?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct AttnResLayerWeights {
    attn_res: AttnResOpWeights,
    mlp_res: AttnResOpWeights,
    attn_norm_gamma: Vec<f32>,
    attention: AttnResAttentionWeights,
    mlp_norm_gamma: Vec<f32>,
    feed_forward: AttnResFeedForwardWeights,
}

impl AttnResLayerWeights {
    fn seeded(config: &AttnResConfig, layer_idx: usize) -> Result<Self, AttnResModelError> {
        let d_ff = config.effective_d_ff()?;
        Ok(Self {
            attn_res: AttnResOpWeights::new(config.d_model),
            mlp_res: AttnResOpWeights::new(config.d_model),
            attn_norm_gamma: vec![1.0; config.d_model],
            attention: AttnResAttentionWeights::seeded(
                &format!("layers.{layer_idx}.attention"),
                config.d_model,
            ),
            mlp_norm_gamma: vec![1.0; config.d_model],
            feed_forward: AttnResFeedForwardWeights::seeded(
                &format!("layers.{layer_idx}.feed_forward"),
                config.d_model,
                d_ff,
            ),
        })
    }

    fn validate(&self, config: &AttnResConfig, layer_idx: usize) -> Result<(), AttnResModelError> {
        self.attn_res
            .validate(&format!("layers.{layer_idx}.attn_res"), config.d_model)?;
        self.mlp_res
            .validate(&format!("layers.{layer_idx}.mlp_res"), config.d_model)?;
        validate_vector_length(
            format!("layers.{layer_idx}.attn_norm_gamma"),
            &self.attn_norm_gamma,
            config.d_model,
        )?;
        validate_vector_length(
            format!("layers.{layer_idx}.mlp_norm_gamma"),
            &self.mlp_norm_gamma,
            config.d_model,
        )?;
        self.attention
            .validate(&format!("layers.{layer_idx}.attention"))?;
        self.feed_forward
            .validate(&format!("layers.{layer_idx}.feed_forward"))?;
        Ok(())
    }
}

/// Stable weight bundle for one AttnRes model family instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResWeightBundle {
    metadata: WeightBundleMetadata,
    token_embeddings: Vec<f32>,
    layers: Vec<AttnResLayerWeights>,
    final_norm_gamma: Vec<f32>,
    lm_head: AttnResLinearWeights,
}

/// Stable named parameter vector exported from one AttnRes weight bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResParameterVector {
    /// Stable parameter identifier.
    pub parameter_id: String,
    /// Logical tensor shape for the parameter.
    pub shape: Shape,
    /// Dense `f32` values in row-major order.
    pub values: Vec<f32>,
}

impl AttnResWeightBundle {
    /// Builds a deterministic reference bundle for the supplied config.
    pub fn seeded_reference(config: &AttnResConfig) -> Result<Self, AttnResModelError> {
        config.validate()?;
        let token_embeddings =
            seeded_values("token_embeddings", config.vocab_size * config.d_model, 0.09);
        let layers = (0..config.num_transformer_layers())
            .map(|layer_idx| AttnResLayerWeights::seeded(config, layer_idx))
            .collect::<Result<Vec<_>, _>>()?;
        let final_norm_gamma = vec![1.0; config.d_model];
        let lm_head =
            AttnResLinearWeights::seeded("lm_head", config.d_model, config.vocab_size, 0.08);

        let bundle = Self {
            metadata: WeightBundleMetadata {
                format: WeightFormat::ProgrammaticFixture,
                source: WeightSource::Fixture,
                quantization: QuantizationMode::None,
                quantization_modes: Vec::new(),
                digest: String::new(),
                tensors: Vec::new(),
                artifacts: Vec::new(),
            },
            token_embeddings,
            layers,
            final_norm_gamma,
            lm_head,
        };
        bundle.rebuild_metadata(config)
    }

    /// Returns the stable metadata for the bundle.
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    /// Returns the token embeddings in row-major `[vocab, d_model]` order.
    #[must_use]
    pub fn token_embeddings(&self) -> &[f32] {
        &self.token_embeddings
    }

    /// Returns one stable flat parameter-map view over the bundle.
    #[must_use]
    pub fn parameter_vectors(&self) -> Vec<AttnResParameterVector> {
        let d_model = self.final_norm_gamma.len();
        let mut vectors = vec![AttnResParameterVector {
            parameter_id: String::from("token_embeddings"),
            shape: Shape::new(vec![self.token_embeddings.len() / d_model, d_model]),
            values: self.token_embeddings.clone(),
        }];
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            vectors.extend([
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.attn_res.pseudo_query"),
                    shape: Shape::new(vec![layer.attn_res.pseudo_query.len()]),
                    values: layer.attn_res.pseudo_query.clone(),
                },
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.attn_res.norm_gamma"),
                    shape: Shape::new(vec![layer.attn_res.norm_gamma.len()]),
                    values: layer.attn_res.norm_gamma.clone(),
                },
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.mlp_res.pseudo_query"),
                    shape: Shape::new(vec![layer.mlp_res.pseudo_query.len()]),
                    values: layer.mlp_res.pseudo_query.clone(),
                },
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.mlp_res.norm_gamma"),
                    shape: Shape::new(vec![layer.mlp_res.norm_gamma.len()]),
                    values: layer.mlp_res.norm_gamma.clone(),
                },
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.attn_norm_gamma"),
                    shape: Shape::new(vec![layer.attn_norm_gamma.len()]),
                    values: layer.attn_norm_gamma.clone(),
                },
                linear_parameter_vector(
                    format!("layers.{layer_idx}.attention.q_proj.weight"),
                    layer.attention.q_proj.input_dim,
                    layer.attention.q_proj.output_dim,
                    layer.attention.q_proj.weight.as_slice(),
                ),
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.attention.q_proj.bias"),
                    shape: Shape::new(vec![layer.attention.q_proj.output_dim]),
                    values: layer.attention.q_proj.bias.clone(),
                },
                linear_parameter_vector(
                    format!("layers.{layer_idx}.attention.k_proj.weight"),
                    layer.attention.k_proj.input_dim,
                    layer.attention.k_proj.output_dim,
                    layer.attention.k_proj.weight.as_slice(),
                ),
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.attention.k_proj.bias"),
                    shape: Shape::new(vec![layer.attention.k_proj.output_dim]),
                    values: layer.attention.k_proj.bias.clone(),
                },
                linear_parameter_vector(
                    format!("layers.{layer_idx}.attention.v_proj.weight"),
                    layer.attention.v_proj.input_dim,
                    layer.attention.v_proj.output_dim,
                    layer.attention.v_proj.weight.as_slice(),
                ),
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.attention.v_proj.bias"),
                    shape: Shape::new(vec![layer.attention.v_proj.output_dim]),
                    values: layer.attention.v_proj.bias.clone(),
                },
                linear_parameter_vector(
                    format!("layers.{layer_idx}.attention.o_proj.weight"),
                    layer.attention.o_proj.input_dim,
                    layer.attention.o_proj.output_dim,
                    layer.attention.o_proj.weight.as_slice(),
                ),
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.attention.o_proj.bias"),
                    shape: Shape::new(vec![layer.attention.o_proj.output_dim]),
                    values: layer.attention.o_proj.bias.clone(),
                },
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.mlp_norm_gamma"),
                    shape: Shape::new(vec![layer.mlp_norm_gamma.len()]),
                    values: layer.mlp_norm_gamma.clone(),
                },
                linear_parameter_vector(
                    format!("layers.{layer_idx}.feed_forward.linear1.weight"),
                    layer.feed_forward.linear1.input_dim,
                    layer.feed_forward.linear1.output_dim,
                    layer.feed_forward.linear1.weight.as_slice(),
                ),
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.feed_forward.linear1.bias"),
                    shape: Shape::new(vec![layer.feed_forward.linear1.output_dim]),
                    values: layer.feed_forward.linear1.bias.clone(),
                },
                linear_parameter_vector(
                    format!("layers.{layer_idx}.feed_forward.linear2.weight"),
                    layer.feed_forward.linear2.input_dim,
                    layer.feed_forward.linear2.output_dim,
                    layer.feed_forward.linear2.weight.as_slice(),
                ),
                AttnResParameterVector {
                    parameter_id: format!("layers.{layer_idx}.feed_forward.linear2.bias"),
                    shape: Shape::new(vec![layer.feed_forward.linear2.output_dim]),
                    values: layer.feed_forward.linear2.bias.clone(),
                },
            ]);
        }
        vectors.extend([
            AttnResParameterVector {
                parameter_id: String::from("final_norm_gamma"),
                shape: Shape::new(vec![self.final_norm_gamma.len()]),
                values: self.final_norm_gamma.clone(),
            },
            linear_parameter_vector(
                String::from("lm_head.weight"),
                self.lm_head.input_dim,
                self.lm_head.output_dim,
                self.lm_head.weight.as_slice(),
            ),
            AttnResParameterVector {
                parameter_id: String::from("lm_head.bias"),
                shape: Shape::new(vec![self.lm_head.output_dim]),
                values: self.lm_head.bias.clone(),
            },
        ]);
        vectors
    }

    /// Returns one named parameter vector when it exists.
    #[must_use]
    pub fn parameter_vector(&self, parameter_id: &str) -> Option<AttnResParameterVector> {
        self.parameter_vectors()
            .into_iter()
            .find(|parameter| parameter.parameter_id == parameter_id)
    }

    /// Returns a rebuilt bundle with the supplied named parameter overrides.
    pub fn with_parameter_overrides(
        &self,
        config: &AttnResConfig,
        overrides: &BTreeMap<String, Vec<f32>>,
    ) -> Result<Self, AttnResModelError> {
        let mut updated = self.clone();
        for (parameter_id, values) in overrides {
            apply_parameter_override(&mut updated, parameter_id.as_str(), values.as_slice())?;
        }
        updated.rebuild_metadata(config)
    }

    /// Returns a copy tagged with explicit artifact-backed provenance.
    #[must_use]
    pub fn with_artifact_metadata(
        &self,
        format: WeightFormat,
        source: WeightSource,
        artifacts: Vec<WeightArtifactMetadata>,
    ) -> Self {
        let mut updated = self.clone();
        updated.metadata.format = format;
        updated.metadata.source = source;
        updated.metadata.artifacts = artifacts;
        updated
    }

    fn validate(&self, config: &AttnResConfig) -> Result<(), AttnResModelError> {
        config.validate()?;
        validate_vector_length(
            "token_embeddings",
            &self.token_embeddings,
            config.vocab_size * config.d_model,
        )?;
        if self.layers.len() != config.num_transformer_layers() {
            return Err(AttnResModelError::LayerCountMismatch {
                actual: self.layers.len(),
                expected: config.num_transformer_layers(),
            });
        }
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            layer.validate(config, layer_idx)?;
        }
        validate_vector_length("final_norm_gamma", &self.final_norm_gamma, config.d_model)?;
        self.lm_head.validate("lm_head")?;
        if self.lm_head.input_dim != config.d_model || self.lm_head.output_dim != config.vocab_size
        {
            return Err(AttnResModelError::InvalidVectorLength {
                name: String::from("lm_head"),
                actual: self.lm_head.input_dim * self.lm_head.output_dim,
                expected: config.d_model * config.vocab_size,
            });
        }
        Ok(())
    }

    fn rebuild_metadata(&self, config: &AttnResConfig) -> Result<Self, AttnResModelError> {
        self.validate(config)?;

        let mut entries: Vec<(WeightTensorMetadata, &[f32])> = vec![(
            WeightTensorMetadata::new(
                "token_embeddings",
                Shape::new(vec![config.vocab_size, config.d_model]),
                DType::F32,
            ),
            self.token_embeddings.as_slice(),
        )];

        for (layer_idx, layer) in self.layers.iter().enumerate() {
            entries.extend([
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attn_res.pseudo_query"),
                        Shape::new(vec![config.d_model]),
                        DType::F32,
                    ),
                    layer.attn_res.pseudo_query.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attn_res.norm_gamma"),
                        Shape::new(vec![config.d_model]),
                        DType::F32,
                    ),
                    layer.attn_res.norm_gamma.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.mlp_res.pseudo_query"),
                        Shape::new(vec![config.d_model]),
                        DType::F32,
                    ),
                    layer.mlp_res.pseudo_query.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.mlp_res.norm_gamma"),
                        Shape::new(vec![config.d_model]),
                        DType::F32,
                    ),
                    layer.mlp_res.norm_gamma.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attn_norm_gamma"),
                        Shape::new(vec![config.d_model]),
                        DType::F32,
                    ),
                    layer.attn_norm_gamma.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attention.q_proj.weight"),
                        Shape::new(vec![
                            layer.attention.q_proj.input_dim,
                            layer.attention.q_proj.output_dim,
                        ]),
                        DType::F32,
                    ),
                    layer.attention.q_proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attention.q_proj.bias"),
                        Shape::new(vec![layer.attention.q_proj.output_dim]),
                        DType::F32,
                    ),
                    layer.attention.q_proj.bias.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attention.k_proj.weight"),
                        Shape::new(vec![
                            layer.attention.k_proj.input_dim,
                            layer.attention.k_proj.output_dim,
                        ]),
                        DType::F32,
                    ),
                    layer.attention.k_proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attention.k_proj.bias"),
                        Shape::new(vec![layer.attention.k_proj.output_dim]),
                        DType::F32,
                    ),
                    layer.attention.k_proj.bias.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attention.v_proj.weight"),
                        Shape::new(vec![
                            layer.attention.v_proj.input_dim,
                            layer.attention.v_proj.output_dim,
                        ]),
                        DType::F32,
                    ),
                    layer.attention.v_proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attention.v_proj.bias"),
                        Shape::new(vec![layer.attention.v_proj.output_dim]),
                        DType::F32,
                    ),
                    layer.attention.v_proj.bias.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attention.o_proj.weight"),
                        Shape::new(vec![
                            layer.attention.o_proj.input_dim,
                            layer.attention.o_proj.output_dim,
                        ]),
                        DType::F32,
                    ),
                    layer.attention.o_proj.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.attention.o_proj.bias"),
                        Shape::new(vec![layer.attention.o_proj.output_dim]),
                        DType::F32,
                    ),
                    layer.attention.o_proj.bias.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.mlp_norm_gamma"),
                        Shape::new(vec![config.d_model]),
                        DType::F32,
                    ),
                    layer.mlp_norm_gamma.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.feed_forward.linear1.weight"),
                        Shape::new(vec![
                            layer.feed_forward.linear1.input_dim,
                            layer.feed_forward.linear1.output_dim,
                        ]),
                        DType::F32,
                    ),
                    layer.feed_forward.linear1.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.feed_forward.linear1.bias"),
                        Shape::new(vec![layer.feed_forward.linear1.output_dim]),
                        DType::F32,
                    ),
                    layer.feed_forward.linear1.bias.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.feed_forward.linear2.weight"),
                        Shape::new(vec![
                            layer.feed_forward.linear2.input_dim,
                            layer.feed_forward.linear2.output_dim,
                        ]),
                        DType::F32,
                    ),
                    layer.feed_forward.linear2.weight.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer_idx}.feed_forward.linear2.bias"),
                        Shape::new(vec![layer.feed_forward.linear2.output_dim]),
                        DType::F32,
                    ),
                    layer.feed_forward.linear2.bias.as_slice(),
                ),
            ]);
        }

        entries.extend([
            (
                WeightTensorMetadata::new(
                    "final_norm_gamma",
                    Shape::new(vec![config.d_model]),
                    DType::F32,
                ),
                self.final_norm_gamma.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "lm_head.weight",
                    Shape::new(vec![self.lm_head.input_dim, self.lm_head.output_dim]),
                    DType::F32,
                ),
                self.lm_head.weight.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "lm_head.bias",
                    Shape::new(vec![self.lm_head.output_dim]),
                    DType::F32,
                ),
                self.lm_head.bias.as_slice(),
            ),
        ]);

        let metadata = build_metadata(&entries);
        Ok(Self {
            metadata,
            token_embeddings: self.token_embeddings.clone(),
            layers: self.layers.clone(),
            final_norm_gamma: self.final_norm_gamma.clone(),
            lm_head: self.lm_head.clone(),
        })
    }
}

/// CPU-reference AttnRes model bound to one descriptor and bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResCpuReferenceModel {
    descriptor: AttnResModelDescriptor,
    weights: AttnResWeightBundle,
}

impl AttnResCpuReferenceModel {
    /// Builds a deterministic reference model using Psionic-owned seeded weights.
    pub fn seeded(
        model_id: impl Into<String>,
        revision: impl Into<String>,
        config: AttnResConfig,
    ) -> Result<Self, AttnResModelError> {
        let weights = AttnResWeightBundle::seeded_reference(&config)?;
        Self::with_weights(
            ModelDescriptor::new(model_id, ATTN_RES_MODEL_FAMILY, revision),
            config,
            weights,
        )
    }

    /// Builds a reference model from one explicit descriptor root and weight bundle.
    pub fn with_weights(
        model: ModelDescriptor,
        config: AttnResConfig,
        weights: AttnResWeightBundle,
    ) -> Result<Self, AttnResModelError> {
        config.validate()?;
        weights.validate(&config)?;
        let descriptor = AttnResModelDescriptor {
            model,
            config: config.clone(),
            weights: weights.metadata.clone(),
        };
        Ok(Self {
            descriptor,
            weights,
        })
    }

    /// Returns the stable descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &AttnResModelDescriptor {
        &self.descriptor
    }

    /// Returns the stable config.
    #[must_use]
    pub fn config(&self) -> &AttnResConfig {
        &self.descriptor.config
    }

    /// Returns the bound weight bundle.
    #[must_use]
    pub fn weights(&self) -> &AttnResWeightBundle {
        &self.weights
    }

    /// Embeds one batch of token sequences.
    pub fn embed_tokens(
        &self,
        inputs: &[TokenSequence],
    ) -> Result<AttnResTensor3, AttnResExecutionError> {
        if inputs.is_empty() {
            return Err(AttnResExecutionError::EmptyBatch);
        }
        let sequence_length = inputs.first().map(TokenSequence::len).unwrap_or(0);
        for (batch_index, tokens) in inputs.iter().enumerate() {
            if tokens.len() != sequence_length {
                return Err(AttnResExecutionError::RaggedBatch {
                    batch_index,
                    expected: sequence_length,
                    actual: tokens.len(),
                });
            }
        }

        let mut embeddings =
            AttnResTensor3::zeros([inputs.len(), sequence_length, self.config().d_model]);
        for (batch_index, tokens) in inputs.iter().enumerate() {
            for (position_index, token) in tokens.as_slice().iter().enumerate() {
                let token_index = token.as_u32() as usize;
                if token_index >= self.config().vocab_size {
                    return Err(AttnResExecutionError::TokenOutOfRange {
                        batch_index,
                        position_index,
                        token: token.as_u32(),
                        vocab_size: self.config().vocab_size,
                    });
                }
                let start = token_index * self.config().d_model;
                let end = start + self.config().d_model;
                let row = &self.weights.token_embeddings[start..end];
                for (feature, value) in row.iter().enumerate() {
                    embeddings.set(batch_index, position_index, feature, *value);
                }
            }
        }
        Ok(embeddings)
    }

    /// Runs the standard CPU-reference forward pass and returns hidden states.
    pub fn forward_hidden(
        &self,
        inputs: &[TokenSequence],
    ) -> Result<AttnResTensor3, AttnResExecutionError> {
        let embeddings = self.embed_tokens(inputs)?;
        self.forward_hidden_from_embeddings(embeddings)
    }

    /// Runs the standard CPU-reference forward pass and returns hidden states plus diagnostics.
    pub fn forward_hidden_with_diagnostics(
        &self,
        inputs: &[TokenSequence],
    ) -> Result<(AttnResTensor3, AttnResDiagnosticsSnapshot), AttnResExecutionError> {
        let embeddings = self.embed_tokens(inputs)?;
        self.forward_hidden_from_embeddings_with_diagnostics(embeddings)
    }

    /// Runs the standard CPU-reference forward pass from precomputed embeddings.
    pub fn forward_hidden_from_embeddings(
        &self,
        embeddings: AttnResTensor3,
    ) -> Result<AttnResTensor3, AttnResExecutionError> {
        self.forward_hidden_from_embeddings_with_diagnostics(embeddings)
            .map(|(hidden, _)| hidden)
    }

    /// Runs the standard CPU-reference forward pass from precomputed embeddings with diagnostics.
    pub fn forward_hidden_from_embeddings_with_diagnostics(
        &self,
        embeddings: AttnResTensor3,
    ) -> Result<(AttnResTensor3, AttnResDiagnosticsSnapshot), AttnResExecutionError> {
        self.validate_embeddings(&embeddings)?;

        let mut state = AttnResBlockState::new(embeddings.clone());
        let mut diagnostics = AttnResDiagnosticsSnapshot {
            batch_size: embeddings.batch_size(),
            sequence_length: embeddings.sequence_length(),
            hidden_size: embeddings.width(),
            final_completed_blocks: 0,
            final_partial_block_present: false,
            sublayers: Vec::with_capacity(self.config().num_layers),
        };

        for (layer_idx, layer) in self.weights.layers.iter().enumerate() {
            let attn_sublayer_index = layer_idx * 2;
            let current_partial = state.partial_block.take();
            let completed_blocks_before = state.completed_blocks.len();
            let (attn_input, attn_routing) = attn_res_forward(
                &layer.attn_res,
                &state.completed_blocks,
                current_partial.as_ref(),
                self.config().rms_norm_eps,
            );

            let mut partial_for_attn = match current_partial.clone() {
                Some(partial) => partial,
                None => AttnResTensor3::zeros(attn_input.shape()),
            };
            let starts_new_block_before = self
                .config()
                .starts_new_block_before_sublayer(attn_sublayer_index)
                .map_err(AttnResExecutionError::from_config)?;
            if starts_new_block_before {
                state.completed_blocks.push(partial_for_attn.clone());
                partial_for_attn = AttnResTensor3::zeros(attn_input.shape());
            }
            let attn_output = attention_sublayer(
                layer,
                &attn_input,
                self.config().num_heads,
                self.config()
                    .head_dim()
                    .map_err(AttnResExecutionError::from_config)?,
                self.config().rms_norm_eps,
            );
            let partial_after_attn = tensor_add(&partial_for_attn, &attn_output);
            diagnostics.sublayers.push(AttnResSublayerSnapshot {
                sublayer_index: attn_sublayer_index,
                transformer_layer_index: layer_idx,
                kind: AttnResSublayerKind::Attention,
                starts_new_block_before,
                completed_blocks_before,
                completed_blocks_after: state.completed_blocks.len(),
                partial_block_present_before: current_partial.is_some(),
                partial_block_present_after: true,
                source_shape: [
                    attn_routing.source_count,
                    embeddings.batch_size(),
                    embeddings.sequence_length(),
                ],
                source_logits: attn_routing.logits,
                routing_weights: attn_routing.routing_weights,
                query_norm: attn_routing.query_norm,
            });

            let mlp_sublayer_index = attn_sublayer_index + 1;
            let completed_blocks_before = state.completed_blocks.len();
            let (mlp_input, mlp_routing) = attn_res_forward(
                &layer.mlp_res,
                &state.completed_blocks,
                Some(&partial_after_attn),
                self.config().rms_norm_eps,
            );
            let mut partial_for_mlp = partial_after_attn;
            let starts_new_block_before = self
                .config()
                .starts_new_block_before_sublayer(mlp_sublayer_index)
                .map_err(AttnResExecutionError::from_config)?;
            if starts_new_block_before {
                state.completed_blocks.push(partial_for_mlp.clone());
                partial_for_mlp = AttnResTensor3::zeros(mlp_input.shape());
            }
            let mlp_output = feed_forward_sublayer(layer, &mlp_input, self.config().rms_norm_eps);
            let partial_after_mlp = tensor_add(&partial_for_mlp, &mlp_output);
            diagnostics.sublayers.push(AttnResSublayerSnapshot {
                sublayer_index: mlp_sublayer_index,
                transformer_layer_index: layer_idx,
                kind: AttnResSublayerKind::FeedForward,
                starts_new_block_before,
                completed_blocks_before,
                completed_blocks_after: state.completed_blocks.len(),
                partial_block_present_before: true,
                partial_block_present_after: true,
                source_shape: [
                    mlp_routing.source_count,
                    embeddings.batch_size(),
                    embeddings.sequence_length(),
                ],
                source_logits: mlp_routing.logits,
                routing_weights: mlp_routing.routing_weights,
                query_norm: mlp_routing.query_norm,
            });

            state.partial_block = Some(partial_after_mlp);
        }

        let final_partial = match state.partial_block {
            Some(partial) => partial,
            None => return Err(AttnResExecutionError::MissingFinalPartialBlock),
        };
        let hidden = rms_norm(
            &final_partial,
            &self.weights.final_norm_gamma,
            self.config().rms_norm_eps,
        );
        diagnostics.final_completed_blocks = state.completed_blocks.len();
        diagnostics.final_partial_block_present = true;
        Ok((hidden, diagnostics))
    }

    /// Runs the standard CPU-reference forward pass and returns logits.
    pub fn forward(
        &self,
        inputs: &[TokenSequence],
    ) -> Result<AttnResTensor3, AttnResExecutionError> {
        let hidden = self.forward_hidden(inputs)?;
        Ok(linear_forward(&self.weights.lm_head, &hidden))
    }

    /// Runs the two-phase CPU-reference forward pass and returns hidden states.
    pub fn forward_two_phase_hidden(
        &self,
        inputs: &[TokenSequence],
    ) -> Result<AttnResTensor3, AttnResExecutionError> {
        let embeddings = self.embed_tokens(inputs)?;
        self.forward_two_phase_hidden_from_embeddings(embeddings)
    }

    /// Runs the two-phase CPU-reference forward pass from precomputed embeddings.
    pub fn forward_two_phase_hidden_from_embeddings(
        &self,
        embeddings: AttnResTensor3,
    ) -> Result<AttnResTensor3, AttnResExecutionError> {
        self.validate_embeddings(&embeddings)?;
        let mut completed_blocks = vec![embeddings];
        let mut current_block: Option<AttnResTensor3> = None;
        let block_size = self
            .config()
            .block_size()
            .map_err(AttnResExecutionError::from_config)?;
        let total_sublayers = self.config().num_layers;
        let head_dim = self
            .config()
            .head_dim()
            .map_err(AttnResExecutionError::from_config)?;
        let mut block_start = 0usize;

        while block_start < total_sublayers {
            if let Some(previous_block) = current_block.take() {
                completed_blocks.push(previous_block);
            }

            let block_end = (block_start + block_size).min(total_sublayers);
            let mut phase1 = Vec::with_capacity(block_end - block_start);
            for sublayer_idx in block_start..block_end {
                phase1.push(phase1_inter_block(
                    self.op_weights_for_sublayer(sublayer_idx),
                    &completed_blocks,
                    self.config().rms_norm_eps,
                ));
            }

            let mut partial: Option<AttnResTensor3> = None;
            for (offset, sublayer_idx) in (block_start..block_end).enumerate() {
                let h = if offset == 0 {
                    normalize_inter_output(
                        &phase1[offset].unnormalized_output,
                        &phase1[offset].sum_exp,
                    )
                } else {
                    let partial_ref = match partial.as_ref() {
                        Some(partial_ref) => partial_ref,
                        None => return Err(AttnResExecutionError::MissingFinalPartialBlock),
                    };
                    let op = self.op_weights_for_sublayer(sublayer_idx);
                    let intra_logit =
                        compute_intra_logit(op, partial_ref, self.config().rms_norm_eps);
                    online_softmax_merge(
                        &phase1[offset].unnormalized_output,
                        &phase1[offset].max_logits,
                        &phase1[offset].sum_exp,
                        &intra_logit,
                        partial_ref,
                    )
                };
                let sublayer_output = self.execute_sublayer(sublayer_idx, &h, head_dim);
                partial = Some(match partial {
                    Some(current) => tensor_add(&current, &sublayer_output),
                    None => sublayer_output,
                });
            }

            current_block = partial;
            block_start = block_end;
        }

        let final_partial = match current_block {
            Some(partial) => partial,
            None => return Err(AttnResExecutionError::MissingFinalPartialBlock),
        };
        Ok(rms_norm(
            &final_partial,
            &self.weights.final_norm_gamma,
            self.config().rms_norm_eps,
        ))
    }

    /// Runs the two-phase CPU-reference forward pass and returns logits.
    pub fn forward_two_phase(
        &self,
        inputs: &[TokenSequence],
    ) -> Result<AttnResTensor3, AttnResExecutionError> {
        let hidden = self.forward_two_phase_hidden(inputs)?;
        Ok(linear_forward(&self.weights.lm_head, &hidden))
    }

    fn validate_embeddings(
        &self,
        embeddings: &AttnResTensor3,
    ) -> Result<(), AttnResExecutionError> {
        if embeddings.width() != self.config().d_model {
            return Err(AttnResExecutionError::EmbeddingWidthMismatch {
                actual: embeddings.width(),
                expected: self.config().d_model,
            });
        }
        Ok(())
    }

    fn op_weights_for_sublayer(&self, sublayer_idx: usize) -> &AttnResOpWeights {
        let layer = &self.weights.layers[sublayer_idx / 2];
        if sublayer_idx.is_multiple_of(2) {
            &layer.attn_res
        } else {
            &layer.mlp_res
        }
    }

    fn execute_sublayer(
        &self,
        sublayer_idx: usize,
        input: &AttnResTensor3,
        head_dim: usize,
    ) -> AttnResTensor3 {
        let layer = &self.weights.layers[sublayer_idx / 2];
        if sublayer_idx.is_multiple_of(2) {
            attention_sublayer(
                layer,
                input,
                self.config().num_heads,
                head_dim,
                self.config().rms_norm_eps,
            )
        } else {
            feed_forward_sublayer(layer, input, self.config().rms_norm_eps)
        }
    }
}

#[derive(Clone, Debug)]
struct RoutingData {
    source_count: usize,
    logits: Vec<f32>,
    routing_weights: Vec<f32>,
    query_norm: f32,
}

#[derive(Clone, Debug)]
struct Phase1InterResult {
    unnormalized_output: AttnResTensor3,
    max_logits: Vec<f32>,
    sum_exp: Vec<f32>,
}

fn attention_sublayer(
    layer: &AttnResLayerWeights,
    input: &AttnResTensor3,
    num_heads: usize,
    head_dim: usize,
    eps: f32,
) -> AttnResTensor3 {
    let normed = rms_norm(input, &layer.attn_norm_gamma, eps);
    attention_forward(&layer.attention, &normed, num_heads, head_dim)
}

fn feed_forward_sublayer(
    layer: &AttnResLayerWeights,
    input: &AttnResTensor3,
    eps: f32,
) -> AttnResTensor3 {
    let normed = rms_norm(input, &layer.mlp_norm_gamma, eps);
    feed_forward_forward(&layer.feed_forward, &normed)
}

fn rms_norm(input: &AttnResTensor3, gamma: &[f32], eps: f32) -> AttnResTensor3 {
    let mut output = AttnResTensor3::zeros(input.shape());
    for batch in 0..input.batch_size() {
        for position in 0..input.sequence_length() {
            let mut mean_square = 0.0f32;
            for feature in 0..input.width() {
                let value = input.get(batch, position, feature);
                mean_square += value * value;
            }
            mean_square /= input.width() as f32;
            let scale = (mean_square + eps).sqrt();
            for (feature, gamma_value) in gamma.iter().enumerate() {
                let value = input.get(batch, position, feature) / scale * *gamma_value;
                output.set(batch, position, feature, value);
            }
        }
    }
    output
}

fn linear_forward(linear: &AttnResLinearWeights, input: &AttnResTensor3) -> AttnResTensor3 {
    let mut output = AttnResTensor3::zeros([
        input.batch_size(),
        input.sequence_length(),
        linear.output_dim,
    ]);
    for batch in 0..input.batch_size() {
        for position in 0..input.sequence_length() {
            for output_feature in 0..linear.output_dim {
                let mut value = linear.bias[output_feature];
                for input_feature in 0..linear.input_dim {
                    let weight_index = input_feature * linear.output_dim + output_feature;
                    value +=
                        input.get(batch, position, input_feature) * linear.weight[weight_index];
                }
                output.set(batch, position, output_feature, value);
            }
        }
    }
    output
}

fn attention_forward(
    weights: &AttnResAttentionWeights,
    input: &AttnResTensor3,
    num_heads: usize,
    head_dim: usize,
) -> AttnResTensor3 {
    let q = linear_forward(&weights.q_proj, input);
    let k = linear_forward(&weights.k_proj, input);
    let v = linear_forward(&weights.v_proj, input);
    let mut merged = AttnResTensor3::zeros(input.shape());

    for batch in 0..input.batch_size() {
        for head in 0..num_heads {
            let feature_start = head * head_dim;
            for target_position in 0..input.sequence_length() {
                let mut scores = vec![f32::NEG_INFINITY; input.sequence_length()];
                for (source_position, score) in
                    scores.iter_mut().enumerate().take(target_position + 1)
                {
                    let mut dot = 0.0f32;
                    for offset in 0..head_dim {
                        let feature = feature_start + offset;
                        dot += q.get(batch, target_position, feature)
                            * k.get(batch, source_position, feature);
                    }
                    *score = dot / (head_dim as f32).sqrt();
                }
                let routing = softmax(&scores);
                for offset in 0..head_dim {
                    let feature = feature_start + offset;
                    let mut value = 0.0f32;
                    for (source_position, routing_weight) in routing.iter().enumerate() {
                        value += routing_weight * v.get(batch, source_position, feature);
                    }
                    merged.set(batch, target_position, feature, value);
                }
            }
        }
    }

    linear_forward(&weights.o_proj, &merged)
}

fn feed_forward_forward(
    weights: &AttnResFeedForwardWeights,
    input: &AttnResTensor3,
) -> AttnResTensor3 {
    let hidden = linear_forward(&weights.linear1, input);
    let activated = map_tensor(&hidden, gelu);
    linear_forward(&weights.linear2, &activated)
}

fn attn_res_forward(
    weights: &AttnResOpWeights,
    blocks: &[AttnResTensor3],
    partial_block: Option<&AttnResTensor3>,
    eps: f32,
) -> (AttnResTensor3, RoutingData) {
    let mut sources = blocks.to_vec();
    if let Some(partial_block) = partial_block {
        sources.push(partial_block.clone());
    }

    let batch_size = sources[0].batch_size();
    let sequence_length = sources[0].sequence_length();
    let hidden_size = sources[0].width();
    let source_count = sources.len();
    let mut logits = vec![0.0f32; source_count * batch_size * sequence_length];

    for (source_index, source) in sources.iter().enumerate() {
        let normed = rms_norm(source, &weights.norm_gamma, eps);
        for batch in 0..batch_size {
            for position in 0..sequence_length {
                let mut logit = 0.0f32;
                for feature in 0..hidden_size {
                    logit += normed.get(batch, position, feature) * weights.pseudo_query[feature];
                }
                logits[(source_index * batch_size + batch) * sequence_length + position] = logit;
            }
        }
    }

    let routing_weights = source_softmax(&logits, source_count, batch_size, sequence_length);
    let mut output = AttnResTensor3::zeros(sources[0].shape());
    for (source_index, source) in sources.iter().enumerate() {
        for batch in 0..batch_size {
            for position in 0..sequence_length {
                let routing_weight = routing_weights
                    [(source_index * batch_size + batch) * sequence_length + position];
                for feature in 0..hidden_size {
                    let index = output.index(batch, position, feature);
                    output.values[index] += routing_weight * source.get(batch, position, feature);
                }
            }
        }
    }

    (
        output,
        RoutingData {
            source_count,
            logits,
            routing_weights,
            query_norm: l2_norm(&weights.pseudo_query),
        },
    )
}

fn phase1_inter_block(
    weights: &AttnResOpWeights,
    blocks: &[AttnResTensor3],
    eps: f32,
) -> Phase1InterResult {
    let batch_size = blocks[0].batch_size();
    let sequence_length = blocks[0].sequence_length();
    let hidden_size = blocks[0].width();
    let source_count = blocks.len();
    let mut logits = vec![0.0f32; source_count * batch_size * sequence_length];

    for (source_index, block) in blocks.iter().enumerate() {
        let normed = rms_norm(block, &weights.norm_gamma, eps);
        for batch in 0..batch_size {
            for position in 0..sequence_length {
                let mut logit = 0.0f32;
                for feature in 0..hidden_size {
                    logit += normed.get(batch, position, feature) * weights.pseudo_query[feature];
                }
                logits[(source_index * batch_size + batch) * sequence_length + position] = logit;
            }
        }
    }

    let mut max_logits = vec![f32::NEG_INFINITY; batch_size * sequence_length];
    for batch in 0..batch_size {
        for position in 0..sequence_length {
            let mut max_value = f32::NEG_INFINITY;
            for source_index in 0..source_count {
                let index = (source_index * batch_size + batch) * sequence_length + position;
                max_value = max_value.max(logits[index]);
            }
            max_logits[batch * sequence_length + position] = max_value;
        }
    }

    let mut sum_exp = vec![0.0f32; batch_size * sequence_length];
    let mut output = AttnResTensor3::zeros(blocks[0].shape());
    for (source_index, block) in blocks.iter().enumerate() {
        for batch in 0..batch_size {
            for position in 0..sequence_length {
                let flat_index = (source_index * batch_size + batch) * sequence_length + position;
                let max_index = batch * sequence_length + position;
                let weight = (logits[flat_index] - max_logits[max_index]).exp();
                sum_exp[max_index] += weight;
                for feature in 0..hidden_size {
                    let output_index = output.index(batch, position, feature);
                    output.values[output_index] += weight * block.get(batch, position, feature);
                }
            }
        }
    }

    Phase1InterResult {
        unnormalized_output: output,
        max_logits,
        sum_exp,
    }
}

fn compute_intra_logit(weights: &AttnResOpWeights, partial: &AttnResTensor3, eps: f32) -> Vec<f32> {
    let normed = rms_norm(partial, &weights.norm_gamma, eps);
    let mut logits = vec![0.0f32; partial.batch_size() * partial.sequence_length()];
    for batch in 0..partial.batch_size() {
        for position in 0..partial.sequence_length() {
            let mut logit = 0.0f32;
            for feature in 0..partial.width() {
                logit += normed.get(batch, position, feature) * weights.pseudo_query[feature];
            }
            logits[batch * partial.sequence_length() + position] = logit;
        }
    }
    logits
}

fn normalize_inter_output(inter_output: &AttnResTensor3, sum_exp: &[f32]) -> AttnResTensor3 {
    let mut output = AttnResTensor3::zeros(inter_output.shape());
    for batch in 0..inter_output.batch_size() {
        for position in 0..inter_output.sequence_length() {
            let scale = sum_exp[batch * inter_output.sequence_length() + position];
            for feature in 0..inter_output.width() {
                output.set(
                    batch,
                    position,
                    feature,
                    inter_output.get(batch, position, feature) / scale,
                );
            }
        }
    }
    output
}

fn online_softmax_merge(
    inter_output: &AttnResTensor3,
    inter_max: &[f32],
    inter_sum_exp: &[f32],
    intra_logit: &[f32],
    intra_value: &AttnResTensor3,
) -> AttnResTensor3 {
    let mut output = AttnResTensor3::zeros(inter_output.shape());
    for batch in 0..inter_output.batch_size() {
        for position in 0..inter_output.sequence_length() {
            let index = batch * inter_output.sequence_length() + position;
            let merged_max = inter_max[index].max(intra_logit[index]);
            let inter_scale = (inter_max[index] - merged_max).exp();
            let intra_scale = (intra_logit[index] - merged_max).exp();
            let denom = inter_sum_exp[index] * inter_scale + intra_scale;
            for feature in 0..inter_output.width() {
                let inter_term = inter_output.get(batch, position, feature) * inter_scale;
                let intra_term = intra_value.get(batch, position, feature) * intra_scale;
                output.set(batch, position, feature, (inter_term + intra_term) / denom);
            }
        }
    }
    output
}

fn tensor_add(left: &AttnResTensor3, right: &AttnResTensor3) -> AttnResTensor3 {
    let mut output = AttnResTensor3::zeros(left.shape());
    for (index, value) in output.values.iter_mut().enumerate() {
        *value = left.values[index] + right.values[index];
    }
    output
}

fn map_tensor(input: &AttnResTensor3, f: fn(f32) -> f32) -> AttnResTensor3 {
    let mut output = AttnResTensor3::zeros(input.shape());
    for (index, value) in input.values.iter().enumerate() {
        output.values[index] = f(*value);
    }
    output
}

fn gelu(value: f32) -> f32 {
    let cubic = value * value * value;
    let inner = 0.797_884_6 * (value + 0.044_715 * cubic);
    0.5 * value * (1.0 + inner.tanh())
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn softmax(values: &[f32]) -> Vec<f32> {
    let max_value = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut exps = values
        .iter()
        .map(|value| {
            if value.is_finite() {
                (value - max_value).exp()
            } else {
                0.0
            }
        })
        .collect::<Vec<_>>();
    let sum = exps.iter().sum::<f32>();
    for value in &mut exps {
        *value /= sum;
    }
    exps
}

fn source_softmax(
    logits: &[f32],
    source_count: usize,
    batch_size: usize,
    sequence_length: usize,
) -> Vec<f32> {
    let mut routing_weights = vec![0.0f32; logits.len()];
    for batch in 0..batch_size {
        for position in 0..sequence_length {
            let mut local = Vec::with_capacity(source_count);
            for source_index in 0..source_count {
                let index = (source_index * batch_size + batch) * sequence_length + position;
                local.push(logits[index]);
            }
            let local_weights = softmax(&local);
            for (source_index, routing_weight) in local_weights.iter().enumerate() {
                let index = (source_index * batch_size + batch) * sequence_length + position;
                routing_weights[index] = *routing_weight;
            }
        }
    }
    routing_weights
}

fn seeded_values(label: &str, len: usize, scale: f32) -> Vec<f32> {
    (0..len)
        .map(|index| {
            let mut hasher = Sha256::new();
            hasher.update(b"psionic_attnres_seed|");
            hasher.update(label.as_bytes());
            hasher.update(index.to_le_bytes());
            let bytes = hasher.finalize();
            let sample = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let centered = (sample as f32 / u32::MAX as f32) * 2.0 - 1.0;
            centered * scale
        })
        .collect()
}

fn linear_parameter_vector(
    parameter_id: String,
    input_dim: usize,
    output_dim: usize,
    values: &[f32],
) -> AttnResParameterVector {
    AttnResParameterVector {
        parameter_id,
        shape: Shape::new(vec![input_dim, output_dim]),
        values: values.to_vec(),
    }
}

fn apply_parameter_override(
    bundle: &mut AttnResWeightBundle,
    parameter_id: &str,
    values: &[f32],
) -> Result<(), AttnResModelError> {
    if parameter_id == "token_embeddings" {
        assign_parameter_values(&mut bundle.token_embeddings, parameter_id, values)?;
        return Ok(());
    }
    if parameter_id == "final_norm_gamma" {
        assign_parameter_values(&mut bundle.final_norm_gamma, parameter_id, values)?;
        return Ok(());
    }
    if let Some(tail) = parameter_id.strip_prefix("lm_head.") {
        apply_linear_override(&mut bundle.lm_head, parameter_id, tail, values)?;
        return Ok(());
    }
    if let Some(rest) = parameter_id.strip_prefix("layers.") {
        let Some((layer_str, tail)) = rest.split_once('.') else {
            return Err(AttnResModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Ok(layer_idx) = layer_str.parse::<usize>() else {
            return Err(AttnResModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        let Some(layer) = bundle.layers.get_mut(layer_idx) else {
            return Err(AttnResModelError::UnknownParameter {
                parameter_id: String::from(parameter_id),
            });
        };
        match tail {
            "attn_res.pseudo_query" => {
                assign_parameter_values(&mut layer.attn_res.pseudo_query, parameter_id, values)?
            }
            "attn_res.norm_gamma" => {
                assign_parameter_values(&mut layer.attn_res.norm_gamma, parameter_id, values)?
            }
            "mlp_res.pseudo_query" => {
                assign_parameter_values(&mut layer.mlp_res.pseudo_query, parameter_id, values)?
            }
            "mlp_res.norm_gamma" => {
                assign_parameter_values(&mut layer.mlp_res.norm_gamma, parameter_id, values)?
            }
            "attn_norm_gamma" => {
                assign_parameter_values(&mut layer.attn_norm_gamma, parameter_id, values)?
            }
            "mlp_norm_gamma" => {
                assign_parameter_values(&mut layer.mlp_norm_gamma, parameter_id, values)?
            }
            tail if tail.starts_with("attention.q_proj.") => apply_linear_override(
                &mut layer.attention.q_proj,
                parameter_id,
                tail.trim_start_matches("attention.q_proj."),
                values,
            )?,
            tail if tail.starts_with("attention.k_proj.") => apply_linear_override(
                &mut layer.attention.k_proj,
                parameter_id,
                tail.trim_start_matches("attention.k_proj."),
                values,
            )?,
            tail if tail.starts_with("attention.v_proj.") => apply_linear_override(
                &mut layer.attention.v_proj,
                parameter_id,
                tail.trim_start_matches("attention.v_proj."),
                values,
            )?,
            tail if tail.starts_with("attention.o_proj.") => apply_linear_override(
                &mut layer.attention.o_proj,
                parameter_id,
                tail.trim_start_matches("attention.o_proj."),
                values,
            )?,
            tail if tail.starts_with("feed_forward.linear1.") => apply_linear_override(
                &mut layer.feed_forward.linear1,
                parameter_id,
                tail.trim_start_matches("feed_forward.linear1."),
                values,
            )?,
            tail if tail.starts_with("feed_forward.linear2.") => apply_linear_override(
                &mut layer.feed_forward.linear2,
                parameter_id,
                tail.trim_start_matches("feed_forward.linear2."),
                values,
            )?,
            _ => {
                return Err(AttnResModelError::UnknownParameter {
                    parameter_id: String::from(parameter_id),
                });
            }
        }
        return Ok(());
    }
    Err(AttnResModelError::UnknownParameter {
        parameter_id: String::from(parameter_id),
    })
}

fn apply_linear_override(
    linear: &mut AttnResLinearWeights,
    parameter_id: &str,
    tail: &str,
    values: &[f32],
) -> Result<(), AttnResModelError> {
    match tail {
        "weight" => assign_parameter_values(&mut linear.weight, parameter_id, values),
        "bias" => assign_parameter_values(&mut linear.bias, parameter_id, values),
        _ => Err(AttnResModelError::UnknownParameter {
            parameter_id: String::from(parameter_id),
        }),
    }
}

fn assign_parameter_values(
    target: &mut Vec<f32>,
    parameter_id: &str,
    values: &[f32],
) -> Result<(), AttnResModelError> {
    validate_vector_length(parameter_id, values, target.len())?;
    target.clear();
    target.extend_from_slice(values);
    Ok(())
}

fn validate_vector_length(
    name: impl Into<String>,
    values: &[f32],
    expected: usize,
) -> Result<(), AttnResModelError> {
    if values.len() != expected {
        return Err(AttnResModelError::InvalidVectorLength {
            name: name.into(),
            actual: values.len(),
            expected,
        });
    }
    Ok(())
}

fn build_metadata(entries: &[(WeightTensorMetadata, &[f32])]) -> WeightBundleMetadata {
    let mut ordered = entries.to_vec();
    ordered.sort_by(|(left, _), (right, _)| left.name.cmp(&right.name));

    let mut hasher = Sha256::new();
    for (metadata, values) in &ordered {
        digest_tensor_values(&mut hasher, metadata, values);
    }

    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source: WeightSource::Fixture,
        quantization: QuantizationMode::None,
        quantization_modes: Vec::new(),
        digest: hex::encode(hasher.finalize()),
        tensors: ordered
            .iter()
            .map(|(metadata, _)| metadata.clone())
            .collect(),
        artifacts: Vec::new(),
    }
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
    use std::collections::BTreeMap;
    use std::error::Error;

    use psionic_runtime::{
        AttnResTwoPhaseParityBudget, AttnResTwoPhaseParityStatus,
        compare_attnres_hidden_two_phase_parity, compare_attnres_logit_two_phase_parity,
    };
    use serde::Deserialize;

    use crate::{TokenId, TokenSequence};

    use super::{
        ATTN_RES_MODEL_FAMILY, AttnResConfig, AttnResCpuReferenceModel, AttnResExecutionError,
        AttnResModelError, AttnResOpWeights, AttnResSublayerKind, AttnResTensor3, attn_res_forward,
    };

    #[derive(Debug, Deserialize)]
    struct ForwardFixture {
        test_cases: Vec<ForwardFixtureCase>,
    }

    #[derive(Debug, Deserialize)]
    struct ForwardFixtureCase {
        d_model: usize,
        blocks: Vec<Vec<Vec<f32>>>,
        partial: Vec<Vec<f32>>,
        expected_output: Vec<Vec<f32>>,
    }

    #[derive(Debug, Deserialize)]
    struct BlockStateFixture {
        test_cases: Vec<BlockStateFixtureCase>,
    }

    #[derive(Debug, Deserialize)]
    struct BlockStateFixtureCase {
        num_layers: usize,
        num_blocks: usize,
        expected_boundaries: Vec<usize>,
    }

    #[test]
    fn seeded_model_descriptor_uses_attnres_family() -> Result<(), Box<dyn Error>> {
        let model = AttnResCpuReferenceModel::seeded(
            "attnres-fixture",
            "v0",
            AttnResConfig::new(8, 4, 2)
                .with_num_heads(2)
                .with_vocab_size(32),
        )?;
        assert_eq!(model.descriptor().model.family, ATTN_RES_MODEL_FAMILY);
        assert!(!model.descriptor().stable_digest().is_empty());
        assert_eq!(
            model.weights().metadata().format,
            crate::WeightFormat::ProgrammaticFixture
        );
        Ok(())
    }

    #[test]
    fn weight_bundle_parameter_vectors_match_metadata_inventory() -> Result<(), Box<dyn Error>> {
        let model = AttnResCpuReferenceModel::seeded(
            "attnres-parameter-map",
            "v0",
            AttnResConfig::new(8, 4, 2)
                .with_num_heads(2)
                .with_vocab_size(16),
        )?;
        let parameters = model.weights().parameter_vectors();
        assert_eq!(parameters.len(), model.weights().metadata().tensors.len());
        assert!(
            parameters
                .iter()
                .any(|parameter| parameter.parameter_id == "lm_head.bias")
        );
        assert!(
            parameters
                .iter()
                .any(|parameter| { parameter.parameter_id == "layers.0.attn_res.pseudo_query" })
        );
        Ok(())
    }

    #[test]
    fn weight_bundle_parameter_overrides_rebuild_metadata() -> Result<(), Box<dyn Error>> {
        let model = AttnResCpuReferenceModel::seeded(
            "attnres-overrides",
            "v0",
            AttnResConfig::new(8, 4, 2)
                .with_num_heads(2)
                .with_vocab_size(16),
        )?;
        let baseline_digest = model.weights().metadata().digest.clone();
        let baseline_bias = model
            .weights()
            .parameter_vector("lm_head.bias")
            .expect("lm_head.bias");
        let overridden = model.weights().with_parameter_overrides(
            model.config(),
            &BTreeMap::from([
                (
                    String::from("layers.0.attn_res.pseudo_query"),
                    vec![0.25; model.config().d_model],
                ),
                (
                    String::from("lm_head.bias"),
                    baseline_bias
                        .values
                        .iter()
                        .map(|value| value + 0.5)
                        .collect::<Vec<_>>(),
                ),
            ]),
        )?;
        assert_ne!(overridden.metadata().digest, baseline_digest);
        assert_eq!(
            overridden
                .parameter_vector("layers.0.attn_res.pseudo_query")
                .expect("routing override")
                .values,
            vec![0.25; model.config().d_model]
        );
        Ok(())
    }

    #[test]
    fn weight_bundle_unknown_override_is_rejected() -> Result<(), Box<dyn Error>> {
        let model = AttnResCpuReferenceModel::seeded(
            "attnres-bad-override",
            "v0",
            AttnResConfig::new(8, 4, 2)
                .with_num_heads(2)
                .with_vocab_size(16),
        )?;
        let error = model
            .weights()
            .with_parameter_overrides(
                model.config(),
                &BTreeMap::from([(
                    String::from("layers.99.attn_res.pseudo_query"),
                    vec![0.0; model.config().d_model],
                )]),
            )
            .expect_err("unknown parameter");
        assert!(matches!(error, AttnResModelError::UnknownParameter { .. }));
        Ok(())
    }

    #[test]
    fn weight_bundle_artifact_metadata_can_be_retagged() -> Result<(), Box<dyn Error>> {
        let model = AttnResCpuReferenceModel::seeded(
            "attnres-artifact",
            "v0",
            AttnResConfig::new(8, 4, 2)
                .with_num_heads(2)
                .with_vocab_size(16),
        )?;
        let retagged = model.weights().with_artifact_metadata(
            crate::WeightFormat::SafeTensors,
            crate::WeightSource::ExternalArtifact,
            vec![crate::WeightArtifactMetadata::new(
                "weights.safetensors",
                128,
                "deadbeef",
            )],
        );
        assert_eq!(retagged.metadata().format, crate::WeightFormat::SafeTensors);
        assert_eq!(
            retagged.metadata().source,
            crate::WeightSource::ExternalArtifact
        );
        assert_eq!(retagged.metadata().artifacts.len(), 1);
        assert_eq!(
            retagged.metadata().digest,
            model.weights().metadata().digest
        );
        Ok(())
    }

    #[test]
    fn zero_query_reference_fixture_means_all_sources() -> Result<(), Box<dyn Error>> {
        let fixture: ForwardFixture = serde_json::from_str(include_str!(
            "../../../fixtures/attnres/attn_res_forward.json"
        ))?;
        let case = &fixture.test_cases[0];
        let blocks = case
            .blocks
            .iter()
            .map(|block| AttnResTensor3::from_nested(vec![block.clone()]))
            .collect::<Result<Vec<_>, _>>()?;
        let partial = AttnResTensor3::from_nested(vec![case.partial.clone()])?;
        let expected = AttnResTensor3::from_nested(vec![case.expected_output.clone()])?;
        let weights = AttnResOpWeights::new(case.d_model);

        let (actual, routing) = attn_res_forward(&weights, &blocks, Some(&partial), 1e-6);
        assert!(routing.query_norm.abs() < 1e-8);
        assert!(actual.max_abs_diff(&expected)? < 1e-6);
        Ok(())
    }

    #[test]
    fn single_completed_block_without_partial_is_identity() -> Result<(), Box<dyn Error>> {
        let source = AttnResTensor3::from_nested(vec![vec![vec![1.0, 2.0, 3.0, 4.0]]])?;
        let weights = AttnResOpWeights::new(4);
        let (actual, _) = attn_res_forward(&weights, std::slice::from_ref(&source), None, 1e-6);
        assert!(actual.max_abs_diff(&source)? < 1e-6);
        Ok(())
    }

    #[test]
    fn routing_weights_sum_to_one_over_depth() -> Result<(), Box<dyn Error>> {
        let blocks = vec![
            AttnResTensor3::from_nested(vec![vec![
                vec![1.0, 2.0, 3.0, 4.0],
                vec![2.0, 3.0, 4.0, 5.0],
            ]])?,
            AttnResTensor3::from_nested(vec![vec![
                vec![4.0, 3.0, 2.0, 1.0],
                vec![5.0, 4.0, 3.0, 2.0],
            ]])?,
        ];
        let partial = AttnResTensor3::from_nested(vec![vec![
            vec![0.5, 0.25, -0.25, -0.5],
            vec![0.25, 0.5, -0.5, -0.25],
        ]])?;
        let mut weights = AttnResOpWeights::new(4);
        weights.pseudo_query = vec![0.25, -0.5, 0.75, -1.0];

        let (_, routing) = attn_res_forward(&weights, &blocks, Some(&partial), 1e-6);
        for batch in 0..1 {
            for position in 0..2 {
                let mut sum = 0.0f32;
                for source_index in 0..routing.source_count {
                    let index = (source_index + batch * routing.source_count) * 2 + position;
                    sum += routing.routing_weights[index];
                }
                assert!((sum - 1.0).abs() < 1.0e-6);
            }
        }
        Ok(())
    }

    #[test]
    fn block_boundary_fixture_matches_attn_sublayer_boundaries() -> Result<(), Box<dyn Error>> {
        let fixture: BlockStateFixture = serde_json::from_str(include_str!(
            "../../../fixtures/attnres/block_state_tracking.json"
        ))?;
        let case = &fixture.test_cases[0];
        let config = AttnResConfig::new(32, case.num_layers, case.num_blocks)
            .with_num_heads(4)
            .with_vocab_size(128);
        let actual = config.boundary_transformer_layers()?;
        assert_eq!(actual, case.expected_boundaries);
        Ok(())
    }

    #[test]
    fn odd_block_size_surfaces_boundary_before_mlp() -> Result<(), Box<dyn Error>> {
        let config = AttnResConfig::new(12, 6, 2)
            .with_num_heads(3)
            .with_vocab_size(32);
        let model = AttnResCpuReferenceModel::seeded("attnres-odd-block", "v0", config)?;
        let embeddings = AttnResTensor3::from_nested(vec![vec![
            vec![
                0.5, 0.1, -0.1, 0.2, 0.0, -0.2, 0.3, -0.3, 0.4, 0.2, 0.1, -0.4,
            ],
            vec![
                0.6, -0.2, 0.2, 0.1, 0.2, -0.1, 0.4, -0.2, 0.3, 0.0, 0.2, -0.3,
            ],
        ]])?;
        let (_, diagnostics) = model.forward_hidden_from_embeddings_with_diagnostics(embeddings)?;
        let boundary = diagnostics
            .sublayers
            .iter()
            .find(|snapshot| snapshot.sublayer_index == 3)
            .ok_or("missing sublayer 3 diagnostic")?;
        assert_eq!(boundary.kind, AttnResSublayerKind::FeedForward);
        assert!(boundary.starts_new_block_before);
        Ok(())
    }

    #[test]
    fn full_attnres_splits_every_sublayer_after_the_first() -> Result<(), Box<dyn Error>> {
        let config = AttnResConfig::new(8, 4, 4)
            .with_num_heads(2)
            .with_vocab_size(32);
        let model = AttnResCpuReferenceModel::seeded("attnres-full", "v0", config)?;
        let embeddings = AttnResTensor3::from_nested(vec![vec![
            vec![0.1, 0.2, 0.3, 0.4, -0.1, -0.2, -0.3, -0.4],
            vec![0.2, 0.3, 0.4, 0.5, -0.2, -0.3, -0.4, -0.5],
        ]])?;
        let (_, diagnostics) = model.forward_hidden_from_embeddings_with_diagnostics(embeddings)?;
        let boundary_indices = diagnostics
            .sublayers
            .iter()
            .filter(|snapshot| snapshot.starts_new_block_before)
            .map(|snapshot| snapshot.sublayer_index)
            .collect::<Vec<_>>();
        assert_eq!(boundary_indices, vec![1, 2, 3]);
        Ok(())
    }

    #[test]
    fn two_phase_hidden_matches_standard_hidden() -> Result<(), Box<dyn Error>> {
        let config = AttnResConfig::new(16, 8, 2)
            .with_num_heads(4)
            .with_vocab_size(64);
        let model = AttnResCpuReferenceModel::seeded("attnres-two-phase", "v0", config)?;
        let batch = vec![
            TokenSequence::new(vec![TokenId(1), TokenId(2), TokenId(3), TokenId(4)]),
            TokenSequence::new(vec![TokenId(4), TokenId(3), TokenId(2), TokenId(1)]),
        ];
        let standard = model.forward_hidden(&batch)?;
        let two_phase = model.forward_two_phase_hidden(&batch)?;
        let budget = AttnResTwoPhaseParityBudget::default();
        let report = compare_attnres_hidden_two_phase_parity(
            standard.values(),
            two_phase.values(),
            budget.hidden,
        )?;
        assert_ne!(report.status, AttnResTwoPhaseParityStatus::OutsideBudget);
        assert!(report.summary.within_budget);
        Ok(())
    }

    #[test]
    fn two_phase_logits_match_standard_logits() -> Result<(), Box<dyn Error>> {
        let config = AttnResConfig::new(12, 6, 3)
            .with_num_heads(3)
            .with_vocab_size(48);
        let model = AttnResCpuReferenceModel::seeded("attnres-two-phase-logits", "v0", config)?;
        let batch = vec![TokenSequence::new(vec![TokenId(1), TokenId(5), TokenId(7)])];
        let standard = model.forward(&batch)?;
        let two_phase = model.forward_two_phase(&batch)?;
        let budget = AttnResTwoPhaseParityBudget::default();
        let report = compare_attnres_logit_two_phase_parity(
            standard.values(),
            two_phase.values(),
            budget.logits,
        )?;
        assert_ne!(report.status, AttnResTwoPhaseParityStatus::OutsideBudget);
        assert!(report.summary.within_budget);
        Ok(())
    }

    #[test]
    fn diagnostics_expose_block_state_and_routing_shapes() -> Result<(), Box<dyn Error>> {
        let config = AttnResConfig::new(8, 4, 2)
            .with_num_heads(2)
            .with_vocab_size(32);
        let model = AttnResCpuReferenceModel::seeded("attnres-diag", "v0", config)?;
        let batch = vec![TokenSequence::new(vec![TokenId(1), TokenId(2), TokenId(3)])];
        let (_, diagnostics) = model.forward_hidden_with_diagnostics(&batch)?;

        assert_eq!(diagnostics.sublayers.len(), 4);
        let first = &diagnostics.sublayers[0];
        assert_eq!(first.kind, AttnResSublayerKind::Attention);
        assert_eq!(first.source_shape, [1, 1, 3]);
        assert_eq!(first.source_logits.len(), 3);
        assert_eq!(first.routing_weights.len(), 3);
        assert!(diagnostics.final_partial_block_present);
        Ok(())
    }

    #[test]
    fn large_magnitude_sources_remain_finite() -> Result<(), Box<dyn Error>> {
        let block = AttnResTensor3::from_nested(vec![vec![
            vec![1.0e9, -1.0e9, 1.0e-9, -1.0e-9],
            vec![2.0e9, -2.0e9, 2.0e-9, -2.0e-9],
        ]])?;
        let partial = AttnResTensor3::from_nested(vec![vec![
            vec![1.5e9, -1.5e9, 1.5e-9, -1.5e-9],
            vec![2.5e9, -2.5e9, 2.5e-9, -2.5e-9],
        ]])?;
        let mut weights = AttnResOpWeights::new(4);
        weights.pseudo_query = vec![0.1, -0.1, 0.2, -0.2];
        let (output, _) = attn_res_forward(&weights, &[block], Some(&partial), 1e-6);
        assert!(output.values().iter().all(|value| value.is_finite()));
        Ok(())
    }

    #[test]
    fn ragged_batch_is_rejected() -> Result<(), Box<dyn Error>> {
        let config = AttnResConfig::new(8, 4, 2)
            .with_num_heads(2)
            .with_vocab_size(32);
        let model = AttnResCpuReferenceModel::seeded("attnres-ragged", "v0", config)?;
        let error = model.embed_tokens(&[
            TokenSequence::new(vec![TokenId(1), TokenId(2)]),
            TokenSequence::new(vec![TokenId(1)]),
        ]);
        assert!(matches!(
            error,
            Err(AttnResExecutionError::RaggedBatch { .. })
        ));
        Ok(())
    }
}
