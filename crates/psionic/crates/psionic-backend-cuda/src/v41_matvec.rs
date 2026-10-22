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
    /// The `consumer-q` profile's 2-bit routed experts (#12465, lossy):
    /// codes `[rows, cols / 4]` (value `j` of a 32-value block in bits
    /// `2j..2j+2` of the block's 8 bytes), one log-scale byte per block
    /// `[rows, cols / 32]` padded to 4 bytes, then one `f32` scale per row.
    /// A value is `row × 2^((u - 255) / 32) × V41_Q2_LEVELS[code]`.
    Q2Row32,
    /// The same with 3-bit codes, 12 bytes per block, and
    /// [`V41_Q3_LEVELS`].
    Q3Row32,
}

/// The 2-bit levels: the Lloyd–Max quantizer of a unit normal.
pub const V41_Q2_LEVELS: [f32; 4] = [-1.5104, -0.4528, 0.4528, 1.5104];
/// The 3-bit levels: the Lloyd–Max quantizer of a unit normal.
pub const V41_Q3_LEVELS: [f32; 8] = [-2.152, -1.344, -0.756, -0.2451, 0.2451, 0.756, 1.344, 2.152];

/// `2^(f / 32)` rounded to `f32`, for `f` in `0..32`; the device kernels
/// carry the same table.
const V41_Q_FRACTIONS: [u32; 32] = [
    1065353216, 1065536903, 1065724611, 1065916431, 1066112450, 1066312762, 1066517459, 1066726640,
    1066940400, 1067158842, 1067382066, 1067610179, 1067843287, 1068081499, 1068324927, 1068573686,
    1068827891, 1069087663, 1069353124, 1069624397, 1069901610, 1070184894, 1070474380, 1070770206,
    1071072509, 1071381432, 1071697119, 1072019719, 1072349383, 1072686266, 1073030525, 1073382323,
];

/// A `consumer-q` block's weight scale: `row × 2^((u - 255) / 32)`, one
/// rounding (the power of two is exact).
#[must_use]
pub fn v41_q_block_scale(row: f32, u: u8) -> f32 {
    let t = i32::from(u) - 255;
    let a = t.div_euclid(32);
    let f = t.rem_euclid(32) as usize;
    let power = f32::from_bits(((127 + a) as u32) << 23);
    row * (f32::from_bits(V41_Q_FRACTIONS[f]) * power)
}

/// Code `j` (0 to 31) of a `consumer-q` block's packed bytes.
#[must_use]
pub fn v41_q_code(bits: usize, block: &[u8], j: usize) -> usize {
    let at = bits * j;
    let mut word = u32::from(block[at / 8]);
    if at / 8 + 1 < block.len() {
        word |= u32::from(block[at / 8 + 1]) << 8;
    }
    ((word >> (at % 8)) & ((1 << bits) - 1)) as usize
}

impl V41MatvecFormat {
    /// Weight bytes of one `rows × cols` matrix.
    #[must_use]
    pub const fn weight_bytes(self, rows: usize, cols: usize) -> usize {
        match self {
            Self::Fp8Block32x32 => rows * cols,
            Self::Fp4E2M1Row32 => rows * cols / 2,
            Self::Q2Row32 => rows * cols / 4,
            Self::Q3Row32 => rows * cols * 3 / 8,
        }
    }

    /// Code bits per value of a `consumer-q` format.
    #[must_use]
    pub const fn q_bits(self) -> Option<usize> {
        match self {
            Self::Q2Row32 => Some(2),
            Self::Q3Row32 => Some(3),
            _ => None,
        }
    }

    /// The device's format id: 0 FP8, 1 FP4, 2 Q2, 3 Q3.
    #[must_use]
    pub const fn id(self) -> i32 {
        match self {
            Self::Fp8Block32x32 => 0,
            Self::Fp4E2M1Row32 => 1,
            Self::Q2Row32 => 2,
            Self::Q3Row32 => 3,
        }
    }

    /// Scale bytes of one `rows × cols` matrix.
    #[must_use]
    pub const fn scale_bytes(self, rows: usize, cols: usize) -> usize {
        match self {
            Self::Fp8Block32x32 => rows.div_ceil(V41_BLOCK) * (cols / V41_BLOCK),
            Self::Fp4E2M1Row32 => rows * (cols / V41_BLOCK),
            Self::Q2Row32 | Self::Q3Row32 => (rows * (cols / V41_BLOCK)).div_ceil(4) * 4 + 4 * rows,
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
    fn psionic_v41_block_matmul(
        fp4: i32,
        weights: *const c_void,
        weight_scales: *const c_void,
        matrix_bytes: i64,
        matrix_scale_bytes: i64,
        matrices: i32,
        entries: *const c_void,
        tiles: *const c_void,
        tile_count: *const c_void,
        max_tiles: i32,
        top_k: i32,
        act_shared: i32,
        act: *const c_void,
        act_scales: *const c_void,
        out: *mut c_void,
        rows: i32,
        cols: i32,
        round_bf16: i32,
        cache_weights: *const c_void,
        cache_scales: *const c_void,
        slot_map: *const c_void,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_importance(
        ids: *const c_void,
        count: i32,
        top_k: i32,
        act_shared: i32,
        act: *const c_void,
        act_scales: *const c_void,
        cols: i32,
        experts: i32,
        sums: *mut c_void,
        counts: *mut c_void,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_group_entries(
        ids: *const c_void,
        count: i32,
        experts: i32,
        scratch: *mut c_void,
        entries: *mut c_void,
        tiles: *mut c_void,
        tile_count: *mut c_void,
        stream: *mut c_void,
    ) -> i32;
}

/// Activation rows one tile of [`CudaSubmission::v41_block_matmul`] takes.
pub const V41_TILE_ROWS: usize = 8;

/// The work list of a batched product (#12464): entries sorted by matrix,
/// cut into tiles of at most [`V41_TILE_ROWS`] entries that share a
/// matrix. Entry `e` reads activation row `e / top_k` when `shared_input`
/// (routed gate and up projections, one row per token) or row `e`
/// otherwise, and writes output row `e`.
#[derive(Clone, Copy, Debug)]
pub struct V41Batch<'a> {
    /// Device `i32` entry ids, sorted by matrix.
    pub entries: &'a CudaBuffer,
    /// Device `int4` tiles: matrix, first entry, count, unused.
    pub tiles: &'a CudaBuffer,
    /// Device `i32`: how many tiles are in use.
    pub tile_count: &'a CudaBuffer,
    /// Tiles the launch covers at most (the grid's height).
    pub max_tiles: usize,
    /// Entries in the list.
    pub count: usize,
    pub top_k: usize,
    pub shared_input: bool,
}

impl V41Batch<'_> {
    /// Tiles a list of `count` entries needs at most when it spans
    /// `matrices` matrices (one partial tile each, plus one for invalid ids).
    #[must_use]
    pub const fn max_tiles(count: usize, matrices: usize) -> usize {
        count.div_ceil(V41_TILE_ROWS) + matrices + 1
    }
}

/// Where a routed projection's cached experts live for a batched product.
#[derive(Clone, Copy, Debug)]
pub struct V41BatchCache<'a> {
    pub weights: &'a CudaBuffer,
    pub scales: &'a CudaBuffer,
    pub slots: usize,
    pub slot_map: &'a CudaBuffer,
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
        self.v41_block_matvec_order(matrix, codes, scales, slots, out, output, 0)
    }

    /// Packed FP8 with the measured B200 32-product sum. With eight
    /// `shards`, rounds each row-parallel rank partial to BF16 before its
    /// F32 sum. This is a numerical diagnostic, separate from the default
    /// serving order; it does not change the checkpoint layout.
    ///
    /// # Errors
    ///
    /// Refuses non-FP8 weights, a rank count other than one or eight, or
    /// columns that do not divide into complete 32-value blocks per rank.
    pub fn v41_block_matvec_reference(
        &mut self,
        matrix: V41Matrix<'_>,
        codes: &CudaBuffer,
        scales: &CudaBuffer,
        slots: V41Slots<'_>,
        out: &CudaBuffer,
        output: V41Output,
        shards: usize,
    ) -> Result<(), RuntimeError> {
        if matrix.format != V41MatvecFormat::Fp8Block32x32
            || !matches!(shards, 1 | 8)
            || !matrix.cols.is_multiple_of(shards * V41_BLOCK)
        {
            return Err(invalid("reference FP8 rank geometry".into()));
        }
        self.v41_block_matvec_order(
            matrix,
            codes,
            scales,
            slots,
            out,
            output,
            (shards as i32) << 8,
        )
    }

    fn v41_block_matvec_order(
        &mut self,
        matrix: V41Matrix<'_>,
        codes: &CudaBuffer,
        scales: &CudaBuffer,
        slots: V41Slots<'_>,
        out: &CudaBuffer,
        output: V41Output,
        order: i32,
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
                matrix.format.id() | order,
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

    /// Calibration for the `consumer-q` quantizer (#12465): adds the square
    /// of each of `count` entries' FP8 activations (row `e / top_k` when
    /// `shared_input`, else `e`) to `sums[ids[e]][column]` (`experts × cols`
    /// floats) and one to `counts[ids[e]]`.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_importance(
        &mut self,
        ids: &CudaBuffer,
        count: usize,
        top_k: usize,
        shared_input: bool,
        codes: &CudaBuffer,
        scales: &CudaBuffer,
        cols: usize,
        experts: usize,
        sums: &CudaBuffer,
        counts: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let rows = if shared_input {
            count / top_k.max(1)
        } else {
            count
        };
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_importance(
                span(ids, 0, count, 4, "expert ids")?.cast_const(),
                int(count, "entries")?,
                int(top_k, "top-k")?,
                i32::from(shared_input),
                span(codes, 0, rows * cols, 1, "activation codes")?.cast_const(),
                span(scales, 0, rows * cols / V41_BLOCK, 1, "activation scales")?.cast_const(),
                int(cols, "cols")?,
                int(experts, "experts")?,
                span(sums, 0, experts * cols, 4, "sums")?,
                span(counts, 0, experts, 4, "counts")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 importance")
    }

    /// Sorts `count` routed entries by expert (`ids[e]`, `i32`) into a
    /// [`V41Batch`]'s entries and tiles. `scratch` holds `2 × (experts + 1)`
    /// `i32` words. Ids outside the experts get NaN outputs.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_group_entries(
        &mut self,
        ids: &CudaBuffer,
        count: usize,
        experts: usize,
        scratch: &CudaBuffer,
        entries: &CudaBuffer,
        tiles: &CudaBuffer,
        tile_count: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if count == 0 || experts == 0 {
            return Err(invalid(format!("{count} entries over {experts} experts")));
        }
        let max_tiles = V41Batch::max_tiles(count, experts);
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_group_entries(
                span(ids, 0, count, 4, "expert ids")?.cast_const(),
                int(count, "entries")?,
                int(experts, "experts")?,
                span(scratch, 0, 2 * (experts + 1), 4, "group scratch")?,
                span(entries, 0, count, 4, "entries")?,
                span(tiles, 0, 4 * max_tiles, 4, "tiles")?,
                span(tile_count, 0, 1, 4, "tile count")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 group entries")
    }

    /// The products of [`Self::v41_block_matvec`] for every entry of a
    /// batch, reading each weight block once per tile: bit-equal to one
    /// matrix-vector product per entry (#12464). `act_rows` and `out_rows`
    /// bound the activation and output arrays. With `cache`, matrix ids go
    /// through its slot map as [`crate::v41_expert_cache::V41CachedStack`]
    /// does, and `matrix` (the host stack) may be `None`.
    ///
    /// # Errors
    ///
    /// Refuses bad dimensions and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_block_matmul(
        &mut self,
        format: V41MatvecFormat,
        matrix: Option<V41Matrix<'_>>,
        cache: Option<V41BatchCache<'_>>,
        rows: usize,
        cols: usize,
        matrices: usize,
        batch: V41Batch<'_>,
        codes: &CudaBuffer,
        scales: &CudaBuffer,
        act_rows: usize,
        out: &CudaBuffer,
        out_rows: usize,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        check_v41_matvec_dims(rows, cols, matrices, 1)?;
        if batch.max_tiles == 0 || batch.max_tiles > 65_535 || batch.top_k == 0 {
            return Err(invalid(format!("{} tiles", batch.max_tiles)));
        }
        let matrix_bytes = format.weight_bytes(rows, cols);
        let scale_bytes = format.scale_bytes(rows, cols);
        let (weights, weight_scales) = match matrix {
            Some(m) => {
                if m.rows != rows || m.cols != cols || m.format != format {
                    return Err(invalid(String::from("matrix shape")));
                }
                (
                    span(m.weights, 0, matrix_bytes * m.matrices, 1, "weights")?.cast_const(),
                    span(m.scales, 0, scale_bytes * m.matrices, 1, "weight scales")?.cast_const(),
                )
            }
            None if cache.is_some() => (std::ptr::null(), std::ptr::null()),
            None => return Err(invalid(String::from("no weights"))),
        };
        let (cache_w, cache_s, slot_map) = match cache {
            Some(c) => (
                span(
                    c.weights,
                    0,
                    matrix_bytes * c.slots.max(1),
                    1,
                    "cache weights",
                )?
                .cast_const(),
                span(c.scales, 0, scale_bytes * c.slots.max(1), 1, "cache scales")?.cast_const(),
                span(c.slot_map, 0, matrices, 4, "slot map")?.cast_const(),
            ),
            None => (std::ptr::null(), std::ptr::null(), std::ptr::null()),
        };
        let to_i64 = |n: usize| i64::try_from(n).map_err(|_| invalid(format!("{n} exceeds i64")));
        // SAFETY: extents were checked; entries index rows below `act_rows`
        // and `out_rows`, which the batch's builder guarantees.
        let code = unsafe {
            psionic_v41_block_matmul(
                format.id(),
                weights,
                weight_scales,
                to_i64(matrix_bytes)?,
                to_i64(scale_bytes)?,
                int(matrices, "matrices")?,
                span(batch.entries, 0, batch.count, 4, "entries")?.cast_const(),
                span(batch.tiles, 0, 4 * batch.max_tiles, 4, "tiles")?.cast_const(),
                span(batch.tile_count, 0, 1, 4, "tile count")?.cast_const(),
                int(batch.max_tiles, "tiles")?,
                int(batch.top_k, "top-k")?,
                i32::from(batch.shared_input),
                span(codes, 0, act_rows * cols, 1, "activation codes")?.cast_const(),
                span(
                    scales,
                    0,
                    act_rows * cols / V41_BLOCK,
                    1,
                    "activation scales",
                )?
                .cast_const(),
                span(out, 0, out_rows * rows, 4, "output")?,
                int(rows, "rows")?,
                int(cols, "cols")?,
                i32::from(output == V41Output::Bf16),
                cache_w,
                cache_s,
                slot_map,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 block-scaled batched product")
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
                        V41MatvecFormat::Q2Row32 | V41MatvecFormat::Q3Row32 => {
                            let bits = format.q_bits().unwrap_or(2);
                            let block_bytes = 4 * bits;
                            let at = (row * blocks + b) * block_bytes;
                            let code = v41_q_code(bits, &weights[at..at + block_bytes], j);
                            if bits == 2 {
                                V41_Q2_LEVELS[code]
                            } else {
                                V41_Q3_LEVELS[code]
                            }
                        }
                    };
                    partial = e4m3(codes[k]).mul_add(w, partial);
                }
                let weight_scale = match format {
                    V41MatvecFormat::Fp8Block32x32 => {
                        e8m0(weight_scales[(row / V41_BLOCK) * blocks + b])
                    }
                    V41MatvecFormat::Fp4E2M1Row32 => e8m0(weight_scales[row * blocks + b]),
                    V41MatvecFormat::Q2Row32 | V41MatvecFormat::Q3Row32 => {
                        let rows_at = (rows * blocks).div_ceil(4) * 4;
                        let d = f32::from_le_bytes(
                            weight_scales[rows_at + 4 * row..rows_at + 4 * row + 4]
                                .try_into()
                                .unwrap_or([0; 4]),
                        );
                        v41_q_block_scale(d, weight_scales[row * blocks + b])
                    }
                };
                acc += partial * e8m0(scales[b]) * weight_scale;
            }
            acc
        })
        .collect()
}

#[cfg(test)]
#[path = "v41_matvec_tests.rs"]
mod tests;
