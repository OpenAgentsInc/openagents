//! V4.1 attention on Metal, a port of `psionic_backend_cuda::v41_attention`
//! (#12392). The geometry, configuration types, and host models are the
//! CUDA backend's.
//!
//! RoPE is the one change of interface: the CUDA kernels compute
//! `cos((double)(float(p) * f))` on the device, and Apple GPUs have no double
//! precision, so [`V41RopeTable`] carries a host-computed table of those
//! values ([`v41_rope_table_host`]) for positions below its `positions`. A
//! position at or past it writes NaN, as a position past 2^20 does on CUDA.

pub use psionic_backend_cuda::v41_attention::{
    V41_CANDIDATE_BLOCK_SIZE, V41_CANDIDATE_BLOCKS, V41_FLAG_POISON, V41_INDEX_SOURCES,
    V41_KV_SOURCES, V41_MAX_HEAD_DIM, V41_MAX_POSITION, V41_MAX_RATIO, V41_SELECT_POISON,
    V41AttentionGeometry, V41CacheQuant, V41Compressor, V41CompressorHost, V41DenseFormat,
    V41DenseHost, V41LayerOwners, V41RopeConfig, v41_bf16_bits, v41_bf16_value,
    v41_cache_quant_host, v41_candidate_blocks_host, v41_compress_ratio, v41_compressed_store_host,
    v41_dense_host, v41_index_scores_host, v41_quant_block_host, v41_rms_norm_host,
    v41_rope_frequencies, v41_rope_host, v41_select_host, v41_sparse_attention_host,
};

use super::matvec::V41Output;
use super::{Args, V41Buffer, V41Submission, int};
use psionic_runtime::RuntimeError;

const THREADS: usize = 256;
const SELECT_THREADS: usize = 1024;
const DENSE_ROWS: usize = 8;
const SCORE_KEYS: usize = 32;
const ATTENTION_HEADS: usize = 4;
const ATTENTION_SPLIT: usize = 128;

fn invalid(message: String) -> RuntimeError {
    RuntimeError::Backend(format!("v41 attention kernels: {message}"))
}

fn rounds(output: V41Output) -> i32 {
    i32::from(output == V41Output::Bf16)
}

fn stride(value: usize, what: &str) -> Result<i64, RuntimeError> {
    i64::try_from(value).map_err(|_| invalid(format!("{what} {value} exceeds i64")))
}

/// `V41AttentionGeometry::check` from the CUDA backend.
fn check_geometry(g: V41AttentionGeometry) -> Result<(), RuntimeError> {
    let shared_floats = g.index_heads * (g.index_dim + 1) + g.index_heads + 16 * g.index_dim;
    if g.heads == 0
        || !g.heads.is_multiple_of(4)
        || g.head_dim == 0
        || g.head_dim > V41_MAX_HEAD_DIM
        || !g.head_dim.is_multiple_of(64)
        || g.rope_dim == 0
        || !g.rope_dim.is_multiple_of(2)
        || g.rope_dim > g.head_dim.min(g.index_dim)
        || g.window == 0
        || g.index_heads == 0
        || g.index_heads > 32
        || g.index_dim == 0
        || !g.index_dim.is_multiple_of(32)
        || g.index_dim > 1024
        || g.index_top_k == 0
        || shared_floats > 12 * 1024
        || !(g.norm_epsilon.is_finite() && g.norm_epsilon > 0.0)
        || i32::try_from(g.heads * g.head_dim).is_err()
        || i32::try_from(g.window + g.index_top_k).is_err()
    {
        return Err(invalid(format!("geometry {g:?}")));
    }
    Ok(())
}

fn check_compressor(c: V41Compressor) -> Result<(), RuntimeError> {
    if !(1..=V41_MAX_RATIO).contains(&c.ratio)
        || c.width == 0
        || i32::try_from(c.state_floats()).is_err()
        || !(c.epsilon.is_finite() && c.epsilon > 0.0)
    {
        return Err(invalid(format!("compressor {c:?}")));
    }
    Ok(())
}

fn quant_code(kind: V41CacheQuant) -> i32 {
    match kind {
        V41CacheQuant::Fp8 => 0,
        V41CacheQuant::IndexFp4 => 1,
        V41CacheQuant::CompressedFp4 => 2,
    }
}

/// Device positions, one `i32` per token, as `V41Steps`.
#[derive(Clone, Copy, Debug)]
pub struct V41Steps<'a> {
    pub positions: &'a V41Buffer,
    pub tokens: usize,
}

/// A row-major buffer of `stride` elements per token, as `V41Rows`.
#[derive(Clone, Copy, Debug)]
pub struct V41Rows<'a> {
    pub buffer: &'a V41Buffer,
    pub stride: usize,
}

/// One layer's window ring, as `V41Ring`.
#[derive(Clone, Copy, Debug)]
pub struct V41Ring<'a> {
    pub buffer: &'a V41Buffer,
    pub rows: usize,
}

/// A KV source layer's shared state, as `V41Source`.
#[derive(Clone, Copy, Debug)]
pub struct V41Source<'a> {
    pub kv: &'a V41Buffer,
    pub index_keys: &'a V41Buffer,
    pub capacity: usize,
}

/// RoPE's cos and sin table: `positions × dim / 2` pairs of floats from
/// [`v41_rope_table_host`].
#[derive(Clone, Copy, Debug)]
pub struct V41RopeTable<'a> {
    pub table: &'a V41Buffer,
    pub dim: usize,
    pub positions: usize,
}

/// The cos and sin of every (position, pair) angle, as the CUDA kernels
/// compute them on the device: `angle = float(p) × f` in float32, then
/// `cos` and `sin` in double precision rounded once. Interleaved
/// `[positions, frequencies.len(), 2]`.
#[must_use]
pub fn v41_rope_table_host(frequencies: &[f32], positions: usize) -> Vec<f32> {
    let mut table = Vec::with_capacity(positions * frequencies.len() * 2);
    for p in 0..positions {
        for f in frequencies {
            let angle = p as f32 * f;
            table.push(f64::from(angle).cos() as f32);
            table.push(f64::from(angle).sin() as f32);
        }
    }
    table
}

/// A dense matrix, as `V41Dense`.
#[derive(Clone, Copy, Debug)]
pub struct V41Dense<'a> {
    pub weights: &'a V41Buffer,
    pub scales: Option<&'a V41Buffer>,
    pub format: V41DenseFormat,
    pub rows: usize,
    pub cols: usize,
    pub groups: usize,
}

/// The source layer's indexer key weights, as `V41IndexKeyWeights`.
#[derive(Clone, Copy, Debug)]
pub struct V41IndexKeyWeights<'a> {
    pub wk: &'a V41Buffer,
    pub k_norm: &'a V41Buffer,
}

/// Layer 20's candidate output, as `V41Candidates`.
#[derive(Clone, Copy, Debug)]
pub struct V41Candidates<'a> {
    pub block_scores: &'a V41Buffer,
    pub flags: &'a V41Buffer,
    pub blocks: &'a V41Buffer,
    pub stride: usize,
    pub blocks_stride: usize,
}

/// Selected rows for the sparse attention, as `V41Compressed`.
#[derive(Clone, Copy, Debug)]
pub struct V41Compressed<'a> {
    pub kv: &'a V41Buffer,
    pub capacity: usize,
    pub indices: V41Rows<'a>,
    pub ratio: usize,
}

impl V41Submission<'_> {
    fn steps(&mut self, steps: V41Steps<'_>) -> Result<u64, RuntimeError> {
        if steps.tokens == 0 || steps.tokens > 65_535 {
            return Err(invalid(format!("{} tokens", steps.tokens)));
        }
        self.span(steps.positions, 0, steps.tokens, 4, "positions")
    }

    fn rope_span(&mut self, rope: V41RopeTable<'_>) -> Result<u64, RuntimeError> {
        self.span(rope.table, 0, rope.positions * rope.dim, 4, "RoPE table")
    }

    /// Rotates the last `rope.dim` values of every head, as
    /// `CudaSubmission::v41_rope`.
    ///
    /// # Errors
    ///
    /// Refuses bad shapes and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_rope(
        &mut self,
        rope: V41RopeTable<'_>,
        values: &V41Buffer,
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
        let positions = self.steps(steps)?;
        let values = self.span(
            values,
            0,
            steps.tokens * heads * head_width,
            4,
            "RoPE values",
        )?;
        let table = self.rope_span(rope)?;
        let args = Args::new()
            .ptr(values)
            .ptr(table)
            .ptr(positions)
            .i32(int(rope.positions, "RoPE positions")?)
            .i32(int(heads, "heads")?)
            .i32(int(head_width, "head width")?)
            .i32(int(rope.dim, "RoPE dim")?)
            .i32(int(back, "back")?)
            .i32(i32::from(inverse))
            .i32(rounds(output));
        self.launch("v41_rope", args, (heads, steps.tokens, 1), 128, 0)
    }

    /// Quantizes and dequantizes in a cache form, as
    /// `CudaSubmission::v41_cache_quant`.
    ///
    /// # Errors
    ///
    /// Refuses a width that isn't a multiple of the block, and buffers too
    /// small for the shape.
    pub fn v41_cache_quant(
        &mut self,
        kind: V41CacheQuant,
        input: &V41Buffer,
        out: &V41Buffer,
        rows: usize,
        width: usize,
    ) -> Result<(), RuntimeError> {
        if rows == 0 || width == 0 || !width.is_multiple_of(kind.block()) {
            return Err(invalid(format!("{kind:?} quantization of {rows}×{width}")));
        }
        let total = rows * width;
        let args = Args::new()
            .ptr(self.span(input, 0, total, 4, "quantization input")?)
            .ptr(self.span(out, 0, total, 4, "quantization output")?)
            .i64(stride(total, "values")?)
            .i32(quant_code(kind));
        self.launch(
            "v41_cache_quant",
            args,
            (total.div_ceil(THREADS), 1, 1),
            THREADS,
            0,
        )
    }

    /// Writes each token's window KV into the ring, as
    /// `CudaSubmission::v41_window_store`.
    ///
    /// # Errors
    ///
    /// Refuses a ring shorter than `window + tokens - 1` rows and buffers too
    /// small for the shapes.
    pub fn v41_window_store(
        &mut self,
        geometry: V41AttentionGeometry,
        input: &V41Buffer,
        steps: V41Steps<'_>,
        ring: V41Ring<'_>,
    ) -> Result<(), RuntimeError> {
        check_geometry(geometry)?;
        if ring.rows < geometry.window + steps.tokens.saturating_sub(1) {
            return Err(invalid(format!(
                "a ring of {} rows for {} tokens",
                ring.rows, steps.tokens
            )));
        }
        let positions = self.steps(steps)?;
        let width = geometry.head_dim;
        let args = Args::new()
            .ptr(self.span(input, 0, steps.tokens * width, 4, "window KV")?)
            .ptr(self.span(ring.buffer, 0, ring.rows * width, 2, "window ring")?)
            .ptr(positions)
            .i32(int(ring.rows, "ring rows")?)
            .i32(int(width, "width")?);
        self.launch("v41_window_store", args, (steps.tokens, 1, 1), 128, 0)
    }

    /// Multiplies input rows by a dense matrix in the CPU's order, as
    /// `CudaSubmission::v41_dense`.
    ///
    /// # Errors
    ///
    /// Refuses bad shapes and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_dense(
        &mut self,
        matrix: V41Dense<'_>,
        input: &V41Buffer,
        out: &V41Buffer,
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
        let weights = self.span(m.weights, 0, m.rows * m.cols, element, "dense weights")?;
        let scales = match (m.format, m.scales) {
            (V41DenseFormat::Fp8Block, Some(scales)) => {
                self.span(scales, 0, (m.rows / 32) * (m.cols / 32), 1, "dense scales")?
            }
            (V41DenseFormat::Fp8Block, None) => {
                return Err(invalid(String::from("FP8 dense weights need scales")));
            }
            _ => 0,
        };
        let format = match m.format {
            V41DenseFormat::F32 => 0,
            V41DenseFormat::Bf16 => 1,
            V41DenseFormat::Fp8Block => 2,
        };
        let args = Args::new()
            .ptr(weights)
            .ptr(scales)
            .ptr(self.span(input, 0, tokens * m.groups * m.cols, 4, "dense input")?)
            .ptr(self.span(out, 0, tokens * m.rows, 4, "dense output")?)
            .i32(format)
            .i32(int(m.rows, "rows")?)
            .i32(int(m.cols, "cols")?)
            .i32(int(m.groups, "groups")?)
            .i32(i32::from(round_input))
            .i32(rounds(output));
        self.launch(
            "v41_dense",
            args,
            (m.rows.div_ceil(DENSE_ROWS), tokens, 1),
            THREADS,
            0,
        )
    }

    /// RMS-normalizes rows with the BF16-rounded weight, as
    /// `CudaSubmission::v41_rms_norm`.
    ///
    /// # Errors
    ///
    /// Refuses bad shapes, a bad epsilon, and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_rms_norm(
        &mut self,
        input: &V41Buffer,
        weight: &V41Buffer,
        out: &V41Buffer,
        tokens: usize,
        width: usize,
        epsilon: f32,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        if tokens == 0
            || tokens > 65_535
            || width == 0
            || width > 8191
            || !(epsilon.is_finite() && epsilon > 0.0)
        {
            return Err(invalid(format!("RMS norm of {tokens}×{width}")));
        }
        let args = Args::new()
            .ptr(self.span(input, 0, tokens * width, 4, "norm input")?)
            .ptr(self.span(weight, 0, width, 4, "norm weight")?)
            .ptr(self.span(out, 0, tokens * width, 4, "norm output")?)
            .i32(int(width, "width")?)
            .f32(epsilon)
            .i32(rounds(output));
        self.launch("v41_rms_norm", args, (tokens, 1, 1), 128, (width + 1) * 4)
    }

    /// Pushes projected values (and gates) through the compressor, as
    /// `CudaSubmission::v41_compress`. Two launches.
    ///
    /// # Errors
    ///
    /// Refuses a bad compressor, gates on ratio 1 or none above it, and
    /// buffers too small.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_compress(
        &mut self,
        compressor: V41Compressor,
        values: &V41Buffer,
        gates: Option<&V41Buffer>,
        norm: &V41Buffer,
        state: &V41Buffer,
        steps: V41Steps<'_>,
        latents: &V41Buffer,
    ) -> Result<(), RuntimeError> {
        let c = compressor;
        check_compressor(c)?;
        if (c.ratio == 1) != gates.is_none() {
            return Err(invalid(String::from(
                "ratio 1 takes no gates and higher ratios need them",
            )));
        }
        let positions = self.steps(steps)?;
        let n = steps.tokens * c.width;
        let values = self.span(values, 0, n, 4, "compressor values")?;
        let gates = self.span_opt(gates, 0, n, 4, "compressor gates")?;
        let norm = self.span(norm, 0, c.width, 4, "compressor norm")?;
        let state = self.span(state, 0, c.state_floats(), 4, "compressor state")?;
        let latents = self.span(latents, 0, n, 4, "latents")?;
        let ratio = int(c.ratio, "ratio")?;
        let width = int(c.width, "width")?;
        self.launch(
            "v41_compress",
            Args::new()
                .ptr(values)
                .ptr(gates)
                .ptr(norm)
                .ptr(state)
                .ptr(positions)
                .ptr(latents)
                .i32(ratio)
                .i32(width)
                .f32(c.epsilon),
            (steps.tokens, 1, 1),
            128,
            (c.width + 1) * 4,
        )?;
        if c.ratio > 1 {
            self.launch(
                "v41_compress_state",
                Args::new()
                    .ptr(values)
                    .ptr(gates)
                    .ptr(state)
                    .ptr(positions)
                    .i32(int(steps.tokens, "tokens")?)
                    .i32(ratio)
                    .i32(width),
                (1, 1, 1),
                128,
                0,
            )?;
        }
        Ok(())
    }

    /// Writes each completed group's index key and compressed KV row, as
    /// `CudaSubmission::v41_compressed_store`.
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
        latents: &V41Buffer,
        steps: V41Steps<'_>,
        source: V41Source<'_>,
    ) -> Result<(), RuntimeError> {
        let g = geometry;
        check_geometry(g)?;
        if !(1..=V41_MAX_RATIO).contains(&ratio) || rope.dim != g.rope_dim {
            return Err(invalid(format!(
                "compressed store at ratio {ratio} with RoPE of {}",
                rope.dim
            )));
        }
        let positions = self.steps(steps)?;
        let args = Args::new()
            .ptr(self.span(latents, 0, steps.tokens * g.head_dim, 4, "latents")?)
            .ptr(self.span(weights.wk, 0, g.index_dim * g.head_dim, 2, "indexer wk")?)
            .ptr(self.span(weights.k_norm, 0, g.index_dim, 4, "indexer k_norm")?)
            .ptr(self.rope_span(rope)?)
            .ptr(positions)
            .ptr(self.span(
                source.kv,
                0,
                source.capacity * g.head_dim,
                2,
                "compressed KV",
            )?)
            .ptr(self.span(
                source.index_keys,
                0,
                source.capacity * g.index_dim,
                2,
                "index keys",
            )?)
            .i32(int(rope.positions, "RoPE positions")?)
            .i32(int(ratio, "ratio")?)
            .i32(int(source.capacity, "capacity")?)
            .i32(int(g.head_dim, "head dim")?)
            .i32(int(g.index_dim, "index dim")?)
            .i32(int(g.rope_dim, "RoPE dim")?)
            .f32(g.norm_epsilon);
        self.launch(
            "v41_compressed_store",
            args,
            (steps.tokens, 1, 1),
            128,
            (g.head_dim + g.index_dim + 1) * 4,
        )
    }

    /// Scores every visible key of the KV source, as
    /// `CudaSubmission::v41_index_scores`.
    ///
    /// # Errors
    ///
    /// Refuses a bad geometry or ratio and buffers too small.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_index_scores(
        &mut self,
        geometry: V41AttentionGeometry,
        query: &V41Buffer,
        weights: &V41Buffer,
        source: V41Source<'_>,
        ratio: usize,
        steps: V41Steps<'_>,
        scores: V41Rows<'_>,
    ) -> Result<(), RuntimeError> {
        let g = geometry;
        check_geometry(g)?;
        if !(1..=V41_MAX_RATIO).contains(&ratio) || scores.stride < source.capacity {
            return Err(invalid(format!(
                "index scores at ratio {ratio} with stride {} for {} keys",
                scores.stride, source.capacity
            )));
        }
        let positions = self.steps(steps)?;
        let width = g.index_heads * g.index_dim;
        let args = Args::new()
            .ptr(self.span(query, 0, steps.tokens * width, 4, "index query")?)
            .ptr(self.span(weights, 0, steps.tokens * g.index_heads, 4, "index weights")?)
            .ptr(self.span(
                source.index_keys,
                0,
                source.capacity * g.index_dim,
                2,
                "index keys",
            )?)
            .ptr(positions)
            .ptr(self.span(
                scores.buffer,
                0,
                (steps.tokens - 1) * scores.stride + source.capacity,
                4,
                "index scores",
            )?)
            .i64(stride(scores.stride, "score stride")?)
            .i32(int(source.capacity, "capacity")?)
            .i32(int(ratio, "ratio")?)
            .i32(int(g.index_heads, "index heads")?)
            .i32(int(g.index_dim, "index dim")?)
            .f32(g.index_scale());
        let shared =
            (g.index_heads * (g.index_dim + 1) + g.index_heads) * 4 + SCORE_KEYS * g.index_dim * 2;
        self.launch(
            "v41_index_scores",
            args,
            (source.capacity.div_ceil(SCORE_KEYS), steps.tokens, 1),
            256,
            shared,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn select(
        &mut self,
        values: u64,
        value_stride: usize,
        flags: u64,
        flag_stride: usize,
        flag_group: usize,
        positions: u64,
        tokens: usize,
        ratio: usize,
        group: usize,
        top_k: usize,
        allow_inf: bool,
        out: u64,
        out_stride: usize,
        flags_out: u64,
        flags_out_stride: usize,
    ) -> Result<(), RuntimeError> {
        let args = Args::new()
            .ptr(values)
            .i64(stride(value_stride, "value stride")?)
            .ptr(flags)
            .i64(stride(flag_stride, "flag stride")?)
            .ptr(positions)
            .ptr(out)
            .i64(stride(out_stride, "index stride")?)
            .ptr(flags_out)
            .i64(stride(flags_out_stride, "flag stride")?)
            .i32(int(flag_group, "flag group")?)
            .i32(int(ratio, "ratio")?)
            .i32(int(group, "group")?)
            .i32(int(top_k, "top k")?)
            .i32(i32::from(allow_inf));
        self.launch("v41_select", args, (tokens, 1, 1), SELECT_THREADS, 0)
    }

    /// Layer 20's candidate blocks, as `CudaSubmission::v41_candidate_blocks`.
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
        let positions = self.steps(steps)?;
        let t = steps.tokens;
        let score_ptr = self.span(scores.buffer, 0, t * scores.stride, 4, "scores")?;
        let block_scores = self.span(
            candidates.block_scores,
            0,
            t * candidates.stride,
            4,
            "block scores",
        )?;
        let flags = self.span(candidates.flags, 0, t * candidates.stride, 1, "flags")?;
        let blocks = self.span(
            candidates.blocks,
            0,
            t * candidates.blocks_stride,
            4,
            "blocks",
        )?;
        let args = Args::new()
            .ptr(score_ptr)
            .i64(stride(scores.stride, "score stride")?)
            .ptr(block_scores)
            .i64(stride(candidates.stride, "block stride")?)
            .ptr(positions)
            .i32(int(ratio, "ratio")?)
            .i32(int(block_size, "block size")?);
        self.launch(
            "v41_block_max",
            args,
            (candidates.stride.div_ceil(THREADS), t, 1),
            THREADS,
            0,
        )?;
        self.select(
            block_scores,
            candidates.stride,
            0,
            0,
            1,
            positions,
            t,
            ratio,
            block_size,
            top_blocks,
            true,
            blocks,
            candidates.blocks_stride,
            flags,
            candidates.stride,
        )
    }

    /// Selects each token's top `top_k` visible scores, as
    /// `CudaSubmission::v41_select_indices`.
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
        let positions = self.steps(steps)?;
        let t = steps.tokens;
        let score_ptr = self.span(scores.buffer, 0, t * scores.stride, 4, "scores")?;
        let (flags, flag_stride, flag_group) = match candidates {
            Some((flags, group)) => {
                if group == 0 || flags.stride * group < scores.stride {
                    return Err(invalid(format!(
                        "candidate flags of {} blocks of {group}",
                        flags.stride
                    )));
                }
                (
                    self.span(flags.buffer, 0, t * flags.stride, 1, "candidate flags")?,
                    flags.stride,
                    group,
                )
            }
            None => (0, 0, 1),
        };
        let out_ptr = self.span(out.buffer, 0, t * out.stride, 4, "selected indices")?;
        self.select(
            score_ptr,
            scores.stride,
            flags,
            flag_stride,
            flag_group,
            positions,
            t,
            ratio,
            1,
            top_k,
            false,
            out_ptr,
            out.stride,
            0,
            0,
        )
    }

    /// Latent sparse attention for each token and head, as
    /// `CudaSubmission::v41_sparse_attention`.
    ///
    /// # Errors
    ///
    /// Refuses a bad geometry, a ring shorter than the window, and buffers
    /// too small.
    pub fn v41_sparse_attention(
        &mut self,
        geometry: V41AttentionGeometry,
        query: &V41Buffer,
        sinks: &V41Buffer,
        ring: V41Ring<'_>,
        compressed: Option<V41Compressed<'_>>,
        steps: V41Steps<'_>,
        out: &V41Buffer,
    ) -> Result<(), RuntimeError> {
        let g = geometry;
        check_geometry(g)?;
        if ring.rows < g.window {
            return Err(invalid(format!("a ring of {} rows", ring.rows)));
        }
        let positions = self.steps(steps)?;
        let t = steps.tokens;
        let width = g.heads * g.head_dim;
        let query = self.span(query, 0, t * width, 4, "query")?;
        let sinks = self.span(sinks, 0, g.heads, 4, "sinks")?;
        let ring_ptr = self.span(ring.buffer, 0, ring.rows * g.head_dim, 2, "window ring")?;
        let out = self.span(out, 0, t * width, 4, "attention output")?;
        let (kv, capacity, indices, index_stride, ratio) = match compressed {
            Some(c) => {
                if !(1..=V41_MAX_RATIO).contains(&c.ratio) || c.indices.stride < g.index_top_k {
                    return Err(invalid(format!(
                        "compressed rows at ratio {} with index rows of {}",
                        c.ratio, c.indices.stride
                    )));
                }
                (
                    self.span(c.kv, 0, c.capacity * g.head_dim, 2, "compressed KV")?,
                    c.capacity,
                    self.span(c.indices.buffer, 0, t * c.indices.stride, 4, "indices")?,
                    c.indices.stride,
                    c.ratio,
                )
            }
            None => (0, 0, 0, 0, 0),
        };
        let max_slots = g.window + if ratio > 0 { g.index_top_k } else { 0 };
        let shared = (ATTENTION_HEADS * (g.head_dim + max_slots) + max_slots) * 4;
        let args = Args::new()
            .ptr(query)
            .ptr(sinks)
            .ptr(ring_ptr)
            .ptr(kv)
            .ptr(indices)
            .i64(stride(index_stride, "index stride")?)
            .ptr(positions)
            .ptr(out)
            .i32(int(ring.rows, "ring rows")?)
            .i32(int(capacity, "capacity")?)
            .i32(int(g.heads, "heads")?)
            .i32(int(g.head_dim, "head dim")?)
            .i32(int(g.window, "window")?)
            .i32(int(ratio, "ratio")?)
            .i32(int(g.index_top_k, "top k")?)
            .f32(g.attention_scale());
        self.launch(
            "v41_sparse_attention",
            args,
            (
                g.heads / ATTENTION_HEADS,
                g.head_dim.div_ceil(ATTENTION_SPLIT),
                t,
            ),
            256,
            shared,
        )
    }
}

#[cfg(test)]
#[path = "attention_tests.rs"]
mod tests;
