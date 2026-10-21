//! A DeepSeek-V4.1 stage on an Apple GPU (#12420): a contiguous layer range
//! whose whole step runs on the device, a port of
//! `psionic_backend_cuda::v41_stage::V41Stage` (#12394).
//!
//! [`V41MetalStage::load`] reads the stage's weights in their checkpoint form
//! (FP8 and FP4 blocks stay packed) into unified memory and allocates every
//! piece of request state there, exactly as the CUDA stage lays it out. The
//! configuration, boundary, input, output, and memory types are the CUDA
//! stage's.
//!
//! [`V41MetalStage::step`] runs one stage step: the step's inputs are written
//! once, then the recorded per-token body runs once for each token and, on
//! the last stage, the recorded tail (final collapse, norm, head, and
//! sampler) runs once, all in one command buffer with one wait. Metal has no
//! graph capture, so the stage records each sequence of dispatches the first
//! time it needs it and replays the recording afterwards, the role CUDA
//! graphs play in the CUDA stage. A token reads its index within the step
//! from a device counter, so no host round trip separates tokens.
//!
//! Each layer runs in the CUDA stage's order with the same kernels, so the
//! two stages compute the same values wherever no intermediate falls below
//! 2^-126 (Apple GPUs flush float32 subnormals); see
//! `docs/psionic/v41-metal-plan.md`.

pub use psionic_backend_cuda::v41_stage::{
    V41_ENGRAM_COLUMNS, V41_ENGRAM_LAYERS, V41_LAYERS, V41_STAGE_CANDIDATE_SOURCE,
    V41_STAGE_INDEX_SOURCES, V41_STAGE_KV_SOURCES, V41Boundary, V41EngramHash, V41HostTensor,
    V41LayerMemory, V41Sampling, V41Site, V41SourceMemory, V41SourceRows, V41StageConfig,
    V41StageMemory, V41StageShape, V41StepInput, V41StepOutput, V41StepStats, V41TensorDtype,
    V41TensorSource, V41TokenMap,
};

use super::attention::{
    V41_MAX_POSITION, V41Candidates, V41Compressed, V41Dense, V41IndexKeyWeights, V41LayerOwners,
    V41Ring, V41RopeTable, V41Rows, V41Source, V41Steps, v41_rope_table_host,
};
use super::attention::{
    V41CacheQuant, V41Compressor, V41DenseFormat, V41RopeConfig, v41_compress_ratio,
    v41_rope_frequencies,
};
use super::matvec::{V41_BLOCK, V41Matrix, V41MatvecFormat, V41Output, V41Slots};
use super::mix::{V41EngramGate, V41EngramTable, V41HcWeights};
use super::{Args, V41Buffer, V41Metal, V41Recording, V41Submission, int, le_bytes};
use psionic_runtime::RuntimeError;
use std::collections::BTreeMap;
use std::time::Instant;

const STAGE_THREADS: usize = 256;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Backend(format!("v41 Metal stage: {}", message.into()))
}

fn blocks_for(n: usize, threads: usize) -> usize {
    n.div_ceil(threads).clamp(1, 65_535)
}

// ---------------------------------------------------------------------------
// Kernel calls
// ---------------------------------------------------------------------------

/// Where [`V41Submission::v41_stage_begin`] reads a token's input, as
/// `V41StageSource`.
#[derive(Clone, Copy, Debug)]
pub enum V41StageSource<'a> {
    /// Stage 0: the token's row of the BF16 embedding.
    Embedding { table: &'a V41Buffer, vocab: usize },
    /// Later stages: row `index` of the step's residual streams and
    /// pre-mixes.
    Previous {
        residual: &'a V41Buffer,
        pre_mix: &'a V41Buffer,
        tokens: usize,
    },
}

impl V41Submission<'_> {
    /// Starts one token of a stage step, as `CudaSubmission::v41_stage_begin`.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_stage_begin(
        &mut self,
        source: V41StageSource<'_>,
        index: &V41Buffer,
        base: &V41Buffer,
        tokens: &V41Buffer,
        max_tokens: usize,
        residual: &V41Buffer,
        pre: &V41Buffer,
        positions: &V41Buffer,
        streams: usize,
        hidden: usize,
    ) -> Result<(), RuntimeError> {
        let elements = streams
            .checked_mul(hidden)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("begin shape"))?;
        let (embed, vocab, residual_in, pre_in) = match source {
            V41StageSource::Embedding { table, vocab } => (
                self.span(table, 0, vocab * hidden, 2, "embedding")?,
                vocab,
                0,
                0,
            ),
            V41StageSource::Previous {
                residual,
                pre_mix,
                tokens,
            } => (
                0,
                0,
                self.span(residual, 0, tokens * elements, 4, "residual input")?,
                self.span(pre_mix, 0, tokens * streams, 4, "pre-mix input")?,
            ),
        };
        let args = Args::new()
            .ptr(self.span(index, 0, 1, 4, "token index")?)
            .ptr(self.span(base, 0, 1, 4, "base position")?)
            .ptr(self.span(tokens, 0, max_tokens, 4, "tokens")?)
            .ptr(embed)
            .ptr(residual_in)
            .ptr(pre_in)
            .ptr(self.span(residual, 0, elements, 4, "residual")?)
            .ptr(self.span(pre, 0, streams, 4, "pre-mix")?)
            .ptr(self.span(positions, 0, 1, 4, "positions")?)
            .i32(int(vocab, "vocabulary")?)
            .i32(int(streams, "streams")?)
            .i32(int(hidden, "hidden")?);
        self.launch(
            "v41_stage_begin",
            args,
            (blocks_for(elements, STAGE_THREADS), 1, 1),
            STAGE_THREADS,
            0,
        )
    }

    /// Copies `words` words between a working buffer and row `index[0]` of a
    /// step-wide array, as `CudaSubmission::v41_stage_copy`.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    pub fn v41_stage_copy(
        &mut self,
        src: &V41Buffer,
        dst: &V41Buffer,
        index: &V41Buffer,
        words: usize,
        rows: usize,
        store: bool,
    ) -> Result<(), RuntimeError> {
        let (src_words, dst_words) = if store {
            (words, words * rows)
        } else {
            (words * rows, words)
        };
        let args = Args::new()
            .ptr(self.span(src, 0, src_words, 4, "copy source")?)
            .ptr(self.span(dst, 0, dst_words, 4, "copy destination")?)
            .ptr(self.span(index, 0, 1, 4, "token index")?)
            .i32(int(words, "words")?);
        let kernel = if store {
            "v41_stage_store"
        } else {
            "v41_stage_load"
        };
        self.launch(
            kernel,
            args,
            (blocks_for(words, STAGE_THREADS), 1, 1),
            STAGE_THREADS,
            0,
        )
    }

    /// Copies `words` words from `src` at word `src_offset` to `dst` at word
    /// `dst_offset`: a device-to-device copy.
    ///
    /// # Errors
    ///
    /// Refuses ranges outside the buffers.
    pub fn v41_copy_words(
        &mut self,
        src: &V41Buffer,
        src_offset: usize,
        dst: &V41Buffer,
        dst_offset: usize,
        words: usize,
    ) -> Result<(), RuntimeError> {
        let args = Args::new()
            .ptr(self.span(src, src_offset, words, 4, "copy source")?)
            .ptr(self.span(dst, dst_offset, words, 4, "copy destination")?)
            .i32(int(words, "words")?);
        self.launch(
            "v41_copy_words",
            args,
            (blocks_for(words, STAGE_THREADS), 1, 1),
            STAGE_THREADS,
            0,
        )
    }

    /// Adds one to the token index.
    ///
    /// # Errors
    ///
    /// Refuses an empty buffer.
    pub fn v41_stage_advance(&mut self, index: &V41Buffer) -> Result<(), RuntimeError> {
        let args = Args::new().ptr(self.span(index, 0, 1, 4, "token index")?);
        self.launch("v41_stage_advance", args, (1, 1, 1), 1, 0)
    }

    /// Hashes token `index[0]`'s n-grams for each Engram layer, as
    /// `CudaSubmission::v41_engram_hash`.
    ///
    /// # Errors
    ///
    /// Refuses more than two layers and buffers too small for the shapes.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_engram_hash(
        &mut self,
        index: &V41Buffer,
        base: &V41Buffer,
        tokens: &V41Buffer,
        max_tokens: usize,
        map: &V41Buffer,
        vocab: usize,
        pad: u32,
        history: &V41Buffer,
        params: &V41Buffer,
        layers: usize,
        heads: usize,
        ids: &V41Buffer,
    ) -> Result<(), RuntimeError> {
        if !(1..=2).contains(&layers) || heads == 0 || 3 * heads != V41_ENGRAM_COLUMNS {
            return Err(invalid(format!("{layers} Engram layers of {heads} heads")));
        }
        let columns = layers * V41_ENGRAM_COLUMNS;
        let args = Args::new()
            .ptr(self.span(index, 0, 1, 4, "token index")?)
            .ptr(self.span(base, 0, 1, 4, "base position")?)
            .ptr(self.span(tokens, 0, max_tokens, 4, "tokens")?)
            .ptr(self.span(map, 0, vocab, 4, "token map")?)
            .ptr(self.span(history, 0, 4, 4, "n-gram history")?)
            .ptr(self.span(
                params,
                0,
                layers * (4 + 2 * V41_ENGRAM_COLUMNS),
                8,
                "hash parameters",
            )?)
            .ptr(self.span(ids, 0, columns, 8, "row ids")?)
            .i32(int(vocab, "vocabulary")?)
            .i32(i32::try_from(pad).map_err(|_| invalid("pad id"))?)
            .i32(int(layers, "layers")?)
            .i32(int(heads, "heads")?);
        self.launch("v41_engram_hash", args, (1, 1, 1), 64, 0)
    }

    /// Samples one token from `vocab` logits into `token[0]`, as
    /// `CudaSubmission::v41_sample`: the same weights, top-p bisection, and
    /// draw, with the double sums in software binary64.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    pub fn v41_sample(
        &mut self,
        logits: &V41Buffer,
        vocab: usize,
        settings: &V41Buffer,
        seed: &V41Buffer,
        positions: &V41Buffer,
        token: &V41Buffer,
    ) -> Result<(), RuntimeError> {
        if vocab == 0 {
            return Err(invalid("empty vocabulary"));
        }
        let args = Args::new()
            .ptr(self.span(logits, 0, vocab, 4, "logits")?)
            .ptr(self.span(settings, 0, 2, 4, "sampling settings")?)
            .ptr(self.span(seed, 0, 1, 8, "seed")?)
            .ptr(self.span(positions, 0, 1, 4, "positions")?)
            .ptr(self.span(token, 0, 1, 4, "token")?)
            .i32(int(vocab, "vocabulary")?);
        self.launch("v41_sample", args, (1, 1, 1), 1024, 0)
    }

    /// Sets layer 20's candidate flags from a previous stage's block ids, as
    /// `CudaSubmission::v41_candidate_flags`.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    pub fn v41_candidate_flags(
        &mut self,
        blocks: &V41Buffer,
        rows: usize,
        index: &V41Buffer,
        top_blocks: usize,
        flags: &V41Buffer,
        capacity: usize,
    ) -> Result<(), RuntimeError> {
        let args = Args::new()
            .ptr(self.span(blocks, 0, rows * top_blocks, 4, "candidate blocks")?)
            .ptr(self.span(index, 0, 1, 4, "token index")?)
            .ptr(self.span(flags, 0, capacity, 1, "candidate flags")?)
            .i32(int(top_blocks, "top blocks")?)
            .i32(int(capacity, "capacity")?);
        self.launch("v41_candidate_flags", args, (1, 1, 1), STAGE_THREADS, 0)
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// `V41StageShape::check` from the CUDA backend.
fn check_shape(shape: &V41StageShape) -> Result<(), RuntimeError> {
    let g = shape.attention;
    let multiple = |n: usize| n > 0 && n.is_multiple_of(V41_BLOCK);
    if !multiple(shape.hidden)
        || !multiple(shape.q_rank)
        || shape.o_groups == 0
        || !(g.heads * g.head_dim).is_multiple_of(shape.o_groups)
        || !multiple(g.heads * g.head_dim / shape.o_groups)
        || !multiple(shape.o_groups * shape.o_rank)
        || !multiple(shape.expert_width)
        || !multiple(shape.engram_width)
        || shape.hc.width != shape.hidden
        || shape.hc.streams == 0
        || shape.vocabulary == 0
        || 3 * shape.engram_heads != V41_ENGRAM_COLUMNS
        || shape.candidate_blocks == 0
        || shape.candidate_block_size == 0
    {
        return Err(invalid(format!("model shape {shape:?}")));
    }
    Ok(())
}

fn engram_layers(config: &V41StageConfig) -> Vec<usize> {
    V41_ENGRAM_LAYERS
        .into_iter()
        .filter(|l| config.layers.contains(l))
        .collect()
}

fn last_source(layer: usize) -> Option<usize> {
    V41_STAGE_KV_SOURCES
        .iter()
        .copied()
        .filter(|s| *s <= layer)
        .last()
}

/// An Engram layer's table in unified memory: FP8 codes `[rows, width]` and
/// UE8M0 scales `[rows, width / 32]`.
#[derive(Debug)]
pub struct V41MetalEngramTable {
    pub layer: usize,
    pub codes: V41Buffer,
    pub scales: V41Buffer,
    pub rows: u64,
}

// ---------------------------------------------------------------------------
// Weights and state
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Packed {
    format: V41MatvecFormat,
    weights: V41Buffer,
    scales: V41Buffer,
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
    projection: V41Buffer,
    scale: V41Buffer,
    base: V41Buffer,
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
    wkv: V41Buffer,
    wgate: Option<V41Buffer>,
    format: V41DenseFormat,
    norm: V41Buffer,
    wk: V41Buffer,
    k_norm: V41Buffer,
}

#[derive(Debug)]
struct IndexerWeights {
    wq_b: Packed,
    weights_proj: V41Buffer,
}

#[derive(Debug)]
struct EngramWeights {
    slot: usize,
    q: V41Buffer,
    k: V41Buffer,
    wkv: Packed,
}

#[derive(Debug)]
struct LayerWeights {
    layer: usize,
    hc_attn: HcWeights,
    hc_ffn: HcWeights,
    attn_norm: V41Buffer,
    ffn_norm: V41Buffer,
    wq_a: Packed,
    q_norm: V41Buffer,
    wq_b: Packed,
    wkv: Packed,
    kv_norm: V41Buffer,
    sinks: V41Buffer,
    wo_a: V41Buffer,
    wo_a_scales: V41Buffer,
    wo_b: Packed,
    compressor: Option<CompressorWeights>,
    indexer: Option<IndexerWeights>,
    gate: V41Buffer,
    bias: V41Buffer,
    w1: Packed,
    w2: Packed,
    w3: Packed,
    shared: [Packed; 3],
    engram: Option<EngramWeights>,
    ring: V41Buffer,
    mixes_attn: V41Buffer,
    mixes_ffn: V41Buffer,
    observed: Vec<(V41Site, V41Buffer, usize)>,
}

#[derive(Debug)]
struct SourceState {
    kv: V41Buffer,
    index_keys: V41Buffer,
    capacity: usize,
    ratio: usize,
    state: Option<V41Buffer>,
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

/// Every buffer a step reads or writes besides layer weights.
#[derive(Debug)]
struct Work {
    index: V41Buffer,
    base: V41Buffer,
    positions: V41Buffer,
    tokens: V41Buffer,
    residual: [V41Buffer; 2],
    pre_in: V41Buffer,
    scratch: V41Buffer,
    collapsed: V41Buffer,
    normalized: V41Buffer,
    act_codes: V41Buffer,
    act_scales: V41Buffer,
    act2_codes: V41Buffer,
    act2_scales: V41Buffer,
    q_low: V41Buffer,
    q_norm: V41Buffer,
    q_heads: V41Buffer,
    kv: V41Buffer,
    kv_norm: V41Buffer,
    values: V41Buffer,
    gates: V41Buffer,
    latents: V41Buffer,
    index_query: V41Buffer,
    index_weights: V41Buffer,
    scores: V41Buffer,
    block_scores: V41Buffer,
    flags: V41Buffer,
    blocks: V41Buffer,
    block_capacity: usize,
    heads: V41Buffer,
    heads_q: V41Buffer,
    low_rank: V41Buffer,
    attn_out: V41Buffer,
    logits: V41Buffer,
    ids: V41Buffer,
    route_weights: V41Buffer,
    g: V41Buffer,
    u: V41Buffer,
    h: V41Buffer,
    y: V41Buffer,
    sg: V41Buffer,
    su: V41Buffer,
    sh: V41Buffer,
    sy: V41Buffer,
    moe_out: V41Buffer,
    engram_ids: V41Buffer,
    engram_slot_ids: V41Buffer,
    engram_rows: V41Buffer,
    engram_projected: V41Buffer,
    history: V41Buffer,
    residual_in: V41Buffer,
    pre_mix_in: V41Buffer,
    residual_out: V41Buffer,
    pre_mix_out: V41Buffer,
    selection_in: V41Buffer,
    candidates_in: V41Buffer,
    selection_out: V41Buffer,
    candidates_out: V41Buffer,
    head_logits: V41Buffer,
    settings: V41Buffer,
    seed: V41Buffer,
    token: V41Buffer,
}

/// A loaded stage on an Apple GPU. See the module documentation.
pub struct V41MetalStage {
    config: V41StageConfig,
    layers: Vec<LayerWeights>,
    sources: BTreeMap<usize, SourceState>,
    selections: BTreeMap<usize, V41Buffer>,
    rope_window: V41Buffer,
    rope_compressed: V41Buffer,
    rope_positions: usize,
    embed: Option<V41Buffer>,
    head: Option<(V41Buffer, V41Buffer)>,
    engram_tables: Vec<V41MetalEngramTable>,
    engram_map: Option<V41Buffer>,
    engram_params: Option<V41Buffer>,
    work: Work,
    body: Option<V41Recording>,
    body_observed: Option<V41Recording>,
    tail: Option<V41Recording>,
    history: Vec<u32>,
    weight_bytes: usize,
    state_bytes: usize,
    memory: V41StageMemory,
    last_gpu_seconds: f64,
}

impl std::fmt::Debug for V41MetalStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("V41MetalStage")
            .field("layers", &self.config.layers)
            .field("weight_bytes", &self.weight_bytes)
            .field("state_bytes", &self.state_bytes)
            .finish_non_exhaustive()
    }
}

struct Loader<'a> {
    metal: &'a V41Metal,
    source: &'a dyn V41TensorSource,
    bytes: usize,
    params: u64,
    state: usize,
}

impl Loader<'_> {
    fn zeros(&mut self, bytes: usize) -> Result<V41Buffer, RuntimeError> {
        self.bytes += bytes.max(4);
        self.metal.zeros(bytes.max(4))
    }

    fn state_zeros(&mut self, bytes: usize) -> Result<V41Buffer, RuntimeError> {
        self.state += bytes.max(4);
        self.zeros(bytes)
    }

    fn upload(&mut self, bytes: &[u8]) -> Result<V41Buffer, RuntimeError> {
        self.bytes += bytes.len().max(4);
        let buffer = self.metal.zeros(bytes.len().max(4))?;
        buffer.write_at(0, bytes)?;
        Ok(buffer)
    }

    fn floats(&mut self, values: &[f32]) -> Result<V41Buffer, RuntimeError> {
        self.upload(&le_bytes(values, f32::to_le_bytes))
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

    fn f32s(&mut self, name: &str, shape: &[usize], bf16: bool) -> Result<V41Buffer, RuntimeError> {
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

    fn bf16_words(&mut self, name: &str, shape: &[usize]) -> Result<V41Buffer, RuntimeError> {
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
        let weights = self.zeros(weight_bytes * stems.len())?;
        let scales = self.zeros(scale_bytes * stems.len())?;
        self.params += (rows * cols * stems.len()) as u64;
        for (i, stem) in stems.iter().enumerate() {
            let w = self.tensor(&format!("{stem}.weight"), weight_dtype, &weight_shape)?;
            let s = self.tensor(
                &format!("{stem}.scale"),
                V41TensorDtype::F8E8M0,
                &scale_shape,
            )?;
            weights.write_at(i * weight_bytes, &w.bytes)?;
            scales.write_at(i * scale_bytes, &s.bytes)?;
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

impl V41MetalStage {
    /// Loads a stage's weights from `source` and allocates its request
    /// state, as `V41Stage::load`. `engram_tables` holds the table of each
    /// Engram layer in the stage. RoPE tables cover `config.max_context`
    /// positions. `experts_on_host` changes nothing: memory is unified.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration, missing or misshapen tensors, a missing
    /// Engram table, and allocation failures.
    #[allow(clippy::too_many_lines)]
    pub fn load(
        metal: &V41Metal,
        config: V41StageConfig,
        source: &dyn V41TensorSource,
        engram_tables: Vec<V41MetalEngramTable>,
    ) -> Result<Self, RuntimeError> {
        let shape = config.shape;
        check_shape(&shape)?;
        let g = shape.attention;
        let (h, s) = (shape.hidden, shape.hc.streams);
        if config.layers.is_empty()
            || config.layers.end > V41_LAYERS
            || config.max_context == 0
            || config.max_context > V41_MAX_POSITION
            || config.max_step_tokens == 0
            || config.max_step_tokens > config.max_context
        {
            return Err(invalid(format!(
                "layers {:?}, context {}, step tokens {}",
                config.layers, config.max_context, config.max_step_tokens
            )));
        }
        let engram_layers = engram_layers(&config);
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
            metal,
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
        // RoPE tables for every position a request may reach.
        let rope_positions = config.max_context;
        let window = v41_rope_frequencies(V41RopeConfig::window(g.rope_dim))?;
        let compressed = v41_rope_frequencies(V41RopeConfig::compressed(g.rope_dim))?;
        let mut rope_window = loader.floats(&v41_rope_table_host(&window, rope_positions))?;
        let mut rope_compressed =
            loader.floats(&v41_rope_table_host(&compressed, rope_positions))?;
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
            rope_positions,
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
            last_gpu_seconds: 0.0,
        })
    }

    /// The stage's configuration.
    #[must_use]
    pub fn config(&self) -> &V41StageConfig {
        &self.config
    }

    /// Bytes of weights, and of request state and scratch.
    #[must_use]
    pub fn device_bytes(&self) -> (usize, usize) {
        (self.weight_bytes, self.state_bytes)
    }

    /// Where the stage's bytes went.
    #[must_use]
    pub fn memory(&self) -> &V41StageMemory {
        &self.memory
    }

    /// The GPU time of the last step's command buffer, in seconds.
    #[must_use]
    pub fn last_gpu_seconds(&self) -> f64 {
        self.last_gpu_seconds
    }

    /// Per-kernel dispatch counts of the recorded per-token body and tail.
    #[must_use]
    pub fn recorded_kernels(
        &self,
    ) -> (BTreeMap<&'static str, usize>, BTreeMap<&'static str, usize>) {
        (
            self.body
                .as_ref()
                .map(V41Recording::kernel_counts)
                .unwrap_or_default(),
            self.tail
                .as_ref()
                .map(V41Recording::kernel_counts)
                .unwrap_or_default(),
        )
    }

    fn rope(&self, layer: usize) -> V41RopeTable<'_> {
        V41RopeTable {
            table: if v41_compress_ratio(layer) == Some(0) {
                &self.rope_window
            } else {
                &self.rope_compressed
            },
            dim: self.config.shape.attention.rope_dim,
            positions: self.rope_positions,
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
        s: &mut V41Submission<'_>,
        layer: &LayerWeights,
        site: V41Site,
        src: &V41Buffer,
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
    fn record_body(&self, s: &mut V41Submission<'_>, observe: bool) -> Result<(), RuntimeError> {
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
        let engram_layers = engram_layers(c);
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
        let mut previous: &V41Buffer = &wk.pre_in;
        let single = V41Slots::single();
        for lw in &self.layers {
            let layer = lw.layer;
            let ratio = v41_compress_ratio(layer).ok_or_else(|| invalid("layer"))?;
            let owners = V41LayerOwners::for_layer(layer).ok_or_else(|| invalid("layer"))?;
            let ob = |s: &mut V41Submission<'_>, site: V41Site, src: &V41Buffer| {
                if observe {
                    self.observe(s, lw, site, src)
                } else {
                    Ok(())
                }
            };
            if let Some(e) = &lw.engram {
                let table = self
                    .engram_tables
                    .iter()
                    .find(|t| t.layer == layer)
                    .ok_or_else(|| invalid("Engram table"))?;
                let ids = &wk.engram_slot_ids;
                s.v41_copy_words(
                    &wk.engram_ids,
                    e.slot * V41_ENGRAM_COLUMNS * 2,
                    ids,
                    0,
                    V41_ENGRAM_COLUMNS * 2,
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
        // The final residual stays in `residual[0]` for the tail.
        if cur != 0 {
            s.v41_copy_words(&wk.residual[1], 0, &wk.residual[0], 0, streams * h)?;
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
            s.v41_copy_words(previous, 0, &wk.pre_in, 0, streams)?;
        }
        s.v41_stage_advance(&wk.index)
    }

    #[allow(clippy::too_many_lines)]
    fn record_attention(
        &self,
        s: &mut V41Submission<'_>,
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
                        self.observe(s, lw, V41Site::Selection, selection)?;
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

    fn record_moe(&self, s: &mut V41Submission<'_>, lw: &LayerWeights) -> Result<(), RuntimeError> {
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
    fn record_tail(&self, s: &mut V41Submission<'_>) -> Result<(), RuntimeError> {
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

    /// Runs one step, as `V41Stage::step`.
    ///
    /// # Errors
    ///
    /// Refuses input that doesn't match the stage's boundary or capacity,
    /// and any device failure. A failed step leaves the request unusable:
    /// start it again from position 0.
    pub fn step(
        &mut self,
        metal: &V41Metal,
        input: &V41StepInput<'_>,
        observe: bool,
    ) -> Result<V41StepOutput, RuntimeError> {
        self.step_with_output(metal, input, observe, true)
    }

    /// Updates state for a chunk, omitting the head and sampler when `emit`
    /// is false (more prompt chunks follow), as `V41Stage::step_with_output`.
    ///
    /// # Errors
    ///
    /// As [`Self::step`].
    #[allow(clippy::too_many_lines)]
    pub fn step_with_output(
        &mut self,
        metal: &V41Metal,
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
        let upload = |stats: &mut V41StepStats,
                      buffer: &V41Buffer,
                      offset: usize,
                      bytes: &[u8]|
         -> Result<(), RuntimeError> {
            stats.host_to_device_copies += 1;
            stats.host_to_device_bytes += bytes.len();
            buffer.write_at(offset, bytes)
        };
        upload(&mut stats, &self.work.index, 0, &0_i32.to_le_bytes())?;
        upload(
            &mut stats,
            &self.work.base,
            0,
            &i32::try_from(input.position)
                .map_err(|_| invalid("position"))?
                .to_le_bytes(),
        )?;
        upload(
            &mut stats,
            &self.work.tokens,
            0,
            &le_bytes(input.tokens, u32::to_le_bytes),
        )?;
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
                &self.work.residual_in,
                0,
                &le_bytes(residual, f32::to_le_bytes),
            )?;
            upload(
                &mut stats,
                &self.work.pre_mix_in,
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
                    let source = self.sources.get(&layer).ok_or_else(|| invalid("source"))?;
                    if first + count > source.capacity {
                        return Err(invalid("source capacity"));
                    }
                    if count > 0 {
                        let kv = le_bytes(&rows.kv, u16::to_le_bytes);
                        upload(&mut stats, &source.kv, first * g.head_dim * 2, &kv)?;
                        if let Some(keys) = &rows.index_keys {
                            let keys = le_bytes(keys, u16::to_le_bytes);
                            upload(
                                &mut stats,
                                &source.index_keys,
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
                        &self.work.selection_in,
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
                        &self.work.candidates_in,
                        0,
                        &le_bytes(rows, i32::to_le_bytes),
                    )?;
                }
                (false, None) => {}
                _ => return Err(invalid("the boundary's candidates")),
            }
        }
        let sample = c.samples() && emit;
        if sample {
            let s = input.sampling;
            let mut settings = s.temperature.to_le_bytes().to_vec();
            settings.extend_from_slice(&s.top_p.to_le_bytes());
            upload(&mut stats, &self.work.settings, 0, &settings)?;
            upload(&mut stats, &self.work.seed, 0, &s.seed.to_le_bytes())?;
        }

        // Record the body (and tail) the first time; then one command buffer
        // with the body once per token and the tail once.
        let device_started = Instant::now();
        let recorded = if observe {
            self.body_observed.is_some()
        } else {
            self.body.is_some()
        };
        if !recorded {
            let mut submission = metal.begin_submission();
            self.record_body(&mut submission, observe)?;
            let recording = submission.finish();
            stats.graph_captures += 1;
            if observe {
                self.body_observed = Some(recording);
            } else {
                self.body = Some(recording);
            }
        }
        if sample && self.tail.is_none() {
            let mut submission = metal.begin_submission();
            self.record_tail(&mut submission)?;
            self.tail = Some(submission.finish());
            stats.graph_captures += 1;
        }
        let body = if observe {
            self.body_observed.as_ref()
        } else {
            self.body.as_ref()
        }
        .ok_or_else(|| invalid("body"))?;
        let mut parts = vec![(body, t)];
        stats.graph_launches += t;
        if sample {
            parts.push((self.tail.as_ref().ok_or_else(|| invalid("tail"))?, 1));
            stats.graph_launches += 1;
        }
        self.last_gpu_seconds = metal.run(&parts)?;
        stats.synchronizations += 1;
        stats.device_seconds = device_started.elapsed().as_secs_f64();

        // The step's output: one read each.
        let mut out = V41StepOutput::default();
        let read = |stats: &mut V41StepStats, buffer: &V41Buffer, bytes: usize| -> Vec<u8> {
            stats.device_to_host_copies += 1;
            stats.device_to_host_bytes += bytes;
            buffer.bytes()[..bytes].to_vec()
        };
        let history_before = self.history.clone();
        self.history.extend_from_slice(input.tokens);
        let keep = self.history.len().saturating_sub(3);
        self.history.drain(..keep);
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
        if c.samples() {
            if emit {
                let bytes = read(&mut stats, &self.work.token, 4);
                let token = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                out.token = Some(u32::try_from(token).map_err(|_| invalid("nonfinite logits"))?);
            }
        } else {
            out.residual = to_f32(read(
                &mut stats,
                &self.work.residual_out,
                4 * t * streams * h,
            ));
            out.pre_mix = to_f32(read(&mut stats, &self.work.pre_mix_out, 4 * t * streams));
            let boundary = V41Boundary::at(c.layers.end);
            if let Some((layer, keys)) = boundary.kv {
                let source = self.sources.get(&layer).ok_or_else(|| invalid("source"))?;
                let ratio = source.ratio.max(1);
                let first = input.position / ratio;
                let count = (input.position + t) / ratio - first;
                let words =
                    |stats: &mut V41StepStats, buffer: &V41Buffer, width: usize| -> Vec<u16> {
                        if count == 0 {
                            return Vec::new();
                        }
                        stats.device_to_host_copies += 1;
                        stats.device_to_host_bytes += count * width * 2;
                        buffer.bytes()[first * width * 2..(first + count) * width * 2]
                            .chunks_exact(2)
                            .map(|w| u16::from_le_bytes([w[0], w[1]]))
                            .collect()
                    };
                out.source_rows = Some(V41SourceRows {
                    layer,
                    first_row: first,
                    kv: words(&mut stats, &source.kv, g.head_dim),
                    index_keys: keys.then(|| words(&mut stats, &source.index_keys, g.index_dim)),
                });
            }
            if let Some(owner) = boundary.selection {
                let rows = if c.layers.contains(&owner) {
                    to_i32(read(
                        &mut stats,
                        &self.work.selection_out,
                        4 * t * g.index_top_k,
                    ))
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
                    ))
                } else {
                    input
                        .candidates
                        .ok_or_else(|| invalid("candidates to forward"))?
                        .to_vec()
                });
            }
            let mut history = vec![u32::MAX; 3 - history_before.len().min(3)];
            history.extend_from_slice(&history_before);
            out.engram_history = history;
        }
        stats.seconds = started.elapsed().as_secs_f64();
        out.stats = stats;
        Ok(out)
    }

    /// The observations of the last observed step, as `V41Stage::observations`.
    #[must_use]
    pub fn observations(&self, tokens: usize) -> BTreeMap<usize, BTreeMap<V41Site, Vec<u32>>> {
        let mut out = BTreeMap::new();
        for lw in &self.layers {
            let mut sites = BTreeMap::new();
            for (site, buffer, words) in &lw.observed {
                sites.insert(
                    *site,
                    buffer.bytes()[..4 * words * tokens]
                        .chunks_exact(4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect(),
                );
            }
            out.insert(lw.layer, sites);
        }
        out
    }

    /// The last stage's logits of the last token of the last step.
    ///
    /// # Errors
    ///
    /// Refuses a stage that doesn't sample.
    pub fn logits(&self) -> Result<Vec<f32>, RuntimeError> {
        if !self.config.samples() {
            return Err(invalid("only the last stage has logits"));
        }
        Ok(self.work.head_logits.f32s()[..self.config.shape.vocabulary].to_vec())
    }

    /// The final collapse and norm of the last token, from the last stage.
    #[must_use]
    pub fn final_norm(&self) -> Vec<f32> {
        self.work.normalized.f32s()[..self.config.shape.hidden].to_vec()
    }

    /// The index selection the stage holds for `layer`'s consumers.
    ///
    /// # Errors
    ///
    /// Refuses a layer without one.
    pub fn selection(&self, layer: usize) -> Result<Vec<i32>, RuntimeError> {
        let buffer = self
            .selections
            .get(&layer)
            .ok_or_else(|| invalid(format!("no selection for layer {layer}")))?;
        Ok(buffer.i32s()[..self.config.shape.attention.index_top_k].to_vec())
    }
}

#[cfg(test)]
#[path = "stage_tests.rs"]
mod tests;
