//! DSpark's window-only draft blocks and transactional verification (#12469).
//!
//! Reimplemented from DeepSeek-V4.1-Flash-1017's MIT inference/model.py.
//! Draft arithmetic can change a proposal; only the backbone decides which
//! tokens survive. A rejected block restores every persistent request buffer.

use super::*;
use crate::CudaCommandWait;

pub const BLOCK: usize = 5;
pub const VERIFY: usize = BLOCK + 1;
pub const TARGETS: [usize; 3] = [37, 38, 39];

unsafe extern "C" {
    fn psionic_v41_dspark_mean(
        input: *const c_void,
        output: *mut c_void,
        positions: *const c_void,
        tokens: i32,
        streams: i32,
        hidden: i32,
        window: i32,
        slot: i32,
        stream: *mut c_void,
    ) -> i32;
}

pub(super) struct Capture {
    targets: CudaBuffer,
    residual: CudaBuffer,
    mixes: CudaBuffer,
    logits: CudaBuffer,
    shape: V41StageShape,
}

impl Capture {
    pub(super) fn record(
        &self,
        s: &mut CudaSubmission,
        layer: usize,
        residual: &CudaBuffer,
        positions: &CudaBuffer,
        tokens: usize,
    ) -> Result<(), RuntimeError> {
        let Some(slot) = TARGETS.iter().position(|x| *x == layer) else {
            return Ok(());
        };
        let h = self.shape.hidden;
        let streams = self.shape.hc.streams;
        let window = self.shape.attention.window;
        let src = span(residual, 0, tokens * h * streams, 4, "DSpark streams")?;
        let dst = span(
            &self.targets,
            0,
            window * TARGETS.len() * h,
            4,
            "DSpark targets",
        )?;
        let pos = span(positions, 0, tokens, 4, "DSpark positions")?;
        // SAFETY: all extents were checked above; the kernel writes only
        // the last window of positions, with one writer per target word.
        let code = unsafe {
            psionic_v41_dspark_mean(
                src,
                dst,
                pos,
                int(tokens, "tokens")?,
                int(streams, "streams")?,
                int(h, "hidden")?,
                int(window, "window")?,
                int(slot, "target slot")?,
                s.platform.raw_stream()?,
            )
        };
        s.clef_launch(code, "DSpark target means")
    }

    pub(super) fn keep_tail(
        &self,
        s: &mut CudaSubmission,
        residual: &CudaBuffer,
        mixes: &CudaBuffer,
        tokens: usize,
    ) -> Result<(), RuntimeError> {
        let n = tokens.min(VERIFY);
        let r = self.shape.hc.elements() * 4;
        let m = self.shape.hc.mixes() * 4;
        s.copy_buffer_region(residual, (tokens - n) * r, &self.residual, 0, n * r)?;
        s.copy_buffer_region(mixes, (tokens - n) * m, &self.mixes, 0, n * m)
    }
}

/// Reusable device backups of all state that affects a later token.
/// Cache heat and timing counters describe work performed and are not rolled back.
pub struct Snapshot {
    buffers: Vec<(CudaBuffer, CudaBuffer)>,
    history: Vec<u32>,
    engram: Option<([u32; 3], usize)>,
}

impl V41Stage {
    /// Enables the three backbone feature rings and verification heads.
    /// Call before the first step, so captured graphs include the features.
    pub fn enable_dspark(&mut self, backend: &mut CudaBackend) -> Result<(), RuntimeError> {
        if self.config.layers != (0..40)
            || self.prefill.is_none()
            || self.config.max_step_tokens < VERIFY
        {
            return Err(invalid(
                "DSpark needs one whole-model stage, layer-major prefill, and at least six step tokens",
            ));
        }
        if self.body.is_some() || self.body_observed.is_some() {
            return Err(invalid("enable DSpark before the first step"));
        }
        let shape = self.config.shape;
        let mut f = |n| backend.byte_buffer(&vec![0; 4 * n]);
        self.dspark = Some(Capture {
            targets: f(shape.attention.window * TARGETS.len() * shape.hidden)?,
            residual: f(VERIFY * shape.hc.elements())?,
            mixes: f(VERIFY * shape.hc.mixes())?,
            logits: f(VERIFY * shape.vocabulary)?,
            shape,
        });
        Ok(())
    }

    /// Allocates a backup once; [`Self::save_dspark`] refreshes it per block.
    pub fn dspark_snapshot(&self, backend: &mut CudaBackend) -> Result<Snapshot, RuntimeError> {
        let mut state: Vec<&CudaBuffer> = Vec::new();
        for l in &self.layers {
            state.extend([&l.ring, &l.mixes_attn, &l.mixes_ffn]);
        }
        for source in self.sources.values() {
            state.extend([&source.kv, &source.index_keys]);
            state.extend(source.state.as_ref());
        }
        state.extend(self.selections.values());
        state.extend([
            &self.work.history,
            &self.work.flags,
            &self.work.blocks,
            &self.work.residual[0],
            &self.work.residual[1],
            &self.work.pre_in,
            &self.work.head_logits,
            &self.work.positions,
            &self.work.index,
        ]);
        if let Some(capture) = &self.dspark {
            state.push(&capture.targets);
        }
        let buffers = state
            .into_iter()
            .map(|b| {
                backend
                    .byte_buffer(&vec![0; b.byte_len()])
                    .map(|backup| (b.clone(), backup))
            })
            .collect::<Result<_, _>>()?;
        let mut snapshot = Snapshot {
            buffers,
            history: Vec::new(),
            engram: None,
        };
        self.save_dspark(backend, &mut snapshot)?;
        Ok(snapshot)
    }

    pub fn save_dspark(
        &self,
        backend: &CudaBackend,
        snapshot: &mut Snapshot,
    ) -> Result<(), RuntimeError> {
        let mut s = backend.begin_submission()?;
        for (live, backup) in &snapshot.buffers {
            s.copy_buffer(live, backup)?;
        }
        s.commit(CudaCommandWait::Completed)?;
        snapshot.history.clone_from(&self.history);
        snapshot.engram = self.engram_staging.as_ref().map(EngramStaging::checkpoint);
        Ok(())
    }

    pub fn restore_dspark(
        &mut self,
        backend: &CudaBackend,
        snapshot: &Snapshot,
    ) -> Result<(), RuntimeError> {
        let mut s = backend.begin_submission()?;
        for (live, backup) in &snapshot.buffers {
            s.copy_buffer(backup, live)?;
        }
        s.commit(CudaCommandWait::Completed)?;
        self.history.clone_from(&snapshot.history);
        if let (Some(staging), Some(history)) = (&mut self.engram_staging, snapshot.engram) {
            staging.restore(history);
        }
        Ok(())
    }

    /// Runs a block layer by layer and returns the target distribution
    /// after every input token, including the final bonus distribution.
    pub fn verify_dspark(
        &mut self,
        backend: &CudaBackend,
        position: usize,
        tokens: &[u32],
    ) -> Result<Vec<Vec<f32>>, RuntimeError> {
        if tokens.is_empty() || tokens.len() > VERIFY || self.dspark.is_none() {
            return Err(invalid("DSpark verification takes one to six tokens"));
        }
        self.step_with_output(
            backend,
            &V41StepInput {
                position,
                tokens,
                residual: None,
                pre_mix: None,
                source_rows: None,
                selection: None,
                candidates: None,
                sampling: V41Sampling::greedy(),
            },
            false,
            false,
        )?;
        let c = self
            .dspark
            .as_ref()
            .ok_or_else(|| invalid("DSpark capture"))?;
        let mut s = backend.begin_submission()?;
        for t in 0..tokens.len() {
            if tokens.len() > 1 {
                s.copy_buffer_region(
                    &c.residual,
                    4 * t * c.shape.hc.elements(),
                    &self.work.residual[0],
                    0,
                    4 * c.shape.hc.elements(),
                )?;
                s.copy_buffer_region(
                    &c.mixes,
                    4 * t * c.shape.hc.mixes(),
                    &self.work.pre_in,
                    0,
                    4 * c.shape.hc.mixes(),
                )?;
            }
            self.record_tail(&mut s)?;
            s.copy_buffer_region(
                &self.work.head_logits,
                0,
                &c.logits,
                4 * t * c.shape.vocabulary,
                4 * c.shape.vocabulary,
            )?;
        }
        s.commit(CudaCommandWait::Completed)?;
        let data = floats(&c.logits, tokens.len() * c.shape.vocabulary)?;
        Ok(data
            .chunks_exact(c.shape.vocabulary)
            .map(<[f32]>::to_vec)
            .collect())
    }
}

fn floats(buffer: &CudaBuffer, n: usize) -> Result<Vec<f32>, RuntimeError> {
    Ok(buffer
        .read_bytes_at_offset(0, 4 * n)?
        .chunks_exact(4)
        .map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]]))
        .collect())
}

fn upload(buffer: &mut CudaBuffer, values: &[f32]) -> Result<(), RuntimeError> {
    buffer.write_bytes_at_offset(0, &le_bytes(values, f32::to_le_bytes))
}

/// One layer loader with the draft checkpoint namespace in place of layer 0.
struct DraftSource<'a> {
    inner: &'a dyn V41TensorSource,
    layer: usize,
}
impl V41TensorSource for DraftSource<'_> {
    fn tensor(&self, name: &str) -> Result<V41HostTensor, RuntimeError> {
        let name = name.strip_prefix("layers.0.").map_or_else(
            || name.to_owned(),
            |part| format!("mtp.{}.{part}", self.layer),
        );
        self.inner.tensor(&name)
    }
}

/// The matching checkpoint's three window-only draft blocks. The shared
/// backbone embedding and output head stay shared on the device.
pub struct Drafter {
    blocks: Vec<V41Stage>,
    projection: Packed,
    main_norm: CudaBuffer,
    norm: CudaBuffer,
    markov_embed: CudaBuffer,
    markov_head: CudaBuffer,
    confidence: CudaBuffer,
    confidence_input: CudaBuffer,
    confidence_score: CudaBuffer,
    projected: CudaBuffer,
    hidden: CudaBuffer,
    mean: CudaBuffer,
    codes: CudaBuffer,
    scales: CudaBuffer,
    combined: CudaBuffer,
    query: CudaBuffer,
    residual: Vec<CudaBuffer>,
    mixes: Vec<CudaBuffer>,
    attention: Vec<CudaBuffer>,
    fake_position: CudaBuffer,
    head: CudaBuffer,
    seeded_until: usize,
    markov_rank: usize,
    noise: u32,
}

pub struct Proposal {
    pub logits: Vec<Vec<f32>>,
    pub confidence: Vec<f32>,
}

impl Drafter {
    /// Holds room while the backbone chooses its automatic expert cache.
    /// Dropping this allocation before loading the draft blocks leaves room
    /// for their native experts, dense weights, and verification snapshots.
    pub fn reservation(backend: &mut CudaBackend) -> Result<CudaBuffer, RuntimeError> {
        let device = backend
            .selected_device()
            .ok_or_else(|| invalid("CUDA device"))?
            .device
            .clone();
        backend.allocate_buffer(&TensorSpec::new(
            Shape::new(vec![3_usize << 30]),
            DType::I32,
            device,
        ))
    }

    pub fn load(
        backend: &mut CudaBackend,
        main: &V41Stage,
        source: &dyn V41TensorSource,
        markov_rank: usize,
        noise: u32,
    ) -> Result<Self, RuntimeError> {
        if main.dspark.is_none() {
            return Err(invalid("enable DSpark before loading its drafter"));
        }
        let mut shape = main.config.shape;
        shape.router.experts = shape.router.experts.min(128);
        shape.router.top_k = 3;
        let mut blocks = Vec::new();
        for layer in 0..TARGETS.len() {
            let mut block = V41Stage::load_with(
                backend,
                V41StageConfig {
                    shape,
                    layers: 0..1,
                    max_context: main.config.max_context,
                    max_step_tokens: VERIFY,
                    engram: Vec::new(),
                    token_map: None,
                    observe: false,
                    swap_rope_tables: false,
                    experts_on_host: false,
                },
                &DraftSource {
                    inner: source,
                    layer,
                },
                Vec::new(),
                V41StageOptions {
                    shared_embedding: main.embed.clone(),
                    ..V41StageOptions::default()
                },
            )?;
            // The loader above keeps the layer shape checks and native
            // packed formats; use the backbone's embedding allocation.
            block.embed = main.embed.clone();
            block.work.head_logits = backend.byte_buffer(&vec![0; 4 * shape.vocabulary])?;
            blocks.push(block);
        }
        let h = shape.hidden;
        if markov_rank == 0 || markov_rank > h || noise as usize >= shape.vocabulary {
            return Err(invalid("DSpark Markov rank or noise token"));
        }
        let mut loader = Loader {
            backend,
            source,
            bytes: 0,
            host_bytes: 0,
            params: 0,
            state: 0,
        };
        let projection =
            loader.packed("mtp.0.main_proj", V41MatvecFormat::Fp8Block32x32, h, 3 * h)?;
        let main_norm = loader.f32s("mtp.0.main_norm.weight", &[h], true)?;
        let norm = loader.f32s("mtp.2.norm.weight", &[h], true)?;
        let markov_embed = loader.bf16_words(
            "mtp.2.markov_head.embed.weight",
            &[shape.vocabulary, markov_rank],
        )?;
        let markov_head = loader.bf16_words(
            "mtp.2.markov_head.head.weight",
            &[shape.vocabulary, markov_rank],
        )?;
        let confidence = loader.f32s(
            "mtp.2.confidence_head.proj.weight",
            &[1, h + markov_rank],
            true,
        )?;
        let mut f = |n| loader.state_zeros(4 * n);
        Ok(Self {
            projection,
            main_norm,
            norm,
            markov_embed,
            markov_head,
            confidence,
            confidence_input: f(h + markov_rank)?,
            confidence_score: f(1)?,
            projected: f(h)?,
            hidden: f(h)?,
            mean: f(3 * h)?,
            codes: f((3 * h).div_ceil(4))?,
            scales: f((3 * h / 32).div_ceil(4))?,
            combined: f((shape.attention.window + BLOCK) * shape.attention.head_dim)?,
            query: f(BLOCK * shape.attention.heads * shape.attention.head_dim)?,
            residual: (0..BLOCK)
                .map(|_| f(shape.hc.elements()))
                .collect::<Result<_, _>>()?,
            mixes: (0..BLOCK)
                .map(|_| f(shape.hc.mixes()))
                .collect::<Result<_, _>>()?,
            attention: (0..BLOCK).map(|_| f(h)).collect::<Result<_, _>>()?,
            fake_position: f(1)?,
            head: main
                .head
                .as_ref()
                .ok_or_else(|| invalid("main head"))?
                .0
                .clone(),
            blocks,
            seeded_until: 0,
            markov_rank,
            noise,
        })
    }

    /// A new request starts its draft windows from the backbone's last
    /// window. Later calls add only the tokens the verifier committed.
    pub fn reset(&mut self) {
        self.seeded_until = 0;
    }

    fn seed(
        &mut self,
        backend: &CudaBackend,
        main: &V41Stage,
        end: usize,
    ) -> Result<(), RuntimeError> {
        let shape = main.config.shape;
        let g = shape.attention;
        let h = shape.hidden;
        let capture = main
            .dspark
            .as_ref()
            .ok_or_else(|| invalid("DSpark capture"))?;
        let first = self.seeded_until.max(end.saturating_sub(g.window));
        for position in first..end {
            let mut s = backend.begin_submission()?;
            s.copy_buffer_region(
                &capture.targets,
                4 * (position % g.window) * 3 * h,
                &self.mean,
                0,
                4 * 3 * h,
            )?;
            // The main projection needs more activation storage than a
            // normal hidden vector, so its codes use a dedicated buffer.
            let codes = &self.codes;
            let scales = &self.scales;
            s.v41_act_quant_fp8(&self.mean, codes, scales, 1, 3 * h)?;
            s.v41_block_matvec(
                self.projection.matrix(),
                codes,
                scales,
                V41Slots::single(),
                &self.projected,
                V41Output::Bf16,
            )?;
            s.v41_rms_norm(
                &self.projected,
                &self.main_norm,
                &self.hidden,
                1,
                h,
                g.norm_epsilon,
                V41Output::Bf16,
            )?;
            s.commit(CudaCommandWait::Completed)?;
            for block in &mut self.blocks {
                let wk = &block.work;
                let mut position_buffer = wk.positions.clone();
                position_buffer.write_bytes_at_offset(0, &(position as i32).to_le_bytes())?;
                let lw = &block.layers[0];
                let mut s = backend.begin_submission()?;
                s.v41_act_quant_fp8(&self.hidden, &wk.act_codes, &wk.act_scales, 1, h)?;
                s.v41_block_matvec(
                    lw.wkv.matrix(),
                    &wk.act_codes,
                    &wk.act_scales,
                    V41Slots::single(),
                    &wk.kv,
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
                    V41RopeTable {
                        frequencies: &block.rope_window,
                        dim: g.rope_dim,
                    },
                    &wk.kv_norm,
                    V41Steps {
                        positions: &wk.positions,
                        tokens: 1,
                    },
                    1,
                    g.head_dim,
                    0,
                    false,
                    V41Output::Bf16,
                )?;
                s.v41_window_store(
                    g,
                    &wk.kv_norm,
                    V41Steps {
                        positions: &wk.positions,
                        tokens: 1,
                    },
                    V41Ring {
                        buffer: &lw.ring,
                        rows: g.window,
                    },
                )?;
                s.commit(CudaCommandWait::Completed)?;
            }
        }
        self.seeded_until = end;
        Ok(())
    }

    /// Produces five logits conditioned on the backbone's accepted state.
    /// Markov biases are applied sequentially by [`Self::finish_token`].
    pub fn forward(
        &mut self,
        backend: &CudaBackend,
        main: &V41Stage,
        end: usize,
        first_token: u32,
    ) -> Result<Proposal, RuntimeError> {
        self.seed(backend, main, end)?;
        let shape = main.config.shape;
        let g = shape.attention;
        let h = shape.hidden;
        let embed = main.embed.as_ref().ok_or_else(|| invalid("embedding"))?;
        let initial_mix: Vec<f32> = (0..shape.hc.mixes())
            .map(|i| if i == 0 { 1.0 } else { 0.0 })
            .collect();
        for i in 0..BLOCK {
            let token = if i == 0 { first_token } else { self.noise };
            let row = embed.read_bytes_at_offset(token as usize * h * 2, h * 2)?;
            let values: Vec<f32> = row
                .chunks_exact(2)
                .map(|w| f32::from_bits(u32::from(u16::from_le_bytes([w[0], w[1]])) << 16))
                .collect();
            upload(&mut self.residual[i], &values.repeat(shape.hc.streams))?;
            upload(&mut self.mixes[i], &initial_mix)?;
        }
        let valid = end.min(g.window);
        self.fake_position
            .write_bytes_at_offset(0, &((valid + BLOCK - 1) as i32).to_le_bytes())?;
        for block in &mut self.blocks {
            let lw = &block.layers[0];
            let wk = &block.work;
            let mut position_buffer = wk.positions.clone();
            // DSpark attention is bidirectional within its draft block.
            // Build every Q/KV row before reading any of them.
            let mut s = backend.begin_submission()?;
            s.copy_buffer_region(&lw.ring, 0, &self.combined, 0, valid * g.head_dim * 2)?;
            s.commit(CudaCommandWait::Completed)?;
            // Window rows are BF16 words; attention's combined ring uses
            // the same representation, appended to by window_store below.
            for i in 0..BLOCK {
                position_buffer.write_bytes_at_offset(0, &((end + i) as i32).to_le_bytes())?;
                let mut s = backend.begin_submission()?;
                s.v41_hc_mixes(
                    shape.hc,
                    lw.hc_attn.weights(),
                    &self.residual[i],
                    &wk.scratch,
                    &lw.mixes_attn,
                )?;
                s.v41_hc_collapse(
                    shape.hc,
                    &self.residual[i],
                    &self.mixes[i],
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
                s.v41_act_quant_fp8(&wk.normalized, &wk.act_codes, &wk.act_scales, 1, h)?;
                for (matrix, out) in [(&lw.wq_a, &wk.q_low), (&lw.wkv, &wk.kv)] {
                    s.v41_block_matvec(
                        matrix.matrix(),
                        &wk.act_codes,
                        &wk.act_scales,
                        V41Slots::single(),
                        out,
                        V41Output::Bf16,
                    )?;
                }
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
                    V41Slots::single(),
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
                let steps = V41Steps {
                    positions: &wk.positions,
                    tokens: 1,
                };
                let rope = V41RopeTable {
                    frequencies: &block.rope_window,
                    dim: g.rope_dim,
                };
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
                s.v41_cache_quant(V41CacheQuant::Fp8, &wk.kv_norm, &wk.kv_norm, 1, g.head_dim)?;
                // Convert the already quantized float row to BF16 through
                // window_store, using a temporary position in the joined ring.
                s.copy_buffer_region(
                    &wk.q_heads,
                    0,
                    &self.query,
                    4 * i * g.heads * g.head_dim,
                    4 * g.heads * g.head_dim,
                )?;
                s.copy_buffer(&lw.mixes_attn, &self.mixes[i])?;
                s.commit(CudaCommandWait::Completed)?;
                position_buffer.write_bytes_at_offset(0, &((valid + i) as i32).to_le_bytes())?;
                let mut s = backend.begin_submission()?;
                let mut joined = g;
                joined.window = g.window + BLOCK;
                s.v41_window_store(
                    joined,
                    &wk.kv_norm,
                    V41Steps {
                        positions: &wk.positions,
                        tokens: 1,
                    },
                    V41Ring {
                        buffer: &self.combined,
                        rows: g.window + BLOCK,
                    },
                )?;
                s.commit(CudaCommandWait::Completed)?;
            }
            for i in 0..BLOCK {
                position_buffer.write_bytes_at_offset(0, &((end + i) as i32).to_le_bytes())?;
                let mut s = backend.begin_submission()?;
                s.copy_buffer_region(
                    &self.query,
                    4 * i * g.heads * g.head_dim,
                    &wk.q_heads,
                    0,
                    4 * g.heads * g.head_dim,
                )?;
                let mut joined = g;
                joined.window = valid + BLOCK;
                s.v41_sparse_attention(
                    joined,
                    &wk.q_heads,
                    &lw.sinks,
                    V41Ring {
                        buffer: &self.combined,
                        rows: g.window + BLOCK,
                    },
                    None,
                    V41Steps {
                        positions: &self.fake_position,
                        tokens: 1,
                    },
                    &wk.heads,
                )?;
                s.v41_rope(
                    V41RopeTable {
                        frequencies: &block.rope_window,
                        dim: g.rope_dim,
                    },
                    &wk.heads,
                    V41Steps {
                        positions: &wk.positions,
                        tokens: 1,
                    },
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
                    shape.o_groups,
                    g.heads * g.head_dim / shape.o_groups,
                )?;
                let rows = shape.o_groups * shape.o_rank;
                s.v41_dense(
                    V41Dense {
                        weights: &lw.wo_a,
                        scales: Some(&lw.wo_a_scales),
                        format: V41DenseFormat::Fp8Block,
                        rows,
                        cols: g.heads * g.head_dim / shape.o_groups,
                        groups: shape.o_groups,
                    },
                    &wk.heads_q,
                    &wk.low_rank,
                    1,
                    false,
                    V41Output::Bf16,
                )?;
                s.v41_act_quant_fp8(&wk.low_rank, &wk.act_codes, &wk.act_scales, 1, rows)?;
                s.v41_block_matvec(
                    lw.wo_b.matrix(),
                    &wk.act_codes,
                    &wk.act_scales,
                    V41Slots::single(),
                    &self.attention[i],
                    V41Output::Bf16,
                )?;
                s.v41_hc_expand(
                    shape.hc,
                    &self.attention[i],
                    &self.residual[i],
                    &self.mixes[i],
                    &wk.residual[0],
                    V41Output::Bf16,
                )?;
                s.v41_hc_mixes(
                    shape.hc,
                    lw.hc_ffn.weights(),
                    &wk.residual[0],
                    &wk.scratch,
                    &lw.mixes_ffn,
                )?;
                s.v41_hc_collapse(
                    shape.hc,
                    &wk.residual[0],
                    &self.mixes[i],
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
                s.commit(CudaCommandWait::Completed)?;
                let mut s = backend.begin_submission()?;
                block.record_moe(&mut s, lw)?;
                s.v41_hc_expand(
                    shape.hc,
                    &wk.moe_out,
                    &wk.residual[0],
                    &lw.mixes_ffn,
                    &self.residual[i],
                    V41Output::Bf16,
                )?;
                s.copy_buffer(&lw.mixes_ffn, &self.mixes[i])?;
                s.commit(CudaCommandWait::Completed)?;
            }
        }
        let block = self.blocks.last().ok_or_else(|| invalid("draft blocks"))?;
        let mut logits = Vec::new();
        let mut confidence = Vec::new();
        for i in 0..BLOCK {
            let wk = &block.work;
            let mut s = backend.begin_submission()?;
            s.v41_hc_collapse(
                shape.hc,
                &self.residual[i],
                &self.mixes[i],
                &self.hidden,
                V41Output::Bf16,
            )?;
            s.v41_rms_norm(
                &self.hidden,
                &self.norm,
                &wk.normalized,
                1,
                h,
                g.norm_epsilon,
                V41Output::Bf16,
            )?;
            s.v41_gate_logits(
                &self.head,
                &wk.normalized,
                &wk.head_logits,
                shape.vocabulary,
                h,
            )?;
            s.commit(CudaCommandWait::Completed)?;
            logits.push(floats(&wk.head_logits, shape.vocabulary)?);
            // Keep the pre-norm collapsed hidden for the confidence head.
            confidence.extend(floats(&self.hidden, h)?);
        }
        Ok(Proposal { logits, confidence })
    }

    /// Adds the Markov distribution for the preceding proposed token and
    /// evaluates confidence from the same Markov embedding.
    pub fn finish_token(
        &self,
        backend: &CudaBackend,
        token: u32,
        hidden: &[f32],
        logits: &mut [f32],
    ) -> Result<f32, RuntimeError> {
        let bytes = self
            .markov_embed
            .read_bytes_at_offset(token as usize * self.markov_rank * 2, self.markov_rank * 2)?;
        let embed: Vec<f32> = bytes
            .chunks_exact(2)
            .map(|x| f32::from_bits(u32::from(u16::from_le_bytes([x[0], x[1]])) << 16))
            .collect();
        let mut vector = self.projected.clone();
        upload(&mut vector, &embed)?;
        let last = self.blocks.last().ok_or_else(|| invalid("draft blocks"))?;
        let mut s = backend.begin_submission()?;
        s.v41_gate_logits(
            &self.markov_head,
            &vector,
            &last.work.head_logits,
            logits.len(),
            self.markov_rank,
        )?;
        s.commit(CudaCommandWait::Completed)?;
        let bias = floats(&last.work.head_logits, logits.len())?;
        for (logit, bias) in logits.iter_mut().zip(bias) {
            *logit += bias;
        }
        let mut input = self.confidence_input.clone();
        upload(
            &mut input,
            &hidden.iter().chain(&embed).copied().collect::<Vec<_>>(),
        )?;
        let mut s = backend.begin_submission()?;
        s.v41_dense(
            V41Dense {
                weights: &self.confidence,
                scales: None,
                format: V41DenseFormat::F32,
                rows: 1,
                cols: hidden.len() + embed.len(),
                groups: 1,
            },
            &input,
            &self.confidence_score,
            1,
            false,
            V41Output::F32,
        )?;
        s.commit(CudaCommandWait::Completed)?;
        Ok(floats(&self.confidence_score, 1)?[0])
    }
}
