//! V4.1 block-scaled matrix-vector kernels on Metal, a port of
//! `psionic_backend_cuda::v41_matvec` (#12391). The formats, the dimension
//! checks, and the host model are the CUDA backend's.

pub use psionic_backend_cuda::v41_matvec::{
    V41_BLOCK, V41MatvecFormat, V41Output, check_v41_matvec_dims, v41_block_matvec_host,
    v41_round_bf16,
};

use super::{Args, V41Buffer, V41Submission, int, invalid};
use psionic_runtime::RuntimeError;

const WARPS: usize = 8;
const THREADS: usize = WARPS * 32;

/// A matrix, or a stack of same-shape matrices, as `V41Matrix` in the CUDA
/// backend.
#[derive(Clone, Copy, Debug)]
pub struct V41Matrix<'a> {
    pub format: V41MatvecFormat,
    pub weights: &'a V41Buffer,
    pub scales: &'a V41Buffer,
    pub rows: usize,
    pub cols: usize,
    pub matrices: usize,
}

/// What each slot of a launch reads and writes, as `V41Slots`.
#[derive(Clone, Copy, Debug)]
pub struct V41Slots<'a> {
    pub count: usize,
    pub ids: Option<&'a V41Buffer>,
    pub shared_input: bool,
}

impl V41Slots<'_> {
    /// One slot, matrix 0, activation row 0.
    #[must_use]
    pub const fn single() -> Self {
        Self {
            count: 1,
            ids: None,
            shared_input: true,
        }
    }
}

impl V41Submission<'_> {
    /// Quantizes `rows × cols` float32 activations to FP8 E4M3 codes and
    /// UE8M0 scales, as `CudaSubmission::v41_act_quant_fp8`.
    ///
    /// # Errors
    ///
    /// Refuses bad dimensions or buffers too small for them.
    pub fn v41_act_quant_fp8(
        &mut self,
        input: &V41Buffer,
        codes: &V41Buffer,
        scales: &V41Buffer,
        rows: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        check_v41_matvec_dims(1, cols, 1, rows.max(1))?;
        if rows == 0 {
            return Err(invalid("v41 block matvec", "no activation rows"));
        }
        let values = rows * cols;
        let args = Args::new()
            .ptr(self.span(input, 0, values, 4, "activation input")?)
            .ptr(self.span(codes, 0, values, 1, "activation codes")?)
            .ptr(self.span(scales, 0, values / V41_BLOCK, 1, "activation scales")?)
            .i32(int(rows, "rows")?)
            .i32(int(cols, "cols")?);
        let warps = rows * (cols / V41_BLOCK);
        self.launch(
            "v41_act_quant",
            args,
            (warps.div_ceil(WARPS), 1, 1),
            THREADS,
            0,
        )
    }

    /// Multiplies FP8 activations by a block-scaled matrix for each slot,
    /// as `CudaSubmission::v41_block_matvec`.
    ///
    /// # Errors
    ///
    /// Refuses bad dimensions and buffers too small for them.
    pub fn v41_block_matvec(
        &mut self,
        matrix: V41Matrix<'_>,
        codes: &V41Buffer,
        scales: &V41Buffer,
        slots: V41Slots<'_>,
        out: &V41Buffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        let (rows, cols) = (matrix.rows, matrix.cols);
        check_v41_matvec_dims(rows, cols, matrix.matrices, slots.count)?;
        let matrix_bytes = matrix.format.weight_bytes(rows, cols);
        let matrix_scale_bytes = matrix.format.scale_bytes(rows, cols);
        let weights = self.span(
            matrix.weights,
            0,
            matrix_bytes
                .checked_mul(matrix.matrices)
                .ok_or_else(|| invalid("v41 block matvec", "weight size overflow"))?,
            1,
            "weights",
        )?;
        let weight_scales = self.span(
            matrix.scales,
            0,
            matrix_scale_bytes
                .checked_mul(matrix.matrices)
                .ok_or_else(|| invalid("v41 block matvec", "scale size overflow"))?,
            1,
            "weight scales",
        )?;
        let inputs = if slots.shared_input { 1 } else { slots.count };
        let act = self.span(codes, 0, inputs * cols, 1, "activation codes")?;
        let act_scales = self.span(scales, 0, inputs * cols / V41_BLOCK, 1, "activation scales")?;
        let ids = self.span_opt(slots.ids, 0, slots.count, 4, "matrix ids")?;
        let out = self.span(out, 0, slots.count * rows, 4, "output")?;
        let to_i64 = |n: usize| {
            i64::try_from(n).map_err(|_| invalid("v41 block matvec", format!("{n} exceeds i64")))
        };
        let args = Args::new()
            .ptr(weights)
            .ptr(weight_scales)
            .i64(to_i64(matrix_bytes)?)
            .i64(to_i64(matrix_scale_bytes)?)
            .ptr(ids)
            .ptr(act)
            .ptr(act_scales)
            .ptr(out)
            .i32(int(matrix.matrices, "matrices")?)
            .i32(i32::from(slots.shared_input))
            .i32(int(rows, "rows")?)
            .i32(int(cols, "cols")?)
            .i32(i32::from(output == V41Output::Bf16))
            .i32(i32::from(matrix.format == V41MatvecFormat::Fp4E2M1Row32));
        self.launch(
            "v41_block_matvec",
            args,
            (rows.div_ceil(WARPS), slots.count, 1),
            THREADS,
            0,
        )
    }
}

#[cfg(test)]
#[path = "matvec_tests.rs"]
mod tests;

/// Flushes a subnormal to zero of the same sign, as Apple GPUs do with every
/// float32 result.
#[must_use]
pub fn ftz(x: f32) -> f32 {
    if x.is_subnormal() {
        0.0_f32.copysign(x)
    } else {
        x
    }
}

fn e4m3(byte: u8) -> f32 {
    let magnitude = u32::from(byte & 0x7f);
    let value = if magnitude == 0x7f {
        f32::NAN
    } else {
        f32::from_bits(magnitude << 20) * f32::from_bits((120 + 127) << 23)
    };
    f32::from_bits(value.to_bits() | (u32::from(byte & 0x80) << 24))
}

fn e2m1(code: u8) -> f32 {
    let value = f32::from_bits(u32::from(code & 7) << 22) * f32::from_bits((126 + 127) << 23);
    f32::from_bits(value.to_bits() | (u32::from(code & 8) << 28))
}

fn e8m0(byte: u8) -> f32 {
    match byte {
        0xff => f32::NAN,
        0 => f32::from_bits(0x0040_0000),
        exponent => f32::from_bits(u32::from(exponent) << 23),
    }
}

/// The Metal kernel's arithmetic on the host: the CUDA backend's
/// [`v41_block_matvec_host`] with every result's subnormals flushed to zero,
/// as an Apple GPU computes it (a scale of 2^-127 still multiplies exactly).
/// The two agree whenever no product or sum falls below 2^-126.
///
/// # Panics
///
/// Panics when a slice is shorter than the shape needs.
#[must_use]
pub fn v41_block_matvec_host_ftz(
    format: V41MatvecFormat,
    weights: &[u8],
    weight_scales: &[u8],
    rows: usize,
    cols: usize,
    codes: &[u8],
    scales: &[u8],
) -> Vec<f32> {
    let blocks = cols / V41_BLOCK;
    (0..rows)
        .map(|row| {
            let mut acc = 0.0_f32;
            for b in 0..blocks {
                let mut partial = 0.0_f32;
                for j in 0..V41_BLOCK {
                    let k = b * V41_BLOCK + j;
                    let w = match format {
                        V41MatvecFormat::Fp8Block32x32 => e4m3(weights[row * cols + k]),
                        V41MatvecFormat::Fp4E2M1Row32 => {
                            let byte = weights[row * cols / 2 + k / 2];
                            e2m1(if k % 2 == 0 { byte & 15 } else { byte >> 4 })
                        }
                    };
                    partial = ftz(e4m3(codes[k]).mul_add(w, partial));
                }
                let weight_scale = match format {
                    V41MatvecFormat::Fp8Block32x32 => weight_scales[(row / V41_BLOCK) * blocks + b],
                    V41MatvecFormat::Fp4E2M1Row32 => weight_scales[row * blocks + b],
                };
                let scaled = ftz(ftz(partial * e8m0(scales[b])) * e8m0(weight_scale));
                acc = ftz(acc + scaled);
            }
            acc
        })
        .collect()
}
