use psionic_array::ArrayContext;
use psionic_core::{DType, Device, Shape, TensorData, TensorSpec};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Dense four-dimensional tensor used by the reusable attention primitive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttentionTensor4 {
    /// Public shape in `[batch, heads, rows, cols]` order.
    pub shape: [usize; 4],
    /// Public row-major values for replay-safe export.
    pub values: Vec<f32>,
}

impl AttentionTensor4 {
    /// Creates one tensor from a shape and flat row-major values.
    pub fn new(shape: [usize; 4], values: Vec<f32>) -> Result<Self, AttentionTensorError> {
        let expected = shape.iter().copied().product::<usize>();
        if values.len() != expected {
            return Err(AttentionTensorError::InvalidValueCount {
                shape,
                actual: values.len(),
                expected,
            });
        }
        Ok(Self { shape, values })
    }

    /// Creates one zero tensor.
    #[must_use]
    pub fn zeros(shape: [usize; 4]) -> Self {
        let expected = shape.iter().copied().product::<usize>();
        Self {
            shape,
            values: vec![0.0; expected],
        }
    }

    /// Creates one tensor from nested `[batch][head][row][col]` values.
    pub fn from_nested(values: Vec<Vec<Vec<Vec<f32>>>>) -> Result<Self, AttentionTensorError> {
        if values.is_empty() {
            return Err(AttentionTensorError::EmptyBatch);
        }
        let head_count = values.first().map(Vec::len).unwrap_or(0);
        if head_count == 0 {
            return Err(AttentionTensorError::EmptyHeadCount);
        }
        let row_count = values
            .first()
            .and_then(|batch| batch.first())
            .map(Vec::len)
            .unwrap_or(0);
        if row_count == 0 {
            return Err(AttentionTensorError::EmptyRowCount);
        }
        let col_count = values
            .first()
            .and_then(|batch| batch.first())
            .and_then(|head| head.first())
            .map(Vec::len)
            .unwrap_or(0);
        if col_count == 0 {
            return Err(AttentionTensorError::EmptyColCount);
        }

        let mut flat = Vec::with_capacity(values.len() * head_count * row_count * col_count);
        for (batch_index, batch) in values.iter().enumerate() {
            if batch.len() != head_count {
                return Err(AttentionTensorError::RaggedHeadCount {
                    batch_index,
                    expected: head_count,
                    actual: batch.len(),
                });
            }
            for (head_index, head) in batch.iter().enumerate() {
                if head.len() != row_count {
                    return Err(AttentionTensorError::RaggedRowCount {
                        batch_index,
                        head_index,
                        expected: row_count,
                        actual: head.len(),
                    });
                }
                for (row_index, row) in head.iter().enumerate() {
                    if row.len() != col_count {
                        return Err(AttentionTensorError::RaggedColCount {
                            batch_index,
                            head_index,
                            row_index,
                            expected: col_count,
                            actual: row.len(),
                        });
                    }
                    flat.extend_from_slice(row);
                }
            }
        }

        Self::new([values.len(), head_count, row_count, col_count], flat)
    }

    /// Returns the tensor shape.
    #[must_use]
    pub const fn shape(&self) -> [usize; 4] {
        self.shape
    }

    /// Returns the batch size.
    #[must_use]
    pub const fn batch_size(&self) -> usize {
        self.shape[0]
    }

    /// Returns the head count.
    #[must_use]
    pub const fn head_count(&self) -> usize {
        self.shape[1]
    }

    /// Returns the row count.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.shape[2]
    }

    /// Returns the column count.
    #[must_use]
    pub const fn col_count(&self) -> usize {
        self.shape[3]
    }

    /// Returns the flat row-major values.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Returns the tensor as a `psionic-core` tensor spec.
    #[must_use]
    pub fn tensor_spec(&self) -> TensorSpec {
        TensorSpec::new(
            Shape::new(vec![
                self.batch_size(),
                self.head_count(),
                self.row_count(),
                self.col_count(),
            ]),
            DType::F32,
            Device::cpu(),
        )
    }

    /// Returns the tensor as dense host-visible core tensor data.
    #[must_use]
    pub fn tensor_data(&self) -> TensorData {
        TensorData::F32(self.values.clone())
    }

    /// Returns one element.
    #[must_use]
    pub fn get(&self, batch: usize, head: usize, row: usize, col: usize) -> f32 {
        self.values[self.index(batch, head, row, col)]
    }

    /// Sets one element.
    pub fn set(&mut self, batch: usize, head: usize, row: usize, col: usize, value: f32) {
        let index = self.index(batch, head, row, col);
        self.values[index] = value;
    }

    /// Returns one contiguous `[rows, cols]` matrix for one batch and head.
    #[must_use]
    pub fn matrix_values(&self, batch: usize, head: usize) -> Vec<f32> {
        let matrix_len = self.row_count() * self.col_count();
        let start = (batch * self.head_count() + head) * matrix_len;
        self.values[start..start + matrix_len].to_vec()
    }

    /// Writes one contiguous `[rows, cols]` matrix for one batch and head.
    pub fn set_matrix_values(
        &mut self,
        batch: usize,
        head: usize,
        values: &[f32],
    ) -> Result<(), AttentionTensorError> {
        let expected = self.row_count() * self.col_count();
        if values.len() != expected {
            return Err(AttentionTensorError::InvalidValueCount {
                shape: [1, 1, self.row_count(), self.col_count()],
                actual: values.len(),
                expected,
            });
        }
        let start = (batch * self.head_count() + head) * expected;
        self.values[start..start + expected].copy_from_slice(values);
        Ok(())
    }

    /// Returns the maximum absolute difference between two tensors.
    pub fn max_abs_diff(&self, other: &Self) -> Result<f32, AttentionTensorError> {
        if self.shape != other.shape {
            return Err(AttentionTensorError::ShapeMismatch {
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

    #[must_use]
    fn index(&self, batch: usize, head: usize, row: usize, col: usize) -> usize {
        (((batch * self.head_count()) + head) * self.row_count() + row) * self.col_count() + col
    }
}

/// Dense broadcast mask over `[batch, query, key]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionMask {
    /// Public shape in `[batch, query, key]` order.
    pub shape: [usize; 3],
    /// Public allow/deny values in row-major order.
    pub allowed: Vec<bool>,
}

impl AttentionMask {
    /// Creates one mask from a shape and flat row-major values.
    pub fn new(shape: [usize; 3], allowed: Vec<bool>) -> Result<Self, AttentionMaskError> {
        let expected = shape.iter().copied().product::<usize>();
        if allowed.len() != expected {
            return Err(AttentionMaskError::InvalidValueCount {
                shape,
                actual: allowed.len(),
                expected,
            });
        }
        Ok(Self { shape, allowed })
    }

    /// Creates one causal mask that blocks future keys.
    #[must_use]
    pub fn causal(batch_size: usize, query_length: usize, key_length: usize) -> Self {
        let mut allowed = vec![false; batch_size * query_length * key_length];
        for batch in 0..batch_size {
            for query_index in 0..query_length {
                for key_index in 0..key_length {
                    let index = (batch * query_length + query_index) * key_length + key_index;
                    allowed[index] = key_index <= query_index;
                }
            }
        }
        Self {
            shape: [batch_size, query_length, key_length],
            allowed,
        }
    }

    /// Creates one padding mask from valid-key flags for each batch element.
    pub fn from_padding_tokens(
        valid_tokens: Vec<Vec<bool>>,
        query_length: usize,
    ) -> Result<Self, AttentionMaskError> {
        if valid_tokens.is_empty() {
            return Err(AttentionMaskError::EmptyBatch);
        }
        if query_length == 0 {
            return Err(AttentionMaskError::EmptyQueryLength);
        }
        let key_length = valid_tokens.first().map(Vec::len).unwrap_or(0);
        if key_length == 0 {
            return Err(AttentionMaskError::EmptyKeyLength);
        }

        let mut allowed = Vec::with_capacity(valid_tokens.len() * query_length * key_length);
        for (batch_index, batch) in valid_tokens.iter().enumerate() {
            if batch.len() != key_length {
                return Err(AttentionMaskError::RaggedKeyLength {
                    batch_index,
                    expected: key_length,
                    actual: batch.len(),
                });
            }
            for _ in 0..query_length {
                allowed.extend(batch.iter().copied());
            }
        }

        Self::new([valid_tokens.len(), query_length, key_length], allowed)
    }

    /// Returns the mask shape.
    #[must_use]
    pub const fn shape(&self) -> [usize; 3] {
        self.shape
    }

    /// Returns whether one query-to-key edge is allowed.
    #[must_use]
    pub fn allows(&self, batch: usize, query: usize, key: usize) -> bool {
        self.allowed[self.index(batch, query, key)]
    }

    /// Combines two masks with logical AND.
    pub fn combine(&self, other: &Self) -> Result<Self, AttentionMaskError> {
        if self.shape != other.shape {
            return Err(AttentionMaskError::ShapeMismatch {
                left: self.shape,
                right: other.shape,
            });
        }
        Ok(Self {
            shape: self.shape,
            allowed: self
                .allowed
                .iter()
                .zip(other.allowed.iter())
                .map(|(left, right)| *left && *right)
                .collect(),
        })
    }

    #[must_use]
    fn index(&self, batch: usize, query: usize, key: usize) -> usize {
        (batch * self.shape[1] + query) * self.shape[2] + key
    }
}

/// Exported probability trace for one attention pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttentionProbabilityTrace {
    /// Attention probabilities in `[batch, heads, query, key]` order.
    pub probabilities: AttentionTensor4,
}

impl AttentionProbabilityTrace {
    /// Returns the probability trace as a core tensor spec.
    #[must_use]
    pub fn tensor_spec(&self) -> TensorSpec {
        self.probabilities.tensor_spec()
    }

    /// Returns the probability trace as dense host-visible core tensor data.
    #[must_use]
    pub fn tensor_data(&self) -> TensorData {
        self.probabilities.tensor_data()
    }
}

/// Output of one scaled dot-product attention pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScaledDotProductAttentionOutput {
    /// Context tensor in `[batch, heads, query, value_width]` order.
    pub context: AttentionTensor4,
    /// Probability trace in `[batch, heads, query, key]` order.
    pub probability_trace: AttentionProbabilityTrace,
}

/// Ragged or shape failures while constructing one attention tensor.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AttentionTensorError {
    /// Flat values did not match the declared shape.
    #[error("invalid value count {actual} for shape {shape:?}; expected {expected}")]
    InvalidValueCount {
        /// Declared shape.
        shape: [usize; 4],
        /// Actual flat value count.
        actual: usize,
        /// Expected flat value count.
        expected: usize,
    },
    /// Nested construction requires at least one batch.
    #[error("attention tensor input must contain at least one batch")]
    EmptyBatch,
    /// Nested construction requires at least one head.
    #[error("attention tensor input must contain at least one head")]
    EmptyHeadCount,
    /// Nested construction requires at least one row.
    #[error("attention tensor input must contain at least one row")]
    EmptyRowCount,
    /// Nested construction requires at least one column.
    #[error("attention tensor input must contain at least one column")]
    EmptyColCount,
    /// Nested construction saw inconsistent head counts.
    #[error(
        "attention tensor input is ragged at batch {batch_index}: expected head count {expected}, got {actual}"
    )]
    RaggedHeadCount {
        /// Batch index that diverged.
        batch_index: usize,
        /// Expected head count.
        expected: usize,
        /// Actual head count.
        actual: usize,
    },
    /// Nested construction saw inconsistent row counts.
    #[error(
        "attention tensor input is ragged at batch {batch_index}, head {head_index}: expected row count {expected}, got {actual}"
    )]
    RaggedRowCount {
        /// Batch index that diverged.
        batch_index: usize,
        /// Head index that diverged.
        head_index: usize,
        /// Expected row count.
        expected: usize,
        /// Actual row count.
        actual: usize,
    },
    /// Nested construction saw inconsistent column counts.
    #[error(
        "attention tensor input is ragged at batch {batch_index}, head {head_index}, row {row_index}: expected column count {expected}, got {actual}"
    )]
    RaggedColCount {
        /// Batch index that diverged.
        batch_index: usize,
        /// Head index that diverged.
        head_index: usize,
        /// Row index that diverged.
        row_index: usize,
        /// Expected column count.
        expected: usize,
        /// Actual column count.
        actual: usize,
    },
    /// Tensor comparisons require matching shapes.
    #[error("tensor shapes do not match: left {left:?}, right {right:?}")]
    ShapeMismatch {
        /// Left-hand shape.
        left: [usize; 4],
        /// Right-hand shape.
        right: [usize; 4],
    },
}

/// Shape or ragged failures while constructing or combining one attention mask.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AttentionMaskError {
    /// Flat values did not match the declared shape.
    #[error("invalid mask value count {actual} for shape {shape:?}; expected {expected}")]
    InvalidValueCount {
        /// Declared mask shape.
        shape: [usize; 3],
        /// Actual flat value count.
        actual: usize,
        /// Expected flat value count.
        expected: usize,
    },
    /// Padding-mask construction requires at least one batch element.
    #[error("padding mask input must contain at least one batch")]
    EmptyBatch,
    /// Padding-mask construction requires a positive query length.
    #[error("padding mask query length must be positive")]
    EmptyQueryLength,
    /// Padding-mask construction requires at least one key position.
    #[error("padding mask input must contain at least one key position")]
    EmptyKeyLength,
    /// Padding-mask construction saw inconsistent key lengths.
    #[error(
        "padding mask input is ragged at batch {batch_index}: expected key length {expected}, got {actual}"
    )]
    RaggedKeyLength {
        /// Batch index that diverged.
        batch_index: usize,
        /// Expected key length.
        expected: usize,
        /// Actual key length.
        actual: usize,
    },
    /// Mask combinations require identical shapes.
    #[error("mask shapes do not match: left {left:?}, right {right:?}")]
    ShapeMismatch {
        /// Left-hand shape.
        left: [usize; 3],
        /// Right-hand shape.
        right: [usize; 3],
    },
}

/// Execution failures for the reusable scaled dot-product attention primitive.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ScaledDotProductAttentionError {
    /// The query tensor width must be strictly positive.
    #[error("attention head width must be positive")]
    EmptyHeadDim,
    /// Query, key, and value must share a batch size.
    #[error(
        "attention tensors must share batch size; query={query_batch}, key={key_batch}, value={value_batch}"
    )]
    BatchSizeMismatch {
        /// Query batch size.
        query_batch: usize,
        /// Key batch size.
        key_batch: usize,
        /// Value batch size.
        value_batch: usize,
    },
    /// Query, key, and value must share a head count.
    #[error(
        "attention tensors must share head count; query={query_heads}, key={key_heads}, value={value_heads}"
    )]
    HeadCountMismatch {
        /// Query head count.
        query_heads: usize,
        /// Key head count.
        key_heads: usize,
        /// Value head count.
        value_heads: usize,
    },
    /// Query and key must share the same head width.
    #[error("query width {query_width} must match key width {key_width}")]
    QueryKeyWidthMismatch {
        /// Query width.
        query_width: usize,
        /// Key width.
        key_width: usize,
    },
    /// Key and value must share the same row count.
    #[error("key row count {key_rows} must match value row count {value_rows}")]
    KeyValueRowMismatch {
        /// Key row count.
        key_rows: usize,
        /// Value row count.
        value_rows: usize,
    },
    /// A supplied mask must match the query/key broadcast shape.
    #[error("mask shape {actual:?} must match [batch, query, key] shape {expected:?}")]
    MaskShapeMismatch {
        /// Expected mask shape.
        expected: [usize; 3],
        /// Actual mask shape.
        actual: [usize; 3],
    },
    /// The array substrate refused or failed one matrix product.
    #[error("array-backed matmul failed during `{operation}`: {detail}")]
    ArrayMatmulFailed {
        /// Logical operation label.
        operation: &'static str,
        /// Plain-language refusal or failure detail.
        detail: String,
    },
    /// The array substrate did not return dense `f32` data.
    #[error("array-backed matmul for `{operation}` did not return dense f32 data")]
    NonF32ArrayResult {
        /// Logical operation label.
        operation: &'static str,
    },
}

/// Runs one exact scaled dot-product attention pass with optional masking.
pub fn scaled_dot_product_attention(
    query: &AttentionTensor4,
    key: &AttentionTensor4,
    value: &AttentionTensor4,
    mask: Option<&AttentionMask>,
) -> Result<ScaledDotProductAttentionOutput, ScaledDotProductAttentionError> {
    if query.col_count() == 0 {
        return Err(ScaledDotProductAttentionError::EmptyHeadDim);
    }
    if query.batch_size() != key.batch_size() || key.batch_size() != value.batch_size() {
        return Err(ScaledDotProductAttentionError::BatchSizeMismatch {
            query_batch: query.batch_size(),
            key_batch: key.batch_size(),
            value_batch: value.batch_size(),
        });
    }
    if query.head_count() != key.head_count() || key.head_count() != value.head_count() {
        return Err(ScaledDotProductAttentionError::HeadCountMismatch {
            query_heads: query.head_count(),
            key_heads: key.head_count(),
            value_heads: value.head_count(),
        });
    }
    if query.col_count() != key.col_count() {
        return Err(ScaledDotProductAttentionError::QueryKeyWidthMismatch {
            query_width: query.col_count(),
            key_width: key.col_count(),
        });
    }
    if key.row_count() != value.row_count() {
        return Err(ScaledDotProductAttentionError::KeyValueRowMismatch {
            key_rows: key.row_count(),
            value_rows: value.row_count(),
        });
    }

    let expected_mask_shape = [query.batch_size(), query.row_count(), key.row_count()];
    if let Some(mask) = mask {
        if mask.shape() != expected_mask_shape {
            return Err(ScaledDotProductAttentionError::MaskShapeMismatch {
                expected: expected_mask_shape,
                actual: mask.shape(),
            });
        }
    }

    let mut context = AttentionTensor4::zeros([
        query.batch_size(),
        query.head_count(),
        query.row_count(),
        value.col_count(),
    ]);
    let mut probability_trace = AttentionTensor4::zeros([
        query.batch_size(),
        query.head_count(),
        query.row_count(),
        key.row_count(),
    ]);
    let scale = (query.col_count() as f32).sqrt();

    for batch in 0..query.batch_size() {
        for head in 0..query.head_count() {
            let query_matrix = query.matrix_values(batch, head);
            let key_matrix = key.matrix_values(batch, head);
            let value_matrix = value.matrix_values(batch, head);
            let key_transposed = transpose_matrix(key.row_count(), key.col_count(), &key_matrix);

            let mut logits = matmul_with_array(
                "qk_transpose",
                query.row_count(),
                query.col_count(),
                key.row_count(),
                &query_matrix,
                &key_transposed,
            )?;

            for value in &mut logits {
                *value /= scale;
            }

            let mut probabilities = vec![0.0; query.row_count() * key.row_count()];
            for query_index in 0..query.row_count() {
                let row_start = query_index * key.row_count();
                let row_end = row_start + key.row_count();
                let row_logits = &logits[row_start..row_end];
                let row_mask = (0..key.row_count())
                    .map(|key_index| match mask {
                        Some(mask) => mask.allows(batch, query_index, key_index),
                        None => true,
                    })
                    .collect::<Vec<_>>();
                apply_stable_softmax(row_logits, &row_mask, &mut probabilities[row_start..row_end]);
            }

            let context_matrix = matmul_with_array(
                "probabilities_value",
                query.row_count(),
                key.row_count(),
                value.col_count(),
                &probabilities,
                &value_matrix,
            )?;

            context
                .set_matrix_values(batch, head, &context_matrix)
                .map_err(|error| ScaledDotProductAttentionError::ArrayMatmulFailed {
                    operation: "probabilities_value_writeback",
                    detail: error.to_string(),
                })?;
            probability_trace
                .set_matrix_values(batch, head, &probabilities)
                .map_err(|error| ScaledDotProductAttentionError::ArrayMatmulFailed {
                    operation: "probabilities_trace_writeback",
                    detail: error.to_string(),
                })?;
        }
    }

    Ok(ScaledDotProductAttentionOutput {
        context,
        probability_trace: AttentionProbabilityTrace {
            probabilities: probability_trace,
        },
    })
}

fn apply_stable_softmax(logits: &[f32], mask: &[bool], output: &mut [f32]) {
    debug_assert_eq!(logits.len(), mask.len());
    debug_assert_eq!(logits.len(), output.len());

    let mut max_value = f32::NEG_INFINITY;
    let mut allowed_count = 0usize;
    for (value, allowed) in logits.iter().zip(mask.iter()) {
        if *allowed {
            max_value = max_value.max(*value);
            allowed_count += 1;
        }
    }

    if allowed_count == 0 {
        output.fill(0.0);
        return;
    }

    let mut sum_exp = 0.0f32;
    for index in 0..logits.len() {
        if mask[index] {
            let shifted = (logits[index] - max_value).exp();
            output[index] = shifted;
            sum_exp += shifted;
        } else {
            output[index] = 0.0;
        }
    }

    for index in 0..output.len() {
        if mask[index] {
            output[index] /= sum_exp;
        }
    }
}

fn matmul_with_array(
    operation: &'static str,
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_cols: usize,
    lhs_values: &[f32],
    rhs_values: &[f32],
) -> Result<Vec<f32>, ScaledDotProductAttentionError> {
    let context = ArrayContext::cpu();
    let lhs = context
        .constant_f32(Shape::new(vec![lhs_rows, lhs_cols]), lhs_values.to_vec())
        .map_err(|error| ScaledDotProductAttentionError::ArrayMatmulFailed {
            operation,
            detail: error.to_string(),
        })?;
    let rhs = context
        .constant_f32(Shape::new(vec![lhs_cols, rhs_cols]), rhs_values.to_vec())
        .map_err(|error| ScaledDotProductAttentionError::ArrayMatmulFailed {
            operation,
            detail: error.to_string(),
        })?;
    let output = lhs
        .matmul(&rhs)
        .map_err(|error| ScaledDotProductAttentionError::ArrayMatmulFailed {
            operation,
            detail: error.to_string(),
        })?;
    let evaluated =
        output
            .eval()
            .map_err(|error| ScaledDotProductAttentionError::ArrayMatmulFailed {
                operation,
                detail: error.to_string(),
            })?;
    let host_data = evaluated
        .to_host_data()
        .map_err(|error| ScaledDotProductAttentionError::ArrayMatmulFailed {
            operation,
            detail: error.to_string(),
        })?;
    let values = host_data
        .as_f32_slice()
        .ok_or(ScaledDotProductAttentionError::NonF32ArrayResult { operation })?;
    Ok(values.to_vec())
}

fn transpose_matrix(rows: usize, cols: usize, values: &[f32]) -> Vec<f32> {
    let mut transposed = vec![0.0; rows * cols];
    for row in 0..rows {
        for col in 0..cols {
            transposed[col * rows + row] = values[row * cols + col];
        }
    }
    transposed
}

#[cfg(test)]
mod tests {
    use super::{
        scaled_dot_product_attention, AttentionMask, AttentionMaskError, AttentionTensor4,
        AttentionTensorError, ScaledDotProductAttentionError,
    };
    use psionic_core::{DType, DeviceKind, Shape, TensorData};

    fn approx_eq(left: f32, right: f32) {
        assert!((left - right).abs() <= 1e-4, "left={left} right={right}");
    }

    #[test]
    fn attention_tensor_roundtrips_nested_values() -> Result<(), AttentionTensorError> {
        let tensor = AttentionTensor4::from_nested(vec![vec![vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
        ]]])?;
        assert_eq!(tensor.shape(), [1, 1, 2, 2]);
        assert_eq!(tensor.tensor_spec().shape(), &Shape::new(vec![1, 1, 2, 2]));
        assert_eq!(tensor.tensor_spec().dtype(), DType::F32);
        assert_eq!(tensor.tensor_spec().device().kind(), DeviceKind::Cpu);
        assert_eq!(tensor.tensor_data(), TensorData::F32(vec![1.0, 2.0, 3.0, 4.0]));
        Ok(())
    }

    #[test]
    fn attention_mask_combination_requires_matching_shapes() {
        let left = AttentionMask::causal(1, 2, 2);
        let right = AttentionMask::causal(1, 3, 3);
        let error = left
            .combine(&right)
            .expect_err("shape mismatch should refuse");
        assert!(matches!(error, AttentionMaskError::ShapeMismatch { .. }));
    }

    #[test]
    fn scaled_dot_product_attention_uses_stable_softmax() -> Result<(), ScaledDotProductAttentionError> {
        let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1000.0, 0.0]]]])
            .expect("query");
        let key = AttentionTensor4::from_nested(vec![vec![vec![
            vec![1000.0, 0.0],
            vec![-1000.0, 0.0],
        ]]])
        .expect("key");
        let value = AttentionTensor4::from_nested(vec![vec![vec![
            vec![5.0, 1.0],
            vec![1.0, 7.0],
        ]]])
        .expect("value");

        let output = scaled_dot_product_attention(&query, &key, &value, None)?;

        assert!(output
            .probability_trace
            .probabilities
            .values()
            .iter()
            .all(|value| value.is_finite()));
        approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 0), 1.0);
        approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 1), 0.0);
        approx_eq(output.context.get(0, 0, 0, 0), 5.0);
        approx_eq(output.context.get(0, 0, 0, 1), 1.0);
        Ok(())
    }

    #[test]
    fn scaled_dot_product_attention_applies_causal_masks() -> Result<(), ScaledDotProductAttentionError> {
        let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![1.0]]]])
            .expect("query");
        let key = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![2.0]]]])
            .expect("key");
        let value = AttentionTensor4::from_nested(vec![vec![vec![vec![10.0], vec![20.0]]]])
            .expect("value");
        let mask = AttentionMask::causal(1, 2, 2);

        let output = scaled_dot_product_attention(&query, &key, &value, Some(&mask))?;

        approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 0), 1.0);
        approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 1), 0.0);
        approx_eq(output.context.get(0, 0, 0, 0), 10.0);
        approx_eq(
            output.probability_trace.probabilities.get(0, 0, 1, 0),
            0.26894143,
        );
        approx_eq(
            output.probability_trace.probabilities.get(0, 0, 1, 1),
            0.7310586,
        );
        approx_eq(output.context.get(0, 0, 1, 0), 17.310585);
        Ok(())
    }

    #[test]
    fn scaled_dot_product_attention_applies_padding_masks() -> Result<(), ScaledDotProductAttentionError> {
        let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![1.0]]]])
            .expect("query");
        let key = AttentionTensor4::from_nested(vec![vec![vec![
            vec![1.0],
            vec![100.0],
            vec![2.0],
        ]]])
        .expect("key");
        let value = AttentionTensor4::from_nested(vec![vec![vec![
            vec![10.0],
            vec![999.0],
            vec![30.0],
        ]]])
        .expect("value");
        let mask = AttentionMask::from_padding_tokens(vec![vec![true, false, true]], 2)
            .expect("padding mask");

        let output = scaled_dot_product_attention(&query, &key, &value, Some(&mask))?;

        approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 1), 0.0);
        approx_eq(output.probability_trace.probabilities.get(0, 0, 1, 1), 0.0);
        approx_eq(output.context.get(0, 0, 0, 0), 24.621172);
        approx_eq(output.context.get(0, 0, 1, 0), 24.621172);
        Ok(())
    }

    #[test]
    fn scaled_dot_product_attention_applies_combined_masks() -> Result<(), ScaledDotProductAttentionError> {
        let query = AttentionTensor4::from_nested(vec![vec![vec![
            vec![1.0],
            vec![1.0],
            vec![1.0],
        ]]])
        .expect("query");
        let key = AttentionTensor4::from_nested(vec![vec![vec![
            vec![1.0],
            vec![2.0],
            vec![3.0],
        ]]])
        .expect("key");
        let value = AttentionTensor4::from_nested(vec![vec![vec![
            vec![10.0],
            vec![20.0],
            vec![999.0],
        ]]])
        .expect("value");
        let causal = AttentionMask::causal(1, 3, 3);
        let padding =
            AttentionMask::from_padding_tokens(vec![vec![true, true, false]], 3).expect("padding");
        let combined = causal.combine(&padding).expect("combined");

        let output = scaled_dot_product_attention(&query, &key, &value, Some(&combined))?;

        approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 1), 0.0);
        approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 2), 0.0);
        approx_eq(output.probability_trace.probabilities.get(0, 0, 1, 2), 0.0);
        approx_eq(output.probability_trace.probabilities.get(0, 0, 2, 2), 0.0);
        approx_eq(output.context.get(0, 0, 0, 0), 10.0);
        approx_eq(output.context.get(0, 0, 1, 0), 17.310585);
        approx_eq(output.context.get(0, 0, 2, 0), 17.310585);
        Ok(())
    }

    #[test]
    fn attention_probability_trace_exports_core_tensor_surface() -> Result<(), ScaledDotProductAttentionError> {
        let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![0.0]]]])
            .expect("query");
        let key = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![0.0]]]])
            .expect("key");
        let value = AttentionTensor4::from_nested(vec![vec![vec![vec![3.0], vec![4.0]]]])
            .expect("value");

        let output = scaled_dot_product_attention(&query, &key, &value, None)?;
        let spec = output.probability_trace.tensor_spec();
        let data = output.probability_trace.tensor_data();

        assert_eq!(spec.shape(), &Shape::new(vec![1, 1, 2, 2]));
        assert_eq!(spec.dtype(), DType::F32);
        assert_eq!(spec.device().kind(), DeviceKind::Cpu);
        let values = data
            .as_f32_slice()
            .expect("probability trace should export dense f32 data");
        assert_eq!(values.len(), 4);
        approx_eq(values[0] + values[1], 1.0);
        approx_eq(values[2] + values[3], 1.0);
        Ok(())
    }
}
