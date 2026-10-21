//! A DeepSeek-V4.1 stage on one GPU (#12394): a contiguous layer range whose
//! whole step runs on the device.
//!
//! [`V41Stage::load`] uploads the stage's weights in their checkpoint form
//! (FP8 and FP4 blocks stay packed) and allocates every piece of request
//! state on the device: each layer's window ring, the compressed KV and index
//! keys of every KV source the stage reads, each compressor's pending group,
//! each index source's selection, layer 20's candidate flags, the mixes of
//! every sub-block, and the Engram n-gram history. Engram tables stay in
//! page-locked host memory, and the device reads only the rows it needs.
//!
//! [`V41Stage::step`] runs one stage step: a prefill chunk or one decode
//! token. The step's inputs are uploaded once (the token ids and position,
//! and from a previous stage the residual streams, the pre-mix, and the
//! shared state rows the W9 contract carries), then one captured CUDA graph
//! runs per token. The graph reads the token's index within the step from a
//! device counter, so it serves every decode position and every prefill
//! token without a host round trip between tokens. The last stage runs a
//! second graph once per step: the final collapse and norm, the head, and
//! sampling, which leaves a single token id to read back. Nothing else
//! crosses to the host during a step.
//!
//! Each layer runs in the order the CPU path (`deepseek_v41_cpu`) runs it:
//! Engram (layers 1 and 14), the attention sub-block's mixes, collapse with
//! the previous sub-block's pre-mix, and norm; attention (W5's kernels around
//! W4's projections); expansion; then the FFN sub-block's mixes, collapse,
//! norm, router, routed and shared experts, sum, and expansion. The
//! arithmetic is the CPU path's default `ReferenceBf16` order, which W4–W6's
//! kernels repeat (see `docs/psionic/deepseek-v41-native.md`, W7).
//!
//! Prefill runs the per-token graph once for each prompt token. In the
//! `ReferenceBf16` order a chunk gives the same results as its tokens one at
//! a time, W4's matrix-vector kernel reads the weights once per slot either
//! way, and W6's mixing and routing kernels take one token. A batched
//! tensor-core prefill is separate work.

use crate::clef_prefill::{int, span};
use crate::v41_attention::{
    V41AttentionGeometry, V41CacheQuant, V41Candidates, V41Compressed, V41Compressor, V41Dense,
    V41DenseFormat, V41IndexKeyWeights, V41LayerOwners, V41Ring, V41RopeConfig, V41RopeTable,
    V41Rows, V41Source, V41Steps, v41_compress_ratio, v41_rope_frequencies,
};
use crate::v41_matvec::{V41_BLOCK, V41Matrix, V41MatvecFormat, V41Output, V41Slots};
use crate::v41_mix::{V41EngramGate, V41EngramTable, V41HcWeights, V41HyperConfig, V41Router};
use crate::{CudaBackend, CudaBuffer, CudaGraphExec, CudaHostBuffer, CudaSubmission};
use psionic_core::{DType, Shape, TensorSpec};
use psionic_runtime::RuntimeError;
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::ops::Range;
use std::time::Instant;

/// Layers whose compressor writes a shared KV source.
pub const V41_STAGE_KV_SOURCES: [usize; 4] = [2, 8, 14, 20];
/// Layers whose indexer writes a shared selection.
pub const V41_STAGE_INDEX_SOURCES: [usize; 8] = [2, 8, 14, 20, 24, 28, 32, 36];
/// The layer whose candidate blocks layers 24–36 read.
pub const V41_STAGE_CANDIDATE_SOURCE: usize = 20;
/// Layers with an Engram lookup.
pub const V41_ENGRAM_LAYERS: [usize; 2] = [1, 14];
/// Layers in the model.
pub const V41_LAYERS: usize = 40;
/// Hash columns per Engram layer: n-gram sizes 2 to 4 times 8 heads.
pub const V41_ENGRAM_COLUMNS: usize = 24;

unsafe extern "C" {
    fn psionic_v41_stage_begin(
        index: *const c_void,
        base: *const c_void,
        tokens: *const c_void,
        embed: *const c_void,
        vocab: i32,
        residual_in: *const c_void,
        pre_in: *const c_void,
        residual: *mut c_void,
        pre: *mut c_void,
        positions: *mut c_void,
        streams: i32,
        hidden: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_stage_copy(
        src: *const c_void,
        dst: *mut c_void,
        index: *const c_void,
        count: i32,
        store: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_stage_advance(index: *mut c_void, stream: *mut c_void) -> i32;
    fn psionic_v41_engram_hash(
        index: *const c_void,
        base: *const c_void,
        tokens: *const c_void,
        map: *const c_void,
        vocab: i32,
        pad: i32,
        history: *mut c_void,
        params: *const c_void,
        layers: i32,
        heads: i32,
        ids: *mut c_void,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_sample(
        logits: *const c_void,
        vocab: i32,
        settings: *const c_void,
        seed: *const c_void,
        positions: *const c_void,
        token: *mut c_void,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_candidate_flags(
        blocks: *const c_void,
        index: *const c_void,
        top_blocks: i32,
        flags: *mut c_void,
        capacity: i32,
        stream: *mut c_void,
    ) -> i32;
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Backend(format!("v41 stage: {}", message.into()))
}

// ---------------------------------------------------------------------------
// Kernel calls
// ---------------------------------------------------------------------------

/// Where [`CudaSubmission::v41_stage_begin`] reads a token's input.
#[derive(Clone, Copy, Debug)]
pub enum V41StageSource<'a> {
    /// Stage 0: the token's row of the BF16 embedding `[vocab, hidden]`,
    /// copied into every stream, and the pre-mix `[1, 0, ..]`.
    Embedding { table: &'a CudaBuffer, vocab: usize },
    /// Later stages: row `index` of the step's residual streams
    /// (`[tokens, streams × hidden]` floats) and pre-mixes
    /// (`[tokens, streams]`).
    Previous {
        residual: &'a CudaBuffer,
        pre_mix: &'a CudaBuffer,
        tokens: usize,
    },
}

impl CudaSubmission {
    /// Starts one token of a stage step: writes `positions[0] = base[0] +
    /// index[0]` and loads the token's residual streams and pre-mix (the
    /// first `streams` floats of `pre`).
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_stage_begin(
        &mut self,
        source: V41StageSource<'_>,
        index: &CudaBuffer,
        base: &CudaBuffer,
        tokens: &CudaBuffer,
        max_tokens: usize,
        residual: &CudaBuffer,
        pre: &CudaBuffer,
        positions: &CudaBuffer,
        streams: usize,
        hidden: usize,
    ) -> Result<(), RuntimeError> {
        let elements = streams
            .checked_mul(hidden)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("begin shape"))?;
        let (embed, vocab, residual_in, pre_in) = match source {
            V41StageSource::Embedding { table, vocab } => (
                span(table, 0, vocab * hidden, 2, "embedding")?.cast_const(),
                vocab,
                std::ptr::null(),
                std::ptr::null(),
            ),
            V41StageSource::Previous {
                residual,
                pre_mix,
                tokens,
            } => (
                std::ptr::null(),
                0,
                span(residual, 0, tokens * elements, 4, "residual input")?.cast_const(),
                span(pre_mix, 0, tokens * streams, 4, "pre-mix input")?.cast_const(),
            ),
        };
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_stage_begin(
                span(index, 0, 1, 4, "token index")?,
                span(base, 0, 1, 4, "base position")?,
                span(tokens, 0, max_tokens, 4, "tokens")?,
                embed,
                int(vocab, "vocabulary")?,
                residual_in,
                pre_in,
                span(residual, 0, elements, 4, "residual")?,
                span(pre, 0, streams, 4, "pre-mix")?,
                span(positions, 0, 1, 4, "positions")?,
                int(streams, "streams")?,
                int(hidden, "hidden")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 stage begin")
    }

    /// Copies `words` 4-byte words between a working buffer and row
    /// `index[0]` of a step-wide array: `store` writes `src` into
    /// `dst[index × words..]`, otherwise `src[index × words..]` is read into
    /// `dst`. `rows` is the array's row count.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    pub fn v41_stage_copy(
        &mut self,
        src: &CudaBuffer,
        dst: &CudaBuffer,
        index: &CudaBuffer,
        words: usize,
        rows: usize,
        store: bool,
    ) -> Result<(), RuntimeError> {
        let (src_words, dst_words) = if store {
            (words, words * rows)
        } else {
            (words * rows, words)
        };
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_stage_copy(
                span(src, 0, src_words, 4, "copy source")?.cast_const(),
                span(dst, 0, dst_words, 4, "copy destination")?,
                span(index, 0, 1, 4, "token index")?.cast_const(),
                int(words, "words")?,
                i32::from(store),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 stage copy")
    }

    /// Adds one to the token index.
    ///
    /// # Errors
    ///
    /// Refuses an empty buffer.
    pub fn v41_stage_advance(&mut self, index: &CudaBuffer) -> Result<(), RuntimeError> {
        // SAFETY: the extent was checked against the buffer.
        let code = unsafe {
            psionic_v41_stage_advance(
                span(index, 0, 1, 4, "token index")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 stage advance")
    }

    /// Hashes token `index[0]`'s n-grams for each Engram layer, as
    /// `NgramHashState::push_compressed`: the token's compressed id from
    /// `map`, then up to three earlier ids from `history` (four `i32`: the
    /// count and the ids, oldest first), the pad id past the sequence start.
    /// Position 0 starts a new history. `params` holds, per layer, 4
    /// multipliers, 24 primes, and 24 offsets (`u64`). Writes 24 `i64` row
    /// ids per layer, or -1 for a token outside the map.
    ///
    /// # Errors
    ///
    /// Refuses more than two layers and buffers too small for the shapes.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_engram_hash(
        &mut self,
        index: &CudaBuffer,
        base: &CudaBuffer,
        tokens: &CudaBuffer,
        max_tokens: usize,
        map: &CudaBuffer,
        vocab: usize,
        pad: u32,
        history: &CudaBuffer,
        params: &CudaBuffer,
        layers: usize,
        heads: usize,
        ids: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if !(1..=2).contains(&layers) || heads == 0 || 3 * heads != V41_ENGRAM_COLUMNS {
            return Err(invalid(format!("{layers} Engram layers of {heads} heads")));
        }
        let columns = layers * V41_ENGRAM_COLUMNS;
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_engram_hash(
                span(index, 0, 1, 4, "token index")?.cast_const(),
                span(base, 0, 1, 4, "base position")?.cast_const(),
                span(tokens, 0, max_tokens, 4, "tokens")?.cast_const(),
                span(map, 0, vocab, 4, "token map")?.cast_const(),
                int(vocab, "vocabulary")?,
                i32::try_from(pad).map_err(|_| invalid("pad id"))?,
                span(history, 0, 4, 4, "n-gram history")?,
                span(
                    params,
                    0,
                    layers * (4 + 2 * V41_ENGRAM_COLUMNS),
                    8,
                    "hash parameters",
                )?
                .cast_const(),
                int(layers, "layers")?,
                int(heads, "heads")?,
                span(ids, 0, columns, 8, "row ids")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 Engram hash")
    }

    /// Samples one token from `vocab` logits into `token[0]`. `settings`
    /// holds the temperature and top-p (two floats); a temperature at or
    /// below zero is greedy, ties to the lower id. Otherwise the weights are
    /// `exp((logit - max) / temperature)`, top-p keeps the largest threshold
    /// whose kept mass reaches top-p of the total (bisection to 2^-40 of the
    /// largest weight), and the draw is `splitmix64(seed ^ position ×
    /// 0xD1B54A32D192ED03)`'s top 53 bits as a fraction of the kept mass,
    /// taken in id order. A nonfinite logit writes -1.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    pub fn v41_sample(
        &mut self,
        logits: &CudaBuffer,
        vocab: usize,
        settings: &CudaBuffer,
        seed: &CudaBuffer,
        positions: &CudaBuffer,
        token: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if vocab == 0 {
            return Err(invalid("empty vocabulary"));
        }
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_sample(
                span(logits, 0, vocab, 4, "logits")?.cast_const(),
                int(vocab, "vocabulary")?,
                span(settings, 0, 2, 4, "sampling settings")?.cast_const(),
                span(seed, 0, 1, 8, "seed")?.cast_const(),
                span(positions, 0, 1, 4, "positions")?.cast_const(),
                span(token, 0, 1, 4, "token")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 sampling")
    }

    /// Clears `flags` (`capacity` bytes) and sets the flag of each kept
    /// block id in row `index[0]` of `blocks` (`top_blocks` `i32` per
    /// token, -1 for none), as layer 20's candidate output reads.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    pub fn v41_candidate_flags(
        &mut self,
        blocks: &CudaBuffer,
        rows: usize,
        index: &CudaBuffer,
        top_blocks: usize,
        flags: &CudaBuffer,
        capacity: usize,
    ) -> Result<(), RuntimeError> {
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_candidate_flags(
                span(blocks, 0, rows * top_blocks, 4, "candidate blocks")?.cast_const(),
                span(index, 0, 1, 4, "token index")?.cast_const(),
                int(top_blocks, "top blocks")?,
                span(flags, 0, capacity, 1, "candidate flags")?,
                int(capacity, "capacity")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 candidate flags")
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// The model's shapes, as `deepseek_v41_cpu`'s `BlockConfig` and
/// `TransformerConfig` give them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41StageShape {
    pub hidden: usize,
    pub q_rank: usize,
    pub o_groups: usize,
    pub o_rank: usize,
    pub attention: V41AttentionGeometry,
    pub hc: V41HyperConfig,
    pub router: V41Router,
    pub expert_width: usize,
    pub swiglu_limit: f32,
    pub vocabulary: usize,
    /// Values per Engram table row (256).
    pub engram_width: usize,
    /// Engram hash heads per n-gram size (8).
    pub engram_heads: usize,
    /// Candidate blocks layer 20 keeps (2,048) and positions per block (8).
    pub candidate_blocks: usize,
    pub candidate_block_size: usize,
}

impl V41StageShape {
    /// DeepSeek-V4.1-Flash.
    #[must_use]
    pub const fn flash() -> Self {
        Self {
            hidden: 5120,
            q_rank: 1280,
            o_groups: 8,
            o_rank: 1024,
            attention: V41AttentionGeometry::flash(),
            hc: V41HyperConfig::flash(),
            router: V41Router::flash(),
            expert_width: 2304,
            swiglu_limit: 10.0,
            vocabulary: 129_280,
            engram_width: 256,
            engram_heads: 8,
            candidate_blocks: 2048,
            candidate_block_size: 8,
        }
    }

    fn check(&self) -> Result<(), RuntimeError> {
        let g = self.attention;
        let multiple = |n: usize| n > 0 && n.is_multiple_of(V41_BLOCK);
        if !multiple(self.hidden)
            || !multiple(self.q_rank)
            || self.o_groups == 0
            || !(g.heads * g.head_dim).is_multiple_of(self.o_groups)
            || !multiple(g.heads * g.head_dim / self.o_groups)
            || !multiple(self.o_groups * self.o_rank)
            || !multiple(self.expert_width)
            || !multiple(self.engram_width)
            || self.hc.width != self.hidden
            || self.hc.streams == 0
            || self.vocabulary == 0
            || 3 * self.engram_heads != V41_ENGRAM_COLUMNS
            || self.candidate_blocks == 0
            || self.candidate_block_size == 0
        {
            return Err(invalid(format!("model shape {self:?}")));
        }
        Ok(())
    }
}

/// One Engram layer's hash constants, as `engram::LayerLayout`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V41EngramHash {
    pub layer: usize,
    pub multipliers: [u64; 4],
    /// 24 primes, n-gram size major then head.
    pub primes: Vec<u64>,
    /// 24 row offsets in the same order.
    pub offsets: Vec<u64>,
    /// Rows in the table.
    pub rows: u64,
}

/// The tokenizer's compressed ids for the Engram hash: one per vocabulary
/// token, and the compressed pad id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V41TokenMap {
    pub ids: Vec<u32>,
    pub pad: u32,
}

/// What a stage holds.
#[derive(Clone, Debug)]
pub struct V41StageConfig {
    pub shape: V41StageShape,
    /// The stage's layers, end exclusive. Layer 0 adds the embeddings and
    /// layer 39 the final norm, head, and sampling.
    pub layers: Range<usize>,
    /// Positions a request may reach; it sizes the KV sources.
    pub max_context: usize,
    /// Tokens one step may carry.
    pub max_step_tokens: usize,
    /// Hash constants of the Engram layers in `layers`.
    pub engram: Vec<V41EngramHash>,
    /// Required when `layers` holds an Engram layer.
    pub token_map: Option<V41TokenMap>,
    /// Keep per-layer observations of each token for parity checks.
    pub observe: bool,
    /// A negative control for G1 v2 (#12405): window layers rotate with the
    /// compressed layers' frequencies and the reverse. Never set to serve.
    pub swap_rope_tables: bool,
}

impl V41StageConfig {
    fn engram_layers(&self) -> Vec<usize> {
        V41_ENGRAM_LAYERS
            .into_iter()
            .filter(|l| self.layers.contains(l))
            .collect()
    }

    /// Whether the stage holds layer 0 and embeds its tokens.
    #[must_use]
    pub fn embeds(&self) -> bool {
        self.layers.start == 0
    }

    /// Whether the stage holds layer 39 and samples.
    #[must_use]
    pub fn samples(&self) -> bool {
        self.layers.end == V41_LAYERS
    }
}

/// What crosses a stage boundary at `layer`, as W9's
/// `deepseek_v41_stage::boundary_sections` lists it (besides the residual
/// streams, the pre-mix, and the Engram history).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V41Boundary {
    /// The KV source whose new rows cross, and whether its index keys do.
    pub kv: Option<(usize, bool)>,
    /// Layer 20's candidate blocks cross.
    pub candidates: bool,
    /// The index source whose selection crosses.
    pub selection: Option<usize>,
}

impl V41Boundary {
    /// The boundary in front of layer `layer` (1 to 39).
    #[must_use]
    pub fn at(layer: usize) -> Self {
        let last = |sources: &[usize]| sources.iter().copied().filter(|s| *s <= layer).last();
        let kv = last(&V41_STAGE_KV_SOURCES)
            .filter(|s| *s < layer)
            .map(|source| {
                let scored_later = V41_STAGE_INDEX_SOURCES
                    .iter()
                    .any(|l| *l >= layer && last_source(*l) == Some(source));
                (source, scored_later)
            });
        Self {
            kv,
            candidates: layer > V41_STAGE_CANDIDATE_SOURCE
                && V41_STAGE_INDEX_SOURCES.iter().any(|l| *l >= layer),
            selection: last(&V41_STAGE_INDEX_SOURCES).filter(|s| *s < layer),
        }
    }
}

fn last_source(layer: usize) -> Option<usize> {
    V41_STAGE_KV_SOURCES
        .iter()
        .copied()
        .filter(|s| *s <= layer)
        .last()
}

/// A tensor's element type in the checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V41TensorDtype {
    F32,
    Bf16,
    F8E4M3,
    F8E8M0,
    /// Packed FP4 E2M1 pairs.
    I8,
}

/// One checkpoint tensor's bytes on the host.
#[derive(Clone, Debug)]
pub struct V41HostTensor {
    pub dtype: V41TensorDtype,
    pub shape: Vec<usize>,
    pub bytes: Vec<u8>,
}

/// Reads checkpoint tensors by name (for example `layers.2.attn.wq_a.weight`).
pub trait V41TensorSource {
    /// The tensor's dtype, shape, and bytes.
    ///
    /// # Errors
    ///
    /// Any failure to find or read it.
    fn tensor(&self, name: &str) -> Result<V41HostTensor, RuntimeError>;
}

/// An Engram layer's table in page-locked host memory: FP8 codes
/// `[rows, width]` and UE8M0 scales `[rows, width / 32]`.
#[derive(Debug)]
pub struct V41EngramHostTable {
    pub layer: usize,
    pub codes: CudaHostBuffer,
    pub scales: CudaHostBuffer,
    pub rows: u64,
}

/// Sampling settings for the last stage, sent with every step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41Sampling {
    /// At or below zero: greedy.
    pub temperature: f32,
    pub top_p: f32,
    pub seed: u64,
}

impl V41Sampling {
    /// Greedy decoding.
    #[must_use]
    pub const fn greedy() -> Self {
        Self {
            temperature: 0.0,
            top_p: 1.0,
            seed: 0,
        }
    }
}

/// New rows of a KV source in a step.
#[derive(Clone, Debug, PartialEq)]
pub struct V41SourceRows {
    pub layer: usize,
    pub first_row: usize,
    /// BF16 words `[rows, head_dim]`.
    pub kv: Vec<u16>,
    /// BF16 words `[rows, index_dim]`, when the boundary carries them.
    pub index_keys: Option<Vec<u16>>,
}

/// One step's input.
#[derive(Clone, Debug)]
pub struct V41StepInput<'a> {
    /// Position of the first token.
    pub position: usize,
    /// The step's token ids (every stage receives them).
    pub tokens: &'a [u32],
    /// From the previous stage: `[tokens, streams × hidden]` floats.
    pub residual: Option<&'a [f32]>,
    /// From the previous stage: `[tokens, streams]` floats.
    pub pre_mix: Option<&'a [f32]>,
    /// The new rows of the KV source the boundary carries.
    pub source_rows: Option<&'a V41SourceRows>,
    /// The boundary's index selection, `[tokens, index_top_k]`.
    pub selection: Option<&'a [i32]>,
    /// Layer 20's kept block ids, `[tokens, candidate_blocks]` (-1 after
    /// the kept ones).
    pub candidates: Option<&'a [i32]>,
    pub sampling: V41Sampling,
}

/// Host and device work in one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V41StepStats {
    pub tokens: usize,
    pub graph_launches: usize,
    /// Graphs captured during this step (the first step of each kind).
    pub graph_captures: usize,
    pub host_to_device_copies: usize,
    pub host_to_device_bytes: usize,
    pub device_to_host_copies: usize,
    pub device_to_host_bytes: usize,
    /// Host waits for the device.
    pub synchronizations: usize,
    /// Wall time from the first upload to the last read.
    pub seconds: f64,
    /// Wall time of the graph launches and the wait for them.
    pub device_seconds: f64,
}

/// One step's output.
#[derive(Clone, Debug, Default)]
pub struct V41StepOutput {
    /// For the next stage: `[tokens, streams × hidden]`.
    pub residual: Vec<f32>,
    /// For the next stage: `[tokens, streams]`.
    pub pre_mix: Vec<f32>,
    pub source_rows: Option<V41SourceRows>,
    /// `(layer, [tokens, index_top_k])`.
    pub selection: Option<(usize, Vec<i32>)>,
    /// `[tokens, candidate_blocks]`.
    pub candidates: Option<Vec<i32>>,
    /// The three token ids before the step's first token, oldest first,
    /// with `u32::MAX` before the sequence start (W9's Engram history).
    pub engram_history: Vec<u32>,
    /// The sampled token, from the last stage.
    pub token: Option<u32>,
    pub stats: V41StepStats,
}

/// Device bytes and parameters of one decoder layer, as the stage loaded it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct V41LayerMemory {
    pub layer: usize,
    /// Weight and scale bytes on the device, routed experts included.
    pub weight_bytes: usize,
    /// The routed experts' share of `weight_bytes`.
    pub routed_expert_bytes: usize,
    /// Parameters (logical elements, scales excluded) in `weight_bytes`.
    pub parameters: u64,
    /// The routed experts' share of `parameters`.
    pub routed_expert_parameters: u64,
    /// The layer's own request state: its window ring and mix buffers.
    pub state_bytes: usize,
}

/// One shared KV source's device state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct V41SourceMemory {
    /// The KV source layer.
    pub layer: usize,
    /// Its compression ratio: one stored row per `ratio` positions.
    pub ratio: usize,
    /// Rows allocated: `max_context / ratio + 1`.
    pub capacity: usize,
    /// Compressed KV rows, BF16.
    pub kv_bytes: usize,
    /// Indexer keys, BF16.
    pub index_key_bytes: usize,
    /// The compressor's pending group, when the source layer is in the stage.
    pub compressor_state_bytes: usize,
}

/// Where a loaded stage's device and pinned host bytes went. The parts add
/// up to [`V41Stage::device_bytes`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct V41StageMemory {
    pub max_context: usize,
    pub max_step_tokens: usize,
    pub layers: Vec<V41LayerMemory>,
    pub embedding_bytes: usize,
    pub embedding_parameters: u64,
    /// The final norm and output head.
    pub head_bytes: usize,
    pub head_parameters: u64,
    /// RoPE tables, the Engram token map, and its hash constants.
    pub constant_bytes: usize,
    pub sources: Vec<V41SourceMemory>,
    /// Index selections that layers in the stage reuse.
    pub selection_bytes: usize,
    /// Scratch and step buffers.
    pub work_bytes: usize,
    /// The part of `work_bytes` that grows with `max_step_tokens`.
    pub work_step_bytes: usize,
    /// Page-locked host bytes of the stage's Engram tables.
    pub engram_host_bytes: usize,
}

impl V41StageMemory {
    /// Sum of the device parts.
    #[must_use]
    pub fn device_total(&self) -> usize {
        self.layers
            .iter()
            .map(|l| l.weight_bytes + l.state_bytes)
            .sum::<usize>()
            + self.embedding_bytes
            + self.head_bytes
            + self.constant_bytes
            + self
                .sources
                .iter()
                .map(|s| s.kv_bytes + s.index_key_bytes + s.compressor_state_bytes)
                .sum::<usize>()
            + self.selection_bytes
            + self.work_bytes
    }
}

// ---------------------------------------------------------------------------
// Device weights and state
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Packed {
    format: V41MatvecFormat,
    weights: CudaBuffer,
    scales: CudaBuffer,
    rows: usize,
    cols: usize,
    matrices: usize,
}

impl Packed {
    fn matrix(&self) -> V41Matrix<'_> {
        V41Matrix {
            format: self.format,
            weights: &self.weights,
            scales: &self.scales,
            rows: self.rows,
            cols: self.cols,
            matrices: self.matrices,
        }
    }
}

#[derive(Debug)]
struct HcWeights {
    projection: CudaBuffer,
    scale: CudaBuffer,
    base: CudaBuffer,
}

impl HcWeights {
    fn weights(&self) -> V41HcWeights<'_> {
        V41HcWeights {
            projection: &self.projection,
            scale: &self.scale,
            base: &self.base,
        }
    }
}

#[derive(Debug)]
struct CompressorWeights {
    wkv: CudaBuffer,
    wgate: Option<CudaBuffer>,
    format: V41DenseFormat,
    norm: CudaBuffer,
    wk: CudaBuffer,
    k_norm: CudaBuffer,
}

#[derive(Debug)]
struct IndexerWeights {
    wq_b: Packed,
    weights_proj: CudaBuffer,
}

#[derive(Debug)]
struct EngramWeights {
    slot: usize,
    q: CudaBuffer,
    k: CudaBuffer,
    wkv: Packed,
}

#[derive(Debug)]
struct LayerWeights {
    layer: usize,
    hc_attn: HcWeights,
    hc_ffn: HcWeights,
    attn_norm: CudaBuffer,
    ffn_norm: CudaBuffer,
    wq_a: Packed,
    q_norm: CudaBuffer,
    wq_b: Packed,
    wkv: Packed,
    kv_norm: CudaBuffer,
    sinks: CudaBuffer,
    wo_a: CudaBuffer,
    wo_a_scales: CudaBuffer,
    wo_b: Packed,
    compressor: Option<CompressorWeights>,
    indexer: Option<IndexerWeights>,
    gate: CudaBuffer,
    bias: CudaBuffer,
    w1: Packed,
    w2: Packed,
    w3: Packed,
    shared: [Packed; 3],
    engram: Option<EngramWeights>,
    ring: CudaBuffer,
    mixes_attn: CudaBuffer,
    mixes_ffn: CudaBuffer,
    /// Observation buffers, `[max_step_tokens, words]` each.
    observed: Vec<(V41Site, CudaBuffer, usize)>,
}

#[derive(Debug)]
struct SourceState {
    kv: CudaBuffer,
    index_keys: CudaBuffer,
    capacity: usize,
    ratio: usize,
    /// The compressor's pending group, on source layers inside the stage.
    state: Option<CudaBuffer>,
}

impl SourceState {
    fn source(&self) -> V41Source<'_> {
        V41Source {
            kv: &self.kv,
            index_keys: &self.index_keys,
            capacity: self.capacity,
        }
    }
}

/// A per-layer observation the stage can keep for each token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum V41Site {
    /// Engram row ids (`i64`, read as pairs of words).
    EngramIds,
    /// The gathered BF16 Engram rows.
    EngramRows,
    /// The residual after the Engram gate.
    EngramOut,
    /// The residual entering the layer's mixing (`hc.in`).
    HcIn,
    /// The attention sub-block's mixes, `[pre, post, comb]`.
    AttnMixes,
    /// The normalized attention input (`attn.in`).
    AttnIn,
    /// The attention output after `wo_b` (`attn.out`).
    AttnOut,
    /// The residual after the attention expansion.
    AttnExpanded,
    /// The FFN sub-block's mixes.
    FfnMixes,
    /// The normalized FFN input (`ffn.gate.in`).
    FfnIn,
    /// Selected expert ids (`i32`).
    ExpertIds,
    /// Their route weights.
    RouteWeights,
    /// The MoE output (`ffn.out`).
    FfnOut,
    /// The layer's output residual (`hc.out`).
    HcOut,
    /// The index selection (`i32`), on index source layers.
    Selection,
}

impl V41Site {
    /// Whether the site holds integers.
    #[must_use]
    pub const fn integer(self) -> bool {
        matches!(self, Self::EngramIds | Self::ExpertIds | Self::Selection)
    }
}

/// Every buffer a step's graph reads or writes besides layer weights.
#[derive(Debug)]
struct Work {
    index: CudaBuffer,
    base: CudaBuffer,
    positions: CudaBuffer,
    tokens: CudaBuffer,
    residual: [CudaBuffer; 2],
    pre_in: CudaBuffer,
    scratch: CudaBuffer,
    collapsed: CudaBuffer,
    normalized: CudaBuffer,
    act_codes: CudaBuffer,
    act_scales: CudaBuffer,
    act2_codes: CudaBuffer,
    act2_scales: CudaBuffer,
    q_low: CudaBuffer,
    q_norm: CudaBuffer,
    q_heads: CudaBuffer,
    kv: CudaBuffer,
    kv_norm: CudaBuffer,
    values: CudaBuffer,
    gates: CudaBuffer,
    latents: CudaBuffer,
    index_query: CudaBuffer,
    index_weights: CudaBuffer,
    scores: CudaBuffer,
    block_scores: CudaBuffer,
    flags: CudaBuffer,
    blocks: CudaBuffer,
    block_capacity: usize,
    heads: CudaBuffer,
    heads_q: CudaBuffer,
    low_rank: CudaBuffer,
    attn_out: CudaBuffer,
    logits: CudaBuffer,
    ids: CudaBuffer,
    route_weights: CudaBuffer,
    g: CudaBuffer,
    u: CudaBuffer,
    h: CudaBuffer,
    y: CudaBuffer,
    sg: CudaBuffer,
    su: CudaBuffer,
    sh: CudaBuffer,
    sy: CudaBuffer,
    moe_out: CudaBuffer,
    engram_ids: CudaBuffer,
    engram_slot_ids: CudaBuffer,
    engram_rows: CudaBuffer,
    engram_projected: CudaBuffer,
    history: CudaBuffer,
    // Step-wide staging.
    residual_in: CudaBuffer,
    pre_mix_in: CudaBuffer,
    residual_out: CudaBuffer,
    pre_mix_out: CudaBuffer,
    selection_in: CudaBuffer,
    candidates_in: CudaBuffer,
    selection_out: CudaBuffer,
    candidates_out: CudaBuffer,
    // The last stage.
    head_logits: CudaBuffer,
    settings: CudaBuffer,
    seed: CudaBuffer,
    token: CudaBuffer,
}

/// A loaded stage. See the module documentation.
pub struct V41Stage {
    config: V41StageConfig,
    layers: Vec<LayerWeights>,
    sources: BTreeMap<usize, SourceState>,
    selections: BTreeMap<usize, CudaBuffer>,
    rope_window: CudaBuffer,
    rope_compressed: CudaBuffer,
    embed: Option<CudaBuffer>,
    head: Option<(CudaBuffer, CudaBuffer)>,
    engram_tables: Vec<V41EngramHostTable>,
    engram_map: Option<CudaBuffer>,
    engram_params: Option<CudaBuffer>,
    work: Work,
    body: Option<CudaGraphExec>,
    body_observed: Option<CudaGraphExec>,
    tail: Option<CudaGraphExec>,
    history: Vec<u32>,
    weight_bytes: usize,
    state_bytes: usize,
    memory: V41StageMemory,
}

impl std::fmt::Debug for V41Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("V41Stage")
            .field("layers", &self.config.layers)
            .field("weight_bytes", &self.weight_bytes)
            .field("state_bytes", &self.state_bytes)
            .finish_non_exhaustive()
    }
}

struct Loader<'a> {
    backend: &'a mut CudaBackend,
    source: &'a dyn V41TensorSource,
    bytes: usize,
    /// Parameters loaded: logical weight elements, scales excluded.
    params: u64,
    /// Bytes of request state and scratch among `bytes`.
    state: usize,
}

impl Loader<'_> {
    fn alloc(&mut self, bytes: usize) -> Result<CudaBuffer, RuntimeError> {
        let device = self
            .backend
            .selected_device()
            .map(|d| d.device.clone())
            .ok_or_else(|| invalid("no CUDA device"))?;
        // Byte buffers are allocated as whole `i32` words.
        let words = bytes.div_ceil(4).max(1);
        let buffer = self.backend.allocate_buffer(&TensorSpec::new(
            Shape::new(vec![words]),
            DType::I32,
            device,
        ))?;
        self.bytes += 4 * words;
        Ok(buffer)
    }

    fn zeros(&mut self, bytes: usize) -> Result<CudaBuffer, RuntimeError> {
        self.bytes += bytes.max(4);
        self.backend.byte_buffer(&vec![0; bytes.max(4)])
    }

    /// Zeroed request state or scratch.
    fn state_zeros(&mut self, bytes: usize) -> Result<CudaBuffer, RuntimeError> {
        self.state += bytes.max(4);
        self.zeros(bytes)
    }

    fn upload(&mut self, bytes: &[u8]) -> Result<CudaBuffer, RuntimeError> {
        let mut padded;
        let bytes = if bytes.len() < 4 {
            padded = bytes.to_vec();
            padded.resize(4, 0);
            padded.as_slice()
        } else {
            bytes
        };
        self.bytes += bytes.len();
        self.backend.byte_buffer(bytes)
    }

    fn floats(&mut self, values: &[f32]) -> Result<CudaBuffer, RuntimeError> {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.upload(&bytes)
    }

    fn tensor(
        &self,
        name: &str,
        dtype: V41TensorDtype,
        shape: &[usize],
    ) -> Result<V41HostTensor, RuntimeError> {
        let tensor = self.source.tensor(name)?;
        if tensor.dtype != dtype || tensor.shape != shape {
            return Err(invalid(format!(
                "{name} is {:?} {:?}; wants {dtype:?} {shape:?}",
                tensor.dtype, tensor.shape
            )));
        }
        let elements: usize = shape.iter().product();
        let size = match dtype {
            V41TensorDtype::F32 => 4,
            V41TensorDtype::Bf16 => 2,
            _ => 1,
        };
        if tensor.bytes.len() != elements * size {
            return Err(invalid(format!("{name} has {} bytes", tensor.bytes.len())));
        }
        Ok(tensor)
    }

    /// A float32 vector or matrix from F32 or BF16 storage.
    fn f32s(
        &mut self,
        name: &str,
        shape: &[usize],
        bf16: bool,
    ) -> Result<CudaBuffer, RuntimeError> {
        let dtype = if bf16 {
            V41TensorDtype::Bf16
        } else {
            V41TensorDtype::F32
        };
        let tensor = self.tensor(name, dtype, shape)?;
        self.params += shape.iter().product::<usize>() as u64;
        if bf16 {
            let values: Vec<f32> = tensor
                .bytes
                .chunks_exact(2)
                .map(|w| f32::from_bits(u32::from(u16::from_le_bytes([w[0], w[1]])) << 16))
                .collect();
            self.floats(&values)
        } else {
            self.upload(&tensor.bytes)
        }
    }

    fn bf16_words(&mut self, name: &str, shape: &[usize]) -> Result<CudaBuffer, RuntimeError> {
        let tensor = self.tensor(name, V41TensorDtype::Bf16, shape)?;
        self.params += shape.iter().product::<usize>() as u64;
        self.upload(&tensor.bytes)
    }

    fn packed(
        &mut self,
        stem: &str,
        format: V41MatvecFormat,
        rows: usize,
        cols: usize,
    ) -> Result<Packed, RuntimeError> {
        self.stack(&[stem.to_owned()], format, rows, cols)
    }

    /// Matrices of one shape laid out one after another.
    fn stack(
        &mut self,
        stems: &[String],
        format: V41MatvecFormat,
        rows: usize,
        cols: usize,
    ) -> Result<Packed, RuntimeError> {
        let weight_bytes = format.weight_bytes(rows, cols);
        let scale_bytes = format.scale_bytes(rows, cols);
        let (weight_shape, weight_dtype, scale_shape) = match format {
            V41MatvecFormat::Fp8Block32x32 => (
                [rows, cols],
                V41TensorDtype::F8E4M3,
                [rows.div_ceil(V41_BLOCK), cols / V41_BLOCK],
            ),
            V41MatvecFormat::Fp4E2M1Row32 => (
                [rows, cols / 2],
                V41TensorDtype::I8,
                [rows, cols / V41_BLOCK],
            ),
        };
        let mut weights = self.alloc(weight_bytes * stems.len())?;
        let mut scales = self.alloc(scale_bytes * stems.len())?;
        self.params += (rows * cols * stems.len()) as u64;
        for (i, stem) in stems.iter().enumerate() {
            let w = self.tensor(&format!("{stem}.weight"), weight_dtype, &weight_shape)?;
            let s = self.tensor(
                &format!("{stem}.scale"),
                V41TensorDtype::F8E8M0,
                &scale_shape,
            )?;
            weights.write_bytes_at_offset(i * weight_bytes, &w.bytes)?;
            scales.write_bytes_at_offset(i * scale_bytes, &s.bytes)?;
        }
        Ok(Packed {
            format,
            weights,
            scales,
            rows,
            cols,
            matrices: stems.len(),
        })
    }
}

fn words_f32(buffer: &CudaBuffer, len: usize) -> Result<Vec<f32>, RuntimeError> {
    Ok(buffer
        .read_bytes_at_offset(0, len * 4)?
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn words_i32(buffer: &CudaBuffer, len: usize) -> Result<Vec<i32>, RuntimeError> {
    Ok(buffer
        .read_bytes_at_offset(0, len * 4)?
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn le_bytes<T: Copy, const N: usize>(values: &[T], f: impl Fn(T) -> [u8; N]) -> Vec<u8> {
    values.iter().flat_map(|v| f(*v)).collect()
}

impl V41Stage {
    /// Loads a stage's weights from `source` and allocates its request
    /// state. `engram_tables` holds the page-locked table of each Engram
    /// layer in the stage.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration, missing or misshapen tensors, a missing
    /// Engram table, and device allocation failures.
    #[allow(clippy::too_many_lines)]
    pub fn load(
        backend: &mut CudaBackend,
        config: V41StageConfig,
        source: &dyn V41TensorSource,
        engram_tables: Vec<V41EngramHostTable>,
    ) -> Result<Self, RuntimeError> {
        let shape = config.shape;
        shape.check()?;
        let g = shape.attention;
        let (h, s) = (shape.hidden, shape.hc.streams);
        if config.layers.is_empty()
            || config.layers.end > V41_LAYERS
            || config.max_context == 0
            || config.max_context > crate::v41_attention::V41_MAX_POSITION
            || config.max_step_tokens == 0
            || config.max_step_tokens > config.max_context
        {
            return Err(invalid(format!(
                "layers {:?}, context {}, step tokens {}",
                config.layers, config.max_context, config.max_step_tokens
            )));
        }
        let engram_layers = config.engram_layers();
        if !engram_layers.is_empty() {
            let map = config
                .token_map
                .as_ref()
                .ok_or_else(|| invalid("Engram layers need the token map"))?;
            if map.ids.len() != shape.vocabulary {
                return Err(invalid("token map size"));
            }
            for layer in &engram_layers {
                let hash = config
                    .engram
                    .iter()
                    .find(|e| e.layer == *layer)
                    .ok_or_else(|| invalid(format!("no hash constants for layer {layer}")))?;
                if hash.primes.len() != V41_ENGRAM_COLUMNS
                    || hash.offsets.len() != V41_ENGRAM_COLUMNS
                {
                    return Err(invalid("Engram hash constants"));
                }
                if !engram_tables
                    .iter()
                    .any(|t| t.layer == *layer && t.rows == hash.rows)
                {
                    return Err(invalid(format!("no Engram table for layer {layer}")));
                }
            }
        }
        let mut loader = Loader {
            backend,
            source,
            bytes: 0,
            params: 0,
            state: 0,
        };
        let mut memory = V41StageMemory {
            max_context: config.max_context,
            max_step_tokens: config.max_step_tokens,
            ..V41StageMemory::default()
        };
        let fp8 = V41MatvecFormat::Fp8Block32x32;
        let fp4 = V41MatvecFormat::Fp4E2M1Row32;
        let mut layers = Vec::new();
        for layer in config.layers.clone() {
            let (layer_bytes, layer_params, layer_state) =
                (loader.bytes, loader.params, loader.state);
            let ratio = v41_compress_ratio(layer).ok_or_else(|| invalid("layer"))?;
            let name = |part: &str| format!("layers.{layer}.{part}");
            let hc = |loader: &mut Loader<'_>, part: &str| -> Result<HcWeights, RuntimeError> {
                Ok(HcWeights {
                    projection: loader.f32s(
                        &name(&format!("hc_{part}_fn")),
                        &[shape.hc.mixes(), shape.hc.elements()],
                        false,
                    )?,
                    scale: loader.f32s(&name(&format!("hc_{part}_scale")), &[3], false)?,
                    base: loader.f32s(
                        &name(&format!("hc_{part}_base")),
                        &[shape.hc.mixes()],
                        false,
                    )?,
                })
            };
            let hc_attn = hc(&mut loader, "attn")?;
            let hc_ffn = hc(&mut loader, "ffn")?;
            let heads_width = g.heads * g.head_dim;
            let compressor = if V41_STAGE_KV_SOURCES.contains(&layer) {
                let (wkv, wgate, format) = if ratio > 1 {
                    (
                        loader.f32s(&name("attn.compressor.wkv.weight"), &[g.head_dim, h], true)?,
                        Some(loader.f32s(
                            &name("attn.compressor.wgate.weight"),
                            &[g.head_dim, h],
                            true,
                        )?),
                        V41DenseFormat::F32,
                    )
                } else {
                    (
                        loader.bf16_words(&name("attn.compressor.wkv.weight"), &[g.head_dim, h])?,
                        None,
                        V41DenseFormat::Bf16,
                    )
                };
                Some(CompressorWeights {
                    wkv,
                    wgate,
                    format,
                    norm: loader.f32s(&name("attn.compressor.norm.weight"), &[g.head_dim], true)?,
                    wk: loader
                        .bf16_words(&name("attn.indexer.wk.weight"), &[g.index_dim, g.head_dim])?,
                    k_norm: loader.f32s(
                        &name("attn.indexer.k_norm.weight"),
                        &[g.index_dim],
                        true,
                    )?,
                })
            } else {
                None
            };
            let indexer = if V41_STAGE_INDEX_SOURCES.contains(&layer) {
                Some(IndexerWeights {
                    wq_b: loader.packed(
                        &name("attn.indexer.wq_b"),
                        fp8,
                        g.index_heads * g.index_dim,
                        shape.q_rank,
                    )?,
                    weights_proj: loader.bf16_words(
                        &name("attn.indexer.weights_proj.weight"),
                        &[g.index_heads, h],
                    )?,
                })
            } else {
                None
            };
            let engram = match engram_layers.iter().position(|l| *l == layer) {
                Some(slot) => Some(EngramWeights {
                    slot,
                    q: loader.f32s(&name("engram.q_weight"), &[s, h], true)?,
                    k: loader.f32s(&name("engram.k_weight"), &[s, h], true)?,
                    wkv: loader.packed(
                        &name("engram.wkv"),
                        fp8,
                        s * h + h,
                        V41_ENGRAM_COLUMNS * shape.engram_width,
                    )?,
                }),
                None => None,
            };
            let wo_rows = shape.o_groups * shape.o_rank;
            let wo_cols = heads_width / shape.o_groups;
            let wo_a = loader.tensor(
                &name("attn.wo_a.weight"),
                V41TensorDtype::F8E4M3,
                &[wo_rows, wo_cols],
            )?;
            loader.params += (wo_rows * wo_cols) as u64;
            let wo_a_scales = loader.tensor(
                &name("attn.wo_a.scale"),
                V41TensorDtype::F8E8M0,
                &[wo_rows / V41_BLOCK, wo_cols / V41_BLOCK],
            )?;
            let experts: Vec<String> = (0..shape.router.experts)
                .map(|i| name(&format!("ffn.experts.{i}")))
                .collect();
            let w = |part: &str| -> Vec<String> {
                experts.iter().map(|e| format!("{e}.{part}")).collect()
            };
            let (routed_bytes, routed_params) = (loader.bytes, loader.params);
            let w1 = loader.stack(&w("w1"), fp4, shape.expert_width, h)?;
            let w2 = loader.stack(&w("w2"), fp4, h, shape.expert_width)?;
            let w3 = loader.stack(&w("w3"), fp4, shape.expert_width, h)?;
            let (routed_bytes, routed_params) =
                (loader.bytes - routed_bytes, loader.params - routed_params);
            let mut observed = Vec::new();
            if config.observe {
                let mut sites = vec![
                    (V41Site::HcIn, s * h),
                    (V41Site::AttnMixes, shape.hc.mixes()),
                    (V41Site::AttnIn, h),
                    (V41Site::AttnOut, h),
                    (V41Site::AttnExpanded, s * h),
                    (V41Site::FfnMixes, shape.hc.mixes()),
                    (V41Site::FfnIn, h),
                    (V41Site::ExpertIds, shape.router.top_k),
                    (V41Site::RouteWeights, shape.router.top_k),
                    (V41Site::FfnOut, h),
                    (V41Site::HcOut, s * h),
                ];
                if V41_ENGRAM_LAYERS.contains(&layer) {
                    sites.extend([
                        (V41Site::EngramIds, 2 * V41_ENGRAM_COLUMNS),
                        (V41Site::EngramRows, V41_ENGRAM_COLUMNS * shape.engram_width),
                        (V41Site::EngramOut, s * h),
                    ]);
                }
                if V41_STAGE_INDEX_SOURCES.contains(&layer) {
                    sites.push((V41Site::Selection, g.index_top_k));
                }
                for (site, words) in sites {
                    let buffer = loader.state_zeros(4 * words * config.max_step_tokens)?;
                    observed.push((site, buffer, words));
                }
            }
            layers.push(LayerWeights {
                layer,
                hc_attn,
                hc_ffn,
                attn_norm: loader.f32s(&name("attn_norm.weight"), &[h], true)?,
                ffn_norm: loader.f32s(&name("ffn_norm.weight"), &[h], true)?,
                wq_a: loader.packed(&name("attn.wq_a"), fp8, shape.q_rank, h)?,
                q_norm: loader.f32s(&name("attn.q_norm.weight"), &[shape.q_rank], true)?,
                wq_b: loader.packed(&name("attn.wq_b"), fp8, heads_width, shape.q_rank)?,
                wkv: loader.packed(&name("attn.wkv"), fp8, g.head_dim, h)?,
                kv_norm: loader.f32s(&name("attn.kv_norm.weight"), &[g.head_dim], true)?,
                sinks: loader.f32s(&name("attn.attn_sink"), &[g.heads], false)?,
                wo_a: loader.upload(&wo_a.bytes)?,
                wo_a_scales: loader.upload(&wo_a_scales.bytes)?,
                wo_b: loader.packed(&name("attn.wo_b"), fp8, h, wo_rows)?,
                compressor,
                indexer,
                gate: loader.bf16_words(&name("ffn.gate.weight"), &[shape.router.experts, h])?,
                bias: loader.f32s(&name("ffn.gate.bias"), &[shape.router.experts], false)?,
                w1,
                w2,
                w3,
                shared: [
                    loader.packed(&name("ffn.shared_experts.w1"), fp8, shape.expert_width, h)?,
                    loader.packed(&name("ffn.shared_experts.w2"), fp8, h, shape.expert_width)?,
                    loader.packed(&name("ffn.shared_experts.w3"), fp8, shape.expert_width, h)?,
                ],
                engram,
                ring: loader.state_zeros(g.window * g.head_dim * 2)?,
                mixes_attn: loader.state_zeros(4 * shape.hc.mixes())?,
                mixes_ffn: loader.state_zeros(4 * shape.hc.mixes())?,
                observed,
            });
            let state_bytes = loader.state - layer_state;
            memory.layers.push(V41LayerMemory {
                layer,
                weight_bytes: loader.bytes - layer_bytes - state_bytes,
                routed_expert_bytes: routed_bytes,
                parameters: loader.params - layer_params,
                routed_expert_parameters: routed_params,
                state_bytes,
            });
        }
        let (bytes, params) = (loader.bytes, loader.params);
        let embed = if config.embeds() {
            Some(loader.bf16_words("embed.weight", &[shape.vocabulary, h])?)
        } else {
            None
        };
        memory.embedding_bytes = loader.bytes - bytes;
        memory.embedding_parameters = loader.params - params;
        let (bytes, params) = (loader.bytes, loader.params);
        let head = if config.samples() {
            Some((
                loader.bf16_words("head.weight", &[shape.vocabulary, h])?,
                loader.f32s("norm.weight", &[h], true)?,
            ))
        } else {
            None
        };
        memory.head_bytes = loader.bytes - bytes;
        memory.head_parameters = loader.params - params;
        let weight_bytes = loader.bytes;

        // Shared request state.
        let mut sources = BTreeMap::new();
        for layer in config.layers.clone() {
            if let Some(owner) = last_source(layer).filter(|_| v41_compress_ratio(layer) != Some(0))
            {
                if sources.contains_key(&owner) {
                    continue;
                }
                let ratio = v41_compress_ratio(owner).ok_or_else(|| invalid("source"))?;
                let capacity = config.max_context / ratio + 1;
                let state_bytes = 4 * 2 * ratio * g.head_dim;
                let state = if config.layers.contains(&owner) {
                    Some(loader.zeros(state_bytes)?)
                } else {
                    None
                };
                memory.sources.push(V41SourceMemory {
                    layer: owner,
                    ratio,
                    capacity,
                    kv_bytes: (capacity * g.head_dim * 2).max(4),
                    index_key_bytes: (capacity * g.index_dim * 2).max(4),
                    compressor_state_bytes: if state.is_some() {
                        state_bytes.max(4)
                    } else {
                        0
                    },
                });
                sources.insert(
                    owner,
                    SourceState {
                        kv: loader.zeros(capacity * g.head_dim * 2)?,
                        index_keys: loader.zeros(capacity * g.index_dim * 2)?,
                        capacity,
                        ratio,
                        state,
                    },
                );
            }
        }
        let selections_start = loader.bytes;
        let mut selections = BTreeMap::new();
        for layer in config.layers.clone() {
            if let Some(owners) = V41LayerOwners::for_layer(layer) {
                if owners.kv.is_some() {
                    if let Some(index) = owners.index {
                        if !selections.contains_key(&index) {
                            selections.insert(index, loader.zeros(4 * g.index_top_k)?);
                        }
                    }
                }
            }
        }
        memory.selection_bytes = loader.bytes - selections_start;
        let constants_start = loader.bytes;
        let mut rope_window =
            loader.floats(&v41_rope_frequencies(V41RopeConfig::window(g.rope_dim))?)?;
        let mut rope_compressed = loader.floats(&v41_rope_frequencies(
            V41RopeConfig::compressed(g.rope_dim),
        )?)?;
        if config.swap_rope_tables {
            std::mem::swap(&mut rope_window, &mut rope_compressed);
        }
        let (engram_map, engram_params) = if engram_layers.is_empty() {
            (None, None)
        } else {
            let map = config
                .token_map
                .as_ref()
                .ok_or_else(|| invalid("token map"))?;
            let mut params = Vec::new();
            for layer in &engram_layers {
                let hash = config
                    .engram
                    .iter()
                    .find(|e| e.layer == *layer)
                    .ok_or_else(|| invalid("hash constants"))?;
                params.extend_from_slice(&hash.multipliers);
                params.extend_from_slice(&hash.primes);
                params.extend_from_slice(&hash.offsets);
            }
            (
                Some(loader.upload(&le_bytes(&map.ids, u32::to_le_bytes))?),
                Some(loader.upload(&le_bytes(&params, u64::to_le_bytes))?),
            )
        };
        memory.constant_bytes = loader.bytes - constants_start;
        let max_capacity = sources.values().map(|s| s.capacity).max().unwrap_or(1);
        let block_capacity = max_capacity.div_ceil(shape.candidate_block_size);
        let t = config.max_step_tokens;
        let top_k = shape.router.top_k;
        let e = shape.expert_width;
        let max_k = [
            h,
            shape.q_rank,
            shape.o_groups * shape.o_rank,
            V41_ENGRAM_COLUMNS * shape.engram_width,
        ]
        .into_iter()
        .max()
        .unwrap_or(h);
        let work_start = loader.bytes;
        // The step buffers below, per token of `max_step_tokens`.
        let step_floats = 1 + 2 * s * h + 2 * s + 2 * g.index_top_k + 2 * shape.candidate_blocks;
        memory.work_step_bytes = 4 * step_floats * t;
        let mut f = |n: usize| loader.zeros(4 * n);
        let work = Work {
            index: f(1)?,
            base: f(1)?,
            positions: f(1)?,
            tokens: f(t)?,
            residual: [f(s * h)?, f(s * h)?],
            pre_in: f(shape.hc.mixes())?,
            scratch: f(shape.hc.scratch_floats())?,
            collapsed: f(h)?,
            normalized: f(h)?,
            act_codes: f(max_k.div_ceil(4))?,
            act_scales: f(max_k.div_ceil(V41_BLOCK * 4) + 1)?,
            act2_codes: f((top_k * e).max(max_k).div_ceil(4))?,
            act2_scales: f((top_k * e).max(max_k).div_ceil(V41_BLOCK * 4) + 1)?,
            q_low: f(shape.q_rank)?,
            q_norm: f(shape.q_rank)?,
            q_heads: f(g.heads * g.head_dim)?,
            kv: f(g.head_dim)?,
            kv_norm: f(g.head_dim)?,
            values: f(g.head_dim)?,
            gates: f(g.head_dim)?,
            latents: f(g.head_dim)?,
            index_query: f(g.index_heads * g.index_dim)?,
            index_weights: f(g.index_heads)?,
            scores: f(max_capacity)?,
            block_scores: f(block_capacity)?,
            flags: f(block_capacity.div_ceil(4))?,
            blocks: f(shape.candidate_blocks)?,
            block_capacity,
            heads: f(g.heads * g.head_dim)?,
            heads_q: f(g.heads * g.head_dim)?,
            low_rank: f(shape.o_groups * shape.o_rank)?,
            attn_out: f(h)?,
            logits: f(shape.router.experts)?,
            ids: f(top_k)?,
            route_weights: f(top_k)?,
            g: f(top_k * e)?,
            u: f(top_k * e)?,
            h: f(top_k * e)?,
            y: f(top_k * h)?,
            sg: f(e)?,
            su: f(e)?,
            sh: f(e)?,
            sy: f(h)?,
            moe_out: f(h)?,
            engram_ids: f(2 * V41_ENGRAM_COLUMNS * 2)?,
            engram_slot_ids: f(V41_ENGRAM_COLUMNS * 2)?,
            engram_rows: f(V41_ENGRAM_COLUMNS * shape.engram_width)?,
            engram_projected: f(s * h + h)?,
            history: f(4)?,
            residual_in: f(t * s * h)?,
            pre_mix_in: f(t * s)?,
            residual_out: f(t * s * h)?,
            pre_mix_out: f(t * s)?,
            selection_in: f(t * g.index_top_k)?,
            candidates_in: f(t * shape.candidate_blocks)?,
            selection_out: f(t * g.index_top_k)?,
            candidates_out: f(t * shape.candidate_blocks)?,
            head_logits: f(if config.samples() {
                shape.vocabulary
            } else {
                1
            })?,
            settings: f(2)?,
            seed: f(2)?,
            token: f(1)?,
        };
        let state_bytes = loader.bytes - weight_bytes;
        memory.work_bytes = loader.bytes - work_start;
        memory.engram_host_bytes = engram_tables
            .iter()
            .map(|t| t.codes.byte_len() + t.scales.byte_len())
            .sum();
        Ok(Self {
            config,
            layers,
            sources,
            selections,
            rope_window,
            rope_compressed,
            embed,
            head,
            engram_tables,
            engram_map,
            engram_params,
            work,
            body: None,
            body_observed: None,
            tail: None,
            history: Vec::new(),
            weight_bytes,
            state_bytes,
            memory,
        })
    }

    /// The stage's configuration.
    #[must_use]
    pub fn config(&self) -> &V41StageConfig {
        &self.config
    }

    /// Device bytes of weights, and of request state and scratch.
    #[must_use]
    pub fn device_bytes(&self) -> (usize, usize) {
        (self.weight_bytes, self.state_bytes)
    }

    /// Where the device and pinned host bytes went.
    #[must_use]
    pub fn memory(&self) -> &V41StageMemory {
        &self.memory
    }

    /// The stage's page-locked Engram tables.
    #[must_use]
    pub fn engram_tables(&self) -> &[V41EngramHostTable] {
        &self.engram_tables
    }

    fn rope(&self, layer: usize) -> V41RopeTable<'_> {
        V41RopeTable {
            frequencies: if v41_compress_ratio(layer) == Some(0) {
                &self.rope_window
            } else {
                &self.rope_compressed
            },
            dim: self.config.shape.attention.rope_dim,
        }
    }

    fn steps(&self) -> V41Steps<'_> {
        V41Steps {
            positions: &self.work.positions,
            tokens: 1,
        }
    }

    fn observe(
        &self,
        s: &mut CudaSubmission,
        layer: &LayerWeights,
        site: V41Site,
        src: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if let Some((_, buffer, words)) = layer.observed.iter().find(|(x, _, _)| *x == site) {
            s.v41_stage_copy(
                src,
                buffer,
                &self.work.index,
                *words,
                self.config.max_step_tokens,
                true,
            )?;
        }
        Ok(())
    }

    /// Records one token's pass through the stage's layers.
    #[allow(clippy::too_many_lines)]
    fn record_body(&self, s: &mut CudaSubmission, observe: bool) -> Result<(), RuntimeError> {
        let c = &self.config;
        let shape = c.shape;
        let g = shape.attention;
        let (h, streams) = (shape.hidden, shape.hc.streams);
        let wk = &self.work;
        let t = c.max_step_tokens;
        let boundary = (c.layers.start > 0).then(|| V41Boundary::at(c.layers.start));
        let input = match &self.embed {
            Some(table) => V41StageSource::Embedding {
                table,
                vocab: shape.vocabulary,
            },
            None => V41StageSource::Previous {
                residual: &wk.residual_in,
                pre_mix: &wk.pre_mix_in,
                tokens: t,
            },
        };
        s.v41_stage_begin(
            input,
            &wk.index,
            &wk.base,
            &wk.tokens,
            t,
            &wk.residual[0],
            &wk.pre_in,
            &wk.positions,
            streams,
            h,
        )?;
        if let Some(boundary) = boundary {
            if let Some(owner) = boundary.selection {
                let target = self
                    .selections
                    .get(&owner)
                    .ok_or_else(|| invalid("selection buffer"))?;
                s.v41_stage_copy(&wk.selection_in, target, &wk.index, g.index_top_k, t, false)?;
            }
            if boundary.candidates {
                s.v41_candidate_flags(
                    &wk.candidates_in,
                    t,
                    &wk.index,
                    shape.candidate_blocks,
                    &wk.flags,
                    wk.block_capacity,
                )?;
            }
        }
        let engram_layers = c.engram_layers();
        if let (Some(map), Some(params)) = (&self.engram_map, &self.engram_params) {
            let token_map = c.token_map.as_ref().ok_or_else(|| invalid("token map"))?;
            s.v41_engram_hash(
                &wk.index,
                &wk.base,
                &wk.tokens,
                t,
                map,
                shape.vocabulary,
                token_map.pad,
                &wk.history,
                params,
                engram_layers.len(),
                shape.engram_heads,
                &wk.engram_ids,
            )?;
        }
        let mut cur = 0_usize;
        let mixes_none = &wk.pre_in;
        let mut previous: &CudaBuffer = mixes_none;
        let single = V41Slots::single();
        for lw in &self.layers {
            let layer = lw.layer;
            let ratio = v41_compress_ratio(layer).ok_or_else(|| invalid("layer"))?;
            let owners = V41LayerOwners::for_layer(layer).ok_or_else(|| invalid("layer"))?;
            let ob = |s: &mut CudaSubmission, site: V41Site, src: &CudaBuffer| {
                if observe {
                    self.observe(s, lw, site, src)
                } else {
                    Ok(())
                }
            };
            // Engram.
            if let Some(e) = &lw.engram {
                let table = self
                    .engram_tables
                    .iter()
                    .find(|t| t.layer == layer)
                    .ok_or_else(|| invalid("Engram table"))?;
                let ids = &wk.engram_slot_ids;
                s.copy_buffer_region(
                    &wk.engram_ids,
                    e.slot * V41_ENGRAM_COLUMNS * 8,
                    ids,
                    0,
                    V41_ENGRAM_COLUMNS * 8,
                )?;
                s.v41_engram_gather(
                    V41EngramTable {
                        codes: &table.codes,
                        scales: &table.scales,
                        rows: table.rows,
                        width: shape.engram_width,
                    },
                    ids,
                    V41_ENGRAM_COLUMNS,
                    &wk.engram_rows,
                    V41Output::Bf16,
                )?;
                ob(s, V41Site::EngramIds, ids)?;
                ob(s, V41Site::EngramRows, &wk.engram_rows)?;
                let k = V41_ENGRAM_COLUMNS * shape.engram_width;
                s.v41_act_quant_fp8(&wk.engram_rows, &wk.act_codes, &wk.act_scales, 1, k)?;
                s.v41_block_matvec(
                    e.wkv.matrix(),
                    &wk.act_codes,
                    &wk.act_scales,
                    single,
                    &wk.engram_projected,
                    V41Output::Bf16,
                )?;
                s.v41_engram_gate(
                    V41EngramGate {
                        streams,
                        width: h,
                        epsilon: g.norm_epsilon,
                    },
                    &wk.residual[cur],
                    &wk.engram_projected,
                    &e.q,
                    &e.k,
                    &wk.residual[1 - cur],
                    V41Output::Bf16,
                )?;
                cur = 1 - cur;
                ob(s, V41Site::EngramOut, &wk.residual[cur])?;
            }
            ob(s, V41Site::HcIn, &wk.residual[cur])?;
            // Attention sub-block.
            s.v41_hc_mixes(
                shape.hc,
                lw.hc_attn.weights(),
                &wk.residual[cur],
                &wk.scratch,
                &lw.mixes_attn,
            )?;
            s.v41_hc_collapse(
                shape.hc,
                &wk.residual[cur],
                previous,
                &wk.collapsed,
                V41Output::Bf16,
            )?;
            s.v41_rms_norm(
                &wk.collapsed,
                &lw.attn_norm,
                &wk.normalized,
                1,
                h,
                g.norm_epsilon,
                V41Output::Bf16,
            )?;
            ob(s, V41Site::AttnMixes, &lw.mixes_attn)?;
            ob(s, V41Site::AttnIn, &wk.normalized)?;
            self.record_attention(s, lw, ratio, owners, observe)?;
            ob(s, V41Site::AttnOut, &wk.attn_out)?;
            s.v41_hc_expand(
                shape.hc,
                &wk.attn_out,
                &wk.residual[cur],
                &lw.mixes_attn,
                &wk.residual[1 - cur],
                V41Output::Bf16,
            )?;
            cur = 1 - cur;
            ob(s, V41Site::AttnExpanded, &wk.residual[cur])?;
            // FFN sub-block.
            s.v41_hc_mixes(
                shape.hc,
                lw.hc_ffn.weights(),
                &wk.residual[cur],
                &wk.scratch,
                &lw.mixes_ffn,
            )?;
            s.v41_hc_collapse(
                shape.hc,
                &wk.residual[cur],
                &lw.mixes_attn,
                &wk.collapsed,
                V41Output::Bf16,
            )?;
            s.v41_rms_norm(
                &wk.collapsed,
                &lw.ffn_norm,
                &wk.normalized,
                1,
                h,
                g.norm_epsilon,
                V41Output::Bf16,
            )?;
            ob(s, V41Site::FfnMixes, &lw.mixes_ffn)?;
            ob(s, V41Site::FfnIn, &wk.normalized)?;
            self.record_moe(s, lw)?;
            ob(s, V41Site::ExpertIds, &wk.ids)?;
            ob(s, V41Site::RouteWeights, &wk.route_weights)?;
            ob(s, V41Site::FfnOut, &wk.moe_out)?;
            s.v41_hc_expand(
                shape.hc,
                &wk.moe_out,
                &wk.residual[cur],
                &lw.mixes_ffn,
                &wk.residual[1 - cur],
                V41Output::Bf16,
            )?;
            cur = 1 - cur;
            ob(s, V41Site::HcOut, &wk.residual[cur])?;
            previous = &lw.mixes_ffn;
        }
        // The final residual stays in `residual[0]` for the tail graph.
        if cur != 0 {
            s.copy_buffer(&wk.residual[1], &wk.residual[0])?;
        }
        if !c.samples() {
            s.v41_stage_copy(
                &wk.residual[0],
                &wk.residual_out,
                &wk.index,
                streams * h,
                t,
                true,
            )?;
            s.v41_stage_copy(previous, &wk.pre_mix_out, &wk.index, streams, t, true)?;
            let out = V41Boundary::at(c.layers.end);
            if let Some(owner) = out.selection {
                let selection = self
                    .selections
                    .get(&owner)
                    .ok_or_else(|| invalid("selection buffer"))?;
                s.v41_stage_copy(
                    selection,
                    &wk.selection_out,
                    &wk.index,
                    g.index_top_k,
                    t,
                    true,
                )?;
            }
            if out.candidates && c.layers.contains(&V41_STAGE_CANDIDATE_SOURCE) {
                s.v41_stage_copy(
                    &wk.blocks,
                    &wk.candidates_out,
                    &wk.index,
                    shape.candidate_blocks,
                    t,
                    true,
                )?;
            }
        } else {
            // Keep the pre-mix for the final collapse.
            s.copy_buffer(previous, &wk.pre_in)?;
        }
        s.v41_stage_advance(&wk.index)
    }

    #[allow(clippy::too_many_lines)]
    fn record_attention(
        &self,
        s: &mut CudaSubmission,
        lw: &LayerWeights,
        ratio: usize,
        owners: V41LayerOwners,
        observe: bool,
    ) -> Result<(), RuntimeError> {
        let shape = self.config.shape;
        let g = shape.attention;
        let h = shape.hidden;
        let wk = &self.work;
        let layer = lw.layer;
        let single = V41Slots::single();
        let steps = self.steps();
        let rope = self.rope(layer);
        s.v41_act_quant_fp8(&wk.normalized, &wk.act_codes, &wk.act_scales, 1, h)?;
        s.v41_block_matvec(
            lw.wq_a.matrix(),
            &wk.act_codes,
            &wk.act_scales,
            single,
            &wk.q_low,
            V41Output::Bf16,
        )?;
        s.v41_block_matvec(
            lw.wkv.matrix(),
            &wk.act_codes,
            &wk.act_scales,
            single,
            &wk.kv,
            V41Output::Bf16,
        )?;
        s.v41_rms_norm(
            &wk.q_low,
            &lw.q_norm,
            &wk.q_norm,
            1,
            shape.q_rank,
            g.norm_epsilon,
            V41Output::Bf16,
        )?;
        s.v41_act_quant_fp8(&wk.q_norm, &wk.act2_codes, &wk.act2_scales, 1, shape.q_rank)?;
        s.v41_block_matvec(
            lw.wq_b.matrix(),
            &wk.act2_codes,
            &wk.act2_scales,
            single,
            &wk.q_heads,
            V41Output::Bf16,
        )?;
        s.v41_rms_norm(
            &wk.kv,
            &lw.kv_norm,
            &wk.kv_norm,
            1,
            g.head_dim,
            g.norm_epsilon,
            V41Output::Bf16,
        )?;
        s.v41_rope(
            rope,
            &wk.q_heads,
            steps,
            g.heads,
            g.head_dim,
            0,
            false,
            V41Output::Bf16,
        )?;
        s.v41_rope(
            rope,
            &wk.kv_norm,
            steps,
            1,
            g.head_dim,
            0,
            false,
            V41Output::Bf16,
        )?;
        s.v41_window_store(
            g,
            &wk.kv_norm,
            steps,
            V41Ring {
                buffer: &lw.ring,
                rows: g.window,
            },
        )?;
        if let Some(cw) = &lw.compressor {
            let source = self.sources.get(&layer).ok_or_else(|| invalid("source"))?;
            let state = source
                .state
                .as_ref()
                .ok_or_else(|| invalid("compressor state"))?;
            let dense = |w| V41Dense {
                weights: w,
                scales: None,
                format: cw.format,
                rows: g.head_dim,
                cols: h,
                groups: 1,
            };
            let (round_input, output) = if ratio > 1 {
                (false, V41Output::F32)
            } else {
                (true, V41Output::Bf16)
            };
            s.v41_dense(
                dense(&cw.wkv),
                &wk.normalized,
                &wk.values,
                1,
                round_input,
                output,
            )?;
            if let Some(wgate) = &cw.wgate {
                s.v41_dense(
                    dense(wgate),
                    &wk.normalized,
                    &wk.gates,
                    1,
                    round_input,
                    output,
                )?;
            }
            s.v41_compress(
                V41Compressor {
                    ratio,
                    width: g.head_dim,
                    epsilon: g.norm_epsilon,
                },
                &wk.values,
                (ratio > 1).then_some(&wk.gates),
                &cw.norm,
                state,
                steps,
                &wk.latents,
            )?;
            s.v41_compressed_store(
                g,
                ratio,
                V41IndexKeyWeights {
                    wk: &cw.wk,
                    k_norm: &cw.k_norm,
                },
                rope,
                &wk.latents,
                steps,
                source.source(),
            )?;
        }
        let ring = V41Ring {
            buffer: &lw.ring,
            rows: g.window,
        };
        let compressed = match owners.kv {
            Some(owner) => {
                let source = self
                    .sources
                    .get(&owner)
                    .ok_or_else(|| invalid("KV owner"))?;
                let index_owner = owners.index.ok_or_else(|| invalid("index owner"))?;
                let selection = self
                    .selections
                    .get(&index_owner)
                    .ok_or_else(|| invalid("selection"))?;
                if let Some(ix) = &lw.indexer {
                    s.v41_block_matvec(
                        ix.wq_b.matrix(),
                        &wk.act2_codes,
                        &wk.act2_scales,
                        single,
                        &wk.index_query,
                        V41Output::Bf16,
                    )?;
                    s.v41_rope(
                        rope,
                        &wk.index_query,
                        steps,
                        g.index_heads,
                        g.index_dim,
                        0,
                        false,
                        V41Output::Bf16,
                    )?;
                    s.v41_cache_quant(
                        V41CacheQuant::IndexFp4,
                        &wk.index_query,
                        &wk.index_query,
                        g.index_heads,
                        g.index_dim,
                    )?;
                    s.v41_dense(
                        V41Dense {
                            weights: &ix.weights_proj,
                            scales: None,
                            format: V41DenseFormat::Bf16,
                            rows: g.index_heads,
                            cols: h,
                            groups: 1,
                        },
                        &wk.normalized,
                        &wk.index_weights,
                        1,
                        true,
                        V41Output::Bf16,
                    )?;
                    let scores = V41Rows {
                        buffer: &wk.scores,
                        stride: source.capacity,
                    };
                    s.v41_index_scores(
                        g,
                        &wk.index_query,
                        &wk.index_weights,
                        source.source(),
                        source.ratio,
                        steps,
                        scores,
                    )?;
                    if layer == V41_STAGE_CANDIDATE_SOURCE {
                        s.v41_candidate_blocks(
                            scores,
                            source.ratio,
                            steps,
                            V41Candidates {
                                block_scores: &wk.block_scores,
                                flags: &wk.flags,
                                blocks: &wk.blocks,
                                stride: wk.block_capacity,
                                blocks_stride: shape.candidate_blocks,
                            },
                            shape.candidate_blocks,
                            shape.candidate_block_size,
                        )?;
                    }
                    s.v41_select_indices(
                        scores,
                        source.ratio,
                        steps,
                        (layer > V41_STAGE_CANDIDATE_SOURCE).then_some((
                            V41Rows {
                                buffer: &wk.flags,
                                stride: wk.block_capacity,
                            },
                            shape.candidate_block_size,
                        )),
                        g.index_top_k,
                        V41Rows {
                            buffer: selection,
                            stride: g.index_top_k,
                        },
                    )?;
                    if observe {
                        let lw_observed = lw;
                        self.observe(s, lw_observed, V41Site::Selection, selection)?;
                    }
                }
                Some(V41Compressed {
                    kv: &source.kv,
                    capacity: source.capacity,
                    indices: V41Rows {
                        buffer: selection,
                        stride: g.index_top_k,
                    },
                    ratio: source.ratio,
                })
            }
            None => None,
        };
        s.v41_sparse_attention(
            g,
            &wk.q_heads,
            &lw.sinks,
            ring,
            compressed,
            steps,
            &wk.heads,
        )?;
        s.v41_rope(
            rope,
            &wk.heads,
            steps,
            g.heads,
            g.head_dim,
            0,
            true,
            V41Output::Bf16,
        )?;
        s.v41_cache_quant(
            V41CacheQuant::Fp8,
            &wk.heads,
            &wk.heads_q,
            g.heads,
            g.head_dim,
        )?;
        let wo_rows = shape.o_groups * shape.o_rank;
        s.v41_dense(
            V41Dense {
                weights: &lw.wo_a,
                scales: Some(&lw.wo_a_scales),
                format: V41DenseFormat::Fp8Block,
                rows: wo_rows,
                cols: g.heads * g.head_dim / shape.o_groups,
                groups: shape.o_groups,
            },
            &wk.heads_q,
            &wk.low_rank,
            1,
            false,
            V41Output::Bf16,
        )?;
        s.v41_act_quant_fp8(&wk.low_rank, &wk.act_codes, &wk.act_scales, 1, wo_rows)?;
        s.v41_block_matvec(
            lw.wo_b.matrix(),
            &wk.act_codes,
            &wk.act_scales,
            single,
            &wk.attn_out,
            V41Output::Bf16,
        )
    }

    fn record_moe(&self, s: &mut CudaSubmission, lw: &LayerWeights) -> Result<(), RuntimeError> {
        let shape = self.config.shape;
        let wk = &self.work;
        let (h, e, top_k) = (shape.hidden, shape.expert_width, shape.router.top_k);
        let routed = |shared_input| V41Slots {
            count: top_k,
            ids: Some(&wk.ids),
            shared_input,
        };
        let single = V41Slots::single();
        s.v41_gate_logits(
            &lw.gate,
            &wk.normalized,
            &wk.logits,
            shape.router.experts,
            h,
        )?;
        s.v41_route(
            shape.router,
            &wk.logits,
            &lw.bias,
            &wk.ids,
            &wk.route_weights,
        )?;
        s.v41_act_quant_fp8(&wk.normalized, &wk.act_codes, &wk.act_scales, 1, h)?;
        s.v41_block_matvec(
            lw.w1.matrix(),
            &wk.act_codes,
            &wk.act_scales,
            routed(true),
            &wk.g,
            V41Output::Bf16,
        )?;
        s.v41_block_matvec(
            lw.w3.matrix(),
            &wk.act_codes,
            &wk.act_scales,
            routed(true),
            &wk.u,
            V41Output::Bf16,
        )?;
        s.v41_swiglu(
            &wk.g,
            &wk.u,
            Some(&wk.route_weights),
            top_k,
            e,
            shape.swiglu_limit,
            &wk.h,
            V41Output::Bf16,
        )?;
        s.v41_act_quant_fp8(&wk.h, &wk.act2_codes, &wk.act2_scales, top_k, e)?;
        s.v41_block_matvec(
            lw.w2.matrix(),
            &wk.act2_codes,
            &wk.act2_scales,
            routed(false),
            &wk.y,
            V41Output::Bf16,
        )?;
        s.v41_block_matvec(
            lw.shared[0].matrix(),
            &wk.act_codes,
            &wk.act_scales,
            single,
            &wk.sg,
            V41Output::Bf16,
        )?;
        s.v41_block_matvec(
            lw.shared[2].matrix(),
            &wk.act_codes,
            &wk.act_scales,
            single,
            &wk.su,
            V41Output::Bf16,
        )?;
        s.v41_swiglu(
            &wk.sg,
            &wk.su,
            None,
            1,
            e,
            shape.swiglu_limit,
            &wk.sh,
            V41Output::Bf16,
        )?;
        s.v41_act_quant_fp8(&wk.sh, &wk.act2_codes, &wk.act2_scales, 1, e)?;
        s.v41_block_matvec(
            lw.shared[1].matrix(),
            &wk.act2_codes,
            &wk.act2_scales,
            single,
            &wk.sy,
            V41Output::Bf16,
        )?;
        s.v41_moe_combine(&wk.y, top_k, Some(&wk.sy), h, &wk.moe_out, V41Output::Bf16)
    }

    /// Records the last stage's tail: the final collapse with the last
    /// pre-mix, the norm, the head, and sampling.
    fn record_tail(&self, s: &mut CudaSubmission) -> Result<(), RuntimeError> {
        let shape = self.config.shape;
        let wk = &self.work;
        let (head, norm) = self.head.as_ref().ok_or_else(|| invalid("head"))?;
        s.v41_hc_collapse(
            shape.hc,
            &wk.residual[0],
            &wk.pre_in,
            &wk.collapsed,
            V41Output::Bf16,
        )?;
        s.v41_rms_norm(
            &wk.collapsed,
            norm,
            &wk.normalized,
            1,
            shape.hidden,
            shape.attention.norm_epsilon,
            V41Output::Bf16,
        )?;
        s.v41_gate_logits(
            head,
            &wk.normalized,
            &wk.head_logits,
            shape.vocabulary,
            shape.hidden,
        )?;
        s.v41_sample(
            &wk.head_logits,
            shape.vocabulary,
            &wk.settings,
            &wk.seed,
            &wk.positions,
            &wk.token,
        )
    }

    /// Runs one step. With `observe`, the per-layer observations of each
    /// token are kept for [`Self::observations`] (the stage must be loaded
    /// with `observe`).
    ///
    /// # Errors
    ///
    /// Refuses input that doesn't match the stage's boundary or capacity,
    /// and any device failure. A failed step leaves the request unusable:
    /// start it again from position 0.
    #[allow(clippy::too_many_lines)]
    pub fn step(
        &mut self,
        backend: &CudaBackend,
        input: &V41StepInput<'_>,
        observe: bool,
    ) -> Result<V41StepOutput, RuntimeError> {
        self.step_with_output(backend, input, observe, true)
    }

    /// Updates state for a chunk, omitting the vocabulary head and sampler
    /// when more prompt chunks follow. State arithmetic stays unchanged.
    pub fn step_with_output(
        &mut self,
        backend: &CudaBackend,
        input: &V41StepInput<'_>,
        observe: bool,
        emit: bool,
    ) -> Result<V41StepOutput, RuntimeError> {
        let started = Instant::now();
        let c = self.config.clone();
        let shape = c.shape;
        let g = shape.attention;
        let (h, streams) = (shape.hidden, shape.hc.streams);
        let t = input.tokens.len();
        if t == 0
            || t > c.max_step_tokens
            || input.position + t > c.max_context
            || input.tokens.iter().any(|x| *x as usize >= shape.vocabulary)
        {
            return Err(invalid(format!(
                "{t} tokens at {} (step limit {}, context {})",
                input.position, c.max_step_tokens, c.max_context
            )));
        }
        if observe && !c.observe {
            return Err(invalid("the stage was loaded without observations"));
        }
        let mut stats = V41StepStats {
            tokens: t,
            ..V41StepStats::default()
        };
        let upload =
            |stats: &mut V41StepStats, buffer: &mut CudaBuffer, offset: usize, bytes: &[u8]| {
                stats.host_to_device_copies += 1;
                stats.host_to_device_bytes += bytes.len();
                buffer.write_bytes_at_offset(offset, bytes)
            };
        // The step's input: one upload each.
        upload(&mut stats, &mut self.work.index, 0, &0_i32.to_le_bytes())?;
        upload(
            &mut stats,
            &mut self.work.base,
            0,
            &i32::try_from(input.position)
                .map_err(|_| invalid("position"))?
                .to_le_bytes(),
        )?;
        let tokens: Vec<u8> = input.tokens.iter().flat_map(|x| x.to_le_bytes()).collect();
        upload(&mut stats, &mut self.work.tokens, 0, &tokens)?;
        if input.position == 0 {
            self.history.clear();
        }
        if c.layers.start > 0 {
            let residual = input.residual.ok_or_else(|| invalid("missing residual"))?;
            let pre = input.pre_mix.ok_or_else(|| invalid("missing pre-mix"))?;
            if residual.len() != t * streams * h || pre.len() != t * streams {
                return Err(invalid("residual or pre-mix shape"));
            }
            upload(
                &mut stats,
                &mut self.work.residual_in,
                0,
                &le_bytes(residual, f32::to_le_bytes),
            )?;
            upload(
                &mut stats,
                &mut self.work.pre_mix_in,
                0,
                &le_bytes(pre, f32::to_le_bytes),
            )?;
            let boundary = V41Boundary::at(c.layers.start);
            match (boundary.kv, input.source_rows) {
                (Some((layer, keys)), Some(rows)) if rows.layer == layer => {
                    let ratio = v41_compress_ratio(layer).unwrap_or(1).max(1);
                    let first = input.position / ratio;
                    let count = (input.position + t) / ratio - first;
                    if rows.first_row != first
                        || rows.kv.len() != count * g.head_dim
                        || rows.index_keys.as_ref().map(Vec::len)
                            != keys.then_some(count * g.index_dim)
                    {
                        return Err(invalid("source rows"));
                    }
                    let source = self
                        .sources
                        .get_mut(&layer)
                        .ok_or_else(|| invalid("source"))?;
                    if first + count > source.capacity {
                        return Err(invalid("source capacity"));
                    }
                    if count > 0 {
                        let kv = le_bytes(&rows.kv, u16::to_le_bytes);
                        upload(&mut stats, &mut source.kv, first * g.head_dim * 2, &kv)?;
                        if let Some(keys) = &rows.index_keys {
                            let keys = le_bytes(keys, u16::to_le_bytes);
                            upload(
                                &mut stats,
                                &mut source.index_keys,
                                first * g.index_dim * 2,
                                &keys,
                            )?;
                        }
                    }
                }
                (None, None) => {}
                _ => return Err(invalid("the boundary's source rows")),
            }
            match (boundary.selection, input.selection) {
                (Some(_), Some(rows)) if rows.len() == t * g.index_top_k => {
                    upload(
                        &mut stats,
                        &mut self.work.selection_in,
                        0,
                        &le_bytes(rows, i32::to_le_bytes),
                    )?;
                }
                (None, None) => {}
                _ => return Err(invalid("the boundary's selection")),
            }
            match (boundary.candidates, input.candidates) {
                (true, Some(rows)) if rows.len() == t * shape.candidate_blocks => {
                    upload(
                        &mut stats,
                        &mut self.work.candidates_in,
                        0,
                        &le_bytes(rows, i32::to_le_bytes),
                    )?;
                }
                (false, None) => {}
                _ => return Err(invalid("the boundary's candidates")),
            }
        }
        if c.samples() && emit {
            let s = input.sampling;
            let mut settings = s.temperature.to_le_bytes().to_vec();
            settings.extend_from_slice(&s.top_p.to_le_bytes());
            upload(&mut stats, &mut self.work.settings, 0, &settings)?;
            upload(&mut stats, &mut self.work.seed, 0, &s.seed.to_le_bytes())?;
        }

        // One graph per token, enqueued back to back.
        let device_started = Instant::now();
        let mut remaining = t;
        let graph = if observe {
            &self.body_observed
        } else {
            &self.body
        };
        if graph.is_none() {
            let mut submission = backend.begin_captured_submission()?;
            self.record_body(&mut submission, observe)?;
            let (_, exec) = submission.commit_captured(crate::CudaCommandWait::Completed)?;
            stats.graph_captures += 1;
            stats.graph_launches += 1;
            stats.synchronizations += 1;
            remaining -= 1;
            if observe {
                self.body_observed = Some(exec);
            } else {
                self.body = Some(exec);
            }
        }
        let graph = if observe {
            &self.body_observed
        } else {
            &self.body
        }
        .as_ref()
        .ok_or_else(|| invalid("graph"))?;
        for _ in 0..remaining {
            graph.launch_enqueued()?;
            stats.graph_launches += 1;
        }
        if c.samples() && emit {
            if self.tail.is_none() {
                let mut submission = backend.begin_captured_submission()?;
                self.record_tail(&mut submission)?;
                let (_, exec) = submission.commit_captured(crate::CudaCommandWait::Completed)?;
                stats.graph_captures += 1;
                stats.synchronizations += 1;
                self.tail = Some(exec);
            } else {
                self.tail
                    .as_ref()
                    .ok_or_else(|| invalid("tail"))?
                    .launch_enqueued()?;
            }
            stats.graph_launches += 1;
        }
        let graph = if observe {
            &self.body_observed
        } else {
            &self.body
        }
        .as_ref()
        .ok_or_else(|| invalid("graph"))?;
        graph.synchronize()?;
        stats.synchronizations += 1;
        stats.device_seconds = device_started.elapsed().as_secs_f64();

        // The step's output: one read each.
        let mut out = V41StepOutput::default();
        let read = |stats: &mut V41StepStats, buffer: &CudaBuffer, bytes: usize| {
            stats.device_to_host_copies += 1;
            stats.device_to_host_bytes += bytes;
            buffer.read_bytes_at_offset(0, bytes)
        };
        let history_before = self.history.clone();
        self.history.extend_from_slice(input.tokens);
        let keep = self.history.len().saturating_sub(3);
        self.history.drain(..keep);
        if c.samples() {
            if emit {
                let bytes = read(&mut stats, &self.work.token, 4)?;
                let token = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                out.token = Some(u32::try_from(token).map_err(|_| invalid("nonfinite logits"))?);
            }
        } else {
            let to_f32 = |b: Vec<u8>| -> Vec<f32> {
                b.chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect()
            };
            let to_i32 = |b: Vec<u8>| -> Vec<i32> {
                b.chunks_exact(4)
                    .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect()
            };
            out.residual = to_f32(read(
                &mut stats,
                &self.work.residual_out,
                4 * t * streams * h,
            )?);
            out.pre_mix = to_f32(read(&mut stats, &self.work.pre_mix_out, 4 * t * streams)?);
            let boundary = V41Boundary::at(c.layers.end);
            if let Some((layer, keys)) = boundary.kv {
                let source = self.sources.get(&layer).ok_or_else(|| invalid("source"))?;
                let ratio = source.ratio.max(1);
                let first = input.position / ratio;
                let count = (input.position + t) / ratio - first;
                let words = |stats: &mut V41StepStats,
                             buffer: &CudaBuffer,
                             width: usize|
                 -> Result<Vec<u16>, RuntimeError> {
                    if count == 0 {
                        return Ok(Vec::new());
                    }
                    stats.device_to_host_copies += 1;
                    stats.device_to_host_bytes += count * width * 2;
                    Ok(buffer
                        .read_bytes_at_offset(first * width * 2, count * width * 2)?
                        .chunks_exact(2)
                        .map(|w| u16::from_le_bytes([w[0], w[1]]))
                        .collect())
                };
                out.source_rows = Some(V41SourceRows {
                    layer,
                    first_row: first,
                    kv: words(&mut stats, &source.kv, g.head_dim)?,
                    index_keys: if keys {
                        Some(words(&mut stats, &source.index_keys, g.index_dim)?)
                    } else {
                        None
                    },
                });
            }
            if let Some(owner) = boundary.selection {
                let rows = if c.layers.contains(&owner) {
                    to_i32(read(
                        &mut stats,
                        &self.work.selection_out,
                        4 * t * g.index_top_k,
                    )?)
                } else {
                    input
                        .selection
                        .ok_or_else(|| invalid("selection to forward"))?
                        .to_vec()
                };
                out.selection = Some((owner, rows));
            }
            if boundary.candidates {
                out.candidates = Some(if c.layers.contains(&V41_STAGE_CANDIDATE_SOURCE) {
                    to_i32(read(
                        &mut stats,
                        &self.work.candidates_out,
                        4 * t * shape.candidate_blocks,
                    )?)
                } else {
                    input
                        .candidates
                        .ok_or_else(|| invalid("candidates to forward"))?
                        .to_vec()
                });
            }
            // Exactly three entries, oldest first; `u32::MAX` before the
            // sequence start, as W9's synthetic stages write it.
            let mut history = vec![u32::MAX; 3 - history_before.len().min(3)];
            history.extend_from_slice(&history_before);
            out.engram_history = history;
        }
        stats.seconds = started.elapsed().as_secs_f64();
        out.stats = stats;
        Ok(out)
    }

    /// The observations of the last observed step: per stage layer, per
    /// site, `tokens × words` values (integers as their bit patterns).
    ///
    /// # Errors
    ///
    /// Any device read failure.
    pub fn observations(
        &self,
        tokens: usize,
    ) -> Result<BTreeMap<usize, BTreeMap<V41Site, Vec<u32>>>, RuntimeError> {
        let mut out = BTreeMap::new();
        for lw in &self.layers {
            let mut sites = BTreeMap::new();
            for (site, buffer, words) in &lw.observed {
                let bytes = buffer.read_bytes_at_offset(0, 4 * words * tokens)?;
                sites.insert(
                    *site,
                    bytes
                        .chunks_exact(4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect(),
                );
            }
            out.insert(lw.layer, sites);
        }
        Ok(out)
    }

    /// The last stage's logits of the last token of the last step.
    ///
    /// # Errors
    ///
    /// Refuses a stage that doesn't sample, and any device read failure.
    pub fn logits(&self) -> Result<Vec<f32>, RuntimeError> {
        if !self.config.samples() {
            return Err(invalid("only the last stage has logits"));
        }
        words_f32(&self.work.head_logits, self.config.shape.vocabulary)
    }

    /// The final collapse and norm of the last token, from the last stage.
    ///
    /// # Errors
    ///
    /// Any device read failure.
    pub fn final_norm(&self) -> Result<Vec<f32>, RuntimeError> {
        words_f32(&self.work.normalized, self.config.shape.hidden)
    }

    /// The index selection the stage holds for `layer`'s consumers.
    ///
    /// # Errors
    ///
    /// Refuses a layer without one, and any device read failure.
    pub fn selection(&self, layer: usize) -> Result<Vec<i32>, RuntimeError> {
        let buffer = self
            .selections
            .get(&layer)
            .ok_or_else(|| invalid(format!("no selection for layer {layer}")))?;
        words_i32(buffer, self.config.shape.attention.index_top_k)
    }
}

#[cfg(test)]
#[path = "v41_stage_tests.rs"]
mod tests;
