//! Reusable transformer architecture primitives for Psionic.

mod attention;
mod blocks;
mod encoder_decoder;
mod qwen36_loss_masks;
mod tassadar_post_article_canonical_computational_model_contract;
mod tassadar_post_article_anti_drift_stability_closeout_contract;
mod tassadar_post_article_canonical_machine_closure_bundle_contract;
mod tassadar_post_article_canonical_machine_identity_lock_contract;
mod tassadar_post_article_continuation_non_computationality_contract;
mod tassadar_post_article_downward_non_influence_and_served_conformance_contract;
mod tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract;
mod tassadar_post_article_execution_semantics_proof_transport_contract;
mod tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract;
mod tassadar_post_article_plugin_result_binding_contract;
mod tassadar_post_article_weighted_plugin_controller_trace_contract;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use attention::*;
pub use blocks::*;
pub use encoder_decoder::*;
pub use qwen36_loss_masks::*;
pub use psionic_nn::{ActivationKind, LayerError, LayerNorm, Linear};
pub use tassadar_post_article_canonical_computational_model_contract::*;
pub use tassadar_post_article_anti_drift_stability_closeout_contract::*;
pub use tassadar_post_article_canonical_machine_closure_bundle_contract::*;
pub use tassadar_post_article_canonical_machine_identity_lock_contract::*;
pub use tassadar_post_article_continuation_non_computationality_contract::*;
pub use tassadar_post_article_downward_non_influence_and_served_conformance_contract::*;
pub use tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract::*;
pub use tassadar_post_article_execution_semantics_proof_transport_contract::*;
pub use tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract::*;
pub use tassadar_post_article_plugin_result_binding_contract::*;
pub use tassadar_post_article_weighted_plugin_controller_trace_contract::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "reusable transformer architecture primitives";

/// Activation function used by a decoder feed-forward block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationFunction {
    /// Identity activation, useful for deterministic fixture paths.
    Identity,
    /// ReLU activation.
    Relu,
    /// SiLU / SwiGLU-style activation used by the first supported decoder families.
    Silu,
}

/// Attention configuration for a decoder block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderAttentionConfig {
    /// Number of query heads.
    pub head_count: usize,
    /// Number of KV heads.
    pub kv_head_count: usize,
    /// Width of each head.
    pub head_dim: usize,
    /// Rotary dimension reserved for future RoPE support.
    pub rotary_dim: usize,
}

/// Feed-forward configuration for a decoder block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderFeedForwardConfig {
    /// Hidden expansion size.
    pub intermediate_size: usize,
    /// Activation used inside the block.
    pub activation: ActivationFunction,
}

/// Reusable decoder block configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderBlockConfig {
    /// Attention sub-block configuration.
    pub attention: DecoderAttentionConfig,
    /// Feed-forward sub-block configuration.
    pub feed_forward: DecoderFeedForwardConfig,
}

/// Decoder-style transformer configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderConfig {
    /// Model hidden width.
    pub hidden_size: usize,
    /// Number of decoder layers.
    pub layer_count: usize,
    /// Vocabulary size.
    pub vocab_size: usize,
    /// Maximum supported context length.
    pub max_context: usize,
    /// Shared block configuration.
    pub block: DecoderBlockConfig,
}

impl DecoderConfig {
    /// Returns the total KV width per position.
    #[must_use]
    pub fn kv_width(&self) -> usize {
        self.block.attention.kv_head_count * self.block.attention.head_dim
    }
}

/// Configuration for one Attention Residual model family instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResConfig {
    /// Hidden width.
    pub d_model: usize,
    /// Total sublayer count. Each transformer layer contains two sublayers.
    pub num_layers: usize,
    /// Number of residual blocks across the sublayer stack.
    pub num_blocks: usize,
    /// Number of attention heads.
    pub num_heads: usize,
    /// Feed-forward expansion width. `0` means `4 * d_model`.
    pub d_ff: usize,
    /// Vocabulary size for token embeddings and logits.
    pub vocab_size: usize,
    /// RMSNorm epsilon for numerical stability.
    pub rms_norm_eps: f32,
    /// Configured dropout rate. The CPU reference path uses inference semantics
    /// and therefore does not apply dropout.
    pub dropout: f32,
}

impl AttnResConfig {
    /// Creates a config with the historical defaults.
    #[must_use]
    pub fn new(d_model: usize, num_layers: usize, num_blocks: usize) -> Self {
        Self {
            d_model,
            num_layers,
            num_blocks,
            num_heads: 8,
            d_ff: 0,
            vocab_size: 32_000,
            rms_norm_eps: 1e-6,
            dropout: 0.0,
        }
    }

    /// Returns a copy with an explicit head count.
    #[must_use]
    pub fn with_num_heads(mut self, num_heads: usize) -> Self {
        self.num_heads = num_heads;
        self
    }

    /// Returns a copy with an explicit feed-forward width.
    #[must_use]
    pub fn with_d_ff(mut self, d_ff: usize) -> Self {
        self.d_ff = d_ff;
        self
    }

    /// Returns a copy with an explicit vocabulary size.
    #[must_use]
    pub fn with_vocab_size(mut self, vocab_size: usize) -> Self {
        self.vocab_size = vocab_size;
        self
    }

    /// Returns a copy with an explicit RMSNorm epsilon.
    #[must_use]
    pub fn with_rms_norm_eps(mut self, rms_norm_eps: f32) -> Self {
        self.rms_norm_eps = rms_norm_eps;
        self
    }

    /// Returns a copy with an explicit dropout rate.
    #[must_use]
    pub fn with_dropout(mut self, dropout: f32) -> Self {
        self.dropout = dropout;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), AttnResConfigError> {
        if self.d_model == 0 {
            return Err(AttnResConfigError::DModelMustBePositive);
        }
        if self.num_layers == 0 {
            return Err(AttnResConfigError::NumLayersMustBePositive);
        }
        if self.num_blocks == 0 {
            return Err(AttnResConfigError::NumBlocksMustBePositive);
        }
        if self.num_heads == 0 {
            return Err(AttnResConfigError::NumHeadsMustBePositive);
        }
        if !self.num_layers.is_multiple_of(2) {
            return Err(AttnResConfigError::NumLayersMustBeEven {
                num_layers: self.num_layers,
            });
        }
        if !self.num_layers.is_multiple_of(self.num_blocks) {
            return Err(AttnResConfigError::NumLayersMustBeDivisibleByNumBlocks {
                num_layers: self.num_layers,
                num_blocks: self.num_blocks,
            });
        }
        if !self.d_model.is_multiple_of(self.num_heads) {
            return Err(AttnResConfigError::DModelMustBeDivisibleByNumHeads {
                d_model: self.d_model,
                num_heads: self.num_heads,
            });
        }
        if self.vocab_size == 0 {
            return Err(AttnResConfigError::VocabSizeMustBePositive);
        }
        if self.rms_norm_eps <= 0.0 {
            return Err(AttnResConfigError::RmsNormEpsMustBePositive {
                rms_norm_eps: self.rms_norm_eps,
            });
        }
        if !(0.0..=1.0).contains(&self.dropout) {
            return Err(AttnResConfigError::DropoutOutOfRange {
                dropout: self.dropout,
            });
        }
        let _ = self.effective_d_ff()?;
        Ok(())
    }

    /// Returns the effective feed-forward width.
    pub fn effective_d_ff(&self) -> Result<usize, AttnResConfigError> {
        if self.d_ff == 0 {
            self.d_model
                .checked_mul(4)
                .ok_or(AttnResConfigError::EffectiveFeedForwardDimOverflow {
                    d_model: self.d_model,
                })
        } else {
            Ok(self.d_ff)
        }
    }

    /// Returns the number of transformer layers.
    #[must_use]
    pub const fn num_transformer_layers(&self) -> usize {
        self.num_layers / 2
    }

    /// Returns the number of sublayers per block.
    pub fn block_size(&self) -> Result<usize, AttnResConfigError> {
        self.validate()?;
        Ok(self.num_layers / self.num_blocks)
    }

    /// Returns the per-head width.
    pub fn head_dim(&self) -> Result<usize, AttnResConfigError> {
        self.validate()?;
        Ok(self.d_model / self.num_heads)
    }

    /// Returns whether the configuration is full Attention Residuals.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.num_blocks == self.num_layers
    }

    /// Validates one transformer-layer index.
    pub fn validate_layer_index(&self, layer_idx: usize) -> Result<(), AttnResConfigError> {
        self.validate()?;
        let num_transformer_layers = self.num_transformer_layers();
        if layer_idx >= num_transformer_layers {
            return Err(AttnResConfigError::LayerIndexOutOfRange {
                layer_idx,
                num_transformer_layers,
            });
        }
        Ok(())
    }

    /// Returns whether a new block starts immediately before one sublayer.
    pub fn starts_new_block_before_sublayer(
        &self,
        sublayer_idx: usize,
    ) -> Result<bool, AttnResConfigError> {
        self.validate()?;
        Ok(sublayer_idx > 0 && sublayer_idx.is_multiple_of(self.block_size()?))
    }

    /// Returns the transformer layers that align with block boundaries.
    pub fn boundary_transformer_layers(&self) -> Result<Vec<usize>, AttnResConfigError> {
        self.validate()?;
        let block_size = self.block_size()?;
        let mut boundaries = Vec::new();
        for sublayer_idx in 0..self.num_layers {
            if sublayer_idx.is_multiple_of(block_size) {
                boundaries.push(sublayer_idx / 2);
            }
        }
        boundaries.dedup();
        Ok(boundaries)
    }
}

/// Attention Residual configuration failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AttnResConfigError {
    /// `d_model` must be strictly positive.
    #[error("d_model must be positive")]
    DModelMustBePositive,
    /// `num_layers` must be strictly positive.
    #[error("num_layers must be positive")]
    NumLayersMustBePositive,
    /// `num_blocks` must be strictly positive.
    #[error("num_blocks must be positive")]
    NumBlocksMustBePositive,
    /// `num_heads` must be strictly positive.
    #[error("num_heads must be positive")]
    NumHeadsMustBePositive,
    /// `num_layers` must be even because each transformer layer contributes two sublayers.
    #[error("num_layers must be even, got {num_layers}")]
    NumLayersMustBeEven {
        /// Invalid sublayer count.
        num_layers: usize,
    },
    /// `num_layers` must divide evenly across `num_blocks`.
    #[error("num_layers ({num_layers}) must be divisible by num_blocks ({num_blocks})")]
    NumLayersMustBeDivisibleByNumBlocks {
        /// Invalid sublayer count.
        num_layers: usize,
        /// Invalid block count.
        num_blocks: usize,
    },
    /// `d_model` must divide evenly across `num_heads`.
    #[error("d_model ({d_model}) must be divisible by num_heads ({num_heads})")]
    DModelMustBeDivisibleByNumHeads {
        /// Invalid model width.
        d_model: usize,
        /// Invalid head count.
        num_heads: usize,
    },
    /// `vocab_size` must be strictly positive.
    #[error("vocab_size must be positive")]
    VocabSizeMustBePositive,
    /// `rms_norm_eps` must be strictly positive.
    #[error("rms_norm_eps must be positive, got {rms_norm_eps}")]
    RmsNormEpsMustBePositive {
        /// Invalid epsilon.
        rms_norm_eps: f32,
    },
    /// `dropout` must be within the inclusive range `[0, 1]`.
    #[error("dropout must be in [0, 1], got {dropout}")]
    DropoutOutOfRange {
        /// Invalid dropout rate.
        dropout: f32,
    },
    /// The implied `4 * d_model` expansion overflowed.
    #[error("4 * d_model overflowed for d_model {d_model}")]
    EffectiveFeedForwardDimOverflow {
        /// Invalid model width.
        d_model: usize,
    },
    /// The requested transformer-layer index exceeded the config.
    #[error(
        "layer index {layer_idx} must be smaller than transformer layer count {num_transformer_layers}"
    )]
    LayerIndexOutOfRange {
        /// Invalid transformer-layer index.
        layer_idx: usize,
        /// Total transformer-layer count.
        num_transformer_layers: usize,
    },
}

/// Tensor construction failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AttnResTensorError {
    /// Flat values did not match the declared shape.
    #[error("invalid value count {actual} for shape {shape:?}; expected {expected}")]
    InvalidValueCount {
        /// Declared shape.
        shape: [usize; 3],
        /// Actual flat value count.
        actual: usize,
        /// Expected flat value count.
        expected: usize,
    },
    /// Nested construction requires at least one batch element.
    #[error("nested tensor input must contain at least one batch element")]
    EmptyBatch,
    /// Nested construction requires at least one sequence position.
    #[error("nested tensor input must contain at least one sequence position")]
    EmptySequence,
    /// Nested construction requires at least one feature per position.
    #[error("nested tensor input must contain at least one feature value")]
    EmptyWidth,
    /// Nested construction saw inconsistent sequence lengths.
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
    /// Nested construction saw inconsistent widths.
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

/// CPU-reference execution failure for architecture-level AttnRes primitives.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AttnResExecutionError {
    /// The caller supplied no sequences.
    #[error("input batch must not be empty")]
    EmptyBatch,
    /// The caller supplied sequences of different lengths.
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
    /// One token exceeded the configured vocabulary size.
    #[error(
        "token {token} at batch {batch_index}, position {position_index} exceeds vocab size {vocab_size}"
    )]
    TokenOutOfRange {
        /// Batch index containing the token.
        batch_index: usize,
        /// Sequence position containing the token.
        position_index: usize,
        /// Invalid token ID.
        token: u32,
        /// Configured vocabulary size.
        vocab_size: usize,
    },
    /// One tensor width did not match the configured hidden size.
    #[error("embedding width {actual} does not match configured d_model {expected}")]
    EmbeddingWidthMismatch {
        /// Actual hidden width.
        actual: usize,
        /// Expected hidden width.
        expected: usize,
    },
    /// Internal state lost the current partial block unexpectedly.
    #[error("forward pass ended without a final partial block")]
    MissingFinalPartialBlock,
    /// A previously validated config surfaced an unexpected internal error.
    #[error("internal config error: {0}")]
    InternalConfig(String),
}

impl AttnResExecutionError {
    /// Rewraps a validated configuration failure that surfaced during execution.
    #[must_use]
    pub fn from_config(error: AttnResConfigError) -> Self {
        Self::InternalConfig(error.to_string())
    }
}

/// Simple CPU-reference 3D tensor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTensor3 {
    /// Public shape because higher wrapper layers still do direct indexed
    /// tensor assembly above this architecture crate.
    pub shape: [usize; 3],
    /// Public flat values for the same wrapper-layer reason.
    pub values: Vec<f32>,
}

impl AttnResTensor3 {
    /// Creates one tensor from a shape and flat values in row-major `[batch, seq, width]` order.
    pub fn new(shape: [usize; 3], values: Vec<f32>) -> Result<Self, AttnResTensorError> {
        let expected = shape.iter().copied().fold(1usize, usize::saturating_mul);
        if values.len() != expected {
            return Err(AttnResTensorError::InvalidValueCount {
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
        let expected = shape.iter().copied().fold(1usize, usize::saturating_mul);
        Self {
            shape,
            values: vec![0.0; expected],
        }
    }

    /// Creates one tensor from nested `[batch][seq][width]` values.
    pub fn from_nested(values: Vec<Vec<Vec<f32>>>) -> Result<Self, AttnResTensorError> {
        if values.is_empty() {
            return Err(AttnResTensorError::EmptyBatch);
        }
        let sequence_length = values.first().map(Vec::len).unwrap_or(0);
        if sequence_length == 0 {
            return Err(AttnResTensorError::EmptySequence);
        }
        let width = values
            .first()
            .and_then(|batch| batch.first())
            .map(Vec::len)
            .unwrap_or(0);
        if width == 0 {
            return Err(AttnResTensorError::EmptyWidth);
        }

        let mut flat = Vec::with_capacity(values.len() * sequence_length * width);
        for (batch_index, batch) in values.iter().enumerate() {
            if batch.len() != sequence_length {
                return Err(AttnResTensorError::RaggedSequence {
                    batch_index,
                    expected: sequence_length,
                    actual: batch.len(),
                });
            }
            for (position_index, row) in batch.iter().enumerate() {
                if row.len() != width {
                    return Err(AttnResTensorError::RaggedWidth {
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

    /// Returns the flat values in row-major order.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Converts the tensor back into nested `[batch][seq][width]` values.
    #[must_use]
    pub fn to_nested(&self) -> Vec<Vec<Vec<f32>>> {
        let mut nested =
            vec![vec![vec![0.0; self.width()]; self.sequence_length()]; self.batch_size()];
        for batch in 0..self.batch_size() {
            for position in 0..self.sequence_length() {
                for feature in 0..self.width() {
                    nested[batch][position][feature] = self.get(batch, position, feature);
                }
            }
        }
        nested
    }

    /// Returns the maximum absolute difference between two tensors of identical shape.
    pub fn max_abs_diff(&self, other: &Self) -> Result<f32, AttnResTensorError> {
        if self.shape != other.shape {
            return Err(AttnResTensorError::ShapeMismatch {
                left: self.shape,
                right: other.shape,
            });
        }
        Ok(self
            .values
            .iter()
            .zip(other.values.iter())
            .map(|(left, right)| (left - right).abs())
            .fold(0.0f32, f32::max))
    }

    /// Returns the flat row-major index for one element.
    #[must_use]
    pub fn index(&self, batch: usize, position: usize, feature: usize) -> usize {
        (batch * self.sequence_length() + position) * self.width() + feature
    }

    /// Returns one element from the tensor.
    #[must_use]
    pub fn get(&self, batch: usize, position: usize, feature: usize) -> f32 {
        self.values[self.index(batch, position, feature)]
    }

    /// Sets one element in the tensor.
    pub fn set(&mut self, batch: usize, position: usize, feature: usize, value: f32) {
        let index = self.index(batch, position, feature);
        self.values[index] = value;
    }
}

/// One block-state snapshot across AttnRes execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResBlockState {
    /// Public while `psionic-models` still owns the higher wrapper that
    /// mutates this execution state directly.
    pub completed_blocks: Vec<AttnResTensor3>,
    /// Public while `psionic-models` still owns the higher wrapper that
    /// mutates this execution state directly.
    pub partial_block: Option<AttnResTensor3>,
}

impl AttnResBlockState {
    /// Creates the initial block state with embeddings as the first completed block.
    #[must_use]
    pub fn new(token_embeddings: AttnResTensor3) -> Self {
        Self {
            completed_blocks: vec![token_embeddings],
            partial_block: None,
        }
    }

    /// Returns the completed blocks in order.
    #[must_use]
    pub fn completed_blocks(&self) -> &[AttnResTensor3] {
        &self.completed_blocks
    }

    /// Returns the current partial block, if one exists.
    #[must_use]
    pub fn partial_block(&self) -> Option<&AttnResTensor3> {
        self.partial_block.as_ref()
    }

    /// Returns the completed-block count.
    #[must_use]
    pub fn completed_block_count(&self) -> usize {
        self.completed_blocks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActivationFunction, AttnResConfig, AttnResConfigError, AttnResTensor3, AttnResTensorError,
        DecoderAttentionConfig, DecoderBlockConfig, DecoderConfig, DecoderFeedForwardConfig,
    };

    #[test]
    fn decoder_config_reports_kv_width() {
        let config = DecoderConfig {
            hidden_size: 32,
            layer_count: 2,
            vocab_size: 128,
            max_context: 64,
            block: DecoderBlockConfig {
                attention: DecoderAttentionConfig {
                    head_count: 8,
                    kv_head_count: 2,
                    head_dim: 4,
                    rotary_dim: 0,
                },
                feed_forward: DecoderFeedForwardConfig {
                    intermediate_size: 64,
                    activation: ActivationFunction::Silu,
                },
            },
        };

        assert_eq!(config.kv_width(), 8);
    }

    #[test]
    fn attnres_config_rejects_non_divisible_heads() {
        let error = AttnResConfig::new(10, 4, 2)
            .with_num_heads(3)
            .validate()
            .expect_err("config should refuse non-divisible head counts");
        assert!(matches!(
            error,
            AttnResConfigError::DModelMustBeDivisibleByNumHeads { .. }
        ));
    }

    #[test]
    fn attnres_tensor_roundtrips_nested_values() -> Result<(), AttnResTensorError> {
        let tensor = AttnResTensor3::from_nested(vec![vec![vec![1.0, 2.0], vec![3.0, 4.0]]])?;
        assert_eq!(tensor.shape(), [1, 2, 2]);
        assert_eq!(
            tensor.to_nested(),
            vec![vec![vec![1.0, 2.0], vec![3.0, 4.0]]]
        );
        Ok(())
    }

    #[test]
    fn attnres_tensor_refuses_ragged_widths() {
        let error = AttnResTensor3::from_nested(vec![vec![vec![1.0], vec![2.0, 3.0]]])
            .expect_err("tensor should refuse ragged widths");
        assert!(matches!(error, AttnResTensorError::RaggedWidth { .. }));
    }
}
