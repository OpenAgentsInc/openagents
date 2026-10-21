//! DeepSeek-V4.1 block-scaled matrix-vector kernels (#12391).
//!
//! Weights stay on the device in their checkpoint form: FP8 E4M3 with one
//! UE8M0 scale per 32×32 block ([`V41MatvecFormat::Fp8Block32x32`]), or packed
//! FP4 E2M1 with one UE8M0 scale per 32 values along K
//! ([`V41MatvecFormat::Fp4E2M1Row32`]). Activations are FP8 E4M3 with one UE8M0
//! scale per 32 values, written on the device by
//! [`CudaSubmission::v41_act_quant_fp8`] under the same rule as the CPU path's
//! `quantize(round_bf16(x), Fp8)`.
//!
//! [`CudaSubmission::v41_block_matvec`] computes one or more matrix-vector
//! products in one launch. Each slot can pick its matrix from a stack by a
//! device-resident id, so routed experts run without a host round trip. Both
//! calls only enqueue work on the submission's stream: they allocate nothing
//! and never synchronize, so they can be recorded into a CUDA graph.
//!
//! The accumulation order is the reference's (`fp8_gemm_kernel` and
//! `fp4_gemm_kernel`, `inference/kernel.py` at revision 1bae0e1): float32 sums
//! of code products per 32-wide K block, each multiplied by the activation
//! scale and then the weight scale, added in ascending block order. Within a
//! block the products are added sequentially from element 0.
//! [`v41_block_matvec_host`] reproduces the device arithmetic bit for bit.

use crate::clef_prefill::{int, span};
use crate::{CudaBuffer, CudaSubmission};
use psionic_runtime::RuntimeError;
use std::ffi::c_void;

/// Values per scale block along K, and rows per FP8 scale block.
pub const V41_BLOCK: usize = 32;

/// How a V4.1 matrix stores its weights and scales.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V41MatvecFormat {
    /// `F8_E4M3` `[rows, cols]` bytes and `F8_E8M0` scales
    /// `[ceil(rows / 32), cols / 32]`.
    Fp8Block32x32,
    /// Packed E2M1 `[rows, cols / 2]` bytes, the low nibble holding element
    /// `2i`, and `F8_E8M0` scales `[rows, cols / 32]`.
    Fp4E2M1Row32,
}

impl V41MatvecFormat {
    /// Weight bytes of one `rows × cols` matrix.
    #[must_use]
    pub const fn weight_bytes(self, rows: usize, cols: usize) -> usize {
        match self {
            Self::Fp8Block32x32 => rows * cols,
            Self::Fp4E2M1Row32 => rows * cols / 2,
        }
    }

    /// Scale bytes of one `rows × cols` matrix.
    #[must_use]
    pub const fn scale_bytes(self, rows: usize, cols: usize) -> usize {
        match self {
            Self::Fp8Block32x32 => rows.div_ceil(V41_BLOCK) * (cols / V41_BLOCK),
            Self::Fp4E2M1Row32 => rows * (cols / V41_BLOCK),
        }
    }
}

/// A device-resident matrix, or a stack of same-shape matrices laid out one
/// after another (for example all routed experts of one projection).
#[derive(Clone, Copy, Debug)]
pub struct V41Matrix<'a> {
    pub format: V41MatvecFormat,
    /// Packed weights, `matrices × weight_bytes(rows, cols)` bytes.
    pub weights: &'a CudaBuffer,
    /// UE8M0 scales, `matrices × scale_bytes(rows, cols)` bytes.
    pub scales: &'a CudaBuffer,
    /// Output rows (N).
    pub rows: usize,
    /// Input values (K), a multiple of 32.
    pub cols: usize,
    /// Matrices in the stack, at least 1.
    pub matrices: usize,
}

/// What each slot of a launch reads and writes. Slot `s` writes output row
/// `s` (`rows` floats at `s × rows`).
#[derive(Clone, Copy, Debug)]
pub struct V41Slots<'a> {
    /// Slots in the launch, 1 to 65,535.
    pub count: usize,
    /// Device `i32` matrix id per slot. Without it every slot uses matrix 0.
    /// An id outside the stack writes NaN for that slot instead of reading
    /// out of bounds.
    pub ids: Option<&'a CudaBuffer>,
    /// When true every slot reads activation row 0; otherwise slot `s` reads
    /// row `s`.
    pub shared_input: bool,
}

impl V41Slots<'_> {
    /// One slot, matrix 0, activation row 0: an ordinary matrix-vector product.
    #[must_use]
    pub const fn single() -> Self {
        Self {
            count: 1,
            ids: None,
            shared_input: true,
        }
    }
}

/// How the kernel stores each output value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V41Output {
    /// The float32 accumulator.
    F32,
    /// The accumulator rounded to the nearest BF16, ties to even, stored as
    /// float32. This is the reference's BF16 GEMM output.
    Bf16,
}

unsafe extern "C" {
    fn psionic_v41_act_quant_fp8(
        input: *const c_void,
        codes: *mut c_void,
        scales: *mut c_void,
        rows: i32,
        cols: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_block_matvec(
        fp4: i32,
        weights: *const c_void,
        weight_scales: *const c_void,
        matrix_bytes: i64,
        matrix_scale_bytes: i64,
        matrices: i32,
        ids: *const c_void,
        act: *const c_void,
        act_scales: *const c_void,
        act_shared: i32,
        out: *mut c_void,
        rows: i32,
        cols: i32,
        slots: i32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
}

fn invalid(message: String) -> RuntimeError {
    RuntimeError::Backend(format!("v41 block matvec: {message}"))
}

/// Checks a matrix and slot geometry before launch.
///
/// # Errors
///
/// Refuses empty shapes, K not a multiple of 32, an empty stack, a slot count
/// outside 1 to 65,535, and sizes that don't fit `i32` indexing.
pub fn check_v41_matvec_dims(
    rows: usize,
    cols: usize,
    matrices: usize,
    slots: usize,
) -> Result<(), RuntimeError> {
    if rows == 0 || cols == 0 || !cols.is_multiple_of(V41_BLOCK) {
        return Err(invalid(format!(
            "{rows}x{cols} needs nonzero rows and K a multiple of {V41_BLOCK}"
        )));
    }
    if matrices == 0 || i32::try_from(matrices).is_err() {
        return Err(invalid(format!("{matrices} matrices")));
    }
    if !(1..=65_535).contains(&slots) {
        return Err(invalid(format!("{slots} slots; wants 1 to 65535")));
    }
    let outputs = rows.checked_mul(slots);
    let inputs = cols.checked_mul(slots);
    if outputs.is_none_or(|n| i32::try_from(n).is_err())
        || inputs.is_none_or(|n| i32::try_from(n).is_err())
    {
        return Err(invalid(format!(
            "{rows}x{cols} with {slots} slots exceeds i32"
        )));
    }
    Ok(())
}

impl CudaSubmission {
    /// Quantizes `rows × cols` float32 activations to FP8 E4M3 codes
    /// (`rows × cols` bytes) and UE8M0 scales (`rows × cols / 32` bytes).
    ///
    /// Each value is first rounded to BF16. Per block of 32:
    /// `scale = 2^ceil(log2(max(amax, 1e-4) / 448))`, and each code is the
    /// nearest E4M3 value to `x / scale`, ties to even, saturating at 448.
    /// This matches the reference's `act_quant(x, 32, "ue8m0")` and the CPU
    /// path. A block holding a nonfinite value gets scale byte `0xFF` (NaN)
    /// and NaN codes, so products that read it are NaN; the CPU path refuses
    /// such input instead.
    ///
    /// # Errors
    ///
    /// Refuses bad dimensions or buffers too small for them.
    pub fn v41_act_quant_fp8(
        &mut self,
        input: &CudaBuffer,
        codes: &CudaBuffer,
        scales: &CudaBuffer,
        rows: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        check_v41_matvec_dims(1, cols, 1, rows.max(1))?;
        if rows == 0 {
            return Err(invalid(String::from("no activation rows")));
        }
        let values = rows * cols;
        let input = span(input, 0, values, 4, "activation input")?;
        let codes = span(codes, 0, values, 1, "activation codes")?;
        let scales = span(scales, 0, values / V41_BLOCK, 1, "activation scales")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_act_quant_fp8(
                input,
                codes,
                scales,
                int(rows, "rows")?,
                int(cols, "cols")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 FP8 activation quantization")
    }

    /// Multiplies FP8 activations by a block-scaled matrix for each slot,
    /// writing `slots.count × matrix.rows` floats to `out`.
    ///
    /// `codes` and `scales` hold one activation row (`cols` codes and
    /// `cols / 32` scales) per slot, or one row in all when
    /// `slots.shared_input` is set.
    ///
    /// # Errors
    ///
    /// Refuses bad dimensions and buffers too small for them.
    pub fn v41_block_matvec(
        &mut self,
        matrix: V41Matrix<'_>,
        codes: &CudaBuffer,
        scales: &CudaBuffer,
        slots: V41Slots<'_>,
        out: &CudaBuffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        let (rows, cols) = (matrix.rows, matrix.cols);
        check_v41_matvec_dims(rows, cols, matrix.matrices, slots.count)?;
        let matrix_bytes = matrix.format.weight_bytes(rows, cols);
        let matrix_scale_bytes = matrix.format.scale_bytes(rows, cols);
        let weights = span(
            matrix.weights,
            0,
            matrix_bytes
                .checked_mul(matrix.matrices)
                .ok_or_else(|| invalid(String::from("weight size overflow")))?,
            1,
            "weights",
        )?;
        let weight_scales = span(
            matrix.scales,
            0,
            matrix_scale_bytes
                .checked_mul(matrix.matrices)
                .ok_or_else(|| invalid(String::from("scale size overflow")))?,
            1,
            "weight scales",
        )?;
        let inputs = if slots.shared_input { 1 } else { slots.count };
        let act = span(codes, 0, inputs * cols, 1, "activation codes")?;
        let act_scales = span(scales, 0, inputs * cols / V41_BLOCK, 1, "activation scales")?;
        let ids = match slots.ids {
            Some(ids) => span(ids, 0, slots.count, 4, "matrix ids")?.cast_const(),
            None => std::ptr::null(),
        };
        let out = span(out, 0, slots.count * rows, 4, "output")?;
        let to_i64 = |n: usize| i64::try_from(n).map_err(|_| invalid(format!("{n} exceeds i64")));
        // SAFETY: dimensions, every buffer extent, and the stack size were
        // checked above; ids outside the stack are refused on the device.
        let code = unsafe {
            psionic_v41_block_matvec(
                i32::from(matrix.format == V41MatvecFormat::Fp4E2M1Row32),
                weights,
                weight_scales,
                to_i64(matrix_bytes)?,
                to_i64(matrix_scale_bytes)?,
                int(matrix.matrices, "matrices")?,
                ids,
                act,
                act_scales,
                i32::from(slots.shared_input),
                out,
                int(rows, "rows")?,
                int(cols, "cols")?,
                int(slots.count, "slots")?,
                i32::from(output == V41Output::Bf16),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 block-scaled matvec")
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

/// Rounds to the nearest BF16, ties to even, as the device does.
#[must_use]
pub fn v41_round_bf16(value: f32) -> f32 {
    half::bf16::from_f32(value).to_f32()
}

/// The device's matrix-vector arithmetic on the host, in the same order:
/// for one matrix and one activation row, returns `rows` float32 outputs
/// (not BF16-rounded). Use it to check a kernel bit for bit.
///
/// # Panics
///
/// Panics when a slice is shorter than the shape needs.
#[must_use]
pub fn v41_block_matvec_host(
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
                    partial = e4m3(codes[k]).mul_add(w, partial);
                }
                let weight_scale = match format {
                    V41MatvecFormat::Fp8Block32x32 => weight_scales[(row / V41_BLOCK) * blocks + b],
                    V41MatvecFormat::Fp4E2M1Row32 => weight_scales[row * blocks + b],
                };
                acc += partial * e8m0(scales[b]) * e8m0(weight_scale);
            }
            acc
        })
        .collect()
}

#[cfg(test)]
#[path = "v41_matvec_tests.rs"]
mod tests;
