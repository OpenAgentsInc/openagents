use psionic_core::Shape;
use psionic_nn::{LayerError, NnTensor};
use psionic_transformer::{
    AttentionMask, AttentionMaskError, AttentionTensor4, AttentionTensorError,
    ScaledDotProductAttentionError, scaled_dot_product_attention,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionReferenceConfig {
    pub query_block_rows: usize,
    pub key_block_rows: usize,
}

impl Cs336A2FlashAttentionReferenceConfig {
    #[must_use]
    pub const fn bounded_default() -> Self {
        Self {
            query_block_rows: 4,
            key_block_rows: 4,
        }
    }

    fn validate(self) -> Result<Self, Cs336A2FlashAttentionReferenceError> {
        if self.query_block_rows == 0 {
            return Err(Cs336A2FlashAttentionReferenceError::InvalidConfiguration(
                "query_block_rows must be positive".into(),
            ));
        }
        if self.key_block_rows == 0 {
            return Err(Cs336A2FlashAttentionReferenceError::InvalidConfiguration(
                "key_block_rows must be positive".into(),
            ));
        }
        Ok(self)
    }
}

impl Default for Cs336A2FlashAttentionReferenceConfig {
    fn default() -> Self {
        Self::bounded_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionReferenceForwardStats {
    pub query_block_rows: usize,
    pub key_block_rows: usize,
    pub key_tiles_processed_per_query_tile: usize,
    pub score_tile_elements: usize,
    pub probability_tile_elements: usize,
    pub saved_lse_elements: usize,
    pub saved_output_elements: usize,
    pub peak_temporary_elements: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionReferenceBackwardStats {
    pub query_block_rows: usize,
    pub key_block_rows: usize,
    pub score_tile_elements: usize,
    pub probability_tile_elements: usize,
    pub peak_temporary_elements: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionReferenceForward {
    pub output: AttentionTensor4,
    pub logsumexp: NnTensor,
    pub stats: Cs336A2FlashAttentionReferenceForwardStats,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionReferenceBackward {
    pub d_query: AttentionTensor4,
    pub d_key: AttentionTensor4,
    pub d_value: AttentionTensor4,
    pub stats: Cs336A2FlashAttentionReferenceBackwardStats,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2NaiveAttentionForward {
    pub output: AttentionTensor4,
    pub probability_trace: AttentionTensor4,
    pub logsumexp: NnTensor,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2NaiveAttentionBackward {
    pub d_query: AttentionTensor4,
    pub d_key: AttentionTensor4,
    pub d_value: AttentionTensor4,
}

#[derive(Debug, Error)]
pub enum Cs336A2FlashAttentionReferenceError {
    #[error("invalid CS336 A2 FlashAttention reference configuration: {0}")]
    InvalidConfiguration(String),
    #[error(transparent)]
    Attention(#[from] ScaledDotProductAttentionError),
    #[error(transparent)]
    AttentionMask(#[from] AttentionMaskError),
    #[error(transparent)]
    AttentionTensor(#[from] AttentionTensorError),
    #[error(transparent)]
    Layer(#[from] LayerError),
}

pub fn cs336_a2_naive_attention_forward(
    query: &AttentionTensor4,
    key: &AttentionTensor4,
    value: &AttentionTensor4,
    mask: Option<&AttentionMask>,
) -> Result<Cs336A2NaiveAttentionForward, Cs336A2FlashAttentionReferenceError> {
    validate_attention_inputs(query, key, value, mask)?;
    let output = scaled_dot_product_attention(query, key, value, mask)?;
    let logsumexp = compute_logsumexp_tensor(query, key, mask)?;
    Ok(Cs336A2NaiveAttentionForward {
        output: output.context,
        probability_trace: output.probability_trace.probabilities,
        logsumexp,
    })
}

pub fn cs336_a2_naive_attention_backward(
    query: &AttentionTensor4,
    key: &AttentionTensor4,
    value: &AttentionTensor4,
    forward: &Cs336A2NaiveAttentionForward,
    grad_output: &AttentionTensor4,
    mask: Option<&AttentionMask>,
) -> Result<Cs336A2NaiveAttentionBackward, Cs336A2FlashAttentionReferenceError> {
    validate_attention_inputs(query, key, value, mask)?;
    validate_grad_output_shape(forward.output.shape(), grad_output.shape())?;
    let mut d_query = AttentionTensor4::zeros(query.shape());
    let mut d_key = AttentionTensor4::zeros(key.shape());
    let mut d_value = AttentionTensor4::zeros(value.shape());
    let score_scale = inverse_head_width_scale(query.col_count());

    for batch in 0..query.batch_size() {
        for head in 0..query.head_count() {
            let mut d_row = vec![0.0; query.row_count()];
            for query_index in 0..query.row_count() {
                let mut sum = 0.0f32;
                for value_index in 0..value.col_count() {
                    sum += grad_output.get(batch, head, query_index, value_index)
                        * forward.output.get(batch, head, query_index, value_index);
                }
                d_row[query_index] = sum;
            }

            for query_index in 0..query.row_count() {
                for key_index in 0..key.row_count() {
                    let probability =
                        forward
                            .probability_trace
                            .get(batch, head, query_index, key_index);
                    if probability == 0.0 {
                        continue;
                    }

                    let mut d_probability = 0.0f32;
                    for value_index in 0..value.col_count() {
                        d_probability += grad_output.get(batch, head, query_index, value_index)
                            * value.get(batch, head, key_index, value_index);
                        let d_value_contribution =
                            probability * grad_output.get(batch, head, query_index, value_index);
                        let updated =
                            d_value.get(batch, head, key_index, value_index) + d_value_contribution;
                        d_value.set(batch, head, key_index, value_index, updated);
                    }

                    let d_score = probability * (d_probability - d_row[query_index]);
                    for head_dim_index in 0..query.col_count() {
                        let d_query_contribution =
                            d_score * key.get(batch, head, key_index, head_dim_index) * score_scale;
                        let d_key_contribution = d_score
                            * query.get(batch, head, query_index, head_dim_index)
                            * score_scale;
                        d_query.set(
                            batch,
                            head,
                            query_index,
                            head_dim_index,
                            d_query.get(batch, head, query_index, head_dim_index)
                                + d_query_contribution,
                        );
                        d_key.set(
                            batch,
                            head,
                            key_index,
                            head_dim_index,
                            d_key.get(batch, head, key_index, head_dim_index) + d_key_contribution,
                        );
                    }
                }
            }
        }
    }

    Ok(Cs336A2NaiveAttentionBackward {
        d_query,
        d_key,
        d_value,
    })
}

pub fn cs336_a2_flash_attention_reference_forward(
    query: &AttentionTensor4,
    key: &AttentionTensor4,
    value: &AttentionTensor4,
    mask: Option<&AttentionMask>,
    config: Cs336A2FlashAttentionReferenceConfig,
) -> Result<Cs336A2FlashAttentionReferenceForward, Cs336A2FlashAttentionReferenceError> {
    validate_attention_inputs(query, key, value, mask)?;
    let config = config.validate()?;
    let mut output = AttentionTensor4::zeros([
        query.batch_size(),
        query.head_count(),
        query.row_count(),
        value.col_count(),
    ]);
    let mut logsumexp_values =
        vec![f32::NEG_INFINITY; query.batch_size() * query.head_count() * query.row_count()];
    let score_scale = inverse_head_width_scale(query.col_count());

    for batch in 0..query.batch_size() {
        for head in 0..query.head_count() {
            for query_start in (0..query.row_count()).step_by(config.query_block_rows) {
                let query_block_len =
                    usize::min(config.query_block_rows, query.row_count() - query_start);
                let mut running_max = vec![f32::NEG_INFINITY; query_block_len];
                let mut running_norm = vec![0.0f32; query_block_len];
                let mut running_output = vec![0.0f32; query_block_len * value.col_count()];

                for key_start in (0..key.row_count()).step_by(config.key_block_rows) {
                    let key_block_len =
                        usize::min(config.key_block_rows, key.row_count() - key_start);
                    let mut tile_scores = vec![f32::NEG_INFINITY; query_block_len * key_block_len];
                    let mut tile_row_max = vec![f32::NEG_INFINITY; query_block_len];
                    let mut tile_row_sum = vec![0.0f32; query_block_len];
                    let mut tile_weighted_values =
                        vec![0.0f32; query_block_len * value.col_count()];

                    for query_offset in 0..query_block_len {
                        let query_index = query_start + query_offset;
                        for key_offset in 0..key_block_len {
                            let key_index = key_start + key_offset;
                            if is_masked(mask, batch, query_index, key_index) {
                                continue;
                            }
                            let score =
                                dot_attention_rows(query, key, batch, head, query_index, key_index)
                                    * score_scale;
                            tile_scores[query_offset * key_block_len + key_offset] = score;
                            tile_row_max[query_offset] = tile_row_max[query_offset].max(score);
                        }
                    }

                    for query_offset in 0..query_block_len {
                        let row_max = tile_row_max[query_offset];
                        if row_max == f32::NEG_INFINITY {
                            continue;
                        }
                        for key_offset in 0..key_block_len {
                            let score = tile_scores[query_offset * key_block_len + key_offset];
                            if score == f32::NEG_INFINITY {
                                continue;
                            }
                            let probability = (score - row_max).exp();
                            tile_row_sum[query_offset] += probability;
                            for value_index in 0..value.col_count() {
                                tile_weighted_values
                                    [query_offset * value.col_count() + value_index] += probability
                                    * value.get(batch, head, key_start + key_offset, value_index);
                            }
                        }
                    }

                    for query_offset in 0..query_block_len {
                        let next_max = running_max[query_offset].max(tile_row_max[query_offset]);
                        if next_max == f32::NEG_INFINITY {
                            continue;
                        }
                        let previous_scale = if running_max[query_offset] == f32::NEG_INFINITY {
                            0.0
                        } else {
                            (running_max[query_offset] - next_max).exp()
                        };
                        let current_scale = if tile_row_max[query_offset] == f32::NEG_INFINITY {
                            0.0
                        } else {
                            (tile_row_max[query_offset] - next_max).exp()
                        };
                        let next_norm = previous_scale * running_norm[query_offset]
                            + current_scale * tile_row_sum[query_offset];
                        if next_norm == 0.0 {
                            continue;
                        }

                        for value_index in 0..value.col_count() {
                            let row_index = query_offset * value.col_count() + value_index;
                            let previous_weighted = previous_scale
                                * running_norm[query_offset]
                                * running_output[row_index];
                            let current_weighted = current_scale * tile_weighted_values[row_index];
                            running_output[row_index] =
                                (previous_weighted + current_weighted) / next_norm;
                        }
                        running_max[query_offset] = next_max;
                        running_norm[query_offset] = next_norm;
                    }
                }

                for query_offset in 0..query_block_len {
                    let query_index = query_start + query_offset;
                    for value_index in 0..value.col_count() {
                        output.set(
                            batch,
                            head,
                            query_index,
                            value_index,
                            running_output[query_offset * value.col_count() + value_index],
                        );
                    }
                    logsumexp_values[lse_index(
                        batch,
                        head,
                        query_index,
                        query.head_count(),
                        query.row_count(),
                    )] = if running_norm[query_offset] == 0.0 {
                        f32::NEG_INFINITY
                    } else {
                        running_max[query_offset] + running_norm[query_offset].ln()
                    };
                }
            }
        }
    }

    let logsumexp = NnTensor::f32(
        Shape::new(vec![
            query.batch_size(),
            query.head_count(),
            query.row_count(),
        ]),
        logsumexp_values,
    )?;
    Ok(Cs336A2FlashAttentionReferenceForward {
        output,
        logsumexp,
        stats: Cs336A2FlashAttentionReferenceForwardStats {
            query_block_rows: config.query_block_rows,
            key_block_rows: config.key_block_rows,
            key_tiles_processed_per_query_tile: key.row_count().div_ceil(config.key_block_rows),
            score_tile_elements: config.query_block_rows * config.key_block_rows,
            probability_tile_elements: config.query_block_rows * config.key_block_rows,
            saved_lse_elements: query.batch_size() * query.head_count() * query.row_count(),
            saved_output_elements: query.batch_size()
                * query.head_count()
                * query.row_count()
                * value.col_count(),
            peak_temporary_elements: (config.query_block_rows * config.key_block_rows * 2)
                + (config.query_block_rows * value.col_count()),
        },
    })
}

pub fn cs336_a2_flash_attention_reference_backward(
    query: &AttentionTensor4,
    key: &AttentionTensor4,
    value: &AttentionTensor4,
    forward: &Cs336A2FlashAttentionReferenceForward,
    grad_output: &AttentionTensor4,
    mask: Option<&AttentionMask>,
    config: Cs336A2FlashAttentionReferenceConfig,
) -> Result<Cs336A2FlashAttentionReferenceBackward, Cs336A2FlashAttentionReferenceError> {
    validate_attention_inputs(query, key, value, mask)?;
    let config = config.validate()?;
    validate_grad_output_shape(forward.output.shape(), grad_output.shape())?;
    validate_logsumexp_shape(query, &forward.logsumexp)?;
    let logsumexp_values = forward.logsumexp.as_f32_slice()?;
    let score_scale = inverse_head_width_scale(query.col_count());
    let mut d_query = AttentionTensor4::zeros(query.shape());
    let mut d_key = AttentionTensor4::zeros(key.shape());
    let mut d_value = AttentionTensor4::zeros(value.shape());

    for batch in 0..query.batch_size() {
        for head in 0..query.head_count() {
            let mut d_row = vec![0.0f32; query.row_count()];
            for query_index in 0..query.row_count() {
                let mut dot = 0.0f32;
                for value_index in 0..value.col_count() {
                    dot += grad_output.get(batch, head, query_index, value_index)
                        * forward.output.get(batch, head, query_index, value_index);
                }
                d_row[query_index] = dot;
            }

            for query_start in (0..query.row_count()).step_by(config.query_block_rows) {
                let query_block_len =
                    usize::min(config.query_block_rows, query.row_count() - query_start);
                for key_start in (0..key.row_count()).step_by(config.key_block_rows) {
                    let key_block_len =
                        usize::min(config.key_block_rows, key.row_count() - key_start);
                    let mut tile_probabilities = vec![0.0f32; query_block_len * key_block_len];
                    let mut tile_d_score = vec![0.0f32; query_block_len * key_block_len];

                    for query_offset in 0..query_block_len {
                        let query_index = query_start + query_offset;
                        let lse = logsumexp_values[lse_index(
                            batch,
                            head,
                            query_index,
                            query.head_count(),
                            query.row_count(),
                        )];
                        if lse == f32::NEG_INFINITY {
                            continue;
                        }
                        for key_offset in 0..key_block_len {
                            let key_index = key_start + key_offset;
                            if is_masked(mask, batch, query_index, key_index) {
                                continue;
                            }
                            let score =
                                dot_attention_rows(query, key, batch, head, query_index, key_index)
                                    * score_scale;
                            let probability = (score - lse).exp();
                            tile_probabilities[query_offset * key_block_len + key_offset] =
                                probability;
                            let mut d_probability = 0.0f32;
                            for value_index in 0..value.col_count() {
                                d_probability +=
                                    grad_output.get(batch, head, query_index, value_index)
                                        * value.get(batch, head, key_index, value_index);
                                let d_value_contribution = probability
                                    * grad_output.get(batch, head, query_index, value_index);
                                d_value.set(
                                    batch,
                                    head,
                                    key_index,
                                    value_index,
                                    d_value.get(batch, head, key_index, value_index)
                                        + d_value_contribution,
                                );
                            }
                            tile_d_score[query_offset * key_block_len + key_offset] =
                                probability * (d_probability - d_row[query_index]);
                        }
                    }

                    for query_offset in 0..query_block_len {
                        let query_index = query_start + query_offset;
                        for key_offset in 0..key_block_len {
                            let key_index = key_start + key_offset;
                            let d_score = tile_d_score[query_offset * key_block_len + key_offset];
                            if d_score == 0.0 {
                                continue;
                            }
                            for head_dim_index in 0..query.col_count() {
                                d_query.set(
                                    batch,
                                    head,
                                    query_index,
                                    head_dim_index,
                                    d_query.get(batch, head, query_index, head_dim_index)
                                        + d_score
                                            * key.get(batch, head, key_index, head_dim_index)
                                            * score_scale,
                                );
                                d_key.set(
                                    batch,
                                    head,
                                    key_index,
                                    head_dim_index,
                                    d_key.get(batch, head, key_index, head_dim_index)
                                        + d_score
                                            * query.get(batch, head, query_index, head_dim_index)
                                            * score_scale,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(Cs336A2FlashAttentionReferenceBackward {
        d_query,
        d_key,
        d_value,
        stats: Cs336A2FlashAttentionReferenceBackwardStats {
            query_block_rows: config.query_block_rows,
            key_block_rows: config.key_block_rows,
            score_tile_elements: config.query_block_rows * config.key_block_rows,
            probability_tile_elements: config.query_block_rows * config.key_block_rows,
            peak_temporary_elements: config.query_block_rows * config.key_block_rows * 2,
        },
    })
}

fn validate_attention_inputs(
    query: &AttentionTensor4,
    key: &AttentionTensor4,
    value: &AttentionTensor4,
    mask: Option<&AttentionMask>,
) -> Result<(), Cs336A2FlashAttentionReferenceError> {
    if query.col_count() == 0 {
        return Err(ScaledDotProductAttentionError::EmptyHeadDim.into());
    }
    if query.batch_size() != key.batch_size() || key.batch_size() != value.batch_size() {
        return Err(ScaledDotProductAttentionError::BatchSizeMismatch {
            query_batch: query.batch_size(),
            key_batch: key.batch_size(),
            value_batch: value.batch_size(),
        }
        .into());
    }
    if query.head_count() != key.head_count() || key.head_count() != value.head_count() {
        return Err(ScaledDotProductAttentionError::HeadCountMismatch {
            query_heads: query.head_count(),
            key_heads: key.head_count(),
            value_heads: value.head_count(),
        }
        .into());
    }
    if query.col_count() != key.col_count() {
        return Err(ScaledDotProductAttentionError::QueryKeyWidthMismatch {
            query_width: query.col_count(),
            key_width: key.col_count(),
        }
        .into());
    }
    if key.row_count() != value.row_count() {
        return Err(ScaledDotProductAttentionError::KeyValueRowMismatch {
            key_rows: key.row_count(),
            value_rows: value.row_count(),
        }
        .into());
    }
    let expected_mask_shape = [query.batch_size(), query.row_count(), key.row_count()];
    if let Some(mask) = mask {
        if mask.shape() != expected_mask_shape {
            return Err(ScaledDotProductAttentionError::MaskShapeMismatch {
                expected: expected_mask_shape,
                actual: mask.shape(),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_grad_output_shape(
    expected: [usize; 4],
    actual: [usize; 4],
) -> Result<(), Cs336A2FlashAttentionReferenceError> {
    if expected != actual {
        return Err(Cs336A2FlashAttentionReferenceError::InvalidConfiguration(
            format!("grad_output shape {actual:?} must match forward output shape {expected:?}"),
        ));
    }
    Ok(())
}

fn validate_logsumexp_shape(
    query: &AttentionTensor4,
    logsumexp: &NnTensor,
) -> Result<(), Cs336A2FlashAttentionReferenceError> {
    let expected = vec![query.batch_size(), query.head_count(), query.row_count()];
    if logsumexp.dims() != expected.as_slice() {
        return Err(Cs336A2FlashAttentionReferenceError::InvalidConfiguration(
            format!(
                "logsumexp shape {:?} must match {:?}",
                logsumexp.dims(),
                expected
            ),
        ));
    }
    Ok(())
}

fn compute_logsumexp_tensor(
    query: &AttentionTensor4,
    key: &AttentionTensor4,
    mask: Option<&AttentionMask>,
) -> Result<NnTensor, Cs336A2FlashAttentionReferenceError> {
    let mut values =
        vec![f32::NEG_INFINITY; query.batch_size() * query.head_count() * query.row_count()];
    let score_scale = inverse_head_width_scale(query.col_count());
    for batch in 0..query.batch_size() {
        for head in 0..query.head_count() {
            for query_index in 0..query.row_count() {
                let mut row_max = f32::NEG_INFINITY;
                for key_index in 0..key.row_count() {
                    if is_masked(mask, batch, query_index, key_index) {
                        continue;
                    }
                    let score = dot_attention_rows(query, key, batch, head, query_index, key_index)
                        * score_scale;
                    row_max = row_max.max(score);
                }
                let value = if row_max == f32::NEG_INFINITY {
                    f32::NEG_INFINITY
                } else {
                    let mut sum = 0.0f32;
                    for key_index in 0..key.row_count() {
                        if is_masked(mask, batch, query_index, key_index) {
                            continue;
                        }
                        let score =
                            dot_attention_rows(query, key, batch, head, query_index, key_index)
                                * score_scale;
                        sum += (score - row_max).exp();
                    }
                    row_max + sum.ln()
                };
                values[lse_index(
                    batch,
                    head,
                    query_index,
                    query.head_count(),
                    query.row_count(),
                )] = value;
            }
        }
    }
    Ok(NnTensor::f32(
        Shape::new(vec![
            query.batch_size(),
            query.head_count(),
            query.row_count(),
        ]),
        values,
    )?)
}

fn dot_attention_rows(
    query: &AttentionTensor4,
    key: &AttentionTensor4,
    batch: usize,
    head: usize,
    query_index: usize,
    key_index: usize,
) -> f32 {
    let mut dot = 0.0f32;
    for head_dim_index in 0..query.col_count() {
        dot += query.get(batch, head, query_index, head_dim_index)
            * key.get(batch, head, key_index, head_dim_index);
    }
    dot
}

fn inverse_head_width_scale(head_width: usize) -> f32 {
    1.0 / (head_width as f32).sqrt()
}

fn is_masked(mask: Option<&AttentionMask>, batch: usize, query: usize, key: usize) -> bool {
    mask.is_some_and(|mask| !mask.allows(batch, query, key))
}

fn lse_index(batch: usize, head: usize, row: usize, head_count: usize, row_count: usize) -> usize {
    (batch * head_count + head) * row_count + row
}

#[cfg(test)]
mod tests {
    use super::{
        Cs336A2FlashAttentionReferenceConfig, cs336_a2_flash_attention_reference_backward,
        cs336_a2_flash_attention_reference_forward, cs336_a2_naive_attention_backward,
        cs336_a2_naive_attention_forward,
    };
    use psionic_transformer::{AttentionMask, AttentionTensor4};

    fn deterministic_attention_tensor(
        shape: [usize; 4],
        start: f32,
        step: f32,
    ) -> AttentionTensor4 {
        let mut values = Vec::with_capacity(shape.iter().product());
        for index in 0..shape.iter().product::<usize>() {
            values.push(start + index as f32 * step);
        }
        AttentionTensor4::new(shape, values).expect("deterministic attention tensor")
    }

    fn max_abs_diff(left: &[f32], right: &[f32]) -> f32 {
        left.iter()
            .zip(right.iter())
            .map(|(left, right)| (left - right).abs())
            .fold(0.0f32, f32::max)
    }

    #[test]
    fn cs336_a2_flash_attention_reference_forward_matches_naive_baseline()
    -> Result<(), Box<dyn std::error::Error>> {
        let shape = [2, 2, 8, 4];
        let query = deterministic_attention_tensor(shape, -0.2, 0.01);
        let key = deterministic_attention_tensor(shape, 0.1, 0.02);
        let value = deterministic_attention_tensor(shape, -0.3, 0.015);
        let mask = AttentionMask::causal(shape[0], shape[2], shape[2]);
        let naive = cs336_a2_naive_attention_forward(&query, &key, &value, Some(&mask))?;
        let reference = cs336_a2_flash_attention_reference_forward(
            &query,
            &key,
            &value,
            Some(&mask),
            Cs336A2FlashAttentionReferenceConfig {
                query_block_rows: 3,
                key_block_rows: 2,
            },
        )?;
        assert!(reference.output.max_abs_diff(&naive.output)? <= 1e-4);
        assert!(
            max_abs_diff(
                reference.logsumexp.as_f32_slice()?,
                naive.logsumexp.as_f32_slice()?
            ) <= 1e-4
        );
        Ok(())
    }

    #[test]
    fn cs336_a2_flash_attention_reference_backward_matches_naive_baseline()
    -> Result<(), Box<dyn std::error::Error>> {
        let shape = [2, 2, 8, 4];
        let query = deterministic_attention_tensor(shape, -0.1, 0.013);
        let key = deterministic_attention_tensor(shape, 0.2, 0.011);
        let value = deterministic_attention_tensor(shape, -0.4, 0.017);
        let grad_output = deterministic_attention_tensor(shape, 0.3, 0.009);
        let mask = AttentionMask::causal(shape[0], shape[2], shape[2]);
        let naive_forward = cs336_a2_naive_attention_forward(&query, &key, &value, Some(&mask))?;
        let naive_backward = cs336_a2_naive_attention_backward(
            &query,
            &key,
            &value,
            &naive_forward,
            &grad_output,
            Some(&mask),
        )?;
        let reference_forward = cs336_a2_flash_attention_reference_forward(
            &query,
            &key,
            &value,
            Some(&mask),
            Cs336A2FlashAttentionReferenceConfig {
                query_block_rows: 4,
                key_block_rows: 3,
            },
        )?;
        let reference_backward = cs336_a2_flash_attention_reference_backward(
            &query,
            &key,
            &value,
            &reference_forward,
            &grad_output,
            Some(&mask),
            Cs336A2FlashAttentionReferenceConfig {
                query_block_rows: 4,
                key_block_rows: 3,
            },
        )?;
        assert!(
            reference_backward
                .d_query
                .max_abs_diff(&naive_backward.d_query)?
                <= 1e-4
        );
        assert!(
            reference_backward
                .d_key
                .max_abs_diff(&naive_backward.d_key)?
                <= 1e-4
        );
        assert!(
            reference_backward
                .d_value
                .max_abs_diff(&naive_backward.d_value)?
                <= 1e-4
        );
        Ok(())
    }
}
