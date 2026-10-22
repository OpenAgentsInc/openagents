//! Routed experts computed on the host CPU, bit for bit as the GPU computes
//! them (#12463).
//!
//! A stage with an expert cache in device memory (`v41_expert_cache`) may
//! compute a cache-missed expert on the host instead of reading its weights
//! over the bus. The owner allowed this on 2026-10-21: the bulk of each
//! token's work stays on the GPU, and host arithmetic only supplements it.
//! The answer must not change, so this module repeats the device's
//! arithmetic exactly:
//!
//! - [`v41_act_quant_host`] is `v41_act_quant_fp8`: BF16 rounding, one UE8M0
//!   scale per 32 values, and E4M3 codes rounded half to even.
//! - [`v41_fp4_matvec`] is `v41_block_matvec` for packed FP4 E2M1 weights: a
//!   float32 fused multiply-add chain over each 32-value block from element
//!   0, the block sum times the activation scale and then the weight scale,
//!   and the scaled block sums added in ascending block order.
//! - [`v41_expert_forward`] chains them as the stage's MoE does: the gate
//!   and up projections rounded to BF16, clamped SwiGLU times the route
//!   weight rounded to BF16, the FP8 quantization of that, and the down
//!   projection rounded to BF16.
//!
//! The vector paths keep each block's chain in one SIMD lane, so a lane
//! holds one block of one row and the operations per block are the scalar
//! path's, in the same order. Only independent blocks run side by side; the
//! ordered sum over blocks stays scalar. AVX-512 and AVX2 are chosen at run
//! time; every path equals the scalar one bit for bit, and the tests check
//! it.
//!
//! The design follows Project Maya's CPU expert lane (`mw00/project-maya` at
//! `451f9cb6a2`, `src/ds4/ds4_gguf_cpu.cpp`, MIT), which also sums in the
//! GPU kernel's order so its tiers agree bit for bit. This is a
//! reimplementation for V4.1's native FP4 experts, not a copy.

use crate::v41_matvec::{
    V41_BLOCK, V41_Q2_LEVELS, V41_Q3_LEVELS, V41MatvecFormat, v41_q_block_scale, v41_q_code,
};

/// The E2M1 values of the 16 FP4 codes; code 8 is negative zero.
const E2M1: [f32; 16] = [
    0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, -0.0, -0.5, -1.0, -1.5, -2.0, -3.0, -4.0, -6.0,
];

/// The device's quiet NaN (`CUDART_NAN_F`).
const DEVICE_NAN: f32 = f32::from_bits(0x7fff_ffff);

/// One routed expert's packed FP4 weights in the checkpoint's layout: per
/// projection, `[rows, cols / 2]` code bytes (low nibble first) and
/// `[rows, cols / 32]` UE8M0 scale bytes.
#[derive(Clone, Copy, Debug)]
pub struct V41ExpertWeights<'a> {
    /// Gate projection, `[width, hidden]`.
    pub w1: (&'a [u8], &'a [u8]),
    /// Down projection, `[hidden, width]`.
    pub w2: (&'a [u8], &'a [u8]),
    /// Up projection, `[width, hidden]`.
    pub w3: (&'a [u8], &'a [u8]),
}

/// The shapes an expert forward needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41ExpertDims {
    /// Model width (5,120 for V4.1-Flash).
    pub hidden: usize,
    /// Expert width (2,304).
    pub width: usize,
    /// SwiGLU clamp (10.0); zero or less disables it.
    pub swiglu_limit: f32,
    /// How `[w1, w2, w3]` store their weights: the checkpoint's FP4, or a
    /// `consumer-q` format (#12465).
    pub formats: [V41MatvecFormat; 3],
}

impl V41ExpertDims {
    /// The checkpoint's experts: FP4 everywhere.
    #[must_use]
    pub const fn fp4(hidden: usize, width: usize, swiglu_limit: f32) -> Self {
        let f = V41MatvecFormat::Fp4E2M1Row32;
        Self {
            hidden,
            width,
            swiglu_limit,
            formats: [f, f, f],
        }
    }

    /// Weight and scale bytes of a `rows × cols` FP4 matrix.
    #[must_use]
    pub const fn fp4_bytes(rows: usize, cols: usize) -> (usize, usize) {
        (rows * cols / 2, rows * (cols / V41_BLOCK))
    }

    /// `(rows, cols)` of projection `p` (0 for w1, 1 for w2, 2 for w3).
    #[must_use]
    pub const fn shape(&self, p: usize) -> (usize, usize) {
        if p == 1 {
            (self.hidden, self.width)
        } else {
            (self.width, self.hidden)
        }
    }

    /// Weight and scale bytes of projection `p`.
    #[must_use]
    pub const fn part_bytes(&self, p: usize) -> (usize, usize) {
        let (rows, cols) = self.shape(p);
        let f = self.formats[p];
        (f.weight_bytes(rows, cols), f.scale_bytes(rows, cols))
    }

    /// Bytes of one expert's three projections, weights and scales.
    #[must_use]
    pub const fn expert_bytes(&self) -> usize {
        let (w1, s1) = self.part_bytes(0);
        let (w2, s2) = self.part_bytes(1);
        let (w3, s3) = self.part_bytes(2);
        w1 + s1 + w2 + s2 + w3 + s3
    }
}

/// The device's E4M3 decoding (`e4m3` in `v41_block_matvec.cu`).
#[must_use]
pub fn v41_e4m3(byte: u8) -> f32 {
    let magnitude = u32::from(byte & 0x7f);
    let value = if magnitude == 0x7f {
        DEVICE_NAN
    } else {
        f32::from_bits(magnitude << 20) * f32::from_bits((120 + 127) << 23)
    };
    f32::from_bits(value.to_bits() | (u32::from(byte & 0x80) << 24))
}

/// The device's UE8M0 decoding (`e8m0`).
#[must_use]
pub fn v41_e8m0(byte: u8) -> f32 {
    match byte {
        0xff => DEVICE_NAN,
        0 => f32::from_bits(0x0040_0000),
        exponent => f32::from_bits(u32::from(exponent) << 23),
    }
}

/// The device's BF16 rounding (`bf16_round`): nearest, ties to even, and a
/// NaN kept quiet with its sign.
#[must_use]
pub fn v41_bf16_round(value: f32) -> f32 {
    let bits = value.to_bits();
    if bits & 0x7fff_ffff > 0x7f80_0000 {
        return f32::from_bits((bits | 0x0040_0000) & 0xffff_0000);
    }
    let rounded = bits.wrapping_add(0x7fff + ((bits >> 16) & 1));
    f32::from_bits(rounded & 0xffff_0000)
}

/// The device's `encode_e4m3`: the nearest E4M3 code, ties to the even code,
/// saturating at 448. `value` must be finite.
fn encode_e4m3(value: f32) -> u8 {
    let magnitude = value.abs().min(448.0);
    let mut code = if magnitude < f32::from_bits((127 - 6) << 23) {
        // Subnormal step 2^-9: the product is exact, then round half to even.
        (magnitude * 512.0).round_ties_even() as u32
    } else {
        let bits = magnitude.to_bits();
        let exponent = (bits >> 23) - 120;
        let mut mantissa = (bits >> 20) & 7;
        let rest = bits & 0xf_ffff;
        if rest > 0x8_0000 || (rest == 0x8_0000 && mantissa & 1 == 1) {
            mantissa += 1;
        }
        (exponent << 3) + mantissa
    };
    code = code.min(126);
    (code as u8) | if value.is_sign_negative() { 0x80 } else { 0 }
}

/// The device's `v41_act_quant_fp8` for one row: `cols` FP8 codes and
/// `cols / 32` UE8M0 scales. A block holding a nonfinite value gets scale
/// `0xFF` and NaN codes, as on the device.
///
/// # Panics
///
/// Panics when `cols` isn't a multiple of 32 or a slice is too short.
pub fn v41_act_quant_host(input: &[f32], codes: &mut [u8], scales: &mut [u8]) {
    let cols = input.len();
    assert!(cols.is_multiple_of(V41_BLOCK), "{cols} values");
    for (b, block) in input.chunks_exact(V41_BLOCK).enumerate() {
        let mut values = [0.0_f32; V41_BLOCK];
        let mut bad = false;
        let mut amax = 0.0_f32;
        for (v, x) in values.iter_mut().zip(block) {
            *v = v41_bf16_round(*x);
            bad |= !v.is_finite();
            amax = amax.max(v.abs());
        }
        let mut exponent = 0xff_u32;
        if !bad {
            let bits = (amax.max(1e-4) * (1.0_f32 / 448.0)).to_bits();
            exponent = ((bits >> 23) & 0xff) + u32::from(bits & 0x7f_ffff != 0);
            bad = exponent >= 0xff;
        }
        let out = &mut codes[b * V41_BLOCK..(b + 1) * V41_BLOCK];
        if bad {
            out.fill(0x7f);
            scales[b] = 0xff;
            continue;
        }
        let scale = f32::from_bits(exponent << 23);
        for (c, v) in out.iter_mut().zip(values) {
            *c = encode_e4m3(v / scale);
        }
        scales[b] = exponent as u8;
    }
}

/// FP8 activations decoded once for [`v41_fp4_matvec`]: values block-major
/// transposed (`[32, blocks]`, value `j` of every block together), and the
/// block scales.
#[derive(Clone, Debug, Default)]
pub struct V41DecodedActivation {
    blocks: usize,
    /// `values[j * blocks + b]` is element `b * 32 + j`.
    values: Vec<f32>,
    scales: Vec<f32>,
}

impl V41DecodedActivation {
    /// Decodes `codes` (`32 × blocks` bytes) and `scales` (`blocks` bytes).
    ///
    /// # Panics
    ///
    /// Panics when the lengths disagree.
    #[must_use]
    pub fn new(codes: &[u8], scales: &[u8]) -> Self {
        let mut this = Self::default();
        this.fill(codes, scales);
        this
    }

    /// [`Self::new`] into existing storage.
    ///
    /// # Panics
    ///
    /// Panics when the lengths disagree.
    pub fn fill(&mut self, codes: &[u8], scales: &[u8]) {
        let blocks = codes.len() / V41_BLOCK;
        assert_eq!(codes.len(), blocks * V41_BLOCK);
        assert_eq!(scales.len(), blocks);
        self.blocks = blocks;
        self.values.resize(codes.len(), 0.0);
        for b in 0..blocks {
            for j in 0..V41_BLOCK {
                self.values[j * blocks + b] = v41_e4m3(codes[b * V41_BLOCK + j]);
            }
        }
        self.scales.clear();
        self.scales.extend(scales.iter().map(|s| v41_e8m0(*s)));
    }

    /// Input values (`32 × blocks`).
    #[must_use]
    pub fn cols(&self) -> usize {
        self.blocks * V41_BLOCK
    }
}

/// Which instructions [`v41_fp4_matvec`] uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V41LaneIsa {
    Scalar,
    Avx2,
    Avx512,
}

impl V41LaneIsa {
    /// The widest path this CPU supports.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if std::arch::is_x86_feature_detected!("avx512f") {
                return Self::Avx512;
            }
            if std::arch::is_x86_feature_detected!("avx2")
                && std::arch::is_x86_feature_detected!("fma")
            {
                return Self::Avx2;
            }
        }
        Self::Scalar
    }

    /// Whether this CPU can run the path.
    #[must_use]
    pub fn available(self) -> bool {
        match self {
            Self::Scalar => true,
            Self::Avx2 => {
                #[cfg(target_arch = "x86_64")]
                {
                    std::arch::is_x86_feature_detected!("avx2")
                        && std::arch::is_x86_feature_detected!("fma")
                }
                #[cfg(not(target_arch = "x86_64"))]
                {
                    false
                }
            }
            Self::Avx512 => {
                #[cfg(target_arch = "x86_64")]
                {
                    std::arch::is_x86_feature_detected!("avx512f")
                }
                #[cfg(not(target_arch = "x86_64"))]
                {
                    false
                }
            }
        }
    }
}

/// Rows `rows` of a packed FP4 matrix times a decoded activation, as
/// `v41_block_matvec` computes them (float32, before any BF16 rounding).
/// `weights` and `scales` hold the whole matrix; `out[i]` gets row
/// `rows.start + i`.
///
/// # Panics
///
/// Panics when the slices are too short or `isa` isn't available.
pub fn v41_fp4_matvec(
    isa: V41LaneIsa,
    weights: &[u8],
    scales: &[u8],
    x: &V41DecodedActivation,
    rows: std::ops::Range<usize>,
    out: &mut [f32],
) {
    let blocks = x.blocks;
    let row_bytes = blocks * V41_BLOCK / 2;
    assert!(weights.len() >= rows.end * row_bytes, "weights too short");
    assert!(scales.len() >= rows.end * blocks, "scales too short");
    assert_eq!(out.len(), rows.len());
    assert!(isa.available(), "{isa:?} is not available on this CPU");
    let mut scaled = vec![0.0_f32; blocks];
    for (o, row) in out.iter_mut().zip(rows) {
        let w = &weights[row * row_bytes..(row + 1) * row_bytes];
        let s = &scales[row * blocks..(row + 1) * blocks];
        let done = match isa {
            Isa::Scalar => 0,
            #[cfg(target_arch = "x86_64")]
            Isa::Avx2 => unsafe { x86::blocks_avx2(w, s, x, &mut scaled) },
            #[cfg(target_arch = "x86_64")]
            Isa::Avx512 => unsafe { x86::blocks_avx512(w, s, x, &mut scaled) },
            #[cfg(not(target_arch = "x86_64"))]
            _ => 0,
        };
        for (b, slot) in scaled.iter_mut().enumerate().skip(done) {
            *slot = block_scalar(w, s, x, b);
        }
        // The ordered sum over blocks, as the device adds them.
        let mut acc = 0.0_f32;
        for v in &scaled {
            acc += *v;
        }
        *o = acc;
    }
}

use V41LaneIsa as Isa;

/// Rows `rows` of any routed-expert matrix of `total_rows` rows times a
/// decoded activation: [`v41_fp4_matvec`] for FP4, or the `consumer-q`
/// formats (#12465) in the same order as their device kernel: each
/// block's fused chain from element 0, times the activation scale, times
/// the block's weight scale, added in ascending block order.
///
/// # Panics
///
/// Panics when the slices are too short or `isa` isn't available.
#[allow(clippy::too_many_arguments)]
pub fn v41_expert_matvec(
    isa: V41LaneIsa,
    format: V41MatvecFormat,
    weights: &[u8],
    scales: &[u8],
    total_rows: usize,
    x: &V41DecodedActivation,
    rows: std::ops::Range<usize>,
    out: &mut [f32],
) {
    let Some(bits) = format.q_bits() else {
        return v41_fp4_matvec(isa, weights, scales, x, rows, out);
    };
    let blocks = x.blocks;
    let block_bytes = 4 * bits;
    let row_bytes = blocks * block_bytes;
    let rows_at = (total_rows * blocks).div_ceil(4) * 4;
    assert!(weights.len() >= rows.end * row_bytes, "weights too short");
    assert!(scales.len() >= rows_at + 4 * total_rows, "scales too short");
    assert_eq!(out.len(), rows.len());
    assert!(isa.available(), "{isa:?} is not available on this CPU");
    let levels: &[f32] = if bits == 2 {
        &V41_Q2_LEVELS
    } else {
        &V41_Q3_LEVELS
    };
    let mut scaled = vec![0.0_f32; blocks];
    let mut ws = vec![0.0_f32; blocks];
    for (o, row) in out.iter_mut().zip(rows) {
        let w = &weights[row * row_bytes..(row + 1) * row_bytes];
        let u = &scales[row * blocks..(row + 1) * blocks];
        let d = f32::from_le_bytes(
            scales[rows_at + 4 * row..rows_at + 4 * row + 4]
                .try_into()
                .unwrap_or([0; 4]),
        );
        for (s, u) in ws.iter_mut().zip(u) {
            *s = v41_q_block_scale(d, *u);
        }
        let done = match isa {
            Isa::Scalar => 0,
            #[cfg(target_arch = "x86_64")]
            Isa::Avx2 => unsafe { x86::q_blocks_avx2(bits, w, &ws, x, levels, &mut scaled) },
            #[cfg(target_arch = "x86_64")]
            Isa::Avx512 => unsafe { x86::q_blocks_avx512(bits, w, &ws, x, levels, &mut scaled) },
            #[cfg(not(target_arch = "x86_64"))]
            _ => 0,
        };
        for b in done..blocks {
            let block = &w[b * block_bytes..(b + 1) * block_bytes];
            let mut partial = 0.0_f32;
            for j in 0..V41_BLOCK {
                partial =
                    x.values[j * blocks + b].mul_add(levels[v41_q_code(bits, block, j)], partial);
            }
            scaled[b] = partial * x.scales[b] * ws[b];
        }
        let mut acc = 0.0_f32;
        for v in &scaled {
            acc += *v;
        }
        *o = acc;
    }
}

/// One block of one row: the fused chain from element 0, then the two scale
/// multiplies.
fn block_scalar(w: &[u8], s: &[u8], x: &V41DecodedActivation, b: usize) -> f32 {
    let mut partial = 0.0_f32;
    for j in 0..V41_BLOCK {
        let byte = w[b * 16 + j / 2];
        let code = if j % 2 == 0 { byte & 15 } else { byte >> 4 };
        partial = x.values[j * x.blocks + b].mul_add(E2M1[code as usize], partial);
    }
    partial * x.scales[b] * v41_e8m0(s[b])
}

#[cfg(target_arch = "x86_64")]
mod x86 {
    use super::{E2M1, V41DecodedActivation};
    use std::arch::x86_64::*;

    /// Scaled block sums for the first `blocks / 16 × 16` blocks of a row;
    /// returns how many it wrote.
    #[target_feature(enable = "avx512f")]
    pub(super) unsafe fn blocks_avx512(
        w: &[u8],
        s: &[u8],
        x: &V41DecodedActivation,
        scaled: &mut [f32],
    ) -> usize {
        let blocks = x.blocks;
        let full = blocks / 16 * 16;
        unsafe {
            let table = _mm512_loadu_ps(E2M1.as_ptr());
            let lane = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
            let stride = _mm512_mullo_epi32(lane, _mm512_set1_epi32(16));
            let nibble = _mm512_set1_epi32(15);
            let mut b0 = 0;
            while b0 < full {
                let base = w.as_ptr().add(b0 * 16).cast::<i32>();
                let mut partial = _mm512_setzero_ps();
                for q in 0..4 {
                    let offsets = _mm512_add_epi32(stride, _mm512_set1_epi32(4 * q));
                    let g = _mm512_i32gather_epi32::<1>(offsets, base.cast());
                    for t in 0..8 {
                        let j = (8 * q + t) as usize;
                        let code = _mm512_and_si512(
                            _mm512_srlv_epi32(g, _mm512_set1_epi32(4 * t)),
                            nibble,
                        );
                        let wv = _mm512_permutexvar_ps(code, table);
                        let xv = _mm512_loadu_ps(x.values.as_ptr().add(j * blocks + b0));
                        partial = _mm512_fmadd_ps(xv, wv, partial);
                    }
                }
                let xs = _mm512_loadu_ps(x.scales.as_ptr().add(b0));
                let ws = e8m0_512(_mm512_cvtepu8_epi32(_mm_loadu_si128(
                    s.as_ptr().add(b0).cast(),
                )));
                let out = _mm512_mul_ps(_mm512_mul_ps(partial, xs), ws);
                _mm512_storeu_ps(scaled.as_mut_ptr().add(b0), out);
                b0 += 16;
            }
        }
        full
    }

    #[target_feature(enable = "avx512f")]
    unsafe fn e8m0_512(bytes: __m512i) -> __m512 {
        let shifted = _mm512_slli_epi32::<23>(bytes);
        let zero = _mm512_cmpeq_epi32_mask(bytes, _mm512_setzero_si512());
        let nan = _mm512_cmpeq_epi32_mask(bytes, _mm512_set1_epi32(0xff));
        let v = _mm512_mask_mov_epi32(shifted, zero, _mm512_set1_epi32(0x0040_0000));
        let v = _mm512_mask_mov_epi32(v, nan, _mm512_set1_epi32(0x7fff_ffff));
        _mm512_castsi512_ps(v)
    }

    /// The AVX2 path, eight blocks at a time.
    #[target_feature(enable = "avx2,fma")]
    pub(super) unsafe fn blocks_avx2(
        w: &[u8],
        s: &[u8],
        x: &V41DecodedActivation,
        scaled: &mut [f32],
    ) -> usize {
        let blocks = x.blocks;
        let full = blocks / 8 * 8;
        unsafe {
            let magnitudes = _mm256_loadu_ps(E2M1.as_ptr());
            let stride = _mm256_setr_epi32(0, 16, 32, 48, 64, 80, 96, 112);
            let seven = _mm256_set1_epi32(7);
            let eight = _mm256_set1_epi32(8);
            let mut b0 = 0;
            while b0 < full {
                let base = w.as_ptr().add(b0 * 16).cast::<i32>();
                let mut partial = _mm256_setzero_ps();
                for q in 0..4 {
                    let offsets = _mm256_add_epi32(stride, _mm256_set1_epi32(4 * q));
                    let g = _mm256_i32gather_epi32::<1>(base, offsets);
                    for t in 0..8 {
                        let j = (8 * q + t) as usize;
                        let code = _mm256_srlv_epi32(g, _mm256_set1_epi32(4 * t));
                        let mag =
                            _mm256_permutevar8x32_ps(magnitudes, _mm256_and_si256(code, seven));
                        let sign = _mm256_slli_epi32::<28>(_mm256_and_si256(code, eight));
                        let wv = _mm256_or_ps(mag, _mm256_castsi256_ps(sign));
                        let xv = _mm256_loadu_ps(x.values.as_ptr().add(j * blocks + b0));
                        partial = _mm256_fmadd_ps(xv, wv, partial);
                    }
                }
                let xs = _mm256_loadu_ps(x.scales.as_ptr().add(b0));
                let bytes = _mm256_cvtepu8_epi32(_mm_loadl_epi64(s.as_ptr().add(b0).cast()));
                let shifted = _mm256_slli_epi32::<23>(bytes);
                let zero = _mm256_cmpeq_epi32(bytes, _mm256_setzero_si256());
                let nan = _mm256_cmpeq_epi32(bytes, _mm256_set1_epi32(0xff));
                let v = _mm256_blendv_epi8(shifted, _mm256_set1_epi32(0x0040_0000), zero);
                let v = _mm256_blendv_epi8(v, _mm256_set1_epi32(0x7fff_ffff), nan);
                let out = _mm256_mul_ps(_mm256_mul_ps(partial, xs), _mm256_castsi256_ps(v));
                _mm256_storeu_ps(scaled.as_mut_ptr().add(b0), out);
                b0 += 8;
            }
        }
        full
    }

    /// Where code `j` of a block sits in its gathered 32-bit words: the
    /// word, the shift, and whether it continues into the next word.
    #[inline(always)]
    fn q_word_shift(bits: usize, j: usize) -> (usize, u32, bool) {
        let bit = bits * j;
        let shift = (bit % 32) as u32;
        (bit / 32, shift, shift + bits as u32 > 32)
    }

    /// The `consumer-q` blocks of a row, sixteen at a time.
    #[target_feature(enable = "avx512f")]
    pub(super) unsafe fn q_blocks_avx512(
        bits: usize,
        w: &[u8],
        ws: &[f32],
        x: &V41DecodedActivation,
        levels: &[f32],
        scaled: &mut [f32],
    ) -> usize {
        let blocks = x.blocks;
        let full = blocks / 16 * 16;
        let block_bytes = (4 * bits) as i32;
        unsafe {
            let mut table = [0.0_f32; 16];
            table[..levels.len()].copy_from_slice(levels);
            let table = _mm512_loadu_ps(table.as_ptr());
            let lane = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
            let stride = _mm512_mullo_epi32(lane, _mm512_set1_epi32(block_bytes));
            let mask = _mm512_set1_epi32((1 << bits) - 1);
            let mut b0 = 0;
            while b0 < full {
                let base = w.as_ptr().add(b0 * 4 * bits);
                let mut gathered = [_mm512_setzero_si512(); 3];
                for (k, g) in gathered.iter_mut().enumerate().take(bits) {
                    let offsets = _mm512_add_epi32(stride, _mm512_set1_epi32(4 * k as i32));
                    *g = _mm512_i32gather_epi32::<1>(offsets, base.cast());
                }
                let mut partial = _mm512_setzero_ps();
                for j in 0..32 {
                    let (k, shift, spill) = q_word_shift(bits, j);
                    let mut v = _mm512_srlv_epi32(gathered[k], _mm512_set1_epi32(shift as i32));
                    if spill {
                        let hi = _mm512_sllv_epi32(
                            gathered[k + 1],
                            _mm512_set1_epi32(32 - shift as i32),
                        );
                        v = _mm512_or_si512(v, hi);
                    }
                    let code = _mm512_and_si512(v, mask);
                    let wv = _mm512_permutexvar_ps(code, table);
                    let xv = _mm512_loadu_ps(x.values.as_ptr().add(j * blocks + b0));
                    partial = _mm512_fmadd_ps(xv, wv, partial);
                }
                let xs = _mm512_loadu_ps(x.scales.as_ptr().add(b0));
                let wsv = _mm512_loadu_ps(ws.as_ptr().add(b0));
                _mm512_storeu_ps(
                    scaled.as_mut_ptr().add(b0),
                    _mm512_mul_ps(_mm512_mul_ps(partial, xs), wsv),
                );
                b0 += 16;
            }
        }
        full
    }

    /// The `consumer-q` blocks of a row, eight at a time.
    #[target_feature(enable = "avx2,fma")]
    pub(super) unsafe fn q_blocks_avx2(
        bits: usize,
        w: &[u8],
        ws: &[f32],
        x: &V41DecodedActivation,
        levels: &[f32],
        scaled: &mut [f32],
    ) -> usize {
        let blocks = x.blocks;
        let full = blocks / 8 * 8;
        let block_bytes = (4 * bits) as i32;
        unsafe {
            let mut table = [0.0_f32; 8];
            table[..levels.len()].copy_from_slice(levels);
            let table = _mm256_loadu_ps(table.as_ptr());
            let lane = _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7);
            let stride = _mm256_mullo_epi32(lane, _mm256_set1_epi32(block_bytes));
            let mask = _mm256_set1_epi32((1 << bits) - 1);
            let mut b0 = 0;
            while b0 < full {
                let base = w.as_ptr().add(b0 * 4 * bits).cast::<i32>();
                let mut gathered = [_mm256_setzero_si256(); 3];
                for (k, g) in gathered.iter_mut().enumerate().take(bits) {
                    let offsets = _mm256_add_epi32(stride, _mm256_set1_epi32(4 * k as i32));
                    *g = _mm256_i32gather_epi32::<1>(base, offsets);
                }
                let mut partial = _mm256_setzero_ps();
                for j in 0..32 {
                    let (k, shift, spill) = q_word_shift(bits, j);
                    let mut v = _mm256_srlv_epi32(gathered[k], _mm256_set1_epi32(shift as i32));
                    if spill {
                        let hi = _mm256_sllv_epi32(
                            gathered[k + 1],
                            _mm256_set1_epi32(32 - shift as i32),
                        );
                        v = _mm256_or_si256(v, hi);
                    }
                    let code = _mm256_and_si256(v, mask);
                    let wv = _mm256_permutevar8x32_ps(table, code);
                    let xv = _mm256_loadu_ps(x.values.as_ptr().add(j * blocks + b0));
                    partial = _mm256_fmadd_ps(xv, wv, partial);
                }
                let xs = _mm256_loadu_ps(x.scales.as_ptr().add(b0));
                let wsv = _mm256_loadu_ps(ws.as_ptr().add(b0));
                _mm256_storeu_ps(
                    scaled.as_mut_ptr().add(b0),
                    _mm256_mul_ps(_mm256_mul_ps(partial, xs), wsv),
                );
                b0 += 8;
            }
        }
        full
    }
}

/// Scratch for [`v41_expert_forward`], reused between calls.
#[derive(Clone, Debug, Default)]
pub struct V41ExpertScratch {
    gate: Vec<f32>,
    up: Vec<f32>,
    hidden_codes: Vec<u8>,
    hidden_scales: Vec<u8>,
    decoded: V41DecodedActivation,
}

/// The clamped SwiGLU of one value times the route weight, as
/// `v41_swiglu_kernel` computes it, rounded to BF16.
fn swiglu(gate: f32, up: f32, weight: f32, limit: f32) -> f32 {
    if gate.is_nan() || up.is_nan() {
        return v41_bf16_round(DEVICE_NAN);
    }
    let (mut g, mut u) = (gate, up);
    if limit > 0.0 {
        g = g.min(limit);
        u = u.max(-limit).min(limit);
    }
    v41_bf16_round(g * sigmoid_once(g) * u * weight)
}

/// `exp` in double precision rounded to float once, as the device does.
fn exp_once(x: f32) -> f32 {
    f64::from(x).exp() as f32
}

fn sigmoid_once(x: f32) -> f32 {
    if x >= 0.0 {
        1.0 / (1.0 + exp_once(-x))
    } else {
        let e = exp_once(x);
        e / (1.0 + e)
    }
}

/// The gate and up projections of one expert for rows `rows` of `width`,
/// rounded to BF16: the first half of [`v41_expert_forward`], split so
/// threads can share an expert.
///
/// # Panics
///
/// Panics as [`v41_fp4_matvec`] does.
pub fn v41_expert_gate_up(
    isa: V41LaneIsa,
    dims: V41ExpertDims,
    weights: &V41ExpertWeights<'_>,
    x: &V41DecodedActivation,
    rows: std::ops::Range<usize>,
    gate: &mut [f32],
    up: &mut [f32],
) {
    let (w1, w3) = (dims.formats[0], dims.formats[2]);
    v41_expert_matvec(
        isa,
        w1,
        weights.w1.0,
        weights.w1.1,
        dims.width,
        x,
        rows.clone(),
        gate,
    );
    v41_expert_matvec(isa, w3, weights.w3.0, weights.w3.1, dims.width, x, rows, up);
    for v in gate.iter_mut().chain(up.iter_mut()) {
        *v = v41_bf16_round(*v);
    }
}

/// The SwiGLU and its FP8 quantization: the middle of
/// [`v41_expert_forward`]. Writes `width` codes and `width / 32` scales.
///
/// # Panics
///
/// Panics when the slices disagree with `dims`.
pub fn v41_expert_hidden(
    dims: V41ExpertDims,
    gate: &[f32],
    up: &[f32],
    route_weight: f32,
    codes: &mut [u8],
    scales: &mut [u8],
) {
    let hidden: Vec<f32> = gate
        .iter()
        .zip(up)
        .map(|(g, u)| swiglu(*g, *u, route_weight, dims.swiglu_limit))
        .collect();
    v41_act_quant_host(&hidden, codes, scales);
}

/// The down projection for rows `rows` of `hidden`, rounded to BF16: the
/// last part of [`v41_expert_forward`].
///
/// # Panics
///
/// Panics as [`v41_fp4_matvec`] does.
pub fn v41_expert_down(
    isa: V41LaneIsa,
    dims: V41ExpertDims,
    weights: &V41ExpertWeights<'_>,
    h: &V41DecodedActivation,
    rows: std::ops::Range<usize>,
    out: &mut [f32],
) {
    v41_expert_matvec(
        isa,
        dims.formats[1],
        weights.w2.0,
        weights.w2.1,
        dims.hidden,
        h,
        rows,
        out,
    );
    for v in out.iter_mut() {
        *v = v41_bf16_round(*v);
    }
}

/// One routed expert's output for one token, as the stage's MoE computes
/// it on the device: `out` (`hidden` floats) gets the BF16-rounded down
/// projection of the route-weighted, BF16-rounded clamped SwiGLU of the
/// BF16-rounded gate and up projections of `x`.
///
/// # Panics
///
/// Panics when the slices disagree with `dims`.
pub fn v41_expert_forward(
    isa: V41LaneIsa,
    dims: V41ExpertDims,
    weights: &V41ExpertWeights<'_>,
    x: &V41DecodedActivation,
    route_weight: f32,
    scratch: &mut V41ExpertScratch,
    out: &mut [f32],
) {
    let width = dims.width;
    scratch.gate.resize(width, 0.0);
    scratch.up.resize(width, 0.0);
    scratch.hidden_codes.resize(width, 0);
    scratch.hidden_scales.resize(width / V41_BLOCK, 0);
    v41_expert_gate_up(
        isa,
        dims,
        weights,
        x,
        0..width,
        &mut scratch.gate,
        &mut scratch.up,
    );
    v41_expert_hidden(
        dims,
        &scratch.gate,
        &scratch.up,
        route_weight,
        &mut scratch.hidden_codes,
        &mut scratch.hidden_scales,
    );
    scratch
        .decoded
        .fill(&scratch.hidden_codes, &scratch.hidden_scales);
    v41_expert_down(isa, dims, weights, &scratch.decoded, 0..dims.hidden, out);
}

#[cfg(test)]
#[path = "v41_expert_lane_tests.rs"]
mod tests;
