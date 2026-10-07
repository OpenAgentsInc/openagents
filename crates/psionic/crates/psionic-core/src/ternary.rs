//! Ternary TQ-class quantization formats with cross-backend determinism
//! receipts (psionic#1115).
//!
//! Owned Rust implementation of the TQ1_0 (1.6875 bits per weight) and
//! TQ2_0 (2.0625 bits per weight) ternary `{-1, 0, +1}` block formats from
//! the llama.cpp/BitNet lineage, byte-compatible with the ggml
//! `block_tq1_0` / `block_tq2_0` wire layout (`QK_K = 256`).
//!
//! Read-only references (studied, never vendored):
//! - `projects/repos/llama.cpp/ggml/src/ggml-common.h` (block structs) and
//!   `ggml-quants.c` (`quantize_row_tq{1,2}_0_ref`,
//!   `dequantize_row_tq{1,2}_0`)
//! - `projects/tether/repos/qvac-rnd-fabric-llm-bitnet` (Tether QVAC BitNet
//!   lane: Vulkan/Metal TQ kernels, published lossless-verification evidence)
//!
//! Determinism boundary, stated precisely:
//! - Exact integer, no floating point: trit packing (base-3 ceiling-division
//!   encode for TQ1_0, 2-bit fields for TQ2_0), trit unpacking (mod-256
//!   power-of-three multiply plus `(q * 3) >> 8`), the per-block
//!   trit-times-i8 dot accumulated in `i32`, and both f16 conversions (pure
//!   bit manipulation).
//! - Floating point, elementwise, no accumulation-order dependence: the
//!   block `amax` scan (f32 compares; order-independent result), the single
//!   IEEE-754 f32 division `1.0 / d`, and the per-element `value * id`
//!   multiply plus round-half-away-from-zero trit selection. Each is one
//!   correctly-rounded IEEE-754 operation, deterministic on every conforming
//!   host.
//! - Floating point with a pinned evaluation order: dequantized outputs
//!   (`trit as f32 * d` — exact, because the multiplier is -1, 0, or +1) and
//!   the dot combine `((isum as f32) * d) * activation_scale` accumulated in
//!   ascending block order. Deterministic by specification, not by accident.
//!
//! Claim boundary (openagents
//! `docs/training/2026-06-10-qvac-edge-stack-analysis.md` section 5):
//! quantization-fidelity metrics are not correctness proofs. This lane never
//! borrows Tassadar exact-lane language. What is claimed is reproducibility
//! of the quantized forward path, recorded per backend in typed receipts;
//! a backend without a recorded run stays explicitly pending.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

use crate::philox::PhiloxStream;

/// Stable contract id for the ternary TQ format surface.
pub const TERNARY_TQ_CONTRACT_ID: &str = "psionic.core.ternary_tq_formats.v1";

/// Schema version string for the committed fixture document.
pub const TERNARY_TQ_SCHEMA_VERSION: &str = "psionic.core.ternary_tq_formats.v1";

/// Schema version string for one typed determinism receipt.
pub const TERNARY_TQ_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.core.ternary_tq_determinism_receipt.v1";

/// Issue that owns this contract.
pub const TERNARY_TQ_ISSUE: &str = "psionic#1115";

/// Repo-relative path of the committed fixture.
pub const TERNARY_TQ_FIXTURE_PATH: &str = "fixtures/quant/ternary_tq_formats_v1.json";

/// Repo-relative path of the canonical doc.
pub const TERNARY_TQ_DOC_PATH: &str = "docs/PSIONIC_TERNARY_TQ_FORMATS.md";

/// Repo-relative path of the check script.
pub const TERNARY_TQ_CHECK_SCRIPT_PATH: &str = "scripts/check-ternary-tq-formats.sh";

/// Repo-relative path of this module.
pub const TERNARY_TQ_MODULE_PATH: &str = "crates/psionic-core/src/ternary.rs";

/// Elements per ternary block (`QK_K` in the ggml reference).
pub const TERNARY_BLOCK_ELEMENTS: usize = 256;

/// TQ1_0 packed bytes per block: 48 `qs` + 4 `qh` + 2 fp16 scale.
pub const TQ1_0_BLOCK_BYTES: usize = 54;

/// TQ1_0 base-3-packed body bytes (5 trits per byte).
pub const TQ1_0_QS_BYTES: usize = 48;

/// TQ1_0 tail bytes (4 trits per byte).
pub const TQ1_0_QH_BYTES: usize = 4;

/// TQ2_0 packed bytes per block: 64 `qs` (2 bits per trit) + 2 fp16 scale.
pub const TQ2_0_BLOCK_BYTES: usize = 66;

/// TQ2_0 2-bit-packed body bytes.
pub const TQ2_0_QS_BYTES: usize = 64;

/// Pinned seed for the committed determinism-receipt workload.
pub const TERNARY_RECEIPT_SEED: u64 = 0x1115_7E40_C0DE_2026;

/// Rows in the committed determinism-receipt workload.
pub const TERNARY_RECEIPT_ROWS: usize = 4;

/// Elements per row in the committed determinism-receipt workload.
pub const TERNARY_RECEIPT_ROW_ELEMENTS: usize = 1024;

/// Activation scale for the committed dot workload: 2^-6, exactly
/// representable in f32.
pub const TERNARY_RECEIPT_ACTIVATION_SCALE: f32 = 0.015_625;

/// Philox stream offset that separates activation streams from input streams.
pub const TERNARY_RECEIPT_ACTIVATION_STREAM_BASE: u64 = 256;

/// Plain-language definition of the committed determinism-receipt workload.
pub const TERNARY_RECEIPT_WORKLOAD: &str = "4 rows of 1024 f32 inputs; row r element i = \
     (2*u - 1) as f32 with u = PhiloxStream::new(0x11157E40C0DE2026, r).unit_f64_at(i); \
     i8 activations: row r element i = ((PhiloxStream::new(seed, 256 + r).u32_at(i) >> 8) % 255) \
     - 127, block scale 2^-6; per format: sha256 over the packed row bytes (row-major), sha256 \
     over the little-endian f32 dequantized outputs (row-major), and sha256 over per-block i32 \
     isums (little-endian) followed by the per-row f32 dot output (little-endian), rows ascending";

/// Error surface for the bounded ternary quantization module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TernaryError {
    /// Input length is not a multiple of [`TERNARY_BLOCK_ELEMENTS`].
    LengthNotBlockAligned {
        /// Offending input length.
        len: usize,
    },
    /// Input contains a non-finite value; refused rather than silently
    /// encoded, because NaN/inf would make the scale path undefined.
    NonFiniteInput {
        /// Index of the first non-finite element.
        index: usize,
    },
    /// Packed buffer length is not a whole number of blocks for the format.
    PackedLengthInvalid {
        /// Offending packed length.
        len: usize,
        /// Bytes per block for the requested format.
        bytes_per_block: usize,
    },
    /// Activation length does not match the weight row element count.
    ActivationLengthMismatch {
        /// Expected element count.
        expected: usize,
        /// Provided element count.
        actual: usize,
    },
}

impl fmt::Display for TernaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthNotBlockAligned { len } => write!(
                f,
                "ternary input length {len} is not a multiple of {TERNARY_BLOCK_ELEMENTS}"
            ),
            Self::NonFiniteInput { index } => {
                write!(f, "ternary input element {index} is not finite")
            }
            Self::PackedLengthInvalid {
                len,
                bytes_per_block,
            } => write!(
                f,
                "packed length {len} is not a multiple of {bytes_per_block} bytes per block"
            ),
            Self::ActivationLengthMismatch { expected, actual } => write!(
                f,
                "activation length {actual} does not match weight row element count {expected}"
            ),
        }
    }
}

impl std::error::Error for TernaryError {}

/// Ternary TQ-class format selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TernaryFormat {
    /// 1.6875 bits per weight; 5 trits per byte base-3 packing plus a 4-byte
    /// tail and one fp16 scale. ggml `block_tq1_0` wire layout.
    Tq1_0,
    /// 2.0625 bits per weight; 2 bits per trit plus one fp16 scale. ggml
    /// `block_tq2_0` wire layout.
    Tq2_0,
}

impl TernaryFormat {
    /// Stable lowercase label matching the ggml type name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tq1_0 => "tq1_0",
            Self::Tq2_0 => "tq2_0",
        }
    }

    /// Elements per packed block.
    #[must_use]
    pub const fn elements_per_block(self) -> usize {
        TERNARY_BLOCK_ELEMENTS
    }

    /// Packed bytes per block.
    #[must_use]
    pub const fn bytes_per_block(self) -> usize {
        match self {
            Self::Tq1_0 => TQ1_0_BLOCK_BYTES,
            Self::Tq2_0 => TQ2_0_BLOCK_BYTES,
        }
    }

    /// Packed row size for `elements`, when block-aligned.
    #[must_use]
    pub const fn row_bytes(self, elements: usize) -> Option<usize> {
        if elements == 0 || !elements.is_multiple_of(TERNARY_BLOCK_ELEMENTS) {
            return None;
        }
        Some(elements / TERNARY_BLOCK_ELEMENTS * self.bytes_per_block())
    }
}

// ---------------------------------------------------------------------------
// f16 conversion (pure bit manipulation; IEEE 754 binary16, round-to-nearest-
// even on encode, exact on decode).
// ---------------------------------------------------------------------------

/// Converts an f32 to IEEE 754 binary16 bits with round-to-nearest-even.
/// Pure integer bit manipulation; no platform float library in the path.
#[must_use]
pub fn f32_to_f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let mantissa = bits & 0x007F_FFFF;

    if exp == 0xFF {
        if mantissa == 0 {
            return sign | 0x7C00;
        }
        // NaN: keep the top payload bits, force quiet.
        return sign | 0x7C00 | 0x0200 | ((mantissa >> 13) as u16 & 0x01FF);
    }

    // Re-bias from f32 (bias 127) to f16 (bias 15).
    let half_exp = exp - 112;
    if half_exp >= 0x1F {
        return sign | 0x7C00;
    }
    if half_exp <= 0 {
        // Subnormal or zero result.
        if half_exp < -10 {
            return sign;
        }
        let full = mantissa | 0x0080_0000;
        let shift = (14 - half_exp) as u32;
        let halfway = 1_u32 << (shift - 1);
        let remainder = full & ((1_u32 << shift) - 1);
        let mut out = full >> shift;
        if remainder > halfway || (remainder == halfway && (out & 1) == 1) {
            out += 1;
        }
        return sign | out as u16;
    }

    let mut out = ((half_exp as u32) << 10) | (mantissa >> 13);
    let remainder = mantissa & 0x1FFF;
    if remainder > 0x1000 || (remainder == 0x1000 && (out & 1) == 1) {
        // Carry may overflow into the exponent; that is the correct
        // round-up behavior (including overflow to infinity).
        out += 1;
    }
    sign | out as u16
}

/// Converts IEEE 754 binary16 bits to f32. Exact: every finite f16 value is
/// representable in f32. Pure integer bit manipulation.
#[must_use]
pub fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = (u32::from(bits) & 0x8000) << 16;
    let exp = u32::from(bits >> 10) & 0x1F;
    let mantissa = u32::from(bits) & 0x03FF;

    if exp == 0 {
        if mantissa == 0 {
            return f32::from_bits(sign);
        }
        // Subnormal: value = mantissa * 2^-24; normalize into f32.
        let top = mantissa.ilog2();
        let f32_exp = top + 103;
        let f32_mantissa = (mantissa & !(1_u32 << top)) << (23 - top);
        return f32::from_bits(sign | (f32_exp << 23) | f32_mantissa);
    }
    if exp == 0x1F {
        return f32::from_bits(sign | 0x7F80_0000 | (mantissa << 13));
    }
    f32::from_bits(sign | ((exp + 112) << 23) | (mantissa << 13))
}

// ---------------------------------------------------------------------------
// Pack (quantize) and unpack (dequantize).
// ---------------------------------------------------------------------------

fn validate_input(values: &[f32]) -> Result<(), TernaryError> {
    if values.is_empty() || !values.len().is_multiple_of(TERNARY_BLOCK_ELEMENTS) {
        return Err(TernaryError::LengthNotBlockAligned { len: values.len() });
    }
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(TernaryError::NonFiniteInput { index });
        }
    }
    Ok(())
}

/// Block absolute maximum. f32 compares only; the result is independent of
/// scan order, so this step has no accumulation-order dependence.
fn block_amax(block: &[f32]) -> f32 {
    let mut amax = 0.0_f32;
    for value in block {
        let magnitude = value.abs();
        if magnitude > amax {
            amax = magnitude;
        }
    }
    amax
}

/// Trit selection matching the ggml reference: `lroundf(value * id) + 1`,
/// mapping `{-1, 0, +1}` to `{0, 1, 2}`. One f32 multiply plus
/// round-half-away-from-zero; elementwise, no accumulation.
#[inline]
fn encode_trit(value: f32, inverse_scale: f32) -> u32 {
    ((value * inverse_scale).round() as i32 + 1) as u32
}

/// Base-243 ceiling-division byte encode from the ggml reference:
/// `(q * 256 + 242) / 243` for a 5-digit base-3 value `q` in `0..=242`.
#[inline]
fn ceil_div_243(q: u32) -> u8 {
    ((q * 256).div_ceil(243)) as u8
}

/// Quantizes a block-aligned f32 row into TQ1_0 packed bytes
/// (ggml `block_tq1_0` wire layout, 54 bytes per 256 elements).
pub fn quantize_row_tq1_0(values: &[f32]) -> Result<Vec<u8>, TernaryError> {
    validate_input(values)?;
    let mut packed = Vec::with_capacity(values.len() / TERNARY_BLOCK_ELEMENTS * TQ1_0_BLOCK_BYTES);
    for block in values.chunks_exact(TERNARY_BLOCK_ELEMENTS) {
        let scale = block_amax(block);
        let inverse_scale = if scale != 0.0 { 1.0 / scale } else { 0.0 };

        // Bytes 0..32: 5 trits per byte from elements m + n*32 (160 elements).
        for m in 0..32 {
            let mut q = 0_u32;
            for n in 0..5 {
                q = q * 3 + encode_trit(block[m + n * 32], inverse_scale);
            }
            packed.push(ceil_div_243(q));
        }
        // Bytes 32..48: 5 trits per byte from elements 160 + m + n*16
        // (80 elements).
        for m in 0..16 {
            let mut q = 0_u32;
            for n in 0..5 {
                q = q * 3 + encode_trit(block[160 + m + n * 16], inverse_scale);
            }
            packed.push(ceil_div_243(q));
        }
        // Tail bytes qh[0..4]: 4 trits per byte from elements 240 + j + m*4,
        // shifted to occupy the most significant base-3 digits (16 elements).
        for j in 0..4 {
            let mut q = 0_u32;
            for m in 0..4 {
                q = q * 3 + encode_trit(block[240 + j + m * 4], inverse_scale);
            }
            q *= 3;
            packed.push(ceil_div_243(q));
        }
        packed.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
    }
    Ok(packed)
}

/// Quantizes a block-aligned f32 row into TQ2_0 packed bytes
/// (ggml `block_tq2_0` wire layout, 66 bytes per 256 elements).
pub fn quantize_row_tq2_0(values: &[f32]) -> Result<Vec<u8>, TernaryError> {
    validate_input(values)?;
    let mut packed = Vec::with_capacity(values.len() / TERNARY_BLOCK_ELEMENTS * TQ2_0_BLOCK_BYTES);
    for block in values.chunks_exact(TERNARY_BLOCK_ELEMENTS) {
        let scale = block_amax(block);
        let inverse_scale = if scale != 0.0 { 1.0 / scale } else { 0.0 };

        // 64 qs bytes in two 32-byte groups; byte g*32 + m holds the trits of
        // elements g*128 + m + n*32 in bits 2n..=2n+1.
        for group in 0..2 {
            for m in 0..32 {
                let mut byte = 0_u8;
                for n in 0..4 {
                    let trit = encode_trit(block[group * 128 + m + n * 32], inverse_scale);
                    byte |= ((trit & 3) as u8) << (2 * n);
                }
                packed.push(byte);
            }
        }
        packed.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
    }
    Ok(packed)
}

/// Power-of-three multipliers used by the integer trit decode.
const TRIT_DECODE_MULTIPLIERS: [u8; 5] = [1, 3, 9, 27, 81];

/// Extracts trit digit `n` from one base-243-encoded byte. Pure integer:
/// wrapping (mod 256) multiply, widen, multiply by 3, shift.
#[inline]
fn decode_trit(byte: u8, digit: usize) -> i8 {
    let q = byte.wrapping_mul(TRIT_DECODE_MULTIPLIERS[digit]);
    (((u16::from(q) * 3) >> 8) as i8) - 1
}

/// Unpacks one 54-byte TQ1_0 block into 256 trits in `{-1, 0, +1}`,
/// element order. Pure integer arithmetic; no floating point in the path.
#[must_use]
pub fn unpack_tq1_0_block_trits(block: &[u8; TQ1_0_BLOCK_BYTES]) -> [i8; TERNARY_BLOCK_ELEMENTS] {
    let mut trits = [0_i8; TERNARY_BLOCK_ELEMENTS];
    for m in 0..32 {
        for n in 0..5 {
            trits[m + n * 32] = decode_trit(block[m], n);
        }
    }
    for m in 0..16 {
        for n in 0..5 {
            trits[160 + m + n * 16] = decode_trit(block[32 + m], n);
        }
    }
    for j in 0..4 {
        for n in 0..4 {
            trits[240 + j + n * 4] = decode_trit(block[48 + j], n);
        }
    }
    trits
}

/// Unpacks one 66-byte TQ2_0 block into 256 trits in `{-1, 0, +1}`,
/// element order. Pure integer arithmetic; no floating point in the path.
#[must_use]
pub fn unpack_tq2_0_block_trits(block: &[u8; TQ2_0_BLOCK_BYTES]) -> [i8; TERNARY_BLOCK_ELEMENTS] {
    let mut trits = [0_i8; TERNARY_BLOCK_ELEMENTS];
    for group in 0..2 {
        for m in 0..32 {
            let byte = block[group * 32 + m];
            for n in 0..4 {
                trits[group * 128 + m + n * 32] = (((byte >> (2 * n)) & 3) as i8) - 1;
            }
        }
    }
    trits
}

fn packed_blocks(packed: &[u8], bytes_per_block: usize) -> Result<usize, TernaryError> {
    if packed.is_empty() || !packed.len().is_multiple_of(bytes_per_block) {
        return Err(TernaryError::PackedLengthInvalid {
            len: packed.len(),
            bytes_per_block,
        });
    }
    Ok(packed.len() / bytes_per_block)
}

fn block_array<const N: usize>(chunk: &[u8]) -> [u8; N] {
    let mut block = [0_u8; N];
    block.copy_from_slice(chunk);
    block
}

/// Reads the fp16 block scale of a packed block as f32 (exact decode).
fn block_scale(chunk: &[u8], bytes_per_block: usize) -> f32 {
    f16_bits_to_f32(u16::from_le_bytes([
        chunk[bytes_per_block - 2],
        chunk[bytes_per_block - 1],
    ]))
}

/// Dequantizes TQ1_0 packed bytes to f32. Per element the value is
/// `trit as f32 * d`, which is exact (the multiplier is -1, 0, or +1), so the
/// output bit patterns carry no accumulation-order dependence.
pub fn dequantize_row_tq1_0(packed: &[u8]) -> Result<Vec<f32>, TernaryError> {
    let blocks = packed_blocks(packed, TQ1_0_BLOCK_BYTES)?;
    let mut out = Vec::with_capacity(blocks * TERNARY_BLOCK_ELEMENTS);
    for chunk in packed.chunks_exact(TQ1_0_BLOCK_BYTES) {
        let scale = block_scale(chunk, TQ1_0_BLOCK_BYTES);
        let trits = unpack_tq1_0_block_trits(&block_array::<TQ1_0_BLOCK_BYTES>(chunk));
        for trit in trits {
            out.push(f32::from(trit) * scale);
        }
    }
    Ok(out)
}

/// Dequantizes TQ2_0 packed bytes to f32 with the same exactness posture as
/// [`dequantize_row_tq1_0`].
pub fn dequantize_row_tq2_0(packed: &[u8]) -> Result<Vec<f32>, TernaryError> {
    let blocks = packed_blocks(packed, TQ2_0_BLOCK_BYTES)?;
    let mut out = Vec::with_capacity(blocks * TERNARY_BLOCK_ELEMENTS);
    for chunk in packed.chunks_exact(TQ2_0_BLOCK_BYTES) {
        let scale = block_scale(chunk, TQ2_0_BLOCK_BYTES);
        let trits = unpack_tq2_0_block_trits(&block_array::<TQ2_0_BLOCK_BYTES>(chunk));
        for trit in trits {
            out.push(f32::from(trit) * scale);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Exact-integer ternary x i8 dot path.
// ---------------------------------------------------------------------------

/// Per-block exact integer sums `isum = sum(trit_i * activation_i)` for one
/// packed row against an i8 activation row. Pure integer arithmetic: this is
/// the hot accumulation of the ternary matmul path and has no floating point
/// and therefore no accumulation-order dependence at all.
pub fn ternary_q8_block_isums(
    format: TernaryFormat,
    packed: &[u8],
    activations: &[i8],
) -> Result<Vec<i32>, TernaryError> {
    let bytes_per_block = format.bytes_per_block();
    let blocks = packed_blocks(packed, bytes_per_block)?;
    let expected = blocks * TERNARY_BLOCK_ELEMENTS;
    if activations.len() != expected {
        return Err(TernaryError::ActivationLengthMismatch {
            expected,
            actual: activations.len(),
        });
    }
    let mut isums = Vec::with_capacity(blocks);
    for (block_index, chunk) in packed.chunks_exact(bytes_per_block).enumerate() {
        let trits = match format {
            TernaryFormat::Tq1_0 => {
                unpack_tq1_0_block_trits(&block_array::<TQ1_0_BLOCK_BYTES>(chunk))
            }
            TernaryFormat::Tq2_0 => {
                unpack_tq2_0_block_trits(&block_array::<TQ2_0_BLOCK_BYTES>(chunk))
            }
        };
        let base = block_index * TERNARY_BLOCK_ELEMENTS;
        let mut isum = 0_i32;
        for (offset, trit) in trits.iter().enumerate() {
            isum += i32::from(*trit) * i32::from(activations[base + offset]);
        }
        isums.push(isum);
    }
    Ok(isums)
}

/// Row dot of a packed ternary row against an i8 activation row with one
/// f32 activation scale per row. The per-block integer sums are exact; the
/// float combine `((isum as f32) * d) * activation_scale` is accumulated in
/// ascending block order, which every backend must reproduce.
pub fn ternary_q8_row_dot(
    format: TernaryFormat,
    packed: &[u8],
    activations: &[i8],
    activation_scale: f32,
) -> Result<f32, TernaryError> {
    let bytes_per_block = format.bytes_per_block();
    let isums = ternary_q8_block_isums(format, packed, activations)?;
    let mut acc = 0.0_f32;
    for (block_index, isum) in isums.iter().enumerate() {
        let chunk = &packed[block_index * bytes_per_block..(block_index + 1) * bytes_per_block];
        let scale = block_scale(chunk, bytes_per_block);
        acc += (*isum as f32) * scale * activation_scale;
    }
    Ok(acc)
}

// ---------------------------------------------------------------------------
// Committed determinism-receipt workload.
// ---------------------------------------------------------------------------

/// Deterministic f32 input row `row` for the committed workload, derived from
/// the house Philox 4x32-10 stream (psionic#1116).
#[must_use]
pub fn ternary_receipt_input_row(row: usize) -> Vec<f32> {
    let stream = PhiloxStream::new(TERNARY_RECEIPT_SEED, row as u64);
    (0..TERNARY_RECEIPT_ROW_ELEMENTS)
        .map(|index| {
            let unit = stream.unit_f64_at(index as u64);
            (2.0 * unit - 1.0) as f32
        })
        .collect()
}

/// Deterministic i8 activation row `row` for the committed workload.
#[must_use]
pub fn ternary_receipt_activation_row(row: usize) -> Vec<i8> {
    let stream = PhiloxStream::new(
        TERNARY_RECEIPT_SEED,
        TERNARY_RECEIPT_ACTIVATION_STREAM_BASE + row as u64,
    );
    (0..TERNARY_RECEIPT_ROW_ELEMENTS)
        .map(|index| {
            let draw = stream.u32_at(index as u64);
            (((draw >> 8) % 255) as i32 - 127) as i8
        })
        .collect()
}

/// Digest set produced by one backend run of the committed workload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TernaryWorkloadDigests {
    /// SHA-256 over the TQ1_0 packed row bytes, rows ascending.
    pub tq1_0_packed_sha256: String,
    /// SHA-256 over the little-endian f32 TQ1_0 dequantized outputs.
    pub tq1_0_dequantized_sha256: String,
    /// SHA-256 over TQ1_0 per-block i32 isums then per-row f32 dots.
    pub tq1_0_dot_sha256: String,
    /// SHA-256 over the TQ2_0 packed row bytes, rows ascending.
    pub tq2_0_packed_sha256: String,
    /// SHA-256 over the little-endian f32 TQ2_0 dequantized outputs.
    pub tq2_0_dequantized_sha256: String,
    /// SHA-256 over TQ2_0 per-block i32 isums then per-row f32 dots.
    pub tq2_0_dot_sha256: String,
}

/// Full materialized workload: packed bytes plus the digest set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TernaryWorkloadArtifacts {
    /// Concatenated TQ1_0 packed rows.
    pub tq1_0_packed: Vec<u8>,
    /// Concatenated TQ2_0 packed rows.
    pub tq2_0_packed: Vec<u8>,
    /// Digest set over the workload outputs.
    pub digests: TernaryWorkloadDigests,
}

struct FormatHashers {
    packed: Sha256,
    dequantized: Sha256,
    dot: Sha256,
}

impl FormatHashers {
    fn new() -> Self {
        Self {
            packed: Sha256::new(),
            dequantized: Sha256::new(),
            dot: Sha256::new(),
        }
    }
}

/// Computes the committed determinism workload with the owned CPU
/// implementation and returns the packed bytes plus all digests.
pub fn compute_ternary_workload() -> Result<TernaryWorkloadArtifacts, TernaryError> {
    let mut tq1 = FormatHashers::new();
    let mut tq2 = FormatHashers::new();
    let mut tq1_packed_all = Vec::new();
    let mut tq2_packed_all = Vec::new();

    for row in 0..TERNARY_RECEIPT_ROWS {
        let inputs = ternary_receipt_input_row(row);
        let activations = ternary_receipt_activation_row(row);

        for (format, hashers, packed_all) in [
            (TernaryFormat::Tq1_0, &mut tq1, &mut tq1_packed_all),
            (TernaryFormat::Tq2_0, &mut tq2, &mut tq2_packed_all),
        ] {
            let packed = match format {
                TernaryFormat::Tq1_0 => quantize_row_tq1_0(&inputs)?,
                TernaryFormat::Tq2_0 => quantize_row_tq2_0(&inputs)?,
            };
            let dequantized = match format {
                TernaryFormat::Tq1_0 => dequantize_row_tq1_0(&packed)?,
                TernaryFormat::Tq2_0 => dequantize_row_tq2_0(&packed)?,
            };
            let isums = ternary_q8_block_isums(format, &packed, &activations)?;
            let dot = ternary_q8_row_dot(
                format,
                &packed,
                &activations,
                TERNARY_RECEIPT_ACTIVATION_SCALE,
            )?;

            hashers.packed.update(&packed);
            for value in &dequantized {
                hashers.dequantized.update(value.to_le_bytes());
            }
            for isum in &isums {
                hashers.dot.update(isum.to_le_bytes());
            }
            hashers.dot.update(dot.to_le_bytes());
            packed_all.extend_from_slice(&packed);
        }
    }

    let digests = TernaryWorkloadDigests {
        tq1_0_packed_sha256: hex::encode(tq1.packed.finalize()),
        tq1_0_dequantized_sha256: hex::encode(tq1.dequantized.finalize()),
        tq1_0_dot_sha256: hex::encode(tq1.dot.finalize()),
        tq2_0_packed_sha256: hex::encode(tq2.packed.finalize()),
        tq2_0_dequantized_sha256: hex::encode(tq2.dequantized.finalize()),
        tq2_0_dot_sha256: hex::encode(tq2.dot.finalize()),
    };
    Ok(TernaryWorkloadArtifacts {
        tq1_0_packed: tq1_packed_all,
        tq2_0_packed: tq2_packed_all,
        digests,
    })
}

// ---------------------------------------------------------------------------
// Typed determinism receipts.
// ---------------------------------------------------------------------------

/// Status of one backend determinism receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TernaryReceiptStatus {
    /// The workload ran on this backend and the digests below are real.
    Recorded,
    /// The workload has not run on this backend; no claim is made.
    Pending,
}

/// One typed, schema-versioned cross-backend determinism receipt: an
/// assertion that the committed workload produces these exact digests on the
/// named backend, or an explicit pending marker when no run exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TernaryDeterminismReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Owning contract id.
    pub contract_id: String,
    /// Backend name: `cpu`, `metal`, `cuda`, or `vulkan`.
    pub backend: String,
    /// Recorded or pending.
    pub status: TernaryReceiptStatus,
    /// Plain-language workload definition the digests are over.
    pub workload: String,
    /// Digest set; present only for recorded receipts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digests: Option<TernaryWorkloadDigests>,
    /// Why the receipt is pending; present only for pending receipts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_reason: Option<String>,
}

/// Computes the real CPU determinism receipt on the current host.
pub fn cpu_determinism_receipt() -> Result<TernaryDeterminismReceipt, TernaryError> {
    let artifacts = compute_ternary_workload()?;
    Ok(TernaryDeterminismReceipt {
        schema_version: TERNARY_TQ_RECEIPT_SCHEMA_VERSION.to_string(),
        contract_id: TERNARY_TQ_CONTRACT_ID.to_string(),
        backend: "cpu".to_string(),
        status: TernaryReceiptStatus::Recorded,
        workload: TERNARY_RECEIPT_WORKLOAD.to_string(),
        digests: Some(artifacts.digests),
        pending_reason: None,
    })
}

/// Typed-but-pending receipts for the backends the workload has not run on.
/// A backend moves to recorded only after a real run on that backend
/// reproduces (or fails to reproduce) the digests; it is never claimed early.
#[must_use]
pub fn pending_backend_receipts() -> Vec<TernaryDeterminismReceipt> {
    ["metal", "cuda", "vulkan"]
        .into_iter()
        .map(|backend| TernaryDeterminismReceipt {
            schema_version: TERNARY_TQ_RECEIPT_SCHEMA_VERSION.to_string(),
            contract_id: TERNARY_TQ_CONTRACT_ID.to_string(),
            backend: backend.to_string(),
            status: TernaryReceiptStatus::Pending,
            workload: TERNARY_RECEIPT_WORKLOAD.to_string(),
            digests: None,
            pending_reason: Some(format!(
                "no {backend} ternary kernel run exists yet; this receipt records the typed \
                 contract only and claims nothing about {backend} until the committed workload \
                 executes through a psionic {backend} backend kernel and its digests are pinned \
                 here from that run"
            )),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Fixture document (committed at TERNARY_TQ_FIXTURE_PATH).
// ---------------------------------------------------------------------------

/// Packed-layout spec for one format, pinned in the fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TernaryTqBlockLayoutSpec {
    /// Packed bytes per 256-element block.
    pub bytes_per_block: usize,
    /// Base-3 / 2-bit body bytes.
    pub qs_bytes: usize,
    /// Tail bytes (TQ1_0 only; 0 for TQ2_0).
    pub qh_bytes: usize,
    /// Byte offset of the fp16 little-endian scale.
    pub scale_offset: usize,
    /// Scale encoding description.
    pub scale_encoding: String,
    /// Field order in the packed block.
    pub field_order: String,
    /// Effective bits per weight.
    pub bits_per_weight: String,
}

/// Full layout spec pinned in the fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TernaryTqLayoutSpec {
    /// Elements per block (`QK_K`).
    pub block_elements: usize,
    /// TQ1_0 layout.
    pub tq1_0: TernaryTqBlockLayoutSpec,
    /// TQ2_0 layout.
    pub tq2_0: TernaryTqBlockLayoutSpec,
    /// Wire-compatibility verdict.
    pub wire_compatibility: String,
}

/// Returns the pinned layout spec, including the wire-compatibility verdict.
#[must_use]
pub fn ternary_tq_layout_spec() -> TernaryTqLayoutSpec {
    TernaryTqLayoutSpec {
        block_elements: TERNARY_BLOCK_ELEMENTS,
        tq1_0: TernaryTqBlockLayoutSpec {
            bytes_per_block: TQ1_0_BLOCK_BYTES,
            qs_bytes: TQ1_0_QS_BYTES,
            qh_bytes: TQ1_0_QH_BYTES,
            scale_offset: TQ1_0_QS_BYTES + TQ1_0_QH_BYTES,
            scale_encoding: "fp16 little-endian".to_string(),
            field_order: "qs[48] | qh[4] | d".to_string(),
            bits_per_weight: "1.6875".to_string(),
        },
        tq2_0: TernaryTqBlockLayoutSpec {
            bytes_per_block: TQ2_0_BLOCK_BYTES,
            qs_bytes: TQ2_0_QS_BYTES,
            qh_bytes: 0,
            scale_offset: TQ2_0_QS_BYTES,
            scale_encoding: "fp16 little-endian".to_string(),
            field_order: "qs[64] | d".to_string(),
            bits_per_weight: "2.0625".to_string(),
        },
        wire_compatibility: "byte-compatible with ggml block_tq1_0 / block_tq2_0 (QK_K = 256) as \
                             specified by llama.cpp ggml/src/ggml-common.h and the \
                             quantize_row_tq{1,2}_0_ref / dequantize_row_tq{1,2}_0 reference \
                             algorithms in ggml/src/ggml-quants.c"
            .to_string(),
    }
}

/// One pinned known-answer block: a rule-defined 256-element input and its
/// packed bytes for both formats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TernaryTqKnownAnswerBlock {
    /// Stable case id.
    pub id: String,
    /// Machine-reimplementable input rule.
    pub input_rule: String,
    /// TQ1_0 packed bytes, hex.
    pub tq1_0_packed_hex: String,
    /// TQ2_0 packed bytes, hex.
    pub tq2_0_packed_hex: String,
}

/// The pinned known-answer input blocks.
#[must_use]
pub fn ternary_known_answer_inputs() -> Vec<(String, String, Vec<f32>)> {
    let all_zero = vec![0.0_f32; TERNARY_BLOCK_ELEMENTS];
    let unit_cycle: Vec<f32> = (0..TERNARY_BLOCK_ELEMENTS)
        .map(|i| [1.0_f32, -1.0, 0.0, 1.0, -1.0][i % 5])
        .collect();
    let half_cycle: Vec<f32> = (0..TERNARY_BLOCK_ELEMENTS)
        .map(|i| ((i % 3) as f32 - 1.0) * 0.5)
        .collect();
    vec![
        (
            "all_zero".to_string(),
            "x[i] = 0.0 for i in 0..256".to_string(),
            all_zero,
        ),
        (
            "unit_cycle".to_string(),
            "x[i] = [1.0, -1.0, 0.0, 1.0, -1.0][i % 5] for i in 0..256".to_string(),
            unit_cycle,
        ),
        (
            "half_cycle".to_string(),
            "x[i] = ((i % 3) as f32 - 1.0) * 0.5 for i in 0..256".to_string(),
            half_cycle,
        ),
    ]
}

/// Builds the pinned known-answer block records.
pub fn ternary_known_answer_blocks() -> Result<Vec<TernaryTqKnownAnswerBlock>, TernaryError> {
    ternary_known_answer_inputs()
        .into_iter()
        .map(|(id, input_rule, inputs)| {
            Ok(TernaryTqKnownAnswerBlock {
                id,
                input_rule,
                tq1_0_packed_hex: hex::encode(quantize_row_tq1_0(&inputs)?),
                tq2_0_packed_hex: hex::encode(quantize_row_tq2_0(&inputs)?),
            })
        })
        .collect()
}

/// Workload record pinned in the fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TernaryTqWorkloadRecord {
    /// Pinned Philox seed, hex.
    pub seed: String,
    /// Row count.
    pub rows: usize,
    /// Elements per row.
    pub row_elements: usize,
    /// Machine-reimplementable input rule.
    pub input_rule: String,
    /// Machine-reimplementable activation rule.
    pub activation_rule: String,
    /// Activation scale description.
    pub activation_scale: String,
    /// Digest stream definition.
    pub digest_rule: String,
    /// Full TQ1_0 packed workload bytes, hex (for independent decode checks).
    pub tq1_0_packed_hex: String,
    /// Full TQ2_0 packed workload bytes, hex.
    pub tq2_0_packed_hex: String,
    /// Digest set computed by the owned CPU implementation.
    pub digests: TernaryWorkloadDigests,
}

/// Reference-source record pinned in the fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TernaryTqReferenceSources {
    /// Read-only reference implementations studied, never vendored.
    pub read_only_reference_implementations: Vec<String>,
    /// External evidence claims this lane references but does not assert.
    pub external_reference_claims: String,
}

/// Full fixture document committed at [`TERNARY_TQ_FIXTURE_PATH`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TernaryTqFixtureDocument {
    /// Fixture schema version.
    pub schema_version: String,
    /// Owning contract id.
    pub contract_id: String,
    /// Owning issue.
    pub issue: String,
    /// Implementation module path.
    pub module: String,
    /// Canonical doc path.
    pub doc: String,
    /// Check script path.
    pub check_script: String,
    /// Packed-layout spec and wire-compatibility verdict.
    pub layout: TernaryTqLayoutSpec,
    /// Read-only reference sources.
    pub reference_sources: TernaryTqReferenceSources,
    /// Pinned known-answer blocks.
    pub known_answer_blocks: Vec<TernaryTqKnownAnswerBlock>,
    /// Committed determinism workload and CPU digests.
    pub workload: TernaryTqWorkloadRecord,
    /// Per-backend determinism receipts (recorded or explicitly pending).
    pub determinism_receipts: Vec<TernaryDeterminismReceipt>,
    /// Claim discipline for this lane.
    pub claim_boundary: String,
}

/// Builds the full fixture document from the owned implementation. The
/// generator bin serializes exactly this; the check script regenerates it and
/// compares against the committed copy.
pub fn ternary_tq_fixture_document() -> Result<TernaryTqFixtureDocument, TernaryError> {
    let artifacts = compute_ternary_workload()?;
    let mut receipts = vec![TernaryDeterminismReceipt {
        schema_version: TERNARY_TQ_RECEIPT_SCHEMA_VERSION.to_string(),
        contract_id: TERNARY_TQ_CONTRACT_ID.to_string(),
        backend: "cpu".to_string(),
        status: TernaryReceiptStatus::Recorded,
        workload: TERNARY_RECEIPT_WORKLOAD.to_string(),
        digests: Some(artifacts.digests.clone()),
        pending_reason: None,
    }];
    receipts.extend(pending_backend_receipts());

    Ok(TernaryTqFixtureDocument {
        schema_version: TERNARY_TQ_SCHEMA_VERSION.to_string(),
        contract_id: TERNARY_TQ_CONTRACT_ID.to_string(),
        issue: TERNARY_TQ_ISSUE.to_string(),
        module: TERNARY_TQ_MODULE_PATH.to_string(),
        doc: TERNARY_TQ_DOC_PATH.to_string(),
        check_script: TERNARY_TQ_CHECK_SCRIPT_PATH.to_string(),
        layout: ternary_tq_layout_spec(),
        reference_sources: TernaryTqReferenceSources {
            read_only_reference_implementations: vec![
                "projects/repos/llama.cpp/ggml/src/ggml-common.h (block_tq1_0, block_tq2_0)"
                    .to_string(),
                "projects/repos/llama.cpp/ggml/src/ggml-quants.c (quantize_row_tq{1,2}_0_ref, \
                 dequantize_row_tq{1,2}_0)"
                    .to_string(),
                "projects/tether/repos/qvac-rnd-fabric-llm-bitnet (Tether QVAC BitNet lane)"
                    .to_string(),
            ],
            external_reference_claims: "the QVAC BitNet lane publishes 99.04% same-top-token \
                                        rate, mean KL divergence < 0.0003, and claimed bit-exact \
                                        Vulkan-vs-CPU TQ kernels; those are external reference \
                                        claims that motivated this lane, not assertions made by \
                                        this fixture"
                .to_string(),
        },
        known_answer_blocks: ternary_known_answer_blocks()?,
        workload: TernaryTqWorkloadRecord {
            seed: format!("0x{TERNARY_RECEIPT_SEED:016X}"),
            rows: TERNARY_RECEIPT_ROWS,
            row_elements: TERNARY_RECEIPT_ROW_ELEMENTS,
            input_rule: "row r element i = (2*u - 1) as f32 with u = PhiloxStream::new(seed, \
                         r).unit_f64_at(i) (psionic#1116 Philox 4x32-10)"
                .to_string(),
            activation_rule: "row r element i = ((PhiloxStream::new(seed, 256 + r).u32_at(i) >> \
                              8) % 255) - 127 as i8"
                .to_string(),
            activation_scale: "2^-6 (0.015625, exactly representable in f32)".to_string(),
            digest_rule: "per format, rows ascending: packed = sha256(row packed bytes); \
                          dequantized = sha256(little-endian f32 outputs); dot = sha256(per-block \
                          i32 isums little-endian, then the row f32 dot little-endian)"
                .to_string(),
            tq1_0_packed_hex: hex::encode(&artifacts.tq1_0_packed),
            tq2_0_packed_hex: hex::encode(&artifacts.tq2_0_packed),
            digests: artifacts.digests,
        },
        determinism_receipts: receipts,
        claim_boundary: "quantization-fidelity metrics are not correctness proofs; this lane \
                         records reproducibility of the quantized forward path per backend and \
                         never borrows Tassadar exact-lane language (openagents \
                         docs/training/2026-06-10-qvac-edge-stack-analysis.md section 5). A \
                         backend without a recorded run stays pending; recorded digests come \
                         only from real runs."
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use std::{fs, path::Path};

    use super::{
        TERNARY_BLOCK_ELEMENTS, TERNARY_RECEIPT_ACTIVATION_SCALE, TERNARY_RECEIPT_ROW_ELEMENTS,
        TERNARY_RECEIPT_ROWS, TERNARY_TQ_CONTRACT_ID, TERNARY_TQ_FIXTURE_PATH,
        TERNARY_TQ_RECEIPT_SCHEMA_VERSION, TERNARY_TQ_SCHEMA_VERSION, TQ1_0_BLOCK_BYTES,
        TQ2_0_BLOCK_BYTES, TernaryError, TernaryFormat, TernaryReceiptStatus,
        compute_ternary_workload, cpu_determinism_receipt, dequantize_row_tq1_0,
        dequantize_row_tq2_0, f16_bits_to_f32, f32_to_f16_bits, pending_backend_receipts,
        quantize_row_tq1_0, quantize_row_tq2_0, ternary_known_answer_inputs,
        ternary_q8_block_isums, ternary_q8_row_dot, ternary_receipt_activation_row,
        ternary_receipt_input_row, ternary_tq_fixture_document, unpack_tq1_0_block_trits,
        unpack_tq2_0_block_trits,
    };

    fn fixture_json() -> serde_json::Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(TERNARY_TQ_FIXTURE_PATH);
        let raw = fs::read_to_string(&path).expect("ternary fixture must exist");
        serde_json::from_str(&raw).expect("ternary fixture must be valid JSON")
    }

    #[test]
    fn f16_encode_known_answers() {
        let cases: [(f32, u16); 10] = [
            (0.0, 0x0000),
            (-0.0, 0x8000),
            (1.0, 0x3C00),
            (-2.0, 0xC000),
            (0.5, 0x3800),
            (65504.0, 0x7BFF),
            (65520.0, 0x7C00), // rounds up to infinity
            (f32::INFINITY, 0x7C00),
            (6.103_515_6e-5, 0x0400), // smallest normal f16
            (5.960_464_5e-8, 0x0001), // smallest subnormal f16
        ];
        for (value, expected) in cases {
            assert_eq!(f32_to_f16_bits(value), expected, "f32_to_f16_bits({value})");
        }
        // Round-to-nearest-even at the underflow boundary: 2^-25 ties to zero.
        assert_eq!(f32_to_f16_bits(2.0_f32.powi(-25)), 0x0000);
        assert_eq!(f32_to_f16_bits(-(2.0_f32.powi(-25))), 0x8000);
    }

    #[test]
    fn f16_round_trip_is_exact_for_every_non_nan_pattern() {
        for bits in 0..=u16::MAX {
            let exp = (bits >> 10) & 0x1F;
            let mantissa = bits & 0x03FF;
            if exp == 0x1F && mantissa != 0 {
                continue; // NaN payloads are not required to round-trip
            }
            let value = f16_bits_to_f32(bits);
            assert_eq!(f32_to_f16_bits(value), bits, "f16 bits 0x{bits:04X}");
        }
    }

    #[test]
    fn all_zero_block_packs_to_hand_derived_ggml_bytes() {
        // Hand-derived from the published layout: every trit is 1 (zero),
        // so each TQ1_0 qs byte is ceil((121 * 256) / 243) = 128, each qh
        // byte is ceil((120 * 256) / 243) = 127, and the fp16 scale is 0.
        let zeros = vec![0.0_f32; TERNARY_BLOCK_ELEMENTS];
        let tq1 = quantize_row_tq1_0(&zeros).expect("quantize all-zero tq1_0");
        assert_eq!(tq1.len(), TQ1_0_BLOCK_BYTES);
        assert!(tq1[..48].iter().all(|byte| *byte == 128));
        assert!(tq1[48..52].iter().all(|byte| *byte == 127));
        assert_eq!(&tq1[52..], &[0, 0]);

        // TQ2_0: every 2-bit field holds 0b01 -> byte 0x55; fp16 scale 0.
        let tq2 = quantize_row_tq2_0(&zeros).expect("quantize all-zero tq2_0");
        assert_eq!(tq2.len(), TQ2_0_BLOCK_BYTES);
        assert!(tq2[..64].iter().all(|byte| *byte == 0x55));
        assert_eq!(&tq2[64..], &[0, 0]);
    }

    #[test]
    fn known_answer_blocks_round_trip_exact_trits() {
        for (id, _rule, inputs) in ternary_known_answer_inputs() {
            let amax = inputs.iter().fold(0.0_f32, |acc, v| acc.max(v.abs()));
            let expected_trits: Vec<i8> = inputs
                .iter()
                .map(|v| {
                    if amax == 0.0 {
                        0
                    } else {
                        (v / amax).round() as i8
                    }
                })
                .collect();

            let tq1 = quantize_row_tq1_0(&inputs).expect("tq1_0 quantize");
            let mut block1 = [0_u8; TQ1_0_BLOCK_BYTES];
            block1.copy_from_slice(&tq1);
            assert_eq!(
                unpack_tq1_0_block_trits(&block1).to_vec(),
                expected_trits,
                "tq1_0 trits for {id}"
            );

            let tq2 = quantize_row_tq2_0(&inputs).expect("tq2_0 quantize");
            let mut block2 = [0_u8; TQ2_0_BLOCK_BYTES];
            block2.copy_from_slice(&tq2);
            assert_eq!(
                unpack_tq2_0_block_trits(&block2).to_vec(),
                expected_trits,
                "tq2_0 trits for {id}"
            );

            // Dequantized values are exactly {-d, 0, +d}.
            let expected_values: Vec<f32> = expected_trits
                .iter()
                .map(|t| f32::from(*t) * f16_bits_to_f32(f32_to_f16_bits(amax)))
                .collect();
            assert_eq!(
                dequantize_row_tq1_0(&tq1).expect("tq1_0 dequantize"),
                expected_values,
                "tq1_0 values for {id}"
            );
            assert_eq!(
                dequantize_row_tq2_0(&tq2).expect("tq2_0 dequantize"),
                expected_values,
                "tq2_0 values for {id}"
            );
        }
    }

    #[test]
    fn unit_cycle_dot_against_unit_activations_is_exact() {
        let inputs: Vec<f32> = (0..TERNARY_BLOCK_ELEMENTS)
            .map(|i| [1.0_f32, -1.0, 0.0, 1.0, -1.0][i % 5])
            .collect();
        let activations = vec![1_i8; TERNARY_BLOCK_ELEMENTS];
        // 51 full cycles sum to 0; the final element (index 255, i % 5 == 0)
        // contributes +1.
        for format in [TernaryFormat::Tq1_0, TernaryFormat::Tq2_0] {
            let packed = match format {
                TernaryFormat::Tq1_0 => quantize_row_tq1_0(&inputs),
                TernaryFormat::Tq2_0 => quantize_row_tq2_0(&inputs),
            }
            .expect("quantize");
            let isums = ternary_q8_block_isums(format, &packed, &activations).expect("isums");
            assert_eq!(isums, vec![1]);
            let dot = ternary_q8_row_dot(format, &packed, &activations, 1.0).expect("dot");
            assert!((dot - 1.0).abs() < f32::EPSILON, "{format:?} dot {dot}");
        }
    }

    #[test]
    fn refusals_are_typed() {
        assert_eq!(
            quantize_row_tq1_0(&[0.0; 100]),
            Err(TernaryError::LengthNotBlockAligned { len: 100 })
        );
        let mut values = vec![0.0_f32; TERNARY_BLOCK_ELEMENTS];
        values[7] = f32::NAN;
        assert_eq!(
            quantize_row_tq2_0(&values),
            Err(TernaryError::NonFiniteInput { index: 7 })
        );
        assert_eq!(
            dequantize_row_tq1_0(&[0_u8; 53]),
            Err(TernaryError::PackedLengthInvalid {
                len: 53,
                bytes_per_block: TQ1_0_BLOCK_BYTES
            })
        );
        let packed = vec![0x55_u8; TQ2_0_BLOCK_BYTES];
        assert_eq!(
            ternary_q8_block_isums(TernaryFormat::Tq2_0, &packed, &[0_i8; 100]),
            Err(TernaryError::ActivationLengthMismatch {
                expected: 256,
                actual: 100
            })
        );
    }

    #[test]
    fn workload_is_stable_across_recomputation() {
        let first = compute_ternary_workload().expect("workload");
        let second = compute_ternary_workload().expect("workload");
        assert_eq!(first, second);
        assert_eq!(
            first.tq1_0_packed.len(),
            TERNARY_RECEIPT_ROWS
                * (TERNARY_RECEIPT_ROW_ELEMENTS / TERNARY_BLOCK_ELEMENTS)
                * TQ1_0_BLOCK_BYTES
        );
        assert_eq!(
            first.tq2_0_packed.len(),
            TERNARY_RECEIPT_ROWS
                * (TERNARY_RECEIPT_ROW_ELEMENTS / TERNARY_BLOCK_ELEMENTS)
                * TQ2_0_BLOCK_BYTES
        );
    }

    #[test]
    fn workload_rows_are_deterministic_and_in_range() {
        for row in 0..TERNARY_RECEIPT_ROWS {
            let inputs = ternary_receipt_input_row(row);
            assert_eq!(inputs.len(), TERNARY_RECEIPT_ROW_ELEMENTS);
            assert!(inputs.iter().all(|v| (-1.0..1.0).contains(v)));
            assert_eq!(inputs, ternary_receipt_input_row(row));

            let activations = ternary_receipt_activation_row(row);
            assert_eq!(activations.len(), TERNARY_RECEIPT_ROW_ELEMENTS);
            assert!(activations.iter().all(|a| (-127..=127).contains(a)));
            assert_eq!(activations, ternary_receipt_activation_row(row));
        }
        assert_eq!(TERNARY_RECEIPT_ACTIVATION_SCALE, 0.015_625);
    }

    #[test]
    fn cpu_receipt_is_recorded_and_others_are_pending() {
        let cpu = cpu_determinism_receipt().expect("cpu receipt");
        assert_eq!(cpu.schema_version, TERNARY_TQ_RECEIPT_SCHEMA_VERSION);
        assert_eq!(cpu.backend, "cpu");
        assert_eq!(cpu.status, TernaryReceiptStatus::Recorded);
        assert!(cpu.digests.is_some());
        assert!(cpu.pending_reason.is_none());

        let pending = pending_backend_receipts();
        assert_eq!(
            pending
                .iter()
                .map(|r| r.backend.as_str())
                .collect::<Vec<_>>(),
            vec!["metal", "cuda", "vulkan"]
        );
        for receipt in &pending {
            assert_eq!(receipt.status, TernaryReceiptStatus::Pending);
            assert!(receipt.digests.is_none());
            assert!(receipt.pending_reason.is_some());
        }
    }

    #[test]
    fn committed_fixture_matches_owned_implementation() {
        let fixture = fixture_json();
        assert_eq!(fixture["schema_version"], TERNARY_TQ_SCHEMA_VERSION);
        assert_eq!(fixture["contract_id"], TERNARY_TQ_CONTRACT_ID);
        assert_eq!(fixture["issue"], "psionic#1115");

        let document = ternary_tq_fixture_document().expect("fixture document");
        let regenerated = serde_json::to_value(&document).expect("serialize fixture document");
        assert_eq!(
            fixture, regenerated,
            "committed fixture drifted from the owned implementation; regenerate with \
             cargo run -p psionic-core --bin ternary_tq_fixture"
        );
    }
}
