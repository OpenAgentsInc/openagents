//! Float32 CPU components for the native DeepSeek-V4.1 port.
//!
//! Math reimplemented from the public MIT-licensed reference at checkpoint
//! revision `1bae0e1088993b9b82de08f59de5be5a34e42170`. See
//! `testdata/deepseek_v41_cpu/LICENSE-DeepSeek`. These components do not yet
//! reproduce BF16 rounding or quantized activation boundaries and do not
//! qualify a serving model. Matrix rows can be supplied without expanding a
//! checkpoint in memory.

pub mod hyperconnections;

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
    fn dimensions(&self) -> (usize, usize);
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError>;
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
    let (rows, cols) = matrix.dimensions();
    if rows == 0 || cols == 0 {
        return Err(CpuError::Invalid("empty matrix"));
    }
    shape("matrix input", input, cols)?;
    let mut out = Vec::with_capacity(rows);
    for i in 0..rows {
        let row = matrix.row(i)?;
        shape("matrix row", &row, cols)?;
        out.push(row.iter().zip(input).map(|(a, b)| a * b).sum());
    }
    finite("matrix output", &out)?;
    Ok(out)
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
