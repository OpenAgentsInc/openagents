//! DeepSeek-V4.1 attention kernels (#12392): tail RoPE and YaRN, window and
//! compressed latent attention with the sink, the compressor, the indexer,
//! and top-k and layer-20 candidate selection.
//!
//! A layer's attention runs on the device in these steps (the FP8
//! projections are [`crate::v41_matvec`]'s):
//!
//! 1. The query, window KV, and indexer query are rotated by
//!    [`CudaSubmission::v41_rope`] (the window base 10,000 on layers 0 and 1,
//!    the compressed base 160,000 with YaRN elsewhere).
//! 2. [`CudaSubmission::v41_window_store`] quantizes the window KV row to the
//!    per-32 FP8 form and writes it into the layer's ring at `p % rows`.
//! 3. On the KV source layers (2, 8, 14, and 20), [`CudaSubmission::v41_dense`]
//!    projects the compressor's values and gates,
//!    [`CudaSubmission::v41_compress`] pools completed groups of 2 (layers
//!    2–19) or 1 (layers 20–39) into latents, and
//!    [`CudaSubmission::v41_compressed_store`] writes each latent's index key
//!    and compressed KV row into the source's [`V41Source`].
//! 4. On the index source layers (2, 8, 14, 20, 24, 28, 32, and 36),
//!    [`CudaSubmission::v41_index_scores`] scores every visible key of the
//!    KV source, layer 20 runs [`CudaSubmission::v41_candidate_blocks`], and
//!    [`CudaSubmission::v41_select_indices`] picks the top 512 (from layer
//!    20's candidate blocks on layers 24–36).
//! 5. [`CudaSubmission::v41_sparse_attention`] attends over the window ring
//!    and the selected rows of the source's compressed KV, which every
//!    consumer layer reads in place on the device.
//!
//! [`V41LayerOwners`] says which layer owns each layer's KV, selection, and
//! candidates.
//!
//! Every call only enqueues work: none allocates or synchronizes, so W7 can
//! record them into a CUDA graph. Callers own every buffer. Positions are
//! device `i32` values, one per token, so a graph replays a decode step at any
//! position; the tokens of one call have consecutive positions.
//!
//! The kernels repeat the CPU path's `ReferenceBf16` arithmetic
//! (`deepseek_v41_cpu`) in its default order: sequential dot products, matrix
//! rows, and RMS sums, the 64-slot online softmax of
//! `quantization::sparse_attention_bf16` with the window rows oldest first,
//! and the selection order of `attention::select_indices` (score descending,
//! then the lower position). They don't model the `ReferenceBf16Tp8` order,
//! which keeps masked window slots in ring order and sums each block's weights
//! in a tree. `exp`, `cos`, and `sin` run in double precision and are rounded
//! to float once. The `*_host` functions repeat all of this on the CPU and
//! reproduce the device bit for bit.
//!
//! Nonfinite input writes NaN where the CPU path returns an error. A selection
//! over a NaN or `+inf` score writes `i32::MIN` in every slot, which makes the
//! attention output NaN.

use crate::clef_prefill::{int, span};
use crate::v41_matvec::V41Output;
use crate::{CudaBuffer, CudaSubmission};
use psionic_runtime::RuntimeError;
use std::ffi::c_void;

/// Layers whose compressor writes a shared KV source.
pub const V41_KV_SOURCES: [usize; 4] = [2, 8, 14, 20];
/// Layers whose indexer writes a shared selection.
pub const V41_INDEX_SOURCES: [usize; 8] = [2, 8, 14, 20, 24, 28, 32, 36];
/// Candidate blocks layer 20 keeps for the later indexers.
pub const V41_CANDIDATE_BLOCKS: usize = 2048;
/// Positions per candidate block.
pub const V41_CANDIDATE_BLOCK_SIZE: usize = 8;
/// Positions RoPE accepts, as the CPU path's `Rope::apply_tail`.
pub const V41_MAX_POSITION: usize = 1 << 20;
/// The value a refused selection writes into every slot.
pub const V41_SELECT_POISON: i32 = i32::MIN;
/// The candidate flag a refused candidate selection writes.
pub const V41_FLAG_POISON: u8 = 0xff;
/// Largest head width the attention kernel accepts.
pub const V41_MAX_HEAD_DIM: usize = 512;
/// Largest compression ratio.
pub const V41_MAX_RATIO: usize = 8;

unsafe extern "C" {
    fn psionic_v41_rope(
        values: *mut c_void,
        frequencies: *const c_void,
        positions: *const c_void,
        tokens: i32,
        heads: i32,
        head_width: i32,
        rope_dim: i32,
        back: i32,
        inverse: i32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_cache_quant(
        input: *const c_void,
        out: *mut c_void,
        kind: i32,
        rows: i32,
        width: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_window_store(
        input: *const c_void,
        ring: *mut c_void,
        positions: *const c_void,
        tokens: i32,
        ring_rows: i32,
        width: i32,
        packed: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_dense(
        weights: *const c_void,
        scales: *const c_void,
        input: *const c_void,
        out: *mut c_void,
        format: i32,
        rows: i32,
        cols: i32,
        groups: i32,
        tokens: i32,
        round_input: i32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_dense_fast(
        weights: *const c_void,
        scales: *const c_void,
        input: *const c_void,
        out: *mut c_void,
        format: i32,
        rows: i32,
        cols: i32,
        groups: i32,
        tokens: i32,
        round_input: i32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_rms_norm_fast(
        input: *const c_void,
        weight: *const c_void,
        out: *mut c_void,
        tokens: i32,
        width: i32,
        epsilon: f32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_rms_norm(
        input: *const c_void,
        weight: *const c_void,
        out: *mut c_void,
        tokens: i32,
        width: i32,
        epsilon: f32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_compress(
        values: *const c_void,
        gates: *const c_void,
        norm: *const c_void,
        state: *mut c_void,
        positions: *const c_void,
        latents: *mut c_void,
        tokens: i32,
        ratio: i32,
        width: i32,
        epsilon: f32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_compressed_store(
        latents: *const c_void,
        wk: *const c_void,
        k_norm: *const c_void,
        frequencies: *const c_void,
        positions: *const c_void,
        kv: *mut c_void,
        index_keys: *mut c_void,
        tokens: i32,
        ratio: i32,
        capacity: i32,
        head_dim: i32,
        index_dim: i32,
        rope_dim: i32,
        epsilon: f32,
        packed: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_index_scores(
        query: *const c_void,
        weights: *const c_void,
        keys: *const c_void,
        positions: *const c_void,
        scores: *mut c_void,
        score_stride: i64,
        tokens: i32,
        capacity: i32,
        ratio: i32,
        heads: i32,
        dim: i32,
        scale: f32,
        packed: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_select(
        values: *const c_void,
        value_stride: i64,
        flags: *const c_void,
        flag_stride: i64,
        flag_group: i32,
        positions: *const c_void,
        tokens: i32,
        ratio: i32,
        group: i32,
        top_k: i32,
        allow_inf: i32,
        out: *mut c_void,
        out_stride: i64,
        flags_out: *mut c_void,
        flags_out_stride: i64,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_block_max(
        scores: *const c_void,
        score_stride: i64,
        block_scores: *mut c_void,
        block_stride: i64,
        positions: *const c_void,
        tokens: i32,
        block_capacity: i32,
        ratio: i32,
        block_size: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_sparse_attention(
        query: *const c_void,
        sinks: *const c_void,
        ring: *const c_void,
        ring_rows: i32,
        compressed: *const c_void,
        capacity: i32,
        indices: *const c_void,
        index_stride: i64,
        positions: *const c_void,
        out: *mut c_void,
        tokens: i32,
        heads: i32,
        head_dim: i32,
        window: i32,
        ratio: i32,
        top_k: i32,
        scale: f32,
        packed: i32,
        stream: *mut c_void,
    ) -> i32;
}

fn invalid(message: String) -> RuntimeError {
    RuntimeError::Backend(format!("v41 attention kernels: {message}"))
}

fn rounds(output: V41Output) -> i32 {
    i32::from(output == V41Output::Bf16)
}

fn stride(value: usize, what: &str) -> Result<i64, RuntimeError> {
    i64::try_from(value).map_err(|_| invalid(format!("{what} {value} exceeds i64")))
}

// ---------------------------------------------------------------------------
// Geometry and the layer schedule
// ---------------------------------------------------------------------------

/// Attention geometry, as `deepseek_v41_cpu::projection::Geometry`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41AttentionGeometry {
    /// Attention heads, a multiple of 4 (a kernel block computes four).
    pub heads: usize,
    /// The shared latent KV width (512).
    pub head_dim: usize,
    /// The rotated tail of every head (64).
    pub rope_dim: usize,
    /// Window rows each layer attends to (128).
    pub window: usize,
    pub index_heads: usize,
    pub index_dim: usize,
    /// Compressed rows each token selects (512).
    pub index_top_k: usize,
    pub norm_epsilon: f32,
}

impl V41AttentionGeometry {
    /// DeepSeek-V4.1-Flash.
    #[must_use]
    pub const fn flash() -> Self {
        Self {
            heads: 64,
            head_dim: 512,
            rope_dim: 64,
            window: 128,
            index_heads: 32,
            index_dim: 128,
            index_top_k: 512,
            norm_epsilon: 1e-20,
        }
    }

    /// The attention scale, `head_dim^-0.5` computed in f64 and rounded.
    #[must_use]
    pub fn attention_scale(self) -> f32 {
        (self.head_dim as f64).powf(-0.5) as f32
    }

    /// The index score scale, `index_dim^-0.5 * index_heads^-0.5` in f64,
    /// rounded once, as the CPU path's reference scores compute it.
    #[must_use]
    pub fn index_scale(self) -> f32 {
        ((self.index_dim as f64).powf(-0.5) * (self.index_heads as f64).powf(-0.5)) as f32
    }

    fn check(self) -> Result<(), RuntimeError> {
        let shared_floats =
            self.index_heads * (self.index_dim + 1) + self.index_heads + 16 * self.index_dim;
        if self.heads == 0
            || !self.heads.is_multiple_of(4)
            || self.head_dim == 0
            || self.head_dim > V41_MAX_HEAD_DIM
            || !self.head_dim.is_multiple_of(64)
            || self.rope_dim == 0
            || !self.rope_dim.is_multiple_of(2)
            || self.rope_dim > self.head_dim.min(self.index_dim)
            || self.window == 0
            || self.index_heads == 0
            || self.index_heads > 32
            || self.index_dim == 0
            || !self.index_dim.is_multiple_of(32)
            || self.index_dim > 1024
            || self.index_top_k == 0
            || shared_floats > 12 * 1024
            || !(self.norm_epsilon.is_finite() && self.norm_epsilon > 0.0)
            || i32::try_from(self.heads * self.head_dim).is_err()
            || i32::try_from(self.window + self.index_top_k).is_err()
        {
            return Err(invalid(format!("geometry {self:?}")));
        }
        Ok(())
    }
}

/// The compression ratio of a layer: 0 (window only) for layers 0 and 1, 2
/// for layers 2–19, and 1 for layers 20–39. `None` past layer 39.
#[must_use]
pub const fn v41_compress_ratio(layer: usize) -> Option<usize> {
    match layer {
        0..=1 => Some(0),
        2..=19 => Some(2),
        20..=39 => Some(1),
        _ => None,
    }
}

/// Which layer owns a layer's compressed KV and index keys, its selection,
/// and its candidate blocks, as `shared_state::LayerOwners`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V41LayerOwners {
    pub kv: Option<usize>,
    pub index: Option<usize>,
    pub candidates: Option<usize>,
}

impl V41LayerOwners {
    /// The owners of `layer`, or `None` past layer 39.
    #[must_use]
    pub fn for_layer(layer: usize) -> Option<Self> {
        v41_compress_ratio(layer)?;
        let last = |sources: &[usize]| sources.iter().copied().filter(|s| *s <= layer).last();
        Some(Self {
            kv: last(&V41_KV_SOURCES),
            index: last(&V41_INDEX_SOURCES),
            candidates: (layer >= 20).then_some(20),
        })
    }
}

/// Bytes of one window-ring row of `width` values: BF16 words, or packed
/// FP8 codes and a power-of-two scale per 32 (#12471).
#[must_use]
pub const fn v41_ring_row_bytes(width: usize, packed: bool) -> usize {
    if packed {
        width + width / 32
    } else {
        width * 2
    }
}

/// Bytes of one compressed KV row: BF16 words, or packed FP4 codes and an
/// E4M3 scale per 16 (#12471).
#[must_use]
pub const fn v41_kv_row_bytes(head_dim: usize, packed: bool) -> usize {
    if packed {
        head_dim / 2 + head_dim / 16
    } else {
        head_dim * 2
    }
}

/// Bytes of one index key: BF16 words, or packed FP4 codes and a
/// power-of-two scale per 32 (#12471).
#[must_use]
pub const fn v41_key_row_bytes(index_dim: usize, packed: bool) -> usize {
    if packed {
        index_dim / 2 + index_dim / 32
    } else {
        index_dim * 2
    }
}

/// Device positions, one `i32` per token. A call with several tokens needs
/// consecutive positions.
#[derive(Clone, Copy, Debug)]
pub struct V41Steps<'a> {
    pub positions: &'a CudaBuffer,
    pub tokens: usize,
}

impl V41Steps<'_> {
    fn pointer(self) -> Result<*mut c_void, RuntimeError> {
        if self.tokens == 0 || self.tokens > 65_535 {
            return Err(invalid(format!("{} tokens", self.tokens)));
        }
        span(self.positions, 0, self.tokens, 4, "positions")
    }
}

/// A row-major buffer of `stride` elements per token.
#[derive(Clone, Copy, Debug)]
pub struct V41Rows<'a> {
    pub buffer: &'a CudaBuffer,
    pub stride: usize,
}

/// One layer's window ring: BF16 words `[rows, head_dim]`. Position `p` lives
/// in row `p % rows`. Decode needs `rows >= window`; a call with `n` tokens
/// needs `rows >= window + n - 1`.
#[derive(Clone, Copy, Debug)]
pub struct V41Ring<'a> {
    pub buffer: &'a CudaBuffer,
    pub rows: usize,
}

/// A KV source layer's shared state: compressed KV rows (BF16 words
/// `[capacity, head_dim]`) and index keys (BF16 words
/// `[capacity, index_dim]`). Row `i` holds compression group `i`.
#[derive(Clone, Copy, Debug)]
pub struct V41Source<'a> {
    pub kv: &'a CudaBuffer,
    pub index_keys: &'a CudaBuffer,
    pub capacity: usize,
}

// ---------------------------------------------------------------------------
// RoPE and YaRN
// ---------------------------------------------------------------------------

/// RoPE and YaRN constants, as `deepseek_v41_cpu::rope::RopeConfig`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41RopeConfig {
    pub dim: usize,
    pub base: f32,
    /// 0 turns YaRN off.
    pub original_context: usize,
    pub factor: f32,
    pub beta_fast: f32,
    pub beta_slow: f32,
}

impl V41RopeConfig {
    /// The window-only layers: base 10,000, no YaRN.
    #[must_use]
    pub const fn window(dim: usize) -> Self {
        Self {
            dim,
            base: 10_000.0,
            original_context: 0,
            factor: 16.0,
            beta_fast: 32.0,
            beta_slow: 1.0,
        }
    }

    /// The compressed layers: base 160,000 with YaRN (factor 16, original
    /// context 65,536, betas 32 and 1).
    #[must_use]
    pub const fn compressed(dim: usize) -> Self {
        Self {
            base: 160_000.0,
            original_context: 65_536,
            ..Self::window(dim)
        }
    }

    /// The configuration a layer's attention uses.
    #[must_use]
    pub fn for_layer(layer: usize, dim: usize) -> Option<Self> {
        Some(if v41_compress_ratio(layer)? == 0 {
            Self::window(dim)
        } else {
            Self::compressed(dim)
        })
    }
}

/// The `dim / 2` pair frequencies, computed exactly as `Rope::new` computes
/// them. Upload them once per configuration for [`CudaSubmission::v41_rope`].
///
/// # Errors
///
/// Refuses the configurations `Rope::new` refuses.
pub fn v41_rope_frequencies(c: V41RopeConfig) -> Result<Vec<f32>, RuntimeError> {
    let positive = |x: f32| x.is_finite() && x > 0.0;
    if c.dim == 0
        || !c.dim.is_multiple_of(2)
        || !c.base.is_finite()
        || c.base <= 1.0
        || !positive(c.factor)
        || !positive(c.beta_slow)
        || !positive(c.beta_fast)
        || c.beta_fast < c.beta_slow
    {
        return Err(invalid(format!("RoPE configuration {c:?}")));
    }
    let corrected_dim = |rotations: f32| {
        c.dim as f64
            * (c.original_context as f64 / (rotations as f64 * 2.0 * std::f64::consts::PI)).ln()
            / (2.0 * (c.base as f64).ln())
    };
    let (low, high) = if c.original_context > 0 {
        (
            corrected_dim(c.beta_fast).floor().max(0.0) as f32,
            corrected_dim(c.beta_slow).ceil().min((c.dim - 1) as f64) as f32,
        )
    } else {
        (0.0, 0.0)
    };
    let mut frequencies = Vec::with_capacity(c.dim / 2);
    for i in 0..c.dim / 2 {
        let exponent = (2 * i) as f32 / c.dim as f32;
        let mut f = ((c.base as f64).powf(exponent as f64) as f32).recip();
        if c.original_context > 0 {
            let ramp = ((i as f32 - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
            let smooth = 1.0 - ramp;
            f = f / c.factor * (1.0 - smooth) + f * smooth;
        }
        frequencies.push(f);
    }
    if frequencies.iter().any(|f| !f.is_finite()) {
        return Err(invalid(String::from("nonfinite RoPE frequency")));
    }
    Ok(frequencies)
}

/// Uploaded pair frequencies: `dim / 2` floats from [`v41_rope_frequencies`].
#[derive(Clone, Copy, Debug)]
pub struct V41RopeTable<'a> {
    pub frequencies: &'a CudaBuffer,
    pub dim: usize,
}

/// The cache quantization forms, as `quantization::Quantization`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V41CacheQuant {
    /// FP8 E4M3 with one UE8M0 scale per 32: the window KV and the attention
    /// output before `wo_a`.
    Fp8,
    /// FP4 E2M1 with one UE8M0 scale per 32: index keys and queries.
    IndexFp4,
    /// FP4 E2M1 with one E4M3 scale per 16: the compressed KV.
    CompressedFp4,
}

impl V41CacheQuant {
    const fn code(self) -> i32 {
        match self {
            Self::Fp8 => 0,
            Self::IndexFp4 => 1,
            Self::CompressedFp4 => 2,
        }
    }

    /// Values per scale.
    #[must_use]
    pub const fn block(self) -> usize {
        match self {
            Self::CompressedFp4 => 16,
            _ => 32,
        }
    }
}

/// A dense weight's storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V41DenseFormat {
    /// float32 `[rows, cols]` (the compressor weights `convert.py` promotes).
    F32,
    /// BF16 words `[rows, cols]`.
    Bf16,
    /// FP8 E4M3 `[rows, cols]` with UE8M0 scales per 32×32 block
    /// (`[rows / 32, cols / 32]`), dequantized and rounded to BF16 as the CPU
    /// path's grouped `wo_a` product reads them.
    Fp8Block,
}

/// A dense matrix for [`CudaSubmission::v41_dense`]. With `groups > 1` it's
/// block-diagonal: row `r` reads input slice `r / (rows / groups)` of `cols`
/// values, as `MatrixRows::multiply_grouped`.
#[derive(Clone, Copy, Debug)]
pub struct V41Dense<'a> {
    pub weights: &'a CudaBuffer,
    /// The FP8 block scales; `None` for the other formats.
    pub scales: Option<&'a CudaBuffer>,
    pub format: V41DenseFormat,
    pub rows: usize,
    pub cols: usize,
    pub groups: usize,
}

/// A compressor's shape, as `attention::CompressionState`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41Compressor {
    /// 2 for layers 2–19, 1 for layers 20–39.
    pub ratio: usize,
    pub width: usize,
    pub epsilon: f32,
}

impl V41Compressor {
    /// Floats of state the compressor carries between calls: one value row
    /// and one gate row per slot of the group.
    #[must_use]
    pub const fn state_floats(self) -> usize {
        2 * self.ratio * self.width
    }

    fn check(self) -> Result<(), RuntimeError> {
        if !(1..=V41_MAX_RATIO).contains(&self.ratio)
            || self.width == 0
            || i32::try_from(self.state_floats()).is_err()
            || !(self.epsilon.is_finite() && self.epsilon > 0.0)
        {
            return Err(invalid(format!("compressor {self:?}")));
        }
        Ok(())
    }
}

/// The source layer's indexer key weights: `indexer.wk` (BF16 words
/// `[index_dim, head_dim]`) and `indexer.k_norm` (float32 `[index_dim]`).
#[derive(Clone, Copy, Debug)]
pub struct V41IndexKeyWeights<'a> {
    pub wk: &'a CudaBuffer,
    pub k_norm: &'a CudaBuffer,
}

/// Layer 20's candidate output for [`CudaSubmission::v41_candidate_blocks`]:
/// each block's maximum score (scratch, float32), a keep flag per block (one
/// byte), and the kept block ids in ascending order (`i32`, `top_blocks` per
/// token, -1 after the kept ones). All three have `stride` elements per
/// token, at least the most blocks a token can see.
#[derive(Clone, Copy, Debug)]
pub struct V41Candidates<'a> {
    pub block_scores: &'a CudaBuffer,
    pub flags: &'a CudaBuffer,
    pub blocks: &'a CudaBuffer,
    pub stride: usize,
    pub blocks_stride: usize,
}

/// Selected rows for [`CudaSubmission::v41_sparse_attention`]: the source's
/// compressed KV and each token's selected indices (`index_top_k` per token).
#[derive(Clone, Copy, Debug)]
pub struct V41Compressed<'a> {
    pub kv: &'a CudaBuffer,
    pub capacity: usize,
    pub indices: V41Rows<'a>,
    pub ratio: usize,
}

impl CudaSubmission {
    /// Rotates the last `rope.dim` values of every head of `values`
    /// (`tokens × heads × head_width` floats, in place) by adjacent pairs, as
    /// `Rope::apply_tail`, at position `positions[t] - back`: `back` is 0, or
    /// `ratio - 1` to rotate a completed group at its first token.
    /// `inverse` rotates backwards (the attention output). `V41Output::Bf16`
    /// rounds every value of each head afterwards, as the CPU's `finish`.
    ///
    /// A position outside 0 to 2^20 - 1 writes NaN to that token's heads.
    ///
    /// # Errors
    ///
    /// Refuses bad shapes and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_rope(
        &mut self,
        rope: V41RopeTable<'_>,
        values: &CudaBuffer,
        steps: V41Steps<'_>,
        heads: usize,
        head_width: usize,
        back: usize,
        inverse: bool,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        if rope.dim == 0 || !rope.dim.is_multiple_of(2) || heads == 0 || head_width < rope.dim {
            return Err(invalid(format!(
                "RoPE of {} over {heads} heads of {head_width}",
                rope.dim
            )));
        }
        let positions = steps.pointer()?;
        let values = span(
            values,
            0,
            steps.tokens * heads * head_width,
            4,
            "RoPE values",
        )?;
        let frequencies = span(rope.frequencies, 0, rope.dim / 2, 4, "RoPE frequencies")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_rope(
                values,
                frequencies,
                positions,
                int(steps.tokens, "tokens")?,
                int(heads, "heads")?,
                int(head_width, "head width")?,
                int(rope.dim, "RoPE dim")?,
                int(back, "back")?,
                i32::from(inverse),
                rounds(output),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 RoPE")
    }

    /// Quantizes and dequantizes `rows × width` floats in the cache form
    /// `kind`, as `quantization::inplace_bf16`: round to BF16, quantize per
    /// block, dequantize, round to BF16. `out` may be `input`. A nonfinite
    /// block or a scale the CPU path refuses writes NaN for the block.
    ///
    /// # Errors
    ///
    /// Refuses a width that isn't a multiple of the block, and buffers too
    /// small for the shape.
    pub fn v41_cache_quant(
        &mut self,
        kind: V41CacheQuant,
        input: &CudaBuffer,
        out: &CudaBuffer,
        rows: usize,
        width: usize,
    ) -> Result<(), RuntimeError> {
        if rows == 0 || width == 0 || !width.is_multiple_of(kind.block()) {
            return Err(invalid(format!("{kind:?} quantization of {rows}×{width}")));
        }
        let input = span(input, 0, rows * width, 4, "quantization input")?;
        let out = span(out, 0, rows * width, 4, "quantization output")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_cache_quant(
                input,
                out,
                kind.code(),
                int(rows, "rows")?,
                int(width, "width")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 cache quantization")
    }

    /// Writes each token's rotated window KV (`tokens × head_dim` floats) in
    /// the per-32 FP8 cache form into `ring` row `positions[t] % ring.rows`.
    ///
    /// # Errors
    ///
    /// Refuses a ring shorter than `window + tokens - 1` rows and buffers too
    /// small for the shapes.
    pub fn v41_window_store(
        &mut self,
        geometry: V41AttentionGeometry,
        input: &CudaBuffer,
        steps: V41Steps<'_>,
        ring: V41Ring<'_>,
    ) -> Result<(), RuntimeError> {
        geometry.check()?;
        if ring.rows < geometry.window + steps.tokens.saturating_sub(1) {
            return Err(invalid(format!(
                "a ring of {} rows for {} tokens",
                ring.rows, steps.tokens
            )));
        }
        let positions = steps.pointer()?;
        let width = geometry.head_dim;
        let input = span(input, 0, steps.tokens * width, 4, "window KV")?;
        let ring_ptr = span(
            ring.buffer,
            0,
            ring.rows * v41_ring_row_bytes(width, self.v41_packed_kv),
            1,
            "window ring",
        )?;
        // SAFETY: every extent was checked against its buffer above; the
        // device writes only rows below `ring.rows`.
        let code = unsafe {
            psionic_v41_window_store(
                input,
                ring_ptr,
                positions,
                int(steps.tokens, "tokens")?,
                int(ring.rows, "ring rows")?,
                int(width, "width")?,
                i32::from(self.v41_packed_kv),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 window store")
    }

    /// Multiplies `tokens` input rows by a dense matrix, as the CPU path's
    /// row products: each output is `0 + sum_k w[r, k] x[k]`, summed
    /// sequentially. `round_input` rounds inputs to BF16 first (the CPU's
    /// `linear` on a BF16 matrix); `V41Output::Bf16` rounds the outputs.
    /// Input rows hold `groups × cols` values; outputs `rows`.
    ///
    /// # Errors
    ///
    /// Refuses bad shapes and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_dense(
        &mut self,
        matrix: V41Dense<'_>,
        input: &CudaBuffer,
        out: &CudaBuffer,
        tokens: usize,
        round_input: bool,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        let m = matrix;
        if m.rows == 0
            || m.cols == 0
            || m.groups == 0
            || !m.rows.is_multiple_of(m.groups)
            || tokens == 0
            || tokens > 65_535
            || i32::try_from(m.rows * m.cols).is_err()
            || (m.format == V41DenseFormat::Fp8Block
                && (!m.rows.is_multiple_of(32) || !m.cols.is_multiple_of(32)))
        {
            return Err(invalid(format!(
                "dense {:?} {}×{} in {} groups for {tokens} tokens",
                m.format, m.rows, m.cols, m.groups
            )));
        }
        let element = match m.format {
            V41DenseFormat::F32 => 4,
            V41DenseFormat::Bf16 => 2,
            V41DenseFormat::Fp8Block => 1,
        };
        let weights = span(m.weights, 0, m.rows * m.cols, element, "dense weights")?;
        let scales = match (m.format, m.scales) {
            (V41DenseFormat::Fp8Block, Some(scales)) => {
                span(scales, 0, (m.rows / 32) * (m.cols / 32), 1, "dense scales")?
            }
            (V41DenseFormat::Fp8Block, None) => {
                return Err(invalid(String::from("FP8 dense weights need scales")));
            }
            _ => std::ptr::null_mut(),
        };
        let input = span(input, 0, tokens * m.groups * m.cols, 4, "dense input")?;
        let out = span(out, 0, tokens * m.rows, 4, "dense output")?;
        let format = match m.format {
            V41DenseFormat::F32 => 0,
            V41DenseFormat::Bf16 => 1,
            V41DenseFormat::Fp8Block => 2,
        };
        // SAFETY: every extent was checked against its buffer above.
        let launch = if self.v41_fast_reductions {
            psionic_v41_dense_fast
        } else {
            psionic_v41_dense
        };
        let code = unsafe {
            launch(
                weights,
                scales,
                input,
                out,
                format,
                int(m.rows, "rows")?,
                int(m.cols, "cols")?,
                int(m.groups, "groups")?,
                int(tokens, "tokens")?,
                i32::from(round_input),
                rounds(output),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 dense matrix-vector product")
    }

    /// RMS-normalizes `tokens` rows of `width` floats with the BF16-rounded
    /// `weight`, as the CPU's `rms_norm` (a sequential sum of squares).
    ///
    /// # Errors
    ///
    /// Refuses bad shapes, a bad epsilon, and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_rms_norm(
        &mut self,
        input: &CudaBuffer,
        weight: &CudaBuffer,
        out: &CudaBuffer,
        tokens: usize,
        width: usize,
        epsilon: f32,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        if tokens == 0
            || tokens > 65_535
            || width == 0
            || width > 8192
            || !(epsilon.is_finite() && epsilon > 0.0)
        {
            return Err(invalid(format!("RMS norm of {tokens}×{width}")));
        }
        let input = span(input, 0, tokens * width, 4, "norm input")?;
        let weight = span(weight, 0, width, 4, "norm weight")?;
        let out = span(out, 0, tokens * width, 4, "norm output")?;
        // SAFETY: every extent was checked against its buffer above.
        let launch = if self.v41_fast_reductions {
            psionic_v41_rms_norm_fast
        } else {
            psionic_v41_rms_norm
        };
        let code = unsafe {
            launch(
                input,
                weight,
                out,
                int(tokens, "tokens")?,
                int(width, "width")?,
                epsilon,
                rounds(output),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 RMS norm")
    }

    /// Pushes `tokens` projected compressor values (and gates, for ratios
    /// above 1) through the compressor, as `CompressionState::push_chunk` in
    /// `ReferenceBf16` arithmetic. For each token that completes a group,
    /// writes its latent to the token's row of `latents`: the gated softmax
    /// pool, rounded to BF16, then the RMS norm with the BF16 norm weight,
    /// rounded. Other rows are left as they were. The chunk's trailing
    /// incomplete group goes into `state` for the next call; a reset is a
    /// fresh position 0, since nothing reads state a group doesn't need.
    /// Two launches.
    ///
    /// # Errors
    ///
    /// Refuses a bad compressor, gates on ratio 1 or none above it, and
    /// buffers too small.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_compress(
        &mut self,
        compressor: V41Compressor,
        values: &CudaBuffer,
        gates: Option<&CudaBuffer>,
        norm: &CudaBuffer,
        state: &CudaBuffer,
        steps: V41Steps<'_>,
        latents: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let c = compressor;
        c.check()?;
        if (c.ratio == 1) != gates.is_none() {
            return Err(invalid(String::from(
                "ratio 1 takes no gates and higher ratios need them",
            )));
        }
        let positions = steps.pointer()?;
        let n = steps.tokens * c.width;
        let values = span(values, 0, n, 4, "compressor values")?;
        let gates = match gates {
            Some(gates) => span(gates, 0, n, 4, "compressor gates")?,
            None => std::ptr::null_mut(),
        };
        let norm = span(norm, 0, c.width, 4, "compressor norm")?;
        let state = span(state, 0, c.state_floats(), 4, "compressor state")?;
        let latents = span(latents, 0, n, 4, "latents")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_compress(
                values,
                gates,
                norm,
                state,
                positions,
                latents,
                int(steps.tokens, "tokens")?,
                int(c.ratio, "ratio")?,
                int(c.width, "width")?,
                c.epsilon,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 compressor")
    }

    /// For each token whose group completes, writes source row
    /// `(p + 1) / ratio - 1`: the index key from the unrotated latent
    /// (`wk`, BF16 output, the `k_norm` RMS norm, RoPE at the group's first
    /// position `p + 1 - ratio`, and the per-32 FP4/UE8M0 form) and the
    /// compressed KV (RoPE on the latent at the same position, then the
    /// per-16 FP4/E4M3 form). `rope` is the compressed configuration's table.
    /// A row past `source.capacity` isn't written.
    ///
    /// # Errors
    ///
    /// Refuses a bad geometry or ratio and buffers too small.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_compressed_store(
        &mut self,
        geometry: V41AttentionGeometry,
        ratio: usize,
        weights: V41IndexKeyWeights<'_>,
        rope: V41RopeTable<'_>,
        latents: &CudaBuffer,
        steps: V41Steps<'_>,
        source: V41Source<'_>,
    ) -> Result<(), RuntimeError> {
        let g = geometry;
        g.check()?;
        if !(1..=V41_MAX_RATIO).contains(&ratio) || rope.dim != g.rope_dim {
            return Err(invalid(format!(
                "compressed store at ratio {ratio} with RoPE of {}",
                rope.dim
            )));
        }
        let positions = steps.pointer()?;
        let latents = span(latents, 0, steps.tokens * g.head_dim, 4, "latents")?;
        let wk = span(weights.wk, 0, g.index_dim * g.head_dim, 2, "indexer wk")?;
        let k_norm = span(weights.k_norm, 0, g.index_dim, 4, "indexer k_norm")?;
        let frequencies = span(rope.frequencies, 0, rope.dim / 2, 4, "RoPE frequencies")?;
        let kv = span(
            source.kv,
            0,
            source.capacity * v41_kv_row_bytes(g.head_dim, self.v41_packed_kv),
            1,
            "compressed KV",
        )?;
        let keys = span(
            source.index_keys,
            0,
            source.capacity * v41_key_row_bytes(g.index_dim, self.v41_packed_kv),
            1,
            "index keys",
        )?;
        // SAFETY: every extent was checked against its buffer above; the
        // device writes only rows below `source.capacity`.
        let code = unsafe {
            psionic_v41_compressed_store(
                latents,
                wk,
                k_norm,
                frequencies,
                positions,
                kv,
                keys,
                int(steps.tokens, "tokens")?,
                int(ratio, "ratio")?,
                int(source.capacity, "capacity")?,
                int(g.head_dim, "head dim")?,
                int(g.index_dim, "index dim")?,
                int(g.rope_dim, "RoPE dim")?,
                g.norm_epsilon,
                i32::from(self.v41_packed_kv),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 compressed store")
    }

    /// Scores every visible key of the KV source (`(p + 1) / ratio` of them)
    /// for each token, as the CPU path's reference index scores:
    /// `sum_h max(q_h . k, 0) * bf16(w_h * scale)`. `query` holds
    /// `index_heads × index_dim` rotated, FP4-formed floats per token and
    /// `weights` the `index_heads` projected head weights. Writes
    /// `scores.buffer[t * scores.stride + i]` for visible keys only.
    ///
    /// # Errors
    ///
    /// Refuses a bad geometry or ratio and buffers too small.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_index_scores(
        &mut self,
        geometry: V41AttentionGeometry,
        query: &CudaBuffer,
        weights: &CudaBuffer,
        source: V41Source<'_>,
        ratio: usize,
        steps: V41Steps<'_>,
        scores: V41Rows<'_>,
    ) -> Result<(), RuntimeError> {
        let g = geometry;
        g.check()?;
        if !(1..=V41_MAX_RATIO).contains(&ratio) || scores.stride < source.capacity {
            return Err(invalid(format!(
                "index scores at ratio {ratio} with stride {} for {} keys",
                scores.stride, source.capacity
            )));
        }
        let positions = steps.pointer()?;
        let width = g.index_heads * g.index_dim;
        let query = span(query, 0, steps.tokens * width, 4, "index query")?;
        let weights = span(weights, 0, steps.tokens * g.index_heads, 4, "index weights")?;
        let keys = span(
            source.index_keys,
            0,
            source.capacity * v41_key_row_bytes(g.index_dim, self.v41_packed_kv),
            1,
            "index keys",
        )?;
        let out = span(
            scores.buffer,
            0,
            (steps.tokens - 1) * scores.stride + source.capacity,
            4,
            "index scores",
        )?;
        // SAFETY: every extent was checked against its buffer above; the
        // device reads keys and writes scores only below `source.capacity`
        // (visible keys past it are skipped by the launch's grid).
        let code = unsafe {
            psionic_v41_index_scores(
                query,
                weights,
                keys,
                positions,
                out,
                stride(scores.stride, "score stride")?,
                int(steps.tokens, "tokens")?,
                int(source.capacity, "capacity")?,
                int(ratio, "ratio")?,
                int(g.index_heads, "index heads")?,
                int(g.index_dim, "index dim")?,
                g.index_scale(),
                i32::from(self.v41_packed_kv),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 index scores")
    }

    /// Layer 20's candidate blocks, as `attention::candidate_mask`: each
    /// block of `block_size` visible scores takes its maximum, the newest
    /// block is pinned in, and the `top_blocks` largest are kept (ties to the
    /// lower block). Writes the block maxima, a 0/1 flag per block, and the
    /// kept block ids in ascending order with -1 after them. A NaN or `+inf`
    /// score writes [`V41_FLAG_POISON`] flags and [`V41_SELECT_POISON`] ids.
    /// Two launches.
    ///
    /// # Errors
    ///
    /// Refuses bad sizes and buffers too small.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_candidate_blocks(
        &mut self,
        scores: V41Rows<'_>,
        ratio: usize,
        steps: V41Steps<'_>,
        candidates: V41Candidates<'_>,
        top_blocks: usize,
        block_size: usize,
    ) -> Result<(), RuntimeError> {
        if !(1..=V41_MAX_RATIO).contains(&ratio)
            || top_blocks == 0
            || block_size == 0
            || candidates.blocks_stride < top_blocks
            || candidates.stride * block_size < scores.stride
        {
            return Err(invalid(format!(
                "{top_blocks} candidate blocks of {block_size} over {} scores",
                scores.stride
            )));
        }
        let positions = steps.pointer()?;
        let t = steps.tokens;
        let score_ptr = span(scores.buffer, 0, t * scores.stride, 4, "scores")?;
        let block_scores = span(
            candidates.block_scores,
            0,
            t * candidates.stride,
            4,
            "block scores",
        )?;
        let flags = span(candidates.flags, 0, t * candidates.stride, 1, "flags")?;
        let blocks = span(
            candidates.blocks,
            0,
            t * candidates.blocks_stride,
            4,
            "blocks",
        )?;
        let stream = self.platform.raw_stream()?;
        // SAFETY: every extent was checked against its buffer above; a token
        // sees at most `scores.stride` scores and so at most
        // `candidates.stride` blocks, which the caller's positions keep.
        let code = unsafe {
            psionic_v41_block_max(
                score_ptr,
                stride(scores.stride, "score stride")?,
                block_scores,
                stride(candidates.stride, "block stride")?,
                positions,
                int(t, "tokens")?,
                int(candidates.stride, "block capacity")?,
                int(ratio, "ratio")?,
                int(block_size, "block size")?,
                stream,
            )
        };
        self.clef_launch(code, "v41 candidate block maxima")?;
        // SAFETY: as above.
        let code = unsafe {
            psionic_v41_select(
                block_scores,
                stride(candidates.stride, "block stride")?,
                std::ptr::null(),
                0,
                1,
                positions,
                int(t, "tokens")?,
                int(ratio, "ratio")?,
                int(block_size, "block size")?,
                int(top_blocks, "top blocks")?,
                1,
                blocks,
                stride(candidates.blocks_stride, "blocks stride")?,
                flags,
                stride(candidates.stride, "flag stride")?,
                stream,
            )
        };
        self.clef_launch(code, "v41 candidate selection")
    }

    /// Selects each token's top `top_k` visible scores, as
    /// `attention::select_indices`: finite scores (and, with `candidates`,
    /// only positions in kept blocks), score descending then the lower
    /// position, written in ascending order with -1 after them. A token keeps
    /// `min(top_k, visible)` slots, the slots its attention reads; the rest
    /// of the row holds -1. A NaN or `+inf` score or a poisoned flag writes
    /// [`V41_SELECT_POISON`] in every slot. A position whose visible count
    /// exceeds `scores.stride` sees only the first `scores.stride` scores.
    ///
    /// # Errors
    ///
    /// Refuses bad sizes and buffers too small.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_select_indices(
        &mut self,
        scores: V41Rows<'_>,
        ratio: usize,
        steps: V41Steps<'_>,
        candidates: Option<(V41Rows<'_>, usize)>,
        top_k: usize,
        out: V41Rows<'_>,
    ) -> Result<(), RuntimeError> {
        if !(1..=V41_MAX_RATIO).contains(&ratio) || top_k == 0 || out.stride < top_k {
            return Err(invalid(format!(
                "top {top_k} at ratio {ratio} into rows of {}",
                out.stride
            )));
        }
        let positions = steps.pointer()?;
        let t = steps.tokens;
        let score_ptr = span(scores.buffer, 0, t * scores.stride, 4, "scores")?;
        let (flags, flag_stride, flag_group) = match candidates {
            Some((flags, group)) => {
                if group == 0 || flags.stride * group < scores.stride {
                    return Err(invalid(format!(
                        "candidate flags of {} blocks of {group}",
                        flags.stride
                    )));
                }
                (
                    span(flags.buffer, 0, t * flags.stride, 1, "candidate flags")?.cast_const(),
                    flags.stride,
                    group,
                )
            }
            None => (std::ptr::null(), 0, 1),
        };
        let out_ptr = span(out.buffer, 0, t * out.stride, 4, "selected indices")?;
        // SAFETY: every extent was checked against its buffer above; a token
        // sees at most `scores.stride` scores, which the caller's positions
        // keep.
        let code = unsafe {
            psionic_v41_select(
                score_ptr,
                stride(scores.stride, "score stride")?,
                flags,
                stride(flag_stride, "flag stride")?,
                int(flag_group, "flag group")?,
                positions,
                int(t, "tokens")?,
                int(ratio, "ratio")?,
                1,
                int(top_k, "top k")?,
                0,
                out_ptr,
                stride(out.stride, "index stride")?,
                std::ptr::null_mut(),
                0,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 index selection")
    }

    /// Latent sparse attention for each token and head, as
    /// `quantization::sparse_attention_bf16` over the CPU runtime's slots:
    /// the `min(p + 1, window)` newest window rows oldest first, then, with
    /// `compressed`, the token's `min(top_k, (p + 1) / ratio)` selected rows
    /// (-1 slots are masked but keep their place in the 64-slot blocks). K
    /// and V are the same BF16 row. Each block updates the running maximum,
    /// rescales, and adds `bf16(exp(s - max)) × row`; the sink adds
    /// `exp(sink - max)` to the denominator only. The output (`tokens ×
    /// heads × head_dim` floats) is rounded to BF16.
    ///
    /// An index below -1, at or past the visible rows, or at or past
    /// `compressed.capacity` writes NaN for every head and is never read; a
    /// nonfinite score writes NaN for its head, and a negative position for
    /// all of them.
    ///
    /// # Errors
    ///
    /// Refuses a bad geometry, a ring shorter than the window, and buffers
    /// too small.
    pub fn v41_sparse_attention(
        &mut self,
        geometry: V41AttentionGeometry,
        query: &CudaBuffer,
        sinks: &CudaBuffer,
        ring: V41Ring<'_>,
        compressed: Option<V41Compressed<'_>>,
        steps: V41Steps<'_>,
        out: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let g = geometry;
        g.check()?;
        if ring.rows < g.window {
            return Err(invalid(format!("a ring of {} rows", ring.rows)));
        }
        let positions = steps.pointer()?;
        let t = steps.tokens;
        let width = g.heads * g.head_dim;
        let query = span(query, 0, t * width, 4, "query")?;
        let sinks = span(sinks, 0, g.heads, 4, "sinks")?;
        let ring_ptr = span(
            ring.buffer,
            0,
            ring.rows * v41_ring_row_bytes(g.head_dim, self.v41_packed_kv),
            1,
            "window ring",
        )?;
        let out = span(out, 0, t * width, 4, "attention output")?;
        let (kv, capacity, indices, index_stride, ratio) = match compressed {
            Some(c) => {
                if !(1..=V41_MAX_RATIO).contains(&c.ratio) || c.indices.stride < g.index_top_k {
                    return Err(invalid(format!(
                        "compressed rows at ratio {} with index rows of {}",
                        c.ratio, c.indices.stride
                    )));
                }
                (
                    span(
                        c.kv,
                        0,
                        c.capacity * v41_kv_row_bytes(g.head_dim, self.v41_packed_kv),
                        1,
                        "compressed KV",
                    )?
                    .cast_const(),
                    c.capacity,
                    span(c.indices.buffer, 0, t * c.indices.stride, 4, "indices")?.cast_const(),
                    c.indices.stride,
                    c.ratio,
                )
            }
            None => (std::ptr::null(), 0, std::ptr::null(), 0, 0),
        };
        // SAFETY: every extent was checked against its buffer above. The
        // device reads compressed rows only at selected ids below both the
        // visible count and `capacity`; any other id poisons the output
        // instead of being read.
        let code = unsafe {
            psionic_v41_sparse_attention(
                query,
                sinks,
                ring_ptr,
                int(ring.rows, "ring rows")?,
                kv,
                int(capacity, "capacity")?,
                indices,
                stride(index_stride, "index stride")?,
                positions,
                out,
                int(t, "tokens")?,
                int(g.heads, "heads")?,
                int(g.head_dim, "head dim")?,
                int(g.window, "window")?,
                int(ratio, "ratio")?,
                int(g.index_top_k, "top k")?,
                g.attention_scale(),
                i32::from(self.v41_packed_kv),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 sparse attention")
    }
}

// ---------------------------------------------------------------------------
// Host models
// ---------------------------------------------------------------------------

fn bf16(x: f32) -> f32 {
    half::bf16::from_f32(x).to_f32()
}

/// Decodes one BF16 word.
#[must_use]
pub fn v41_bf16_value(bits: u16) -> f32 {
    f32::from_bits(u32::from(bits) << 16)
}

/// Rounds to BF16 and returns the word, as the caches store it.
#[must_use]
pub fn v41_bf16_bits(value: f32) -> u16 {
    half::bf16::from_f32(value).to_bits()
}

fn exp_once(x: f32) -> f32 {
    f64::from(x).exp() as f32
}

fn e4m3(byte: u32) -> f32 {
    let magnitude = byte & 0x7f;
    let value = if magnitude == 0x7f {
        f32::NAN
    } else {
        f32::from_bits(magnitude << 20) * f32::from_bits(0x7b80_0000)
    };
    f32::from_bits(value.to_bits() | ((byte & 0x80) << 24))
}

const E2M1: [f32; 8] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

fn e2m1(code: u32) -> f32 {
    let v = E2M1[(code & 7) as usize];
    if code & 8 != 0 { -v } else { v }
}

fn e8m0(byte: u32) -> f32 {
    match byte {
        0xff => f32::NAN,
        0 => f32::from_bits(0x0040_0000),
        b => f32::from_bits(b << 23),
    }
}

fn nearest(x: f32, values: impl Fn(usize) -> f32, len: usize) -> u32 {
    let mut lo = 0;
    let mut hi = len;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if values(mid) < x {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let upper = lo.min(len - 1);
    if upper == 0 {
        return 0;
    }
    let lower = upper - 1;
    let a = x - values(lower);
    let b = values(upper) - x;
    if a < b || (a == b && lower % 2 == 0) {
        lower as u32
    } else {
        upper as u32
    }
}

fn encode_fp8(value: f32) -> u32 {
    let sign = if value.is_sign_negative() { 0x80 } else { 0 };
    nearest(value.abs().min(448.0), |i| e4m3(i as u32), 127) | sign
}

fn encode_fp4(value: f32) -> u32 {
    let sign = if value.is_sign_negative() { 8 } else { 0 };
    nearest(value.abs().min(6.0), |i| E2M1[i], 8) | sign
}

fn ceil_power(value: f32) -> f32 {
    if !value.is_finite() || value < f32::MIN_POSITIVE {
        return f32::NAN;
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 255) + u32::from(bits & 0x7f_ffff != 0);
    if exponent >= 255 {
        return f32::NAN;
    }
    f32::from_bits(exponent << 23)
}

/// One block of the device's cache quantizer (`kind.block()` values).
#[must_use]
pub fn v41_quant_block_host(kind: V41CacheQuant, input: &[f32]) -> Vec<f32> {
    let x: Vec<f32> = input.iter().map(|v| bf16(*v)).collect();
    let finite = x.iter().all(|v| v.is_finite());
    let amax = x.iter().fold(0.0_f32, |a, v| a.max(v.abs()));
    let scale = match kind {
        V41CacheQuant::Fp8 => ceil_power(amax.max(1e-4) * (1.0 / 448.0)),
        V41CacheQuant::IndexFp4 => ceil_power(amax.max(6.0 * f32::MIN_POSITIVE) * (1.0 / 6.0)),
        V41CacheQuant::CompressedFp4 => {
            let value = amax.max(6.0 * 2.0_f32.powi(-9)) / 6.0;
            if value > 448.0 {
                f32::NAN
            } else {
                e4m3(encode_fp8(value))
            }
        }
    };
    if !finite || scale.is_nan() {
        return vec![f32::NAN; x.len()];
    }
    let out: Vec<f32> = x
        .iter()
        .map(|v| {
            let q = v / scale;
            let decoded = if kind == V41CacheQuant::Fp8 {
                e4m3(encode_fp8(q))
            } else {
                e2m1(encode_fp4(q))
            };
            bf16(decoded * scale)
        })
        .collect();
    if out.iter().all(|v| v.is_finite()) {
        out
    } else {
        vec![f32::NAN; out.len()]
    }
}

/// [`CudaSubmission::v41_cache_quant`] on the host.
///
/// # Panics
///
/// Panics when the length isn't a multiple of the block.
#[must_use]
pub fn v41_cache_quant_host(kind: V41CacheQuant, input: &[f32]) -> Vec<f32> {
    assert!(input.len().is_multiple_of(kind.block()));
    input
        .chunks_exact(kind.block())
        .flat_map(|b| v41_quant_block_host(kind, b))
        .collect()
}

/// One head's tail rotation as the device does it, in place.
fn rope_head_host(
    head: &mut [f32],
    frequencies: &[f32],
    position: i64,
    inverse: bool,
    round: bool,
) {
    let dim = frequencies.len() * 2;
    let prefix = head.len() - dim;
    let valid = (0..V41_MAX_POSITION as i64).contains(&position);
    for v in &mut head[..prefix] {
        let x = if valid { *v } else { f32::NAN };
        *v = if round { bf16(x) } else { x };
    }
    for (pair, f) in head[prefix..].chunks_exact_mut(2).zip(frequencies) {
        let (a, b) = (pair[0], pair[1]);
        let (first, second) = if valid {
            let angle = position as f32 * f;
            let c = f64::from(angle).cos() as f32;
            let s = f64::from(angle).sin() as f32;
            let s = if inverse { -s } else { s };
            (a * c - b * s, a * s + b * c)
        } else {
            (f32::NAN, f32::NAN)
        };
        pair[0] = if round { bf16(first) } else { first };
        pair[1] = if round { bf16(second) } else { second };
    }
}

/// [`CudaSubmission::v41_rope`] on the host for one token: `values` holds
/// `heads × head_width` floats; `position` is already `p - back`.
///
/// # Panics
///
/// Panics when the shape doesn't divide.
pub fn v41_rope_host(
    frequencies: &[f32],
    values: &mut [f32],
    head_width: usize,
    position: i64,
    inverse: bool,
    output: V41Output,
) {
    assert!(values.len().is_multiple_of(head_width));
    for head in values.chunks_exact_mut(head_width) {
        rope_head_host(
            head,
            frequencies,
            position,
            inverse,
            output == V41Output::Bf16,
        );
    }
}

/// Dense weights on the host, in the device's storage formats.
#[derive(Clone, Copy, Debug)]
pub enum V41DenseHost<'a> {
    F32(&'a [f32]),
    Bf16(&'a [u16]),
    Fp8Block { codes: &'a [u8], scales: &'a [u8] },
}

impl V41DenseHost<'_> {
    fn weight(self, row: usize, col: usize, cols: usize) -> f32 {
        match self {
            Self::F32(w) => w[row * cols + col],
            Self::Bf16(w) => v41_bf16_value(w[row * cols + col]),
            Self::Fp8Block { codes, scales } => bf16(
                e4m3(u32::from(codes[row * cols + col]))
                    * e8m0(u32::from(scales[(row / 32) * (cols / 32) + col / 32])),
            ),
        }
    }
}

/// [`CudaSubmission::v41_dense`] on the host for one token.
#[must_use]
pub fn v41_dense_host(
    weights: V41DenseHost<'_>,
    input: &[f32],
    rows: usize,
    cols: usize,
    groups: usize,
    round_input: bool,
    output: V41Output,
) -> Vec<f32> {
    (0..rows)
        .map(|r| {
            let group = r / (rows / groups);
            let mut sum = -0.0_f32;
            for k in 0..cols {
                let x = input[group * cols + k];
                let x = if round_input { bf16(x) } else { x };
                sum += weights.weight(r, k, cols) * x;
            }
            let total = 0.0 + sum;
            if output == V41Output::Bf16 {
                bf16(total)
            } else {
                total
            }
        })
        .collect()
}

/// [`CudaSubmission::v41_rms_norm`] on the host for one row.
#[must_use]
pub fn v41_rms_norm_host(
    input: &[f32],
    weight: &[f32],
    epsilon: f32,
    output: V41Output,
) -> Vec<f32> {
    let mut sum = -0.0_f32;
    for x in input {
        sum += x * x;
    }
    let variance = sum / input.len() as f32;
    let scale = if variance.is_finite() {
        1.0 / (variance + epsilon).sqrt()
    } else {
        f32::NAN
    };
    input
        .iter()
        .zip(weight)
        .map(|(x, w)| {
            let v = bf16(*w) * (x * scale);
            if output == V41Output::Bf16 {
                bf16(v)
            } else {
                v
            }
        })
        .collect()
}

/// The device compressor on the host, with its state.
#[derive(Clone, Debug)]
pub struct V41CompressorHost {
    pub compressor: V41Compressor,
    pub state: Vec<f32>,
}

impl V41CompressorHost {
    /// A compressor whose state holds `fill` (the device buffer's initial
    /// contents).
    #[must_use]
    pub fn new(compressor: V41Compressor, fill: f32) -> Self {
        Self {
            compressor,
            state: vec![fill; compressor.state_floats()],
        }
    }

    /// [`CudaSubmission::v41_compress`] on the host: returns each token's
    /// latent, `None` where its group is incomplete.
    ///
    /// # Panics
    ///
    /// Panics when the shapes don't match.
    #[must_use]
    pub fn push(
        &mut self,
        values: &[f32],
        gates: Option<&[f32]>,
        norm: &[f32],
        positions: &[i32],
    ) -> Vec<Option<Vec<f32>>> {
        let c = self.compressor;
        let (ratio, width) = (c.ratio, c.width);
        let mut out = Vec::with_capacity(positions.len());
        for (t, &position) in positions.iter().enumerate() {
            if position < 0 || (position as usize + 1) % ratio != 0 {
                out.push(None);
                continue;
            }
            let p = position as usize;
            let mut pooled = vec![0.0_f32; width];
            for (d, pooled) in pooled.iter_mut().enumerate() {
                let mut mv = [0.0_f32; V41_MAX_RATIO];
                let mut mg = [0.0_f32; V41_MAX_RATIO];
                for r in 0..ratio {
                    let source = t as i64 - (ratio as i64 - 1) + r as i64;
                    if source >= 0 {
                        let s = source as usize;
                        mv[r] = values[s * width + d];
                        mg[r] = if ratio > 1 {
                            gates.map_or(0.0, |g| g[s * width + d])
                        } else {
                            0.0
                        };
                    } else {
                        let slot = (p + 1 - ratio + r) % ratio;
                        mv[r] = self.state[2 * slot * width + d];
                        mg[r] = self.state[(2 * slot + 1) * width + d];
                    }
                }
                let value = if ratio == 1 {
                    mv[0]
                } else {
                    let max = mg[..ratio].iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));
                    let mut denominator = -0.0_f32;
                    for g in &mg[..ratio] {
                        denominator += exp_once(g - max);
                    }
                    let mut value = -0.0_f32;
                    for r in 0..ratio {
                        value += mv[r] * (exp_once(mg[r] - max) / denominator);
                    }
                    value
                };
                *pooled = bf16(value);
            }
            out.push(Some(v41_rms_norm_host(
                &pooled,
                norm,
                c.epsilon,
                V41Output::Bf16,
            )));
        }
        if ratio > 1
            && let Some(&last) = positions.last()
            && last >= 0
        {
            let pending = (last as usize + 1) % ratio;
            let tokens = positions.len();
            for k in 0..pending {
                if tokens < pending - k {
                    continue;
                }
                let t = tokens - pending + k;
                let slot = positions[t] as usize % ratio;
                for d in 0..width {
                    self.state[2 * slot * width + d] = values[t * width + d];
                    self.state[(2 * slot + 1) * width + d] =
                        gates.map_or(0.0, |g| g[t * width + d]);
                }
            }
        }
        out
    }
}

/// [`CudaSubmission::v41_compressed_store`] on the host for one completed
/// group: returns `(index key, compressed KV)` as BF16-exact floats.
///
/// # Panics
///
/// Panics when the shapes don't match the geometry.
#[must_use]
pub fn v41_compressed_store_host(
    geometry: V41AttentionGeometry,
    ratio: usize,
    wk: &[u16],
    k_norm: &[f32],
    frequencies: &[f32],
    latent: &[f32],
    position: usize,
) -> (Vec<f32>, Vec<f32>) {
    let g = geometry;
    let group_position = (position + 1 - ratio) as i64;
    let key = v41_dense_host(
        V41DenseHost::Bf16(wk),
        latent,
        g.index_dim,
        g.head_dim,
        1,
        true,
        V41Output::Bf16,
    );
    let mut key = v41_rms_norm_host(&key, k_norm, g.norm_epsilon, V41Output::Bf16);
    rope_head_host(&mut key, frequencies, group_position, false, true);
    let key = v41_cache_quant_host(V41CacheQuant::IndexFp4, &key);
    let mut kv = latent.to_vec();
    rope_head_host(&mut kv, frequencies, group_position, false, true);
    let kv = v41_cache_quant_host(V41CacheQuant::CompressedFp4, &kv);
    (key, kv)
}

/// [`CudaSubmission::v41_index_scores`] on the host for one token: scores of
/// the first `visible` keys (`keys` holds BF16 words).
#[must_use]
pub fn v41_index_scores_host(
    geometry: V41AttentionGeometry,
    query: &[f32],
    weights: &[f32],
    keys: &[u16],
    visible: usize,
) -> Vec<f32> {
    let (heads, dim) = (geometry.index_heads, geometry.index_dim);
    let scale = geometry.index_scale();
    let w: Vec<f32> = weights.iter().map(|w| bf16(w * scale)).collect();
    (0..visible)
        .map(|j| {
            let key = &keys[j * dim..(j + 1) * dim];
            let mut score = 0.0_f32;
            for h in 0..heads {
                let mut dot = -0.0_f32;
                for i in 0..dim {
                    dot += query[h * dim + i] * v41_bf16_value(key[i]);
                }
                if !dot.is_finite() {
                    return f32::NAN;
                }
                score += dot.max(0.0) * w[h];
            }
            score
        })
        .collect()
}

/// The device's selection on the host for one token: the top `top_k` of
/// `values` by `total_cmp` descending then the lower index, among values
/// that aren't -inf and whose flag (if any) is set; ascending, padded with -1
/// to `top_k`. Returns all-poison where the device poisons. Only the first
/// `min(top_k, values.len())` slots are selected.
#[must_use]
pub fn v41_select_host(
    values: &[f32],
    flags: Option<(&[u8], usize)>,
    top_k: usize,
    allow_inf: bool,
) -> Vec<i32> {
    let flag = |i: usize| flags.map_or(1, |(f, group)| f[i / group]);
    let poison = values.iter().enumerate().any(|(i, v)| {
        v.is_nan() || (*v == f32::INFINITY && !allow_inf) || flag(i) == V41_FLAG_POISON
    });
    if poison {
        return vec![V41_SELECT_POISON; top_k];
    }
    let count = top_k.min(values.len());
    let mut eligible: Vec<usize> = (0..values.len())
        .filter(|i| values[*i] != f32::NEG_INFINITY && flag(*i) != 0)
        .collect();
    eligible.sort_by(|a, b| values[*b].total_cmp(&values[*a]).then_with(|| a.cmp(b)));
    eligible.truncate(count);
    eligible.sort_unstable();
    let mut out: Vec<i32> = eligible.into_iter().map(|i| i as i32).collect();
    out.resize(top_k, -1);
    out
}

/// [`CudaSubmission::v41_candidate_blocks`] on the host for one token:
/// returns `(block maxima, flags, kept blocks)`.
#[must_use]
pub fn v41_candidate_blocks_host(
    scores: &[f32],
    top_blocks: usize,
    block_size: usize,
) -> (Vec<f32>, Vec<u8>, Vec<i32>) {
    let visible = scores.len();
    let count = visible.div_ceil(block_size);
    let maxima: Vec<f32> = (0..count)
        .map(|b| {
            let mut max = f32::NEG_INFINITY;
            let mut poison = false;
            for v in &scores[b * block_size..((b + 1) * block_size).min(visible)] {
                poison |= v.is_nan() || *v == f32::INFINITY;
                max = max.max(*v);
            }
            if b == (visible - 1) / block_size {
                max = f32::INFINITY;
            }
            if poison { f32::NAN } else { max }
        })
        .collect();
    let blocks = v41_select_host(&maxima, None, top_blocks, true);
    let flags = if blocks.first() == Some(&V41_SELECT_POISON) {
        vec![V41_FLAG_POISON; count]
    } else {
        let mut flags = vec![0_u8; count];
        for b in blocks.iter().filter(|b| **b >= 0) {
            flags[*b as usize] = 1;
        }
        flags
    };
    (maxima, flags, blocks)
}

/// [`CudaSubmission::v41_sparse_attention`] on the host for one token and
/// all heads. `window_rows` are the `min(p + 1, window)` newest window rows
/// oldest first, `compressed` the source's rows, and `indices` the token's
/// `min(top_k, visible)` selected slots; all BF16-exact floats.
#[must_use]
pub fn v41_sparse_attention_host(
    geometry: V41AttentionGeometry,
    query: &[f32],
    sinks: &[f32],
    window_rows: &[f32],
    compressed: &[f32],
    indices: &[i32],
    visible: usize,
) -> Vec<f32> {
    let g = geometry;
    let d = g.head_dim;
    let n_window = window_rows.len() / d;
    let slots = n_window + indices.len();
    let row = |slot: usize| -> Option<&[f32]> {
        if slot < n_window {
            Some(&window_rows[slot * d..(slot + 1) * d])
        } else {
            let id = indices[slot - n_window];
            (id >= 0).then(|| &compressed[id as usize * d..(id as usize + 1) * d])
        }
    };
    let mut out = vec![0.0_f32; g.heads * d];
    for h in 0..g.heads {
        let q: Vec<f32> = query[h * d..(h + 1) * d].iter().map(|v| bf16(*v)).collect();
        let poison_index = indices
            .iter()
            .any(|id| *id < -1 || *id as i64 >= visible as i64);
        let mut scores = vec![0.0_f32; slots];
        let mut poison = poison_index;
        if !poison {
            for (slot, score) in scores.iter_mut().enumerate() {
                if let Some(kv) = row(slot) {
                    let mut dot = -0.0_f32;
                    for i in 0..d {
                        dot += q[i] * bf16(kv[i]);
                    }
                    *score = dot * g.attention_scale();
                    poison |= !score.is_finite();
                }
            }
        }
        let head = &mut out[h * d..(h + 1) * d];
        if poison {
            head.fill(f32::NAN);
            continue;
        }
        let mut max = -1e30_f32;
        let mut sum = 0.0_f32;
        let mut acc = vec![0.0_f32; d];
        for chunk in (0..slots).step_by(64) {
            let end = (chunk + 64).min(slots);
            let previous = max;
            for slot in chunk..end {
                if row(slot).is_some() {
                    max = max.max(scores[slot]);
                }
            }
            let rescale = exp_once(previous - max);
            sum *= rescale;
            let weights: Vec<f32> = (chunk..end)
                .map(|slot| {
                    if row(slot).is_some() {
                        exp_once(scores[slot] - max)
                    } else {
                        0.0
                    }
                })
                .collect();
            for slot in chunk..end {
                if row(slot).is_some() {
                    sum += weights[slot - chunk];
                }
            }
            for (i, a) in acc.iter_mut().enumerate() {
                let mut value = *a * rescale;
                for slot in chunk..end {
                    if let Some(kv) = row(slot) {
                        value += bf16(weights[slot - chunk]) * bf16(kv[i]);
                    }
                }
                *a = value;
            }
        }
        let sum = sum + exp_once(sinks[h] - max);
        for (o, a) in head.iter_mut().zip(&acc) {
            *o = bf16(a / sum);
        }
    }
    out
}

#[cfg(test)]
#[path = "v41_attention_tests.rs"]
mod tests;
