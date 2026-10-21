//! Float32 CPU components for the native DeepSeek-V4.1 port.
//!
//! Math reimplemented from the public MIT-licensed reference at checkpoint
//! revision `1bae0e1088993b9b82de08f59de5be5a34e42170`. See
//! `testdata/deepseek_v41_cpu/LICENSE-DeepSeek`. These components do not
//! qualify a serving model. Float32 equations and explicit BF16/activation
//! boundaries are available separately; device parity remains unqualified.
//! Matrix rows can be supplied without expanding a
//! checkpoint in memory.

pub mod attention;
pub mod block;
pub mod engram;
pub mod generation;
pub mod hyperconnections;
pub mod moe;
pub mod projection;
pub mod quantization;
pub mod rope;
pub mod shared_state;
pub mod sm100;
pub mod transformer;

use thiserror::Error;

/// Invalid input or unavailable matrix data in a CPU component.
#[derive(Debug, Error)]
pub enum CpuError {
    #[error("invalid V4.1 CPU input: {0}")]
    Invalid(&'static str),
    #[error("{name} has {actual} elements; expected {expected}")]
    Shape {
        name: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{0} contains a nonfinite value")]
    NonFinite(&'static str),
    #[error("Engram token normalization failed: {0}")]
    Tokenization(String),
    #[error(transparent)]
    Weights(#[from] crate::deepseek_v41_weights::V41WeightsError),
}

pub(crate) fn shape(name: &'static str, values: &[f32], expected: usize) -> Result<(), CpuError> {
    if values.len() != expected {
        return Err(CpuError::Shape {
            name,
            expected,
            actual: values.len(),
        });
    }
    finite(name, values)
}

pub(crate) fn finite(name: &'static str, values: &[f32]) -> Result<(), CpuError> {
    if values.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(CpuError::NonFinite(name))
    }
}

pub(crate) fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

pub(crate) fn product(a: usize, b: usize) -> Result<usize, CpuError> {
    a.checked_mul(b).ok_or(CpuError::Invalid("shape overflow"))
}

pub(crate) fn sigmoid(x: f32) -> f32 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

/// Read one logical float32 row of a weight matrix.
pub trait MatrixRows {
    /// Independent rows may run concurrently; each row keeps its float32 sum order.
    fn multiply(&self, input: &[f32], bf16_weights: bool) -> Result<Vec<f32>, CpuError> {
        self.multiply_partitioned(input, bf16_weights, None, 1)
    }
    /// Optional K blocks and per-rank BF16 partials preserve oracle GEMM boundaries.
    fn multiply_partitioned(
        &self,
        input: &[f32],
        bf16_weights: bool,
        block: Option<usize>,
        shards: usize,
    ) -> Result<Vec<f32>, CpuError> {
        multiply_rows(self, input, bf16_weights, block, shards)
    }
    /// Block-diagonal output projection: a row reads only its group's input.
    fn multiply_grouped(
        &self,
        input: &[f32],
        groups: usize,
        bf16_weights: bool,
    ) -> Result<Vec<f32>, CpuError> {
        let (rows, cols) = self.dimensions();
        validate_groups(rows, cols, input.len(), groups)?;
        (0..rows)
            .map(|i| {
                let group = i / (rows / groups);
                multiply_row(
                    &self.row(i)?,
                    &input[group * cols..(group + 1) * cols],
                    bf16_weights,
                    None,
                    1,
                )
            })
            .collect()
    }
    /// Row products with a caller-supplied reduction, such as a measured
    /// device accumulation order. `groups` splits the input as in
    /// [`MatrixRows::multiply_grouped`]; 1 uses the whole input for every row.
    fn multiply_reduced(
        &self,
        input: &[f32],
        groups: usize,
        bf16_weights: bool,
        reduce: &(dyn Fn(&[f32], &[f32]) -> f32 + Sync),
    ) -> Result<Vec<f32>, CpuError> {
        let (rows, cols) = self.dimensions();
        validate_groups(rows, cols, input.len(), groups)?;
        let out = (0..rows)
            .map(|i| {
                let row = self.row(i)?;
                let row = if bf16_weights {
                    quantization::round_bf16(&row)?
                } else {
                    row
                };
                let group = i / (rows / groups);
                Ok(reduce(&row, &input[group * cols..(group + 1) * cols]))
            })
            .collect::<Result<Vec<_>, CpuError>>()?;
        finite("reduced matrix output", &out)?;
        Ok(out)
    }
    fn dimensions(&self) -> (usize, usize);
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError>;
    fn precision(&self) -> quantization::MatrixPrecision {
        quantization::MatrixPrecision::F32
    }
}

/// Matrix backed by positioned checkpoint reads, including packed FP4 rows.
pub struct CheckpointMatrix<'a> {
    checkpoint: &'a crate::deepseek_v41_weights::DeepSeekV41Checkpoint,
    name: String,
    dimensions: (usize, usize),
    precision: quantization::MatrixPrecision,
}

impl<'a> CheckpointMatrix<'a> {
    pub fn new(
        checkpoint: &'a crate::deepseek_v41_weights::DeepSeekV41Checkpoint,
        name: impl Into<String>,
    ) -> Result<Self, CpuError> {
        use crate::deepseek_v41_weights::V41BlockFormat;
        let name = name.into();
        let entry = checkpoint.entry(&name)?;
        let [rows, stored_cols] = entry.shape.as_slice() else {
            return Err(CpuError::Invalid(
                "checkpoint matrix must have two dimensions",
            ));
        };
        let format = checkpoint.block_format(&name)?;
        let cols = if format == V41BlockFormat::Fp4E2M1Row32 {
            stored_cols
                .checked_mul(2)
                .ok_or(CpuError::Invalid("shape overflow"))?
        } else {
            *stored_cols
        };
        let dimensions = (
            usize::try_from(*rows).map_err(|_| CpuError::Invalid("shape overflow"))?,
            usize::try_from(cols).map_err(|_| CpuError::Invalid("shape overflow"))?,
        );
        if dimensions.0 == 0 || dimensions.1 == 0 {
            return Err(CpuError::Invalid("empty matrix"));
        }
        Ok(Self {
            checkpoint,
            name,
            dimensions,
            precision: match format {
                V41BlockFormat::F32 => quantization::MatrixPrecision::F32,
                V41BlockFormat::Bf16 => quantization::MatrixPrecision::Bf16,
                V41BlockFormat::Fp4E2M1Row32 => quantization::MatrixPrecision::Fp4,
                _ => quantization::MatrixPrecision::Fp8,
            },
        })
    }
}

impl MatrixRows for CheckpointMatrix<'_> {
    fn precision(&self) -> quantization::MatrixPrecision {
        self.precision
    }
    fn dimensions(&self) -> (usize, usize) {
        self.dimensions
    }
    fn multiply_partitioned(
        &self,
        input: &[f32],
        bf16_weights: bool,
        block: Option<usize>,
        shards: usize,
    ) -> Result<Vec<f32>, CpuError> {
        use rayon::prelude::*;
        shape("checkpoint matrix input", input, self.dimensions.1)?;
        validate_reduction(input.len(), block, shards)?;
        // Decoded BF16 checkpoint rows are already exactly BF16. Parallelism
        // changes the order between rows only, never the sum within a row.
        let out: Result<Vec<_>, CpuError> = (0..self.dimensions.0)
            .into_par_iter()
            .map(|i| {
                let row = self.row(i)?;
                let row = if bf16_weights && self.precision != quantization::MatrixPrecision::Bf16 {
                    quantization::round_bf16(&row)?
                } else {
                    row
                };
                Ok(row_product(&row, input, block, shards))
            })
            .collect();
        let out = out?;
        finite("checkpoint matrix output", &out)?;
        Ok(out)
    }
    fn multiply_reduced(
        &self,
        input: &[f32],
        groups: usize,
        bf16_weights: bool,
        reduce: &(dyn Fn(&[f32], &[f32]) -> f32 + Sync),
    ) -> Result<Vec<f32>, CpuError> {
        use rayon::prelude::*;
        let (rows, cols) = self.dimensions;
        validate_groups(rows, cols, input.len(), groups)?;
        let out = (0..rows)
            .into_par_iter()
            .map(|i| {
                let row = self.row(i)?;
                let row = if bf16_weights && self.precision != quantization::MatrixPrecision::Bf16 {
                    quantization::round_bf16(&row)?
                } else {
                    row
                };
                let group = i / (rows / groups);
                Ok(reduce(&row, &input[group * cols..(group + 1) * cols]))
            })
            .collect::<Result<Vec<_>, CpuError>>()?;
        finite("reduced matrix output", &out)?;
        Ok(out)
    }
    fn multiply_grouped(
        &self,
        input: &[f32],
        groups: usize,
        bf16_weights: bool,
    ) -> Result<Vec<f32>, CpuError> {
        use rayon::prelude::*;
        let (rows, cols) = self.dimensions;
        validate_groups(rows, cols, input.len(), groups)?;
        (0..rows)
            .into_par_iter()
            .map(|i| {
                let group = i / (rows / groups);
                multiply_row(
                    &self.row(i)?,
                    &input[group * cols..(group + 1) * cols],
                    bf16_weights,
                    None,
                    1,
                )
            })
            .collect()
    }
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
        if index >= self.dimensions.0 {
            return Err(CpuError::Invalid("matrix row out of range"));
        }
        let row = self.checkpoint.dequantize_row(&self.name, index as u64)?;
        shape("checkpoint matrix row", &row, self.dimensions.1)?;
        Ok(row)
    }
}

/// Borrowed row-major matrix, useful for small CPU component fixtures.
pub struct MatrixRef<'a> {
    rows: usize,
    cols: usize,
    values: &'a [f32],
}

impl<'a> MatrixRef<'a> {
    pub fn new(rows: usize, cols: usize, values: &'a [f32]) -> Result<Self, CpuError> {
        if rows == 0 || cols == 0 {
            return Err(CpuError::Invalid("empty matrix"));
        }
        shape("matrix", values, product(rows, cols)?)?;
        Ok(Self { rows, cols, values })
    }
}

impl MatrixRows for MatrixRef<'_> {
    fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
        if index >= self.rows {
            return Err(CpuError::Invalid("matrix row out of range"));
        }
        Ok(self.values[index * self.cols..(index + 1) * self.cols].to_vec())
    }
}

/// Evaluate a matrix-vector product one row at a time with float32 sums.
pub fn matvec(matrix: &dyn MatrixRows, input: &[f32]) -> Result<Vec<f32>, CpuError> {
    matrix.multiply(input, false)
}
fn multiply_rows<M: MatrixRows + ?Sized>(
    matrix: &M,
    input: &[f32],
    bf16_weights: bool,
    block: Option<usize>,
    shards: usize,
) -> Result<Vec<f32>, CpuError> {
    let (rows, cols) = matrix.dimensions();
    if rows == 0 || cols == 0 {
        return Err(CpuError::Invalid("empty matrix"));
    }
    shape("matrix input", input, cols)?;
    validate_reduction(cols, block, shards)?;
    let mut out = Vec::with_capacity(rows);
    for i in 0..rows {
        let mut row = matrix.row(i)?;
        shape("matrix row", &row, cols)?;
        if bf16_weights {
            row = quantization::round_bf16(&row)?;
        }
        out.push(row_product(&row, input, block, shards));
    }
    finite("matrix output", &out)?;
    Ok(out)
}

fn validate_groups(rows: usize, cols: usize, input: usize, groups: usize) -> Result<(), CpuError> {
    if groups == 0
        || rows == 0
        || cols == 0
        || !rows.is_multiple_of(groups)
        || cols.checked_mul(groups) != Some(input)
    {
        return Err(CpuError::Invalid("grouped matrix shape"));
    }
    Ok(())
}
/// One row's explicit reduction, shared by checkpoint and resident CPU weights.
pub fn multiply_row(
    row: &[f32],
    input: &[f32],
    bf16_weights: bool,
    block: Option<usize>,
    shards: usize,
) -> Result<f32, CpuError> {
    if row.len() != input.len() {
        return Err(CpuError::Invalid("matrix row/input lengths"));
    }
    validate_reduction(row.len(), block, shards)?;
    let rounded;
    let row = if bf16_weights {
        rounded = quantization::round_bf16(row)?;
        &rounded
    } else {
        row
    };
    let output = row_product(row, input, block, shards);
    if !output.is_finite() {
        return Err(CpuError::NonFinite("matrix row product"));
    }
    Ok(output)
}

fn validate_reduction(cols: usize, block: Option<usize>, shards: usize) -> Result<(), CpuError> {
    if shards == 0
        || cols == 0
        || !cols.is_multiple_of(shards)
        || block.is_some_and(|b| b == 0 || !(cols / shards).is_multiple_of(b))
    {
        return Err(CpuError::Invalid("matrix reduction block or shard width"));
    }
    Ok(())
}
fn row_product(row: &[f32], input: &[f32], block: Option<usize>, shards: usize) -> f32 {
    let width = row.len() / shards;
    let mut total = 0.0;
    for (w, x) in row.chunks_exact(width).zip(input.chunks_exact(width)) {
        let part = if let Some(block) = block {
            w.chunks_exact(block)
                .zip(x.chunks_exact(block))
                .map(|(w, x)| w.iter().zip(x).map(|(w, x)| w * x).sum::<f32>())
                .sum::<f32>()
        } else {
            w.iter().zip(x).map(|(w, x)| w * x).sum::<f32>()
        };
        total += if shards > 1 {
            quantization::bf16(part)
        } else {
            part
        };
    }
    total
}

/// Contiguous float32 row reduction from PyTorch v2.13 CUDA Reduce.cuh.
/// Four independent vector-load accumulators and descending lane/warp trees
/// preserve batch-shape effects. Independently reimplemented from public ATen.
pub fn reference_row_sum(values: &[f32], rows: usize) -> Result<f32, CpuError> {
    if values.len() < 128 || !values.len().is_multiple_of(4) || rows == 0 {
        return Err(CpuError::Invalid("reference reduction shape"));
    }
    finite("reference reduction input", values)?;
    let floor = |n: usize| 1usize << (usize::BITS - 1 - n.max(1).leading_zeros());
    let dim0 = floor((values.len() / 4).min(512));
    let height = floor(rows.min(512)).min(512 / dim0.min(32));
    let width = dim0.min(512 / height);
    let warps = if values.len().div_ceil(width) >= (height * 16).min(256) {
        height
    } else {
        1
    };
    let lanes = width * warps;
    let mut totals = vec![0.0f32; lanes];
    for (lane, total) in totals.iter_mut().enumerate() {
        let mut accum = [0.0f32; 4];
        for group in (lane..values.len() / 4).step_by(lanes) {
            for j in 0..4 {
                accum[j] += values[group * 4 + j];
            }
        }
        *total = ((accum[0] + accum[1]) + accum[2]) + accum[3];
    }
    for warp in 0..warps {
        let mut offset = width / 2;
        while offset > 0 {
            for lane in 0..offset {
                let at = warp * width + lane;
                totals[at] += totals[at + offset];
            }
            offset /= 2;
        }
    }
    let mut offset = warps / 2;
    while offset > 0 {
        for warp in 0..offset {
            totals[warp * width] += totals[(warp + offset) * width];
        }
        offset /= 2;
    }
    Ok(totals[0])
}
/// Applies MeanOps' float32 reciprocal factor after the row sum.
pub fn reference_row_mean(values: &[f32], rows: usize) -> Result<f32, CpuError> {
    Ok(reference_row_sum(values, rows)? * (1.0 / values.len() as f32))
}
/// RMS normalization with the oracle's contiguous CUDA float32 reduction.
pub fn rms_norm_reference(
    input: &[f32],
    weight: &[f32],
    epsilon: f32,
    rows: usize,
) -> Result<Vec<f32>, CpuError> {
    shape("reference RMS weight", weight, input.len())?;
    if !positive(epsilon) {
        return Err(CpuError::Invalid("reference RMS epsilon"));
    }
    let squares: Vec<_> = input.iter().map(|x| x * x).collect();
    let variance = reference_row_mean(&squares, rows)?;
    // CUDA rsqrt is one operation. Rounding sqrt to F32 before its reciprocal
    // introduces a second rounding boundary, which can flip a later BF16 tie.
    let scale = reference_rsqrt(variance + epsilon)?;
    let out: Vec<_> = input
        .iter()
        .zip(weight)
        .map(|(x, w)| w * (x * scale))
        .collect();
    finite("reference RMS output", &out)?;
    Ok(out)
}

fn reference_rsqrt(value: f32) -> Result<f32, CpuError> {
    if !positive(value) {
        return Err(CpuError::Invalid("reference reciprocal square root"));
    }
    Ok((1.0 / (value as f64).sqrt()) as f32)
}

/// Per-vector RMS normalization; the checkpoint uses epsilon `1e-20`.
pub fn rms_norm(input: &[f32], weight: &[f32], epsilon: f32) -> Result<Vec<f32>, CpuError> {
    if input.is_empty() || !positive(epsilon) {
        return Err(CpuError::Invalid("RMS normalization dimensions or epsilon"));
    }
    shape("RMS weight", weight, input.len())?;
    finite("RMS input", input)?;
    let variance = input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32;
    if !variance.is_finite() {
        return Err(CpuError::NonFinite("RMS variance"));
    }
    let scale = (variance + epsilon).sqrt().recip();
    let out: Vec<_> = input
        .iter()
        .zip(weight)
        .map(|(x, w)| w * (x * scale))
        .collect();
    finite("RMS output", &out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oracle_bf16_ties_use_one_reciprocal_sqrt_rounding() -> Result<(), CpuError> {
        // Original-weight #12371 observations: p1 replay g2 layer 39, and
        // p3 decode g2 layer 14. Variances use the recorded CUDA row layout.
        for (bits, input, weight, expected) in [
            (
                1092534930,
                -3.65625_f32,
                0.02392578125_f32,
                -0.0277099609375_f32,
            ),
            (1041155562, -0.478515625, 0.029052734375, -0.037109375),
        ] {
            let variance = f32::from_bits(bits);
            let scale = reference_rsqrt(variance)?;
            assert_eq!(quantization::bf16(weight * (input * scale)), expected);
            assert_ne!(
                quantization::bf16(weight * (input * variance.sqrt().recip())),
                expected
            );
        }
        assert!(reference_rsqrt(0.0).is_err());
        assert!(reference_rsqrt(f32::NAN).is_err());
        Ok(())
    }
    use crate::deepseek_v41_weights::{CheckpointOpenOptions, DeepSeekV41Checkpoint};

    #[test]
    fn reference_reduction_handles_prefill_decode_and_refuses_bad_shapes() -> Result<(), CpuError> {
        for rows in [1, 4, 41, 20000] {
            assert_eq!(reference_row_mean(&vec![2.0; 5120], rows)?, 2.0);
            assert_eq!(reference_row_sum(&vec![2.0; 5120], rows)?, 10240.0);
        }
        assert!(reference_row_mean(&[1.0; 32], 41).is_err());
        assert!(reference_row_mean(&[1.0; 5120], 0).is_err());
        assert!(reference_row_mean(&[f32::NAN; 5120], 41).is_err());
        assert!(reference_row_mean(&[1.0; 129], 41).is_err());
        Ok(())
    }
    #[test]
    fn checkpoint_matrix_uses_logical_fp4_width_and_positioned_rows()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let header = serde_json::to_vec(&serde_json::json!({
            "probe.weight": { "dtype": "I8", "shape": [3,16], "data_offsets": [0,48] },
            "probe.scale": { "dtype": "F8_E8M0", "shape": [3,1], "data_offsets": [48,51] }
        }))?;
        let mut shard = (header.len() as u64).to_le_bytes().to_vec();
        shard.extend(header);
        shard.extend([0x71; 16]);
        shard.extend([0x72; 16]);
        shard.extend([0x73; 16]);
        shard.extend([127; 3]);
        std::fs::write(dir.path().join("probe.safetensors"), shard)?;
        std::fs::write(
            dir.path().join("model.safetensors.index.json"),
            serde_json::to_vec(&serde_json::json!({
                "metadata": { "total_size": 51 },
                "weight_map": { "probe.weight": "probe.safetensors", "probe.scale": "probe.safetensors" }
            }))?,
        )?;
        let checkpoint = DeepSeekV41Checkpoint::open(
            dir.path(),
            CheckpointOpenOptions {
                headers_only: false,
                pin: None,
                check_model_map: false,
            },
        )?;
        let matrix = CheckpointMatrix::new(&checkpoint, "probe.weight")?;
        assert_eq!(matrix.dimensions(), (3, 32));
        assert_eq!(&matrix.row(0)?[..4], &[0.5, 6.0, 0.5, 6.0]);
        assert_eq!(matvec(&matrix, &[1.0; 32])?, vec![104.0, 112.0, 120.0]);
        assert!(matrix.row(3).is_err());
        Ok(())
    }
}
