use psionic_core::Shape;
use psionic_nn::{
    Activation, ActivationKind, Dropout, Embedding, LayerError, LayerNorm, Linear,
    ModuleStateError, NnTensor,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    scaled_dot_product_attention, AttentionMask, AttentionProbabilityTrace, AttentionTensor4,
    AttentionTensorError, ScaledDotProductAttentionError,
};

/// Execution posture for reusable Transformer blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformerExecutionMode {
    Eval,
    Train { seed: u64 },
}

/// Output of one reusable multi-head attention pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiHeadAttentionOutput {
    pub hidden_state: NnTensor,
    pub probability_trace: AttentionProbabilityTrace,
}

/// Output of one reusable decoder block pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformerDecoderBlockOutput {
    pub hidden_state: NnTensor,
    pub attention_trace: AttentionProbabilityTrace,
}

/// Deterministic sinusoidal positional encoding binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinusoidalPositionalEncoding {
    hidden_size: usize,
    max_positions: usize,
}

impl SinusoidalPositionalEncoding {
    pub fn new(hidden_size: usize, max_positions: usize) -> Result<Self, TransformerBlockError> {
        if hidden_size == 0 {
            return Err(TransformerBlockError::InvalidConfiguration {
                component: "sinusoidal_positional_encoding",
                message: String::from("hidden_size must be positive"),
            });
        }
        if max_positions == 0 {
            return Err(TransformerBlockError::InvalidConfiguration {
                component: "sinusoidal_positional_encoding",
                message: String::from("max_positions must be positive"),
            });
        }
        Ok(Self {
            hidden_size,
            max_positions,
        })
    }

    #[must_use]
    pub const fn hidden_size(&self) -> usize {
        self.hidden_size
    }

    #[must_use]
    pub const fn max_positions(&self) -> usize {
        self.max_positions
    }

    pub fn bindings(
        &self,
        batch_size: usize,
        sequence_length: usize,
    ) -> Result<NnTensor, TransformerBlockError> {
        if sequence_length > self.max_positions {
            return Err(TransformerBlockError::InvalidConfiguration {
                component: "sinusoidal_positional_encoding",
                message: format!(
                    "sequence_length {sequence_length} exceeds max_positions {}",
                    self.max_positions
                ),
            });
        }
        let mut values = vec![0.0; batch_size * sequence_length * self.hidden_size];
        for batch in 0..batch_size {
            for position in 0..sequence_length {
                for channel in 0..self.hidden_size {
                    let index = ((batch * sequence_length) + position) * self.hidden_size + channel;
                    values[index] = positional_encoding_value(position, channel, self.hidden_size);
                }
            }
        }
        NnTensor::f32(
            Shape::new(vec![batch_size, sequence_length, self.hidden_size]),
            values,
        )
        .map_err(Into::into)
    }
}

/// Token embedding plus positional encoding binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformerEmbeddings {
    vocab_size: usize,
    token_embedding: Embedding,
    positional_encoding: SinusoidalPositionalEncoding,
    dropout: Dropout,
}

impl TransformerEmbeddings {
    pub fn from_f32_table(
        module_id: impl Into<String>,
        vocab_size: usize,
        hidden_size: usize,
        max_positions: usize,
        table: Vec<f32>,
        dropout_probability: f32,
    ) -> Result<Self, TransformerBlockError> {
        let module_id = module_id.into();
        Ok(Self {
            vocab_size,
            token_embedding: Embedding::from_f32_table(
                format!("{module_id}.token_embedding"),
                vocab_size,
                hidden_size,
                table,
            )?,
            positional_encoding: SinusoidalPositionalEncoding::new(hidden_size, max_positions)?,
            dropout: Dropout::new(format!("{module_id}.dropout"), dropout_probability)?,
        })
    }

    #[must_use]
    pub const fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    #[must_use]
    pub const fn hidden_size(&self) -> usize {
        self.positional_encoding.hidden_size()
    }

    #[must_use]
    pub const fn max_positions(&self) -> usize {
        self.positional_encoding.max_positions()
    }

    pub fn token_embedding_table_f32(&self) -> Result<&[f32], LayerError> {
        self.token_embedding.weight_f32()
    }

    pub fn forward(
        &self,
        index_shape: Shape,
        token_ids: &[usize],
        mode: TransformerExecutionMode,
    ) -> Result<NnTensor, TransformerBlockError> {
        if index_shape.dims().len() != 2 {
            return Err(TransformerBlockError::ShapeMismatch {
                component: "transformer_embeddings",
                expected: String::from("rank-2 [batch, seq] token indices"),
                actual: index_shape.dims().to_vec(),
            });
        }
        let token_embeddings = self
            .token_embedding
            .forward_with_shape(index_shape.clone(), token_ids)?;
        let dims = index_shape.dims();
        let positional = self.positional_encoding.bindings(dims[0], dims[1])?;
        let bound = add_tensors(
            "transformer_embeddings.binding",
            &token_embeddings,
            &positional,
        )?;
        apply_dropout(&self.dropout, &bound, mode, 1)
    }
}

/// Reusable multi-head projection and merge block above the owned attention primitive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiHeadAttention {
    hidden_size: usize,
    head_count: usize,
    head_dim: usize,
    query_projection: Linear,
    key_projection: Linear,
    value_projection: Linear,
    output_projection: Linear,
    dropout: Dropout,
}

impl MultiHeadAttention {
    #[allow(clippy::too_many_arguments)]
    pub fn from_f32_parts(
        module_id: impl Into<String>,
        hidden_size: usize,
        head_count: usize,
        query_weight: Vec<f32>,
        query_bias: Option<Vec<f32>>,
        key_weight: Vec<f32>,
        key_bias: Option<Vec<f32>>,
        value_weight: Vec<f32>,
        value_bias: Option<Vec<f32>>,
        output_weight: Vec<f32>,
        output_bias: Option<Vec<f32>>,
        dropout_probability: f32,
    ) -> Result<Self, TransformerBlockError> {
        if hidden_size == 0 {
            return Err(TransformerBlockError::InvalidConfiguration {
                component: "multi_head_attention",
                message: String::from("hidden_size must be positive"),
            });
        }
        if head_count == 0 {
            return Err(TransformerBlockError::InvalidConfiguration {
                component: "multi_head_attention",
                message: String::from("head_count must be positive"),
            });
        }
        if hidden_size % head_count != 0 {
            return Err(TransformerBlockError::InvalidConfiguration {
                component: "multi_head_attention",
                message: format!(
                    "hidden_size {hidden_size} must be divisible by head_count {head_count}"
                ),
            });
        }
        let module_id = module_id.into();
        Ok(Self {
            hidden_size,
            head_count,
            head_dim: hidden_size / head_count,
            query_projection: Linear::from_f32_parts(
                format!("{module_id}.query_projection"),
                hidden_size,
                hidden_size,
                query_weight,
                query_bias,
            )?,
            key_projection: Linear::from_f32_parts(
                format!("{module_id}.key_projection"),
                hidden_size,
                hidden_size,
                key_weight,
                key_bias,
            )?,
            value_projection: Linear::from_f32_parts(
                format!("{module_id}.value_projection"),
                hidden_size,
                hidden_size,
                value_weight,
                value_bias,
            )?,
            output_projection: Linear::from_f32_parts(
                format!("{module_id}.output_projection"),
                hidden_size,
                hidden_size,
                output_weight,
                output_bias,
            )?,
            dropout: Dropout::new(format!("{module_id}.dropout"), dropout_probability)?,
        })
    }

    #[must_use]
    pub const fn hidden_size(&self) -> usize {
        self.hidden_size
    }

    #[must_use]
    pub const fn head_count(&self) -> usize {
        self.head_count
    }

    #[must_use]
    pub const fn head_dim(&self) -> usize {
        self.head_dim
    }

    pub fn query_weight_f32(&self) -> Result<&[f32], LayerError> {
        self.query_projection.weight_f32()
    }

    pub fn query_bias_f32(&self) -> Result<Option<&[f32]>, LayerError> {
        self.query_projection.bias_f32()
    }

    pub fn key_weight_f32(&self) -> Result<&[f32], LayerError> {
        self.key_projection.weight_f32()
    }

    pub fn key_bias_f32(&self) -> Result<Option<&[f32]>, LayerError> {
        self.key_projection.bias_f32()
    }

    pub fn value_weight_f32(&self) -> Result<&[f32], LayerError> {
        self.value_projection.weight_f32()
    }

    pub fn value_bias_f32(&self) -> Result<Option<&[f32]>, LayerError> {
        self.value_projection.bias_f32()
    }

    pub fn output_weight_f32(&self) -> Result<&[f32], LayerError> {
        self.output_projection.weight_f32()
    }

    pub fn output_bias_f32(&self) -> Result<Option<&[f32]>, LayerError> {
        self.output_projection.bias_f32()
    }

    pub fn forward(
        &self,
        query: &NnTensor,
        key: &NnTensor,
        value: &NnTensor,
        mask: Option<&AttentionMask>,
        mode: TransformerExecutionMode,
    ) -> Result<MultiHeadAttentionOutput, TransformerBlockError> {
        validate_hidden_tensor_shape("multi_head_attention.query_input", query, self.hidden_size)?;
        validate_hidden_tensor_shape("multi_head_attention.key_input", key, self.hidden_size)?;
        validate_hidden_tensor_shape("multi_head_attention.value_input", value, self.hidden_size)?;
        let projected_query = self.query_projection.forward(query)?;
        let projected_key = self.key_projection.forward(key)?;
        let projected_value = self.value_projection.forward(value)?;
        let attention_output = scaled_dot_product_attention(
            &nn_tensor_to_attention_tensor4(
                &projected_query,
                self.head_count,
                self.head_dim,
                "multi_head_attention.query",
            )?,
            &nn_tensor_to_attention_tensor4(
                &projected_key,
                self.head_count,
                self.head_dim,
                "multi_head_attention.key",
            )?,
            &nn_tensor_to_attention_tensor4(
                &projected_value,
                self.head_count,
                self.head_dim,
                "multi_head_attention.value",
            )?,
            mask,
        )?;
        let merged = attention_tensor4_to_nn_tensor(
            &attention_output.context,
            "multi_head_attention.merge",
        )?;
        let projected_output = self.output_projection.forward(&merged)?;
        let hidden_state = apply_dropout(&self.dropout, &projected_output, mode, 17)?;
        Ok(MultiHeadAttentionOutput {
            hidden_state,
            probability_trace: attention_output.probability_trace,
        })
    }
}

/// Reusable position-wise feed-forward block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PositionwiseFeedForward {
    hidden_size: usize,
    intermediate_size: usize,
    input_projection: Linear,
    activation: Activation,
    output_projection: Linear,
    dropout: Dropout,
}

impl PositionwiseFeedForward {
    #[allow(clippy::too_many_arguments)]
    pub fn from_f32_parts(
        module_id: impl Into<String>,
        hidden_size: usize,
        intermediate_size: usize,
        activation_kind: ActivationKind,
        input_weight: Vec<f32>,
        input_bias: Option<Vec<f32>>,
        output_weight: Vec<f32>,
        output_bias: Option<Vec<f32>>,
        dropout_probability: f32,
    ) -> Result<Self, TransformerBlockError> {
        if hidden_size == 0 {
            return Err(TransformerBlockError::InvalidConfiguration {
                component: "positionwise_feed_forward",
                message: String::from("hidden_size must be positive"),
            });
        }
        if intermediate_size == 0 {
            return Err(TransformerBlockError::InvalidConfiguration {
                component: "positionwise_feed_forward",
                message: String::from("intermediate_size must be positive"),
            });
        }
        let module_id = module_id.into();
        Ok(Self {
            hidden_size,
            intermediate_size,
            input_projection: Linear::from_f32_parts(
                format!("{module_id}.input_projection"),
                hidden_size,
                intermediate_size,
                input_weight,
                input_bias,
            )?,
            activation: Activation::new(format!("{module_id}.activation"), activation_kind)?,
            output_projection: Linear::from_f32_parts(
                format!("{module_id}.output_projection"),
                intermediate_size,
                hidden_size,
                output_weight,
                output_bias,
            )?,
            dropout: Dropout::new(format!("{module_id}.dropout"), dropout_probability)?,
        })
    }

    pub fn forward(
        &self,
        input: &NnTensor,
        mode: TransformerExecutionMode,
    ) -> Result<NnTensor, TransformerBlockError> {
        let projected = self.input_projection.forward(input)?;
        let activated = self.activation.forward(&projected)?;
        let output = self.output_projection.forward(&activated)?;
        apply_dropout(&self.dropout, &output, mode, 29)
    }

    #[must_use]
    pub const fn hidden_size(&self) -> usize {
        self.hidden_size
    }

    #[must_use]
    pub const fn intermediate_size(&self) -> usize {
        self.intermediate_size
    }

    pub fn input_weight_f32(&self) -> Result<&[f32], LayerError> {
        self.input_projection.weight_f32()
    }

    pub fn input_bias_f32(&self) -> Result<Option<&[f32]>, LayerError> {
        self.input_projection.bias_f32()
    }

    pub fn output_weight_f32(&self) -> Result<&[f32], LayerError> {
        self.output_projection.weight_f32()
    }

    pub fn output_bias_f32(&self) -> Result<Option<&[f32]>, LayerError> {
        self.output_projection.bias_f32()
    }
}

/// Reusable decoder-style Transformer block above the owned attention primitive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformerDecoderBlock {
    self_attention: MultiHeadAttention,
    self_attention_norm: LayerNorm,
    feed_forward: PositionwiseFeedForward,
    feed_forward_norm: LayerNorm,
}

impl TransformerDecoderBlock {
    pub fn from_components(
        self_attention: MultiHeadAttention,
        self_attention_norm: LayerNorm,
        feed_forward: PositionwiseFeedForward,
        feed_forward_norm: LayerNorm,
    ) -> Result<Self, TransformerBlockError> {
        let hidden_size = self_attention.hidden_size();
        validate_layer_norm_feature_size(
            &self_attention_norm,
            hidden_size,
            "decoder_block.self_attention_norm",
        )?;
        validate_layer_norm_feature_size(
            &feed_forward_norm,
            hidden_size,
            "decoder_block.feed_forward_norm",
        )?;
        Ok(Self {
            self_attention,
            self_attention_norm,
            feed_forward,
            feed_forward_norm,
        })
    }

    pub fn forward(
        &self,
        input: &NnTensor,
        self_attention_mask: Option<&AttentionMask>,
        mode: TransformerExecutionMode,
    ) -> Result<TransformerDecoderBlockOutput, TransformerBlockError> {
        let attention =
            self.self_attention
                .forward(input, input, input, self_attention_mask, mode)?;
        let attention_residual = add_tensors(
            "decoder_block.self_attention_residual",
            input,
            &attention.hidden_state,
        )?;
        let attention_normed = self.self_attention_norm.forward(&attention_residual)?;
        let feed_forward = self.feed_forward.forward(&attention_normed, mode)?;
        let feed_forward_residual = add_tensors(
            "decoder_block.feed_forward_residual",
            &attention_normed,
            &feed_forward,
        )?;
        let hidden_state = self.feed_forward_norm.forward(&feed_forward_residual)?;
        Ok(TransformerDecoderBlockOutput {
            hidden_state,
            attention_trace: attention.probability_trace,
        })
    }
}

/// Reusable Transformer block composition failures.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TransformerBlockError {
    #[error(transparent)]
    Layer(#[from] LayerError),
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error(transparent)]
    AttentionTensor(#[from] AttentionTensorError),
    #[error(transparent)]
    Attention(#[from] ScaledDotProductAttentionError),
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
) -> Result<(), TransformerBlockError> {
    let weight = layer_norm.module().parameter("weight")?;
    let dims = weight.spec.shape().dims();
    if dims != [expected_feature_size] {
        return Err(TransformerBlockError::ShapeMismatch {
            component,
            expected: format!("layer_norm weight shape [{expected_feature_size}]"),
            actual: dims.to_vec(),
        });
    }
    Ok(())
}

fn apply_dropout(
    dropout: &Dropout,
    input: &NnTensor,
    mode: TransformerExecutionMode,
    salt: u64,
) -> Result<NnTensor, TransformerBlockError> {
    match mode {
        TransformerExecutionMode::Eval => dropout.forward_eval(input).map_err(Into::into),
        TransformerExecutionMode::Train { seed } => dropout
            .forward_train(input, salted_seed(seed, salt))
            .map_err(Into::into),
    }
}

fn salted_seed(seed: u64, salt: u64) -> u64 {
    seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .max(1)
}

fn add_tensors(
    component: &'static str,
    left: &NnTensor,
    right: &NnTensor,
) -> Result<NnTensor, TransformerBlockError> {
    if left.dims() != right.dims() {
        return Err(TransformerBlockError::ShapeMismatch {
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
    NnTensor::f32(Shape::new(left.dims().to_vec()), output).map_err(Into::into)
}

fn validate_hidden_tensor_shape(
    component: &'static str,
    tensor: &NnTensor,
    hidden_size: usize,
) -> Result<(), TransformerBlockError> {
    if tensor.dims().len() != 3 {
        return Err(TransformerBlockError::ShapeMismatch {
            component,
            expected: String::from("rank-3 [batch, seq, hidden] tensor"),
            actual: tensor.dims().to_vec(),
        });
    }
    if tensor.dims()[2] != hidden_size {
        return Err(TransformerBlockError::ShapeMismatch {
            component,
            expected: format!("last dimension {hidden_size}"),
            actual: tensor.dims().to_vec(),
        });
    }
    Ok(())
}

fn nn_tensor_to_attention_tensor4(
    tensor: &NnTensor,
    head_count: usize,
    head_dim: usize,
    component: &'static str,
) -> Result<AttentionTensor4, TransformerBlockError> {
    if tensor.dims().len() != 3 {
        return Err(TransformerBlockError::ShapeMismatch {
            component,
            expected: String::from("rank-3 [batch, seq, hidden] tensor"),
            actual: tensor.dims().to_vec(),
        });
    }
    let dims = tensor.dims();
    let hidden_size = head_count * head_dim;
    if dims[2] != hidden_size {
        return Err(TransformerBlockError::ShapeMismatch {
            component,
            expected: format!("last dimension {hidden_size}"),
            actual: dims.to_vec(),
        });
    }
    let values = tensor.as_f32_slice()?;
    let mut attention = AttentionTensor4::zeros([dims[0], head_count, dims[1], head_dim]);
    for batch in 0..dims[0] {
        for position in 0..dims[1] {
            let base = ((batch * dims[1]) + position) * hidden_size;
            for head in 0..head_count {
                for dim in 0..head_dim {
                    attention.set(
                        batch,
                        head,
                        position,
                        dim,
                        values[base + head * head_dim + dim],
                    );
                }
            }
        }
    }
    Ok(attention)
}

fn attention_tensor4_to_nn_tensor(
    tensor: &AttentionTensor4,
    component: &'static str,
) -> Result<NnTensor, TransformerBlockError> {
    let hidden_size = tensor.head_count() * tensor.col_count();
    if tensor.row_count() == 0 {
        return Err(TransformerBlockError::ShapeMismatch {
            component,
            expected: String::from("row_count > 0"),
            actual: vec![
                tensor.batch_size(),
                tensor.head_count(),
                tensor.row_count(),
                tensor.col_count(),
            ],
        });
    }
    let mut values = vec![0.0; tensor.batch_size() * tensor.row_count() * hidden_size];
    for batch in 0..tensor.batch_size() {
        for position in 0..tensor.row_count() {
            let base = ((batch * tensor.row_count()) + position) * hidden_size;
            for head in 0..tensor.head_count() {
                for dim in 0..tensor.col_count() {
                    values[base + head * tensor.col_count() + dim] =
                        tensor.get(batch, head, position, dim);
                }
            }
        }
    }
    NnTensor::f32(
        Shape::new(vec![tensor.batch_size(), tensor.row_count(), hidden_size]),
        values,
    )
    .map_err(Into::into)
}

fn positional_encoding_value(position: usize, channel: usize, hidden_size: usize) -> f32 {
    let exponent = (2 * (channel / 2)) as f32 / hidden_size as f32;
    let angle = position as f32 / 10_000_f32.powf(exponent);
    if channel.is_multiple_of(2) {
        angle.sin()
    } else {
        angle.cos()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MultiHeadAttention, PositionwiseFeedForward, SinusoidalPositionalEncoding,
        TransformerBlockError, TransformerDecoderBlock, TransformerEmbeddings,
        TransformerExecutionMode,
    };
    use crate::AttentionMask;
    use psionic_core::Shape;
    use psionic_nn::{ActivationKind, LayerNorm, NnTensor};

    fn identity_matrix(size: usize) -> Vec<f32> {
        let mut values = vec![0.0; size * size];
        for index in 0..size {
            values[index * size + index] = 1.0;
        }
        values
    }

    fn approx_eq(left: f32, right: f32) {
        assert!((left - right).abs() <= 1e-4, "left={left} right={right}");
    }

    #[test]
    fn sinusoidal_positional_encoding_matches_reference_values() -> Result<(), TransformerBlockError>
    {
        let encoding = SinusoidalPositionalEncoding::new(4, 8)?;
        let bindings = encoding.bindings(1, 2)?;
        let values = bindings.as_f32_slice()?;

        approx_eq(values[0], 0.0);
        approx_eq(values[1], 1.0);
        approx_eq(values[2], 0.0);
        approx_eq(values[3], 1.0);
        approx_eq(values[4], 0.84147096);
        approx_eq(values[5], 0.5403023);
        approx_eq(values[6], 0.009999833);
        approx_eq(values[7], 0.99995);
        Ok(())
    }

    #[test]
    fn transformer_embeddings_bind_token_and_positional_values() -> Result<(), TransformerBlockError>
    {
        let embeddings =
            TransformerEmbeddings::from_f32_table("embeddings", 4, 4, 8, vec![0.0; 16], 0.0)?;
        let output = embeddings.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            TransformerExecutionMode::Eval,
        )?;
        let values = output.as_f32_slice()?;

        approx_eq(values[0], 0.0);
        approx_eq(values[1], 1.0);
        approx_eq(values[4], 0.84147096);
        approx_eq(values[5], 0.5403023);
        Ok(())
    }

    #[test]
    fn transformer_embeddings_dropout_respects_eval_and_train_posture(
    ) -> Result<(), TransformerBlockError> {
        let embeddings = TransformerEmbeddings::from_f32_table(
            "embeddings",
            2,
            2,
            4,
            vec![1.0, 1.0, 1.0, 1.0],
            0.5,
        )?;
        let eval = embeddings.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            TransformerExecutionMode::Eval,
        )?;
        let train_a = embeddings.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            TransformerExecutionMode::Train { seed: 7 },
        )?;
        let train_b = embeddings.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            TransformerExecutionMode::Train { seed: 7 },
        )?;

        assert_eq!(train_a, train_b);
        assert_ne!(eval, train_a);
        Ok(())
    }

    #[test]
    fn multi_head_attention_rejects_hidden_width_mismatch() -> Result<(), TransformerBlockError> {
        let attention = MultiHeadAttention::from_f32_parts(
            "attn",
            4,
            2,
            identity_matrix(4),
            Some(vec![0.0; 4]),
            identity_matrix(4),
            Some(vec![0.0; 4]),
            identity_matrix(4),
            Some(vec![0.0; 4]),
            identity_matrix(4),
            Some(vec![0.0; 4]),
            0.0,
        )?;
        let input = NnTensor::f32(Shape::new(vec![1, 2, 3]), vec![1.0; 6])?;
        let error = attention
            .forward(&input, &input, &input, None, TransformerExecutionMode::Eval)
            .expect_err("hidden mismatch should refuse");
        assert!(matches!(error, TransformerBlockError::ShapeMismatch { .. }));
        Ok(())
    }

    #[test]
    fn decoder_block_forward_is_deterministic_in_eval_mode() -> Result<(), TransformerBlockError> {
        let attention = MultiHeadAttention::from_f32_parts(
            "attn",
            4,
            2,
            identity_matrix(4),
            Some(vec![0.0; 4]),
            identity_matrix(4),
            Some(vec![0.0; 4]),
            identity_matrix(4),
            Some(vec![0.0; 4]),
            identity_matrix(4),
            Some(vec![0.0; 4]),
            0.0,
        )?;
        let feed_forward = PositionwiseFeedForward::from_f32_parts(
            "ffn",
            4,
            4,
            ActivationKind::Relu,
            identity_matrix(4),
            Some(vec![0.0; 4]),
            identity_matrix(4),
            Some(vec![0.0; 4]),
            0.0,
        )?;
        let block = TransformerDecoderBlock::from_components(
            attention,
            LayerNorm::new("norm1", 4, 1e-5)?,
            feed_forward,
            LayerNorm::new("norm2", 4, 1e-5)?,
        )?;
        let input = NnTensor::f32(
            Shape::new(vec![1, 2, 4]),
            vec![1.0, 2.0, 3.0, 4.0, 2.0, 3.0, 4.0, 5.0],
        )?;
        let mask = AttentionMask::causal(1, 2, 2);

        let first = block.forward(&input, Some(&mask), TransformerExecutionMode::Eval)?;
        let second = block.forward(&input, Some(&mask), TransformerExecutionMode::Eval)?;

        assert_eq!(first, second);
        assert_eq!(first.hidden_state.dims(), &[1, 2, 4]);
        for row in first.hidden_state.as_f32_slice()?.chunks(4) {
            let mean = row.iter().sum::<f32>() / row.len() as f32;
            approx_eq(mean, 0.0);
        }
        Ok(())
    }
}
