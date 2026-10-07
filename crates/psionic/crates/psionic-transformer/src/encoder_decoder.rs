use psionic_core::Shape;
use psionic_nn::{LayerError, LayerNorm, Linear, ModuleStateError, NnTensor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AttentionMask, AttentionMaskError, AttentionProbabilityTrace, MultiHeadAttention,
    PositionwiseFeedForward, TransformerBlockError, TransformerEmbeddings,
    TransformerExecutionMode,
};

/// Paper-faithful encoder-decoder Transformer configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncoderDecoderTransformerConfig {
    pub source_vocab_size: usize,
    pub target_vocab_size: usize,
    pub hidden_size: usize,
    pub feed_forward_size: usize,
    pub head_count: usize,
    pub encoder_layer_count: usize,
    pub decoder_layer_count: usize,
    pub max_source_positions: usize,
    pub max_target_positions: usize,
    pub dropout_probability_bps: u16,
}

impl EncoderDecoderTransformerConfig {
    /// Returns the dropout probability in `[0.0, 1.0]`.
    #[must_use]
    pub fn dropout_probability(&self) -> f32 {
        f32::from(self.dropout_probability_bps) / 10_000.0
    }

    fn validate(&self) -> Result<(), EncoderDecoderTransformerError> {
        if self.source_vocab_size == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("source_vocab_size must be positive"),
            });
        }
        if self.target_vocab_size == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("target_vocab_size must be positive"),
            });
        }
        if self.hidden_size == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("hidden_size must be positive"),
            });
        }
        if self.feed_forward_size == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("feed_forward_size must be positive"),
            });
        }
        if self.head_count == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("head_count must be positive"),
            });
        }
        if !self.hidden_size.is_multiple_of(self.head_count) {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: format!(
                    "hidden_size {} must be divisible by head_count {}",
                    self.hidden_size, self.head_count
                ),
            });
        }
        if self.encoder_layer_count == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("encoder_layer_count must be positive"),
            });
        }
        if self.decoder_layer_count == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("decoder_layer_count must be positive"),
            });
        }
        if self.max_source_positions == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("max_source_positions must be positive"),
            });
        }
        if self.max_target_positions == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("max_target_positions must be positive"),
            });
        }
        if self.dropout_probability() > 1.0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: format!(
                    "dropout_probability_bps {} exceeds 10000",
                    self.dropout_probability_bps
                ),
            });
        }
        Ok(())
    }
}

/// Output of one encoder layer pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformerEncoderLayerOutput {
    pub hidden_state: NnTensor,
    pub self_attention_trace: AttentionProbabilityTrace,
}

/// Output of one decoder layer pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformerDecoderLayerOutput {
    pub hidden_state: NnTensor,
    pub self_attention_trace: AttentionProbabilityTrace,
    pub cross_attention_trace: AttentionProbabilityTrace,
}

/// Full forward-pass output for one encoder-decoder Transformer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncoderDecoderTransformerForwardOutput {
    pub encoder_hidden_state: NnTensor,
    pub decoder_hidden_state: NnTensor,
    pub logits: NnTensor,
    pub encoder_layer_outputs: Vec<TransformerEncoderLayerOutput>,
    pub decoder_layer_outputs: Vec<TransformerDecoderLayerOutput>,
}

/// Reusable encoder layer matching the original Transformer paper structure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformerEncoderLayer {
    self_attention: MultiHeadAttention,
    self_attention_norm: LayerNorm,
    feed_forward: PositionwiseFeedForward,
    feed_forward_norm: LayerNorm,
}

impl TransformerEncoderLayer {
    pub fn from_components(
        self_attention: MultiHeadAttention,
        self_attention_norm: LayerNorm,
        feed_forward: PositionwiseFeedForward,
        feed_forward_norm: LayerNorm,
    ) -> Result<Self, EncoderDecoderTransformerError> {
        let hidden_size = self_attention.hidden_size();
        validate_layer_norm_feature_size(
            &self_attention_norm,
            hidden_size,
            "encoder_layer.self_attention_norm",
        )?;
        validate_layer_norm_feature_size(
            &feed_forward_norm,
            hidden_size,
            "encoder_layer.feed_forward_norm",
        )?;
        if feed_forward.hidden_size() != hidden_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_layer.feed_forward",
                message: format!(
                    "feed-forward hidden_size {} must match attention hidden_size {hidden_size}",
                    feed_forward.hidden_size()
                ),
            });
        }
        Ok(Self {
            self_attention,
            self_attention_norm,
            feed_forward,
            feed_forward_norm,
        })
    }

    #[must_use]
    pub const fn hidden_size(&self) -> usize {
        self.self_attention.hidden_size()
    }

    #[must_use]
    pub const fn self_attention(&self) -> &MultiHeadAttention {
        &self.self_attention
    }

    #[must_use]
    pub const fn self_attention_norm(&self) -> &LayerNorm {
        &self.self_attention_norm
    }

    #[must_use]
    pub const fn feed_forward(&self) -> &PositionwiseFeedForward {
        &self.feed_forward
    }

    #[must_use]
    pub const fn feed_forward_norm(&self) -> &LayerNorm {
        &self.feed_forward_norm
    }

    pub fn forward(
        &self,
        input: &NnTensor,
        self_attention_mask: Option<&AttentionMask>,
        mode: TransformerExecutionMode,
    ) -> Result<TransformerEncoderLayerOutput, EncoderDecoderTransformerError> {
        let attention =
            self.self_attention
                .forward(input, input, input, self_attention_mask, mode)?;
        let attention_residual = add_tensors(
            "encoder_layer.self_attention_residual",
            input,
            &attention.hidden_state,
        )?;
        let attention_normed = self.self_attention_norm.forward(&attention_residual)?;
        let feed_forward = self.feed_forward.forward(&attention_normed, mode)?;
        let feed_forward_residual = add_tensors(
            "encoder_layer.feed_forward_residual",
            &attention_normed,
            &feed_forward,
        )?;
        let hidden_state = self.feed_forward_norm.forward(&feed_forward_residual)?;
        Ok(TransformerEncoderLayerOutput {
            hidden_state,
            self_attention_trace: attention.probability_trace,
        })
    }
}

/// Reusable decoder layer matching the original Transformer paper structure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformerDecoderLayer {
    self_attention: MultiHeadAttention,
    self_attention_norm: LayerNorm,
    cross_attention: MultiHeadAttention,
    cross_attention_norm: LayerNorm,
    feed_forward: PositionwiseFeedForward,
    feed_forward_norm: LayerNorm,
}

impl TransformerDecoderLayer {
    pub fn from_components(
        self_attention: MultiHeadAttention,
        self_attention_norm: LayerNorm,
        cross_attention: MultiHeadAttention,
        cross_attention_norm: LayerNorm,
        feed_forward: PositionwiseFeedForward,
        feed_forward_norm: LayerNorm,
    ) -> Result<Self, EncoderDecoderTransformerError> {
        let hidden_size = self_attention.hidden_size();
        if cross_attention.hidden_size() != hidden_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "decoder_layer.cross_attention",
                message: format!(
                    "cross-attention hidden_size {} must match self-attention hidden_size {hidden_size}",
                    cross_attention.hidden_size()
                ),
            });
        }
        if feed_forward.hidden_size() != hidden_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "decoder_layer.feed_forward",
                message: format!(
                    "feed-forward hidden_size {} must match attention hidden_size {hidden_size}",
                    feed_forward.hidden_size()
                ),
            });
        }
        validate_layer_norm_feature_size(
            &self_attention_norm,
            hidden_size,
            "decoder_layer.self_attention_norm",
        )?;
        validate_layer_norm_feature_size(
            &cross_attention_norm,
            hidden_size,
            "decoder_layer.cross_attention_norm",
        )?;
        validate_layer_norm_feature_size(
            &feed_forward_norm,
            hidden_size,
            "decoder_layer.feed_forward_norm",
        )?;
        Ok(Self {
            self_attention,
            self_attention_norm,
            cross_attention,
            cross_attention_norm,
            feed_forward,
            feed_forward_norm,
        })
    }

    #[must_use]
    pub const fn hidden_size(&self) -> usize {
        self.self_attention.hidden_size()
    }

    #[must_use]
    pub const fn self_attention(&self) -> &MultiHeadAttention {
        &self.self_attention
    }

    #[must_use]
    pub const fn self_attention_norm(&self) -> &LayerNorm {
        &self.self_attention_norm
    }

    #[must_use]
    pub const fn cross_attention(&self) -> &MultiHeadAttention {
        &self.cross_attention
    }

    #[must_use]
    pub const fn cross_attention_norm(&self) -> &LayerNorm {
        &self.cross_attention_norm
    }

    #[must_use]
    pub const fn feed_forward(&self) -> &PositionwiseFeedForward {
        &self.feed_forward
    }

    #[must_use]
    pub const fn feed_forward_norm(&self) -> &LayerNorm {
        &self.feed_forward_norm
    }

    pub fn forward(
        &self,
        input: &NnTensor,
        encoder_hidden_state: &NnTensor,
        self_attention_mask: Option<&AttentionMask>,
        cross_attention_mask: Option<&AttentionMask>,
        mode: TransformerExecutionMode,
    ) -> Result<TransformerDecoderLayerOutput, EncoderDecoderTransformerError> {
        let self_attention =
            self.self_attention
                .forward(input, input, input, self_attention_mask, mode)?;
        let self_attention_residual = add_tensors(
            "decoder_layer.self_attention_residual",
            input,
            &self_attention.hidden_state,
        )?;
        let self_attention_normed = self.self_attention_norm.forward(&self_attention_residual)?;
        let cross_attention = self.cross_attention.forward(
            &self_attention_normed,
            encoder_hidden_state,
            encoder_hidden_state,
            cross_attention_mask,
            mode,
        )?;
        let cross_attention_residual = add_tensors(
            "decoder_layer.cross_attention_residual",
            &self_attention_normed,
            &cross_attention.hidden_state,
        )?;
        let cross_attention_normed = self
            .cross_attention_norm
            .forward(&cross_attention_residual)?;
        let feed_forward = self.feed_forward.forward(&cross_attention_normed, mode)?;
        let feed_forward_residual = add_tensors(
            "decoder_layer.feed_forward_residual",
            &cross_attention_normed,
            &feed_forward,
        )?;
        let hidden_state = self.feed_forward_norm.forward(&feed_forward_residual)?;
        Ok(TransformerDecoderLayerOutput {
            hidden_state,
            self_attention_trace: self_attention.probability_trace,
            cross_attention_trace: cross_attention.probability_trace,
        })
    }
}

/// Canonical paper-faithful encoder-decoder Transformer stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncoderDecoderTransformer {
    config: EncoderDecoderTransformerConfig,
    source_embeddings: TransformerEmbeddings,
    target_embeddings: TransformerEmbeddings,
    encoder_layers: Vec<TransformerEncoderLayer>,
    decoder_layers: Vec<TransformerDecoderLayer>,
    logits_projection: Linear,
}

impl EncoderDecoderTransformer {
    pub fn from_components(
        config: EncoderDecoderTransformerConfig,
        source_embeddings: TransformerEmbeddings,
        target_embeddings: TransformerEmbeddings,
        encoder_layers: Vec<TransformerEncoderLayer>,
        decoder_layers: Vec<TransformerDecoderLayer>,
        logits_projection: Linear,
    ) -> Result<Self, EncoderDecoderTransformerError> {
        config.validate()?;
        if source_embeddings.vocab_size() != config.source_vocab_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.source_embeddings",
                message: format!(
                    "source embedding vocab {} must match config source_vocab_size {}",
                    source_embeddings.vocab_size(),
                    config.source_vocab_size
                ),
            });
        }
        if target_embeddings.vocab_size() != config.target_vocab_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.target_embeddings",
                message: format!(
                    "target embedding vocab {} must match config target_vocab_size {}",
                    target_embeddings.vocab_size(),
                    config.target_vocab_size
                ),
            });
        }
        if source_embeddings.hidden_size() != config.hidden_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.source_embeddings",
                message: format!(
                    "source embedding hidden_size {} must match config hidden_size {}",
                    source_embeddings.hidden_size(),
                    config.hidden_size
                ),
            });
        }
        if target_embeddings.hidden_size() != config.hidden_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.target_embeddings",
                message: format!(
                    "target embedding hidden_size {} must match config hidden_size {}",
                    target_embeddings.hidden_size(),
                    config.hidden_size
                ),
            });
        }
        if source_embeddings.max_positions() != config.max_source_positions {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.source_embeddings",
                message: format!(
                    "source embedding max_positions {} must match config max_source_positions {}",
                    source_embeddings.max_positions(),
                    config.max_source_positions
                ),
            });
        }
        if target_embeddings.max_positions() != config.max_target_positions {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.target_embeddings",
                message: format!(
                    "target embedding max_positions {} must match config max_target_positions {}",
                    target_embeddings.max_positions(),
                    config.max_target_positions
                ),
            });
        }
        if encoder_layers.len() != config.encoder_layer_count {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.encoder_layers",
                message: format!(
                    "encoder layer count {} must match config encoder_layer_count {}",
                    encoder_layers.len(),
                    config.encoder_layer_count
                ),
            });
        }
        if decoder_layers.len() != config.decoder_layer_count {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.decoder_layers",
                message: format!(
                    "decoder layer count {} must match config decoder_layer_count {}",
                    decoder_layers.len(),
                    config.decoder_layer_count
                ),
            });
        }
        if encoder_layers
            .iter()
            .any(|layer| layer.hidden_size() != config.hidden_size)
        {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.encoder_layers",
                message: String::from("all encoder layers must match config hidden_size"),
            });
        }
        if decoder_layers
            .iter()
            .any(|layer| layer.hidden_size() != config.hidden_size)
        {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.decoder_layers",
                message: String::from("all decoder layers must match config hidden_size"),
            });
        }
        if logits_projection.in_features() != config.hidden_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.logits_projection",
                message: format!(
                    "logits projection in_features {} must match config hidden_size {}",
                    logits_projection.in_features(),
                    config.hidden_size
                ),
            });
        }
        if logits_projection.out_features() != config.target_vocab_size {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer.logits_projection",
                message: format!(
                    "logits projection out_features {} must match config target_vocab_size {}",
                    logits_projection.out_features(),
                    config.target_vocab_size
                ),
            });
        }
        Ok(Self {
            config,
            source_embeddings,
            target_embeddings,
            encoder_layers,
            decoder_layers,
            logits_projection,
        })
    }

    #[must_use]
    pub fn config(&self) -> &EncoderDecoderTransformerConfig {
        &self.config
    }

    #[must_use]
    pub const fn source_embeddings(&self) -> &TransformerEmbeddings {
        &self.source_embeddings
    }

    #[must_use]
    pub const fn target_embeddings(&self) -> &TransformerEmbeddings {
        &self.target_embeddings
    }

    #[must_use]
    pub fn encoder_layers(&self) -> &[TransformerEncoderLayer] {
        &self.encoder_layers
    }

    #[must_use]
    pub fn decoder_layers(&self) -> &[TransformerDecoderLayer] {
        &self.decoder_layers
    }

    #[must_use]
    pub const fn logits_projection(&self) -> &Linear {
        &self.logits_projection
    }

    pub fn forward(
        &self,
        source_index_shape: Shape,
        source_token_ids: &[usize],
        target_index_shape: Shape,
        target_token_ids: &[usize],
        mode: TransformerExecutionMode,
    ) -> Result<EncoderDecoderTransformerForwardOutput, EncoderDecoderTransformerError> {
        self.forward_with_masks(
            source_index_shape,
            source_token_ids,
            target_index_shape,
            target_token_ids,
            None,
            None,
            None,
            mode,
        )
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
    ) -> Result<EncoderDecoderTransformerForwardOutput, EncoderDecoderTransformerError> {
        validate_index_shape(
            "encoder_decoder_transformer.source_tokens",
            &source_index_shape,
        )?;
        validate_index_shape(
            "encoder_decoder_transformer.target_tokens",
            &target_index_shape,
        )?;
        let source_dims = source_index_shape.dims();
        let target_dims = target_index_shape.dims();
        if source_dims[0] != target_dims[0] {
            return Err(EncoderDecoderTransformerError::ShapeMismatch {
                component: "encoder_decoder_transformer.batch",
                expected: format!(
                    "matching source and target batch sizes, found source_batch={}",
                    source_dims[0]
                ),
                actual: target_dims.to_vec(),
            });
        }
        if source_dims[1] == 0 || target_dims[1] == 0 {
            return Err(EncoderDecoderTransformerError::InvalidConfiguration {
                component: "encoder_decoder_transformer",
                message: String::from("source and target sequence lengths must be positive"),
            });
        }
        validate_mask_shape(
            encoder_self_attention_mask,
            [source_dims[0], source_dims[1], source_dims[1]],
            "encoder_decoder_transformer.encoder_self_attention_mask",
        )?;
        validate_mask_shape(
            decoder_self_attention_mask,
            [target_dims[0], target_dims[1], target_dims[1]],
            "encoder_decoder_transformer.decoder_self_attention_mask",
        )?;
        validate_mask_shape(
            cross_attention_mask,
            [target_dims[0], target_dims[1], source_dims[1]],
            "encoder_decoder_transformer.cross_attention_mask",
        )?;

        let mut encoder_hidden_state =
            self.source_embeddings
                .forward(source_index_shape, source_token_ids, mode)?;
        let mut encoder_layer_outputs = Vec::with_capacity(self.encoder_layers.len());
        for layer in &self.encoder_layers {
            let output = layer.forward(&encoder_hidden_state, encoder_self_attention_mask, mode)?;
            encoder_hidden_state = output.hidden_state.clone();
            encoder_layer_outputs.push(output);
        }

        let mut decoder_hidden_state =
            self.target_embeddings
                .forward(target_index_shape.clone(), target_token_ids, mode)?;
        let causal_mask = AttentionMask::causal(target_dims[0], target_dims[1], target_dims[1]);
        let combined_decoder_mask = decoder_self_attention_mask
            .map(|mask| causal_mask.combine(mask))
            .transpose()?
            .unwrap_or(causal_mask);
        let mut decoder_layer_outputs = Vec::with_capacity(self.decoder_layers.len());
        for layer in &self.decoder_layers {
            let output = layer.forward(
                &decoder_hidden_state,
                &encoder_hidden_state,
                Some(&combined_decoder_mask),
                cross_attention_mask,
                mode,
            )?;
            decoder_hidden_state = output.hidden_state.clone();
            decoder_layer_outputs.push(output);
        }

        let logits = self.logits_projection.forward(&decoder_hidden_state)?;
        Ok(EncoderDecoderTransformerForwardOutput {
            encoder_hidden_state,
            decoder_hidden_state,
            logits,
            encoder_layer_outputs,
            decoder_layer_outputs,
        })
    }
}

/// Encoder-decoder stack construction or execution failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EncoderDecoderTransformerError {
    #[error(transparent)]
    Block(#[from] TransformerBlockError),
    #[error(transparent)]
    Layer(#[from] LayerError),
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error(transparent)]
    AttentionMask(#[from] AttentionMaskError),
    #[error("component `{component}` invalid configuration: {message}")]
    InvalidConfiguration {
        component: &'static str,
        message: String,
    },
    #[error("component `{component}` expected {expected}, found shape {actual:?}")]
    ShapeMismatch {
        component: &'static str,
        expected: String,
        actual: Vec<usize>,
    },
}

fn validate_layer_norm_feature_size(
    layer_norm: &LayerNorm,
    expected_feature_size: usize,
    component: &'static str,
) -> Result<(), EncoderDecoderTransformerError> {
    let weight = layer_norm.module().parameter("weight")?;
    let dims = weight.spec.shape().dims();
    if dims != [expected_feature_size] {
        return Err(EncoderDecoderTransformerError::ShapeMismatch {
            component,
            expected: format!("layer_norm weight shape [{expected_feature_size}]"),
            actual: dims.to_vec(),
        });
    }
    Ok(())
}

fn validate_index_shape(
    component: &'static str,
    index_shape: &Shape,
) -> Result<(), EncoderDecoderTransformerError> {
    if index_shape.dims().len() != 2 {
        return Err(EncoderDecoderTransformerError::ShapeMismatch {
            component,
            expected: String::from("rank-2 [batch, seq] token indices"),
            actual: index_shape.dims().to_vec(),
        });
    }
    Ok(())
}

fn validate_mask_shape(
    mask: Option<&AttentionMask>,
    expected: [usize; 3],
    component: &'static str,
) -> Result<(), EncoderDecoderTransformerError> {
    if let Some(mask) = mask {
        if mask.shape() != expected {
            return Err(EncoderDecoderTransformerError::ShapeMismatch {
                component,
                expected: format!(
                    "[batch={}, query={}, key={}]",
                    expected[0], expected[1], expected[2]
                ),
                actual: mask.shape().to_vec(),
            });
        }
    }
    Ok(())
}

fn add_tensors(
    component: &'static str,
    left: &NnTensor,
    right: &NnTensor,
) -> Result<NnTensor, EncoderDecoderTransformerError> {
    if left.dims() != right.dims() {
        return Err(EncoderDecoderTransformerError::ShapeMismatch {
            component,
            expected: format!("matching shapes, found left={:?}", left.dims()),
            actual: right.dims().to_vec(),
        });
    }
    let output = left
        .as_f32_slice()?
        .iter()
        .zip(right.as_f32_slice()?.iter())
        .map(|(left, right)| left + right)
        .collect::<Vec<_>>();
    Ok(NnTensor::f32(Shape::new(left.dims().to_vec()), output)?)
}

#[cfg(test)]
mod tests {
    use super::{
        EncoderDecoderTransformer, EncoderDecoderTransformerConfig, EncoderDecoderTransformerError,
        TransformerDecoderLayer, TransformerEncoderLayer,
    };
    use crate::{
        MultiHeadAttention, PositionwiseFeedForward, TransformerEmbeddings,
        TransformerExecutionMode,
    };
    use psionic_core::Shape;
    use psionic_nn::{ActivationKind, LayerNorm, Linear};

    fn identity_matrix(size: usize) -> Vec<f32> {
        let mut values = vec![0.0; size * size];
        for index in 0..size {
            values[index * size + index] = 1.0;
        }
        values
    }

    fn sample_config() -> EncoderDecoderTransformerConfig {
        EncoderDecoderTransformerConfig {
            source_vocab_size: 4,
            target_vocab_size: 4,
            hidden_size: 4,
            feed_forward_size: 4,
            head_count: 2,
            encoder_layer_count: 1,
            decoder_layer_count: 1,
            max_source_positions: 8,
            max_target_positions: 8,
            dropout_probability_bps: 0,
        }
    }

    fn build_model() -> Result<EncoderDecoderTransformer, EncoderDecoderTransformerError> {
        let config = sample_config();
        let source_embeddings = TransformerEmbeddings::from_f32_table(
            "article.source_embeddings",
            config.source_vocab_size,
            config.hidden_size,
            config.max_source_positions,
            vec![
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
            config.dropout_probability(),
        )?;
        let target_embeddings = TransformerEmbeddings::from_f32_table(
            "article.target_embeddings",
            config.target_vocab_size,
            config.hidden_size,
            config.max_target_positions,
            vec![
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
            config.dropout_probability(),
        )?;
        let encoder_layer = TransformerEncoderLayer::from_components(
            MultiHeadAttention::from_f32_parts(
                "article.encoder.self_attention",
                config.hidden_size,
                config.head_count,
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                config.dropout_probability(),
            )?,
            LayerNorm::new(
                "article.encoder.self_attention_norm",
                config.hidden_size,
                1e-5,
            )?,
            PositionwiseFeedForward::from_f32_parts(
                "article.encoder.feed_forward",
                config.hidden_size,
                config.feed_forward_size,
                ActivationKind::Relu,
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                config.dropout_probability(),
            )?,
            LayerNorm::new(
                "article.encoder.feed_forward_norm",
                config.hidden_size,
                1e-5,
            )?,
        )?;
        let decoder_layer = TransformerDecoderLayer::from_components(
            MultiHeadAttention::from_f32_parts(
                "article.decoder.self_attention",
                config.hidden_size,
                config.head_count,
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                config.dropout_probability(),
            )?,
            LayerNorm::new(
                "article.decoder.self_attention_norm",
                config.hidden_size,
                1e-5,
            )?,
            MultiHeadAttention::from_f32_parts(
                "article.decoder.cross_attention",
                config.hidden_size,
                config.head_count,
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                config.dropout_probability(),
            )?,
            LayerNorm::new(
                "article.decoder.cross_attention_norm",
                config.hidden_size,
                1e-5,
            )?,
            PositionwiseFeedForward::from_f32_parts(
                "article.decoder.feed_forward",
                config.hidden_size,
                config.feed_forward_size,
                ActivationKind::Relu,
                identity_matrix(4),
                Some(vec![0.0; 4]),
                identity_matrix(4),
                Some(vec![0.0; 4]),
                config.dropout_probability(),
            )?,
            LayerNorm::new(
                "article.decoder.feed_forward_norm",
                config.hidden_size,
                1e-5,
            )?,
        )?;
        let logits_projection = Linear::from_f32_parts(
            "article.logits_projection",
            config.hidden_size,
            config.target_vocab_size,
            identity_matrix(4),
            Some(vec![0.0; 4]),
        )?;
        EncoderDecoderTransformer::from_components(
            config,
            source_embeddings,
            target_embeddings,
            vec![encoder_layer],
            vec![decoder_layer],
            logits_projection,
        )
    }

    #[test]
    fn encoder_decoder_forward_emits_expected_shapes() -> Result<(), EncoderDecoderTransformerError>
    {
        let model = build_model()?;
        let output = model.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 3]),
            &[0, 1, 2],
            TransformerExecutionMode::Eval,
        )?;

        assert_eq!(output.encoder_hidden_state.dims(), &[1, 2, 4]);
        assert_eq!(output.decoder_hidden_state.dims(), &[1, 3, 4]);
        assert_eq!(output.logits.dims(), &[1, 3, 4]);
        assert_eq!(output.encoder_layer_outputs.len(), 1);
        assert_eq!(output.decoder_layer_outputs.len(), 1);
        assert_eq!(
            output.encoder_layer_outputs[0]
                .self_attention_trace
                .probabilities
                .shape(),
            [1, 2, 2, 2]
        );
        assert_eq!(
            output.decoder_layer_outputs[0]
                .self_attention_trace
                .probabilities
                .shape(),
            [1, 2, 3, 3]
        );
        assert_eq!(
            output.decoder_layer_outputs[0]
                .cross_attention_trace
                .probabilities
                .shape(),
            [1, 2, 3, 2]
        );
        Ok(())
    }

    #[test]
    fn decoder_causal_mask_blocks_future_token_effect() -> Result<(), EncoderDecoderTransformerError>
    {
        let model = build_model()?;
        let left = model.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 3]),
            &[0, 1, 2],
            TransformerExecutionMode::Eval,
        )?;
        let right = model.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 3]),
            &[0, 1, 3],
            TransformerExecutionMode::Eval,
        )?;
        let left_logits = left.logits.as_f32_slice()?;
        let right_logits = right.logits.as_f32_slice()?;

        assert_eq!(&left_logits[..8], &right_logits[..8]);
        assert_ne!(&left_logits[8..12], &right_logits[8..12]);
        Ok(())
    }

    #[test]
    fn cross_attention_changes_decoder_hidden_state_when_encoder_changes(
    ) -> Result<(), EncoderDecoderTransformerError> {
        let model = build_model()?;
        let left = model.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 2]),
            &[0, 1],
            TransformerExecutionMode::Eval,
        )?;
        let right = model.forward(
            Shape::new(vec![1, 2]),
            &[2, 3],
            Shape::new(vec![1, 2]),
            &[0, 1],
            TransformerExecutionMode::Eval,
        )?;

        assert_ne!(left.decoder_hidden_state, right.decoder_hidden_state);
        Ok(())
    }
}
