//! Layer-major prefill with batched weight products (#12464).
//!
//! The ordinary step runs each prompt token through every layer before the
//! next token starts, so every weight is read once per token. Here a step's
//! tokens go through the stage layer by layer instead. Inside a layer, the
//! work that must see tokens in order (hyper-connection mixes, RoPE, the
//! window ring, the compressor, the indexer, attention, routing, and the
//! residual updates) still runs one token at a time, as captured graphs
//! replayed for each token with the same kernels as the ordinary step. The
//! projections between them run for all of the step's tokens at once
//! through `v41_block_matmul`, which reads each weight block once per eight
//! tokens; routed experts are grouped by expert first. Each product sums
//! exactly as the matrix-vector kernel does, so the step's results are
//! bit-equal to the ordinary step's.
//!
//! Prefill that reads every expert once per layer per prompt follows
//! Project Maya (`mw00/project-maya` at `451f9cb6a2`, `ds4_model.cu`
//! prefill; MIT), reimplemented for our stage.

use super::*;
use crate::v41_expert_cache::SendPtr;
use crate::v41_matvec::{V41_TILE_ROWS, V41Batch, V41BatchCache};

/// Device slots a prefill streams uncached experts through, with their
/// page-locked staging (#12464).
pub(super) struct Ring {
    slots: usize,
    /// `[w1, w2, w3]`: weights and scales of `slots` experts.
    stacks: [(CudaBuffer, CudaBuffer); 3],
    /// One `i32` per expert: a ring slot, or the host-lane code (skipped).
    map: CudaBuffer,
    /// `[w1 weights, w1 scales, w2 weights, w2 scales, w3 weights, w3
    /// scales]`, `slots` each.
    staging: Vec<CudaHostBuffer>,
    staging_ptrs: [SendPtr; 6],
    /// Experts streamed since load, and the seconds the host spent copying.
    pub(super) streamed: u64,
    pub(super) copy_seconds: f64,
}

/// Per-token arrays and graphs of the layer-major prefill.
pub(super) struct PrefillWork {
    /// Tokens a step may carry.
    max: usize,
    zero: CudaBuffer,
    pos: CudaBuffer,
    r: CudaBuffer,
    pre0: CudaBuffer,
    ma: CudaBuffer,
    mf: CudaBuffer,
    na: CudaBuffer,
    nf: CudaBuffer,
    q_low: CudaBuffer,
    q_norm: CudaBuffer,
    kv: CudaBuffer,
    kv_norm: CudaBuffer,
    qh: CudaBuffer,
    iq: CudaBuffer,
    hq: CudaBuffer,
    lr: CudaBuffer,
    ao: CudaBuffer,
    ids: CudaBuffer,
    rw: CudaBuffer,
    g: CudaBuffer,
    u: CudaBuffer,
    hh: CudaBuffer,
    y: CudaBuffer,
    sg: CudaBuffer,
    su: CudaBuffer,
    sh: CudaBuffer,
    sy: CudaBuffer,
    codes: CudaBuffer,
    scales: CudaBuffer,
    eids: CudaBuffer,
    /// Index selections per source layer, `[max, top_k]`.
    sel: BTreeMap<usize, CudaBuffer>,
    /// Layer 20's candidate blocks per token.
    cand: CudaBuffer,
    dense_entries: CudaBuffer,
    dense_tiles: CudaBuffer,
    dense_count: CudaBuffer,
    routed_entries: CudaBuffer,
    routed_tiles: CudaBuffer,
    routed_count: CudaBuffer,
    routed_scratch: CudaBuffer,
    reset: Option<CudaGraphExec>,
    begin: Option<CudaGraphExec>,
    pub(super) ring: Option<Ring>,
    /// The second residual array: sub-blocks write their output residual
    /// to the array they didn't read (#12464).
    r2: CudaBuffer,
    /// Per-token hyper-connection scratch, collapsed inputs, router logits,
    /// and MoE sums.
    hc_scratch: CudaBuffer,
    collapsed: CudaBuffer,
    logits: CudaBuffer,
    moe: CudaBuffer,
    /// Per Engram layer: each token's 24 row ids, gathered rows, and
    /// projections.
    eslot: Vec<CudaBuffer>,
    erows: CudaBuffer,
    eproj: CudaBuffer,
    /// The attention phase for every token at once: compressor values,
    /// gates, and latents, indexer weights and scores, candidate block
    /// scores and flags, and the attention heads.
    values: CudaBuffer,
    gates: CudaBuffer,
    latents: CudaBuffer,
    index_weights: CudaBuffer,
    scores: CudaBuffer,
    score_stride: usize,
    block_scores: CudaBuffer,
    flags: CudaBuffer,
    block_stride: usize,
    heads: CudaBuffer,
    /// Seconds in each phase of the layer-major prefill since load, in
    /// [`V41_PREFILL_PHASES`] order.
    pub(super) phase_seconds: Vec<f64>,
    /// Calibration sums for the `consumer-q` quantizer (#12465), per stage
    /// layer: inputs `[experts, hidden]` with counts, and hidden
    /// `[experts, width]` with counts.
    pub(super) calibration: Option<Vec<[CudaBuffer; 4]>>,
    /// Device bytes of all of the above.
    pub(super) bytes: usize,
}

impl std::fmt::Debug for PrefillWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefillWork")
            .field("max", &self.max)
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

fn words(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

impl PrefillWork {
    /// Allocates the arrays for steps of up to `max` tokens.
    pub(super) fn new(
        backend: &mut CudaBackend,
        config: &V41StageConfig,
        max: usize,
        stream: Option<([(usize, usize); 3], usize)>,
    ) -> Result<Self, RuntimeError> {
        let shape = config.shape;
        let g = shape.attention;
        let (h, s, k, e) = (
            shape.hidden,
            shape.hc.streams,
            shape.router.top_k,
            shape.expert_width,
        );
        let mixes = shape.hc.mixes();
        let wo_rows = shape.o_groups * shape.o_rank;
        let mut bytes = 0;
        let mut f = |n: usize| -> Result<CudaBuffer, RuntimeError> {
            let b = backend.byte_buffer(&vec![0; (4 * n).max(4)])?;
            bytes += b.byte_len();
            Ok(b)
        };
        let widest = [h, e, shape.q_rank, wo_rows].into_iter().max().unwrap_or(h);
        let mut sel = BTreeMap::new();
        for layer in config.layers.clone() {
            if V41_STAGE_INDEX_SOURCES.contains(&layer) {
                sel.insert(layer, f(max * g.index_top_k)?);
            }
        }
        // A boundary's incoming selection is kept per token the same way.
        if config.layers.start > 0 {
            if let Some(owner) = V41Boundary::at(config.layers.start).selection {
                if !sel.contains_key(&owner) {
                    sel.insert(owner, f(max * g.index_top_k)?);
                }
            }
        }
        let tiles = max.div_ceil(V41_TILE_ROWS);
        let dense_tiles: Vec<i32> = (0..tiles)
            .flat_map(|t| {
                let first = t * V41_TILE_ROWS;
                [0, first as i32, (max - first).min(V41_TILE_ROWS) as i32, 0]
            })
            .collect();
        let identity: Vec<i32> = (0..(max * k) as i32).collect();
        let routed_tiles = V41Batch::max_tiles(max * k, shape.router.experts);
        let mut this = Self {
            max,
            zero: f(1)?,
            pos: f(max)?,
            r: f(max * s * h)?,
            pre0: f(max * mixes)?,
            ma: f(max * mixes)?,
            mf: f(max * mixes)?,
            na: f(max * h)?,
            nf: f(max * h)?,
            q_low: f(max * shape.q_rank)?,
            q_norm: f(max * shape.q_rank)?,
            kv: f(max * g.head_dim)?,
            kv_norm: f(max * g.head_dim)?,
            qh: f(max * g.heads * g.head_dim)?,
            iq: f(max * g.index_heads * g.index_dim)?,
            hq: f(max * g.heads * g.head_dim)?,
            lr: f(max * wo_rows)?,
            ao: f(max * h)?,
            ids: f(max * k)?,
            rw: f(max * k)?,
            g: f(max * k * e)?,
            u: f(max * k * e)?,
            hh: f(max * k * e)?,
            y: f(max * k * h)?,
            sg: f(max * e)?,
            su: f(max * e)?,
            sh: f(max * e)?,
            sy: f(max * h)?,
            codes: f((max * k * widest).div_ceil(4))?,
            scales: f((max * k * widest / V41_BLOCK).div_ceil(4) + 1)?,
            eids: f(max * 2 * V41_ENGRAM_COLUMNS * 2)?,
            sel,
            cand: f(max * shape.candidate_blocks)?,
            dense_entries: f(max * k)?,
            dense_tiles: f(4 * tiles)?,
            dense_count: f(1)?,
            routed_entries: f(max * k)?,
            routed_tiles: f(4 * routed_tiles)?,
            routed_count: f(1)?,
            routed_scratch: f(2 * (shape.router.experts + 1))?,
            reset: None,
            begin: None,
            ring: None,
            r2: f(max * s * h)?,
            hc_scratch: f(max * shape.hc.scratch_floats())?,
            collapsed: f(max * h)?,
            logits: f(max * shape.router.experts)?,
            moe: f(max * h)?,
            eslot: (0..config.engram_layers().len())
                .map(|_| f(max * V41_ENGRAM_COLUMNS * 2))
                .collect::<Result<_, _>>()?,
            erows: f(max * V41_ENGRAM_COLUMNS * shape.engram_width)?,
            eproj: f(max * (s * h + h))?,
            values: f(max * g.head_dim)?,
            gates: f(max * g.head_dim)?,
            latents: f(max * g.head_dim)?,
            index_weights: f(max * g.index_heads)?,
            scores: f(max * (config.max_context + 1))?,
            score_stride: config.max_context + 1,
            block_scores: f(max * (config.max_context + 1).div_ceil(shape.candidate_block_size))?,
            flags: f(
                (max * (config.max_context + 1).div_ceil(shape.candidate_block_size)).div_ceil(4),
            )?,
            block_stride: (config.max_context + 1).div_ceil(shape.candidate_block_size),
            heads: f(max * g.heads * g.head_dim)?,
            phase_seconds: vec![0.0; V41_PREFILL_PHASES.len()],
            calibration: None,
            bytes: 0,
        };
        this.dense_entries
            .write_bytes_at_offset(0, &words(&identity))?;
        this.dense_tiles
            .write_bytes_at_offset(0, &words(&dense_tiles))?;
        if let Some((parts, slots)) = stream {
            let slots = slots.max(1);
            let mut stacks = Vec::new();
            let mut staging = Vec::new();
            let mut ptrs = Vec::new();
            for p in 0..3 {
                let (wb, sb) = parts[p];
                let w = backend.byte_buffer(&vec![0; wb * slots])?;
                let sc = backend.byte_buffer(&vec![0; sb * slots])?;
                bytes += w.byte_len() + sc.byte_len();
                stacks.push((w, sc));
                for n in [wb, sb] {
                    let host = backend.host_buffer(n * slots)?;
                    let ptr = host.raw_host_ptr().cast::<u8>();
                    if ptr.is_null() {
                        return Err(invalid("page-locked memory needs the Linux CUDA runtime"));
                    }
                    ptrs.push(SendPtr(ptr));
                    staging.push(host);
                }
            }
            let map = backend.byte_buffer(&vec![0; 4 * shape.router.experts])?;
            bytes += map.byte_len();
            this.ring = Some(Ring {
                slots,
                stacks: stacks
                    .try_into()
                    .map_err(|_| invalid("three projections"))?,
                map,
                staging,
                staging_ptrs: ptrs.try_into().map_err(|_| invalid("six parts"))?,
                streamed: 0,
                copy_seconds: 0.0,
            });
        }
        this.bytes = bytes;
        Ok(this)
    }

    fn dense(&self, tokens: usize) -> V41Batch<'_> {
        V41Batch {
            entries: &self.dense_entries,
            tiles: &self.dense_tiles,
            tile_count: &self.dense_count,
            max_tiles: tokens.div_ceil(V41_TILE_ROWS),
            count: tokens,
            top_k: 1,
            shared_input: false,
        }
    }
}

/// The phases of a layer-major prefill layer, for its timings.
pub const V41_PREFILL_PHASES: [&str; 10] = [
    "attention_mixes",
    "query_kv_projections",
    "query_kv_norms",
    "query_index_projections",
    "attention",
    "output_projection",
    "ffn_mixes_routing",
    "experts_device",
    "experts_streamed",
    "expert_sum",
];

impl V41Stage {
    /// Whether a step can run layer-major: a prompt step of more than one
    /// token, without observations, with batched prefill enabled, and with
    /// no cached expert on the host CPU lane.
    pub(super) fn can_prefill_batched(&self, tokens: usize, observe: bool) -> bool {
        self.prefill.is_some()
            && tokens > 1
            && !observe
            && self.expert_cache.as_ref().is_none_or(|c| {
                c.stats().miss_lane == Some(V41MissLane::Bus)
                    || self.prefill.as_ref().is_some_and(|p| p.ring.is_some())
            })
    }

    /// The routed experts of stage layer `i` that its cache doesn't hold,
    /// streamed through the prefill's ring: each is copied once from the
    /// cache's host source into page-locked staging (the next group while
    /// the device computes this one), copied into a ring slot, and its
    /// entries computed with the same batched products. The cached pass
    /// before this skipped exactly these entries.
    #[allow(clippy::too_many_lines)]
    fn stream_routed(
        &mut self,
        backend: &CudaBackend,
        i: usize,
        tokens: usize,
        stats: &mut V41StepStats,
    ) -> Result<(), RuntimeError> {
        let shape = self.config.shape;
        let (h, e, k) = (shape.hidden, shape.expert_width, shape.router.top_k);
        let experts = shape.router.experts;
        let model_layer = self.layers[i].layer;
        let Some(cache) = &self.expert_cache else {
            return Ok(());
        };
        if cache.stats().miss_lane != Some(V41MissLane::Host) {
            return Ok(());
        }
        let Some(p) = self.prefill.as_mut() else {
            return Ok(());
        };
        let Some(ring) = p.ring.as_mut() else {
            return Ok(());
        };
        let slot = cache
            .layer_slot(model_layer)
            .ok_or_else(|| invalid("layer missing from the expert cache"))?;
        let map = cache.map(slot);
        let entries = tokens * k;
        let ids: Vec<i32> = p
            .ids
            .read_bytes_at_offset(0, 4 * entries)?
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        stats.device_to_host_copies += 1;
        stats.device_to_host_bytes += 4 * entries;
        let mut missing: Vec<usize> = ids
            .iter()
            .filter_map(|id| usize::try_from(*id).ok())
            .filter(|id| *id < experts && map[*id] < 0)
            .collect();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return Ok(());
        }
        let streamer = cache.streamer(model_layer);
        let dims = streamer.dims;
        let ptrs = ring.staging_ptrs;
        let groups: Vec<Vec<usize>> = missing.chunks(ring.slots).map(<[usize]>::to_vec).collect();
        let started = Instant::now();
        streamer.copy(model_layer, &groups[0], &ptrs)?;
        ring.copy_seconds += started.elapsed().as_secs_f64();
        for (g, group) in groups.iter().enumerate() {
            // Staging into the ring, and the ring's map.
            for part in 0..6 {
                let (wb, sb) = dims.part_bytes(part / 2);
                let size = if part % 2 == 0 { wb } else { sb };
                let bytes = &ring.staging[part].read_bytes_prefix_view()[..size * group.len()];
                let (w, sc) = &mut ring.stacks[part / 2];
                if part % 2 == 0 {
                    w.write_bytes_at_offset(0, bytes)?;
                } else {
                    sc.write_bytes_at_offset(0, bytes)?;
                }
                stats.host_to_device_copies += 1;
                stats.host_to_device_bytes += bytes.len();
            }
            let mut ring_map = vec![crate::v41_expert_cache::V41_LANE_HOST; experts];
            for (j, expert) in group.iter().enumerate() {
                ring_map[*expert] = i32::try_from(j).map_err(|_| invalid("ring slot"))?;
            }
            ring.map.write_bytes_at_offset(0, &words(&ring_map))?;
            ring.streamed += group.len() as u64;
            let next = groups.get(g + 1);
            let lw = &self.layers[i];
            let (codes, scales, nf, g_out, u_out, hh, y, rw) =
                (&p.codes, &p.scales, &p.nf, &p.g, &p.u, &p.hh, &p.y, &p.rw);
            let routed = V41Batch {
                entries: &p.routed_entries,
                tiles: &p.routed_tiles,
                tile_count: &p.routed_count,
                max_tiles: V41Batch::max_tiles(entries, experts),
                count: entries,
                top_k: k,
                shared_input: true,
            };
            let ring_ref = &*ring;
            let ring_cache = |proj: usize| V41BatchCache {
                weights: &ring_ref.stacks[proj].0,
                scales: &ring_ref.stacks[proj].1,
                slots: ring_ref.slots,
                slot_map: &ring_ref.map,
            };
            let copied = std::thread::scope(|scope| -> Result<f64, RuntimeError> {
                // The next group's host copies run while the device computes
                // this one; staging is free once it's in the ring.
                let handle = next.map(|n| {
                    let streamer = &streamer;
                    scope.spawn(move || {
                        let started = Instant::now();
                        streamer
                            .copy(model_layer, n, &ptrs)
                            .map(|()| started.elapsed().as_secs_f64())
                    })
                });
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                s.v41_act_quant_fp8(nf, codes, scales, tokens, h)?;
                for (proj, out) in [(0, g_out), (2, u_out)] {
                    s.v41_block_matmul(
                        dims.formats[proj],
                        None,
                        Some(ring_cache(proj)),
                        e,
                        h,
                        experts,
                        routed,
                        codes,
                        scales,
                        tokens,
                        out,
                        entries,
                        V41Output::Bf16,
                    )?;
                }
                s.v41_swiglu(
                    g_out,
                    u_out,
                    Some(rw),
                    entries,
                    e,
                    shape.swiglu_limit,
                    hh,
                    V41Output::Bf16,
                )?;
                s.v41_act_quant_fp8(hh, codes, scales, entries, e)?;
                s.v41_block_matmul(
                    dims.formats[1],
                    None,
                    Some(ring_cache(1)),
                    h,
                    e,
                    experts,
                    V41Batch {
                        shared_input: false,
                        ..routed
                    },
                    codes,
                    scales,
                    entries,
                    y,
                    entries,
                    V41Output::Bf16,
                )?;
                s.commit(crate::CudaCommandWait::Completed)?;
                let _ = lw;
                match handle {
                    Some(h) => h.join().map_err(|_| invalid("a staging thread panicked"))?,
                    None => Ok(0.0),
                }
            })?;
            stats.synchronizations += 1;
            ring.copy_seconds += copied;
        }
        Ok(())
    }

    /// Copies `words` words of `src` into row `index` of `array`.
    fn store_row(
        &self,
        s: &mut CudaSubmission,
        src: &CudaBuffer,
        array: &CudaBuffer,
        words: usize,
    ) -> Result<(), RuntimeError> {
        let max = self.prefill.as_ref().map_or(1, |p| p.max);
        s.v41_stage_copy(src, array, &self.work.index, words, max, true)
    }

    /// The begin phase: the token's embedding or incoming residual, its
    /// position, and its Engram row ids.
    fn record_prefill_begin(&self, s: &mut CudaSubmission) -> Result<(), RuntimeError> {
        let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
        let c = &self.config;
        let shape = c.shape;
        let (h, streams) = (shape.hidden, shape.hc.streams);
        let wk = &self.work;
        let t = c.max_step_tokens;
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
        self.store_row(s, &wk.residual[0], &p.r, streams * h)?;
        self.store_row(s, &wk.pre_in, &p.pre0, shape.hc.mixes())?;
        self.store_row(s, &wk.positions, &p.pos, 1)?;
        if let (Some(map), Some(params)) = (&self.engram_map, &self.engram_params) {
            let token_map = c.token_map.as_ref().ok_or_else(|| invalid("token map"))?;
            let layers = c.engram_layers().len();
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
                layers,
                shape.engram_heads,
                &wk.engram_ids,
            )?;
            self.store_row(s, &wk.engram_ids, &p.eids, layers * V41_ENGRAM_COLUMNS * 2)?;
            for (slot, eslot) in p.eslot.iter().enumerate() {
                s.copy_buffer_region(
                    &wk.engram_ids,
                    slot * V41_ENGRAM_COLUMNS * 8,
                    &wk.engram_slot_ids,
                    0,
                    V41_ENGRAM_COLUMNS * 8,
                )?;
                self.store_row(s, &wk.engram_slot_ids, eslot, V41_ENGRAM_COLUMNS * 2)?;
            }
        }
        if let Some(boundary) = (c.layers.start > 0).then(|| V41Boundary::at(c.layers.start)) {
            if let Some(owner) = boundary.selection {
                let sel = p
                    .sel
                    .get(&owner)
                    .ok_or_else(|| invalid("selection array"))?;
                s.copy_buffer_region(
                    &wk.selection_in,
                    0,
                    sel,
                    0,
                    4 * c.max_step_tokens * shape.attention.index_top_k,
                )?;
            }
            if boundary.candidates {
                s.copy_buffer_region(
                    &wk.candidates_in,
                    0,
                    &p.cand,
                    0,
                    4 * c.max_step_tokens * shape.candidate_blocks,
                )?;
            }
        }
        s.v41_stage_advance(&wk.index)
    }

    /// Stage layer `i`'s attention for every token of the step at once:
    /// the attention kernels take a chunk of consecutive positions, so the
    /// window ring (sized for a whole step), the compressor, the indexer,
    /// and the selections run as one call each, in the same arithmetic as
    /// one token at a time.
    #[allow(clippy::too_many_lines)]
    fn attention_batched(
        &mut self,
        backend: &CudaBackend,
        i: usize,
        tokens: usize,
        stats: &mut V41StepStats,
    ) -> Result<(), RuntimeError> {
        let shape = self.config.shape;
        let g = shape.attention;
        let h = shape.hidden;
        let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
        let lw = &self.layers[i];
        let layer = lw.layer;
        let rope = self.rope(layer);
        let ratio = v41_compress_ratio(layer).ok_or_else(|| invalid("layer"))?;
        let owners = V41LayerOwners::for_layer(layer).ok_or_else(|| invalid("layer"))?;
        let steps = V41Steps {
            positions: &p.pos,
            tokens,
        };
        let mut s = backend.begin_submission()?;
        s.set_v41_fast_reductions(self.fast_reductions);
        s.set_v41_packed_kv(self.packed_kv);
        s.v41_rope(
            rope,
            &p.qh,
            steps,
            g.heads,
            g.head_dim,
            0,
            false,
            V41Output::Bf16,
        )?;
        s.v41_rope(
            rope,
            &p.kv_norm,
            steps,
            1,
            g.head_dim,
            0,
            false,
            V41Output::Bf16,
        )?;
        let ring = V41Ring {
            buffer: &lw.ring,
            rows: self.ring_rows,
        };
        s.v41_window_store(g, &p.kv_norm, steps, ring)?;
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
                &p.na,
                &p.values,
                tokens,
                round_input,
                output,
            )?;
            if let Some(wgate) = &cw.wgate {
                s.v41_dense(dense(wgate), &p.na, &p.gates, tokens, round_input, output)?;
            }
            s.v41_compress(
                V41Compressor {
                    ratio,
                    width: g.head_dim,
                    epsilon: g.norm_epsilon,
                },
                &p.values,
                (ratio > 1).then_some(&p.gates),
                &cw.norm,
                state,
                steps,
                &p.latents,
            )?;
            s.v41_compressed_store(
                g,
                ratio,
                V41IndexKeyWeights {
                    wk: &cw.wk,
                    k_norm: &cw.k_norm,
                },
                rope,
                &p.latents,
                steps,
                source.source(),
            )?;
        }
        let compressed = match owners.kv {
            Some(owner) => {
                let source = self
                    .sources
                    .get(&owner)
                    .ok_or_else(|| invalid("KV owner"))?;
                let index_owner = owners.index.ok_or_else(|| invalid("index owner"))?;
                let selection = p
                    .sel
                    .get(&index_owner)
                    .ok_or_else(|| invalid("selection array"))?;
                if let Some(ix) = &lw.indexer {
                    s.v41_rope(
                        rope,
                        &p.iq,
                        steps,
                        g.index_heads,
                        g.index_dim,
                        0,
                        false,
                        V41Output::Bf16,
                    )?;
                    s.v41_cache_quant(
                        V41CacheQuant::IndexFp4,
                        &p.iq,
                        &p.iq,
                        g.index_heads * tokens,
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
                        &p.na,
                        &p.index_weights,
                        tokens,
                        true,
                        V41Output::Bf16,
                    )?;
                    let scores = V41Rows {
                        buffer: &p.scores,
                        stride: source.capacity.min(p.score_stride),
                    };
                    s.v41_index_scores(
                        g,
                        &p.iq,
                        &p.index_weights,
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
                                block_scores: &p.block_scores,
                                flags: &p.flags,
                                blocks: &p.cand,
                                stride: p.block_stride,
                                blocks_stride: shape.candidate_blocks,
                            },
                            shape.candidate_blocks,
                            shape.candidate_block_size,
                        )?;
                    } else if layer > V41_STAGE_CANDIDATE_SOURCE
                        && !self.config.layers.contains(&V41_STAGE_CANDIDATE_SOURCE)
                    {
                        // Layer 20 ran in an earlier stage: its kept blocks
                        // arrived with the step; rebuild every token's flags.
                        s.v41_candidate_flags_tokens(
                            &p.cand,
                            tokens,
                            shape.candidate_blocks,
                            &p.flags,
                            p.block_stride,
                        )?;
                    }
                    s.v41_select_indices(
                        scores,
                        source.ratio,
                        steps,
                        (layer > V41_STAGE_CANDIDATE_SOURCE).then_some((
                            V41Rows {
                                buffer: &p.flags,
                                stride: p.block_stride,
                            },
                            shape.candidate_block_size,
                        )),
                        g.index_top_k,
                        V41Rows {
                            buffer: selection,
                            stride: g.index_top_k,
                        },
                    )?;
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
        s.v41_sparse_attention(g, &p.qh, &lw.sinks, ring, compressed, steps, &p.heads)?;
        s.v41_rope(
            rope,
            &p.heads,
            steps,
            g.heads,
            g.head_dim,
            0,
            true,
            V41Output::Bf16,
        )?;
        s.v41_cache_quant(
            V41CacheQuant::Fp8,
            &p.heads,
            &p.hq,
            g.heads * tokens,
            g.head_dim,
        )?;
        s.commit(crate::CudaCommandWait::Completed)?;
        stats.synchronizations += 1;
        Ok(())
    }

    /// Adds the time since `since` to phase `phase` of the layer-major
    /// prefill's timings and returns now. Each phase ends in a completed
    /// commit, so the times are the device's plus the host's encoding.
    fn prefill_lap(&mut self, since: std::time::Instant, phase: usize) -> std::time::Instant {
        let now = std::time::Instant::now();
        if let Some(p) = self.prefill.as_mut()
            && let Some(t) = p.phase_seconds.get_mut(phase)
        {
            *t += (now - since).as_secs_f64();
        }
        now
    }

    /// Launches the begin graph (each token's embedding or incoming
    /// residual, position, and Engram ids) once for each of `tokens` tokens
    /// from index 0, capturing it on first use.
    fn replay_begin(
        &mut self,
        backend: &CudaBackend,
        tokens: usize,
        stats: &mut V41StepStats,
    ) -> Result<(), RuntimeError> {
        // Index 0 first.
        let reset = self
            .prefill
            .as_ref()
            .and_then(|p| p.reset.as_ref())
            .is_some();
        if !reset {
            let mut s = backend.begin_captured_submission()?;
            let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
            s.copy_buffer(&p.zero, &self.work.index)?;
            let (_, exec) = s.commit_captured(crate::CudaCommandWait::Completed)?;
            stats.graph_captures += 1;
            self.prefill
                .as_mut()
                .ok_or_else(|| invalid("prefill"))?
                .reset = Some(exec);
        } else {
            self.prefill
                .as_ref()
                .and_then(|p| p.reset.as_ref())
                .ok_or_else(|| invalid("reset"))?
                .launch_enqueued()?;
        }
        stats.graph_launches += 1;
        let captured = self.prefill.as_ref().is_some_and(|p| p.begin.is_some());
        let mut remaining = tokens;
        if !captured {
            let mut s = backend.begin_captured_submission()?;
            self.record_prefill_begin(&mut s)?;
            let (_, exec) = s.commit_captured(crate::CudaCommandWait::Completed)?;
            stats.graph_captures += 1;
            stats.graph_launches += 1;
            stats.synchronizations += 1;
            remaining -= 1;
            self.prefill
                .as_mut()
                .ok_or_else(|| invalid("prefill"))?
                .begin = Some(exec);
        }
        let graph = self
            .prefill
            .as_ref()
            .and_then(|p| p.begin.as_ref())
            .ok_or_else(|| invalid("graph"))?;
        for _ in 0..remaining {
            graph.launch_enqueued()?;
            stats.graph_launches += 1;
        }
        Ok(())
    }

    /// Runs a step's tokens layer by layer (see the module documentation).
    /// The caller has uploaded the step's input; this leaves the stage's
    /// state and the step's outputs where the ordinary step leaves them.
    #[allow(clippy::too_many_lines)]
    pub(super) fn prefill_layer_major(
        &mut self,
        backend: &CudaBackend,
        tokens: usize,
        emit: bool,
        stats: &mut V41StepStats,
    ) -> Result<(), RuntimeError> {
        let shape = self.config.shape;
        let g = shape.attention;
        let (h, e, k) = (shape.hidden, shape.expert_width, shape.router.top_k);
        let experts = shape.router.experts;
        let wo_rows = shape.o_groups * shape.o_rank;
        {
            let p = self.prefill.as_mut().ok_or_else(|| invalid("prefill"))?;
            if tokens > p.max {
                return Err(invalid(format!(
                    "{tokens} tokens exceed the prefill's {}",
                    p.max
                )));
            }
            let count =
                i32::try_from(tokens.div_ceil(V41_TILE_ROWS)).map_err(|_| invalid("tiles"))?;
            p.dense_count
                .write_bytes_at_offset(0, &count.to_le_bytes())?;
        }
        self.replay_begin(backend, tokens, stats)?;
        let fp8 = V41MatvecFormat::Fp8Block32x32;
        let fp4 = V41MatvecFormat::Fp4E2M1Row32;
        // The residual sits in `prefill.r` (cur 0) or `prefill.r2` (cur 1);
        // each sub-block writes the array it didn't read.
        let mut cur = 0_usize;
        let hc = shape.hc;
        let (streams, mixes) = (hc.streams, hc.mixes());
        for i in 0..self.layers.len() {
            let mut lap = std::time::Instant::now();
            // Engram, the attention mixes, collapse, and norm, for every
            // token at once.
            {
                let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
                let lw = &self.layers[i];
                let r = [&p.r, &p.r2];
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                if let Some(capture) = &self.dspark {
                    capture.record(&mut s, lw.layer, r[cur], &p.pos, tokens)?;
                }
                if let Some(eng) = &lw.engram {
                    let rows = tokens * V41_ENGRAM_COLUMNS;
                    let (ids, table) = match &self.engram_staging {
                        Some(staging) => {
                            let t = staging
                                .tables
                                .iter()
                                .find(|t| t.layer == lw.layer)
                                .ok_or_else(|| invalid("Engram staging table"))?;
                            (
                                &staging.ids,
                                V41EngramTable {
                                    codes: &t.codes,
                                    scales: &t.scales,
                                    rows: (self.config.max_step_tokens * V41_ENGRAM_COLUMNS) as u64,
                                    width: shape.engram_width,
                                },
                            )
                        }
                        None => {
                            let t = self
                                .engram_tables
                                .iter()
                                .find(|t| t.layer == lw.layer)
                                .ok_or_else(|| invalid("Engram table"))?;
                            (
                                &p.eslot[eng.slot],
                                V41EngramTable {
                                    codes: &t.codes,
                                    scales: &t.scales,
                                    rows: t.rows,
                                    width: shape.engram_width,
                                },
                            )
                        }
                    };
                    s.v41_engram_gather(table, ids, rows, &p.erows, V41Output::Bf16)?;
                    let kk = V41_ENGRAM_COLUMNS * shape.engram_width;
                    s.v41_act_quant_fp8(&p.erows, &p.codes, &p.scales, tokens, kk)?;
                    s.v41_block_matmul(
                        fp8,
                        Some(eng.wkv.matrix()),
                        None,
                        streams * h + h,
                        kk,
                        1,
                        p.dense(tokens),
                        &p.codes,
                        &p.scales,
                        tokens,
                        &p.eproj,
                        tokens,
                        V41Output::Bf16,
                    )?;
                    s.v41_engram_gate_tokens(
                        V41EngramGate {
                            streams,
                            width: h,
                            epsilon: g.norm_epsilon,
                        },
                        r[cur],
                        &p.eproj,
                        &eng.q,
                        &eng.k,
                        r[1 - cur],
                        V41Output::Bf16,
                        tokens,
                    )?;
                    cur = 1 - cur;
                }
                let previous = if i == 0 { &p.pre0 } else { &p.mf };
                s.v41_hc_mixes_tokens(
                    hc,
                    lw.hc_attn.weights(),
                    r[cur],
                    &p.hc_scratch,
                    &p.ma,
                    tokens,
                )?;
                s.v41_hc_collapse_tokens(
                    hc,
                    r[cur],
                    previous,
                    &p.collapsed,
                    V41Output::Bf16,
                    tokens,
                    mixes,
                )?;
                s.v41_rms_norm(
                    &p.collapsed,
                    &lw.attn_norm,
                    &p.na,
                    tokens,
                    h,
                    g.norm_epsilon,
                    V41Output::Bf16,
                )?;
                s.commit(crate::CudaCommandWait::Completed)?;
                lap = self.prefill_lap(lap, 0);
                stats.synchronizations += 1;
            }
            {
                let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
                let lw = &self.layers[i];
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                s.v41_act_quant_fp8(&p.na, &p.codes, &p.scales, tokens, h)?;
                s.v41_block_matmul(
                    fp8,
                    Some(lw.wq_a.matrix()),
                    None,
                    shape.q_rank,
                    h,
                    1,
                    p.dense(tokens),
                    &p.codes,
                    &p.scales,
                    tokens,
                    &p.q_low,
                    tokens,
                    V41Output::Bf16,
                )?;
                s.v41_block_matmul(
                    fp8,
                    Some(lw.wkv.matrix()),
                    None,
                    g.head_dim,
                    h,
                    1,
                    p.dense(tokens),
                    &p.codes,
                    &p.scales,
                    tokens,
                    &p.kv,
                    tokens,
                    V41Output::Bf16,
                )?;
                s.commit(crate::CudaCommandWait::Completed)?;
                lap = self.prefill_lap(lap, 1);
                stats.synchronizations += 1;
            }
            {
                // The query and KV norms, for every token at once.
                let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
                let lw = &self.layers[i];
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                s.v41_rms_norm(
                    &p.q_low,
                    &lw.q_norm,
                    &p.q_norm,
                    tokens,
                    shape.q_rank,
                    g.norm_epsilon,
                    V41Output::Bf16,
                )?;
                s.v41_rms_norm(
                    &p.kv,
                    &lw.kv_norm,
                    &p.kv_norm,
                    tokens,
                    g.head_dim,
                    g.norm_epsilon,
                    V41Output::Bf16,
                )?;
                s.commit(crate::CudaCommandWait::Completed)?;
                lap = self.prefill_lap(lap, 2);
                stats.synchronizations += 1;
            }
            {
                let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
                let lw = &self.layers[i];
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                s.v41_act_quant_fp8(&p.q_norm, &p.codes, &p.scales, tokens, shape.q_rank)?;
                s.v41_block_matmul(
                    fp8,
                    Some(lw.wq_b.matrix()),
                    None,
                    g.heads * g.head_dim,
                    shape.q_rank,
                    1,
                    p.dense(tokens),
                    &p.codes,
                    &p.scales,
                    tokens,
                    &p.qh,
                    tokens,
                    V41Output::Bf16,
                )?;
                if let Some(ix) = &lw.indexer {
                    s.v41_block_matmul(
                        fp8,
                        Some(ix.wq_b.matrix()),
                        None,
                        g.index_heads * g.index_dim,
                        shape.q_rank,
                        1,
                        p.dense(tokens),
                        &p.codes,
                        &p.scales,
                        tokens,
                        &p.iq,
                        tokens,
                        V41Output::Bf16,
                    )?;
                }
                s.commit(crate::CudaCommandWait::Completed)?;
                lap = self.prefill_lap(lap, 3);
                stats.synchronizations += 1;
            }
            self.attention_batched(backend, i, tokens, stats)?;
            lap = self.prefill_lap(lap, 4);
            {
                let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
                let lw = &self.layers[i];
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                s.v41_dense(
                    V41Dense {
                        weights: &lw.wo_a,
                        scales: Some(&lw.wo_a_scales),
                        format: V41DenseFormat::Fp8Block,
                        rows: wo_rows,
                        cols: g.heads * g.head_dim / shape.o_groups,
                        groups: shape.o_groups,
                    },
                    &p.hq,
                    &p.lr,
                    tokens,
                    false,
                    V41Output::Bf16,
                )?;
                s.v41_act_quant_fp8(&p.lr, &p.codes, &p.scales, tokens, wo_rows)?;
                s.v41_block_matmul(
                    fp8,
                    Some(lw.wo_b.matrix()),
                    None,
                    h,
                    wo_rows,
                    1,
                    p.dense(tokens),
                    &p.codes,
                    &p.scales,
                    tokens,
                    &p.ao,
                    tokens,
                    V41Output::Bf16,
                )?;
                s.commit(crate::CudaCommandWait::Completed)?;
                lap = self.prefill_lap(lap, 5);
                stats.synchronizations += 1;
            }
            {
                // The attention expansion, the FFN mixes, collapse, norm, and
                // routing, for every token at once.
                let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
                let lw = &self.layers[i];
                let r = [&p.r, &p.r2];
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                s.v41_hc_expand_tokens(
                    hc,
                    &p.ao,
                    r[cur],
                    &p.ma,
                    r[1 - cur],
                    V41Output::Bf16,
                    tokens,
                    mixes,
                )?;
                let after = 1 - cur;
                s.v41_hc_mixes_tokens(
                    hc,
                    lw.hc_ffn.weights(),
                    r[after],
                    &p.hc_scratch,
                    &p.mf,
                    tokens,
                )?;
                s.v41_hc_collapse_tokens(
                    hc,
                    r[after],
                    &p.ma,
                    &p.collapsed,
                    V41Output::Bf16,
                    tokens,
                    mixes,
                )?;
                s.v41_rms_norm(
                    &p.collapsed,
                    &lw.ffn_norm,
                    &p.nf,
                    tokens,
                    h,
                    g.norm_epsilon,
                    V41Output::Bf16,
                )?;
                s.v41_gate_logits_tokens(&lw.gate, &p.nf, &p.logits, experts, h, tokens)?;
                s.v41_route_tokens(shape.router, &p.logits, &lw.bias, &p.ids, &p.rw, tokens)?;
                s.commit(crate::CudaCommandWait::Completed)?;
                lap = self.prefill_lap(lap, 6);
                stats.synchronizations += 1;
            }
            cur = 1 - cur;
            {
                let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
                let lw = &self.layers[i];
                let entries = tokens * k;
                let routed = V41Batch {
                    entries: &p.routed_entries,
                    tiles: &p.routed_tiles,
                    tile_count: &p.routed_count,
                    max_tiles: V41Batch::max_tiles(entries, experts),
                    count: entries,
                    top_k: k,
                    shared_input: true,
                };
                let cached = self.expert_cache.as_ref().and_then(|cache| {
                    cache
                        .layer_slot(lw.layer)
                        .map(|slot| (cache, &cache.caches[slot]))
                });
                let cache_for = |p: usize| {
                    cached.map(|(cache, lc)| V41BatchCache {
                        weights: &lc.slots[p].0,
                        scales: &lc.slots[p].1,
                        slots: cache.slots(),
                        slot_map: &lc.slot_map,
                    })
                };
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                s.v41_act_quant_fp8(&p.nf, &p.codes, &p.scales, tokens, h)?;
                if let Some(cal) = p.calibration.as_ref().map(|c| &c[i]) {
                    s.v41_importance(
                        &p.ids, entries, k, true, &p.codes, &p.scales, h, experts, &cal[0], &cal[1],
                    )?;
                }
                s.v41_group_entries(
                    &p.ids,
                    entries,
                    experts,
                    &p.routed_scratch,
                    &p.routed_entries,
                    &p.routed_tiles,
                    &p.routed_count,
                )?;
                let routed_format = |proj: usize| {
                    cached.map_or(fp4, |(cache, _)| cache.layer_dims(lw.layer).formats[proj])
                };
                for (proj, packed, out) in [(0, &lw.w1, &p.g), (2, &lw.w3, &p.u)] {
                    s.v41_block_matmul(
                        routed_format(proj),
                        packed.as_ref().map(Packed::matrix),
                        cache_for(proj),
                        e,
                        h,
                        experts,
                        routed,
                        &p.codes,
                        &p.scales,
                        tokens,
                        out,
                        entries,
                        V41Output::Bf16,
                    )?;
                }
                // The shared expert reads the same activations.
                s.v41_block_matmul(
                    fp8,
                    Some(lw.shared[0].matrix()),
                    None,
                    e,
                    h,
                    1,
                    p.dense(tokens),
                    &p.codes,
                    &p.scales,
                    tokens,
                    &p.sg,
                    tokens,
                    V41Output::Bf16,
                )?;
                s.v41_block_matmul(
                    fp8,
                    Some(lw.shared[2].matrix()),
                    None,
                    e,
                    h,
                    1,
                    p.dense(tokens),
                    &p.codes,
                    &p.scales,
                    tokens,
                    &p.su,
                    tokens,
                    V41Output::Bf16,
                )?;
                s.v41_swiglu(
                    &p.g,
                    &p.u,
                    Some(&p.rw),
                    entries,
                    e,
                    shape.swiglu_limit,
                    &p.hh,
                    V41Output::Bf16,
                )?;
                s.v41_act_quant_fp8(&p.hh, &p.codes, &p.scales, entries, e)?;
                s.v41_block_matmul(
                    routed_format(1),
                    lw.w2.as_ref().map(Packed::matrix),
                    cache_for(1),
                    h,
                    e,
                    experts,
                    V41Batch {
                        shared_input: false,
                        ..routed
                    },
                    &p.codes,
                    &p.scales,
                    entries,
                    &p.y,
                    entries,
                    V41Output::Bf16,
                )?;
                s.v41_swiglu(
                    &p.sg,
                    &p.su,
                    None,
                    tokens,
                    e,
                    shape.swiglu_limit,
                    &p.sh,
                    V41Output::Bf16,
                )?;
                s.v41_act_quant_fp8(&p.sh, &p.codes, &p.scales, tokens, e)?;
                s.v41_block_matmul(
                    fp8,
                    Some(lw.shared[1].matrix()),
                    None,
                    h,
                    e,
                    1,
                    p.dense(tokens),
                    &p.codes,
                    &p.scales,
                    tokens,
                    &p.sy,
                    tokens,
                    V41Output::Bf16,
                )?;
                s.commit(crate::CudaCommandWait::Completed)?;
                lap = self.prefill_lap(lap, 7);
                stats.synchronizations += 1;
            }
            self.stream_routed(backend, i, tokens, stats)?;
            if let Some(p) = self.prefill.as_ref().filter(|p| p.calibration.is_some()) {
                let cal = &p
                    .calibration
                    .as_ref()
                    .ok_or_else(|| invalid("calibration"))?[i];
                let mut s = backend.begin_submission()?;
                s.v41_act_quant_fp8(&p.hh, &p.codes, &p.scales, tokens * k, e)?;
                s.v41_importance(
                    &p.ids,
                    tokens * k,
                    k,
                    false,
                    &p.codes,
                    &p.scales,
                    e,
                    experts,
                    &cal[2],
                    &cal[3],
                )?;
                s.commit(crate::CudaCommandWait::Completed)?;
            }
            lap = self.prefill_lap(lap, 8);
            {
                // The expert sum and the FFN expansion, for every token.
                let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
                let r = [&p.r, &p.r2];
                let mut s = backend.begin_submission()?;
                s.set_v41_fast_reductions(self.fast_reductions);
                s.set_v41_packed_kv(self.packed_kv);
                s.v41_moe_combine_tokens(&p.y, k, Some(&p.sy), h, &p.moe, V41Output::Bf16, tokens)?;
                s.v41_hc_expand_tokens(
                    hc,
                    &p.moe,
                    r[cur],
                    &p.mf,
                    r[1 - cur],
                    V41Output::Bf16,
                    tokens,
                    mixes,
                )?;
                s.commit(crate::CudaCommandWait::Completed)?;
                self.prefill_lap(lap, 9);
                stats.synchronizations += 1;
            }
            cur = 1 - cur;
        }
        // Leave the step's outputs where the ordinary step leaves them.
        let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
        let c = &self.config;
        let last = tokens - 1;
        let mut s = backend.begin_submission()?;
        s.set_v41_fast_reductions(self.fast_reductions);
        s.set_v41_packed_kv(self.packed_kv);
        let previous = if self.layers.is_empty() {
            &p.pre0
        } else {
            &p.mf
        };
        let residual = if cur == 0 { &p.r } else { &p.r2 };
        if let Some(capture) = &self.dspark {
            capture.keep_tail(&mut s, residual, previous, tokens)?;
        }
        if c.samples() {
            s.copy_buffer_region(
                residual,
                4 * last * streams * h,
                &self.work.residual[0],
                0,
                4 * streams * h,
            )?;
            s.copy_buffer_region(previous, 4 * last * mixes, &self.work.pre_in, 0, 4 * mixes)?;
        } else {
            for t in 0..tokens {
                s.copy_buffer_region(
                    residual,
                    4 * t * streams * h,
                    &self.work.residual_out,
                    4 * t * streams * h,
                    4 * streams * h,
                )?;
                s.copy_buffer_region(
                    previous,
                    4 * t * mixes,
                    &self.work.pre_mix_out,
                    4 * t * streams,
                    4 * streams,
                )?;
            }
            let out = V41Boundary::at(c.layers.end);
            if let Some(owner) = out.selection {
                if let Some(sel) = p.sel.get(&owner) {
                    s.copy_buffer_region(
                        sel,
                        0,
                        &self.work.selection_out,
                        0,
                        4 * tokens * g.index_top_k,
                    )?;
                }
            }
            if out.candidates && c.layers.contains(&V41_STAGE_CANDIDATE_SOURCE) {
                s.copy_buffer_region(
                    &p.cand,
                    0,
                    &self.work.candidates_out,
                    0,
                    4 * tokens * shape.candidate_blocks,
                )?;
            }
        }
        s.commit(crate::CudaCommandWait::Completed)?;
        stats.synchronizations += 1;
        let _ = emit;
        Ok(())
    }
}

#[cfg(test)]
impl V41Stage {
    /// The prefill's per-token arrays after a step, for diagnostics:
    /// `(name, words per token, values)`.
    pub(super) fn prefill_arrays(
        &self,
        tokens: usize,
    ) -> Result<Vec<(&'static str, Vec<u32>)>, RuntimeError> {
        let p = self.prefill.as_ref().ok_or_else(|| invalid("prefill"))?;
        let shape = self.config.shape;
        let g = shape.attention;
        let h = shape.hidden;
        let read = |b: &CudaBuffer, words: usize| -> Result<Vec<u32>, RuntimeError> {
            Ok(b.read_bytes_at_offset(0, 4 * words * tokens)?
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect())
        };
        Ok(vec![
            ("na", read(&p.na, h)?),
            ("q_low", read(&p.q_low, shape.q_rank)?),
            ("kv", read(&p.kv, g.head_dim)?),
            ("q_norm", read(&p.q_norm, shape.q_rank)?),
            ("qh", read(&p.qh, g.heads * g.head_dim)?),
            ("hq", read(&p.hq, g.heads * g.head_dim)?),
            ("ao", read(&p.ao, h)?),
            ("nf", read(&p.nf, h)?),
            ("ids", read(&p.ids, shape.router.top_k)?),
        ])
    }
}

impl V41Stage {
    /// Starts collecting calibration sums for the `consumer-q` quantizer
    /// (#12465) during layer-major prompt steps, including streamed experts.
    /// Each 32-column group keeps its full activation second moments for
    /// GPTQ feedback. Reserve `4 × experts × (hidden + width) × 32` bytes
    /// per layer when choosing an expert cache size.
    ///
    /// # Errors
    ///
    /// Refuses a stage without layer-major prefill or enough device memory.
    pub fn enable_calibration(&mut self, backend: &mut CudaBackend) -> Result<(), RuntimeError> {
        let shape = self.config.shape;
        let (h, e, n) = (shape.hidden, shape.expert_width, shape.router.experts);
        let layers = self.layers.len();
        let p = self
            .prefill
            .as_mut()
            .ok_or_else(|| invalid("calibration needs the layer-major prefill"))?;
        let mut all = Vec::with_capacity(layers);
        for _ in 0..layers {
            all.push([
                backend.byte_buffer(&vec![0; 4 * n * h * V41_BLOCK])?,
                backend.byte_buffer(&vec![0; 4 * n])?,
                backend.byte_buffer(&vec![0; 4 * n * e * V41_BLOCK])?,
                backend.byte_buffer(&vec![0; 4 * n])?,
            ]);
        }
        p.calibration = Some(all);
        Ok(())
    }

    /// The calibration so far, by model layer: per expert the mean square
    /// of each input column (`[experts × hidden]`) and of each hidden
    /// column (`[experts × width]`).
    ///
    /// # Errors
    ///
    /// Any device read failure.
    pub fn calibration(
        &self,
    ) -> Result<Option<BTreeMap<usize, (Vec<f32>, Vec<f32>)>>, RuntimeError> {
        Ok(self.calibration_hessians()?.map(|layers| {
            layers
                .into_iter()
                .map(|(layer, (input, hidden))| {
                    let diagonal = |data: Vec<f32>| {
                        data.chunks_exact(V41_BLOCK * V41_BLOCK)
                            .flat_map(|h| (0..V41_BLOCK).map(move |j| h[j * V41_BLOCK + j]))
                            .collect()
                    };
                    (layer, (diagonal(input), diagonal(hidden)))
                })
                .collect()
        }))
    }

    /// Per-expert 32-column activation second moments, normalized by the
    /// number of routed entries. Both parts use `[expert, group, 32, 32]`.
    ///
    /// # Errors
    ///
    /// Returns any device read failure.
    pub fn calibration_hessians(
        &self,
    ) -> Result<Option<BTreeMap<usize, (Vec<f32>, Vec<f32>)>>, RuntimeError> {
        let Some(cal) = self.prefill.as_ref().and_then(|p| p.calibration.as_ref()) else {
            return Ok(None);
        };
        let floats = |b: &CudaBuffer| -> Result<Vec<f32>, RuntimeError> {
            Ok(b.read_bytes()?
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect())
        };
        let mut out = BTreeMap::new();
        for (lw, bufs) in self.layers.iter().zip(cal) {
            let mut parts = Vec::new();
            for (sums, counts) in [(&bufs[0], &bufs[1]), (&bufs[2], &bufs[3])] {
                let (sums, counts) = (floats(sums)?, floats(counts)?);
                let cols = sums.len() / counts.len().max(1);
                parts.push(
                    sums.chunks(cols.max(1))
                        .zip(&counts)
                        .flat_map(|(row, n)| {
                            row.iter().map(move |v| if *n > 0.0 { v / n } else { 0.0 })
                        })
                        .collect::<Vec<f32>>(),
                );
            }
            let hidden = parts.pop().unwrap_or_default();
            let inputs = parts.pop().unwrap_or_default();
            out.insert(lw.layer, (inputs, hidden));
        }
        Ok(Some(out))
    }
}
