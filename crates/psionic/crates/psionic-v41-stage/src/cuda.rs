//! `--backend cuda`: W7's device-resident stage (`psionic_backend_cuda::
//! v41_stage::V41Stage`, #12394) behind W9's [`StageExecutor`].
//!
//! The executor maps a stage message onto W7's step input and its output
//! onto the sections W9's contract names for the next boundary:
//!
//! | W9 section | W7 field | Wire dtype |
//! | --- | --- | --- |
//! | `Residual` | `residual`, `[tokens, streams × hidden]` | `F32` |
//! | `PreMix` | `pre_mix`, `[tokens, streams]` | `F32` |
//! | `SourceKv { layer }` | `source_rows.kv`, BF16 words | `Bf16` |
//! | `SourceIndexKeys { layer }` | `source_rows.index_keys` | `Bf16` |
//! | `IndexSelection { layer }` | `selection` | `I32` |
//! | `Candidates { 20 }` | `candidates` (`-1` after the kept blocks) | `U32` (`u32::MAX`) |
//! | `EngramHistory` | `engram_history` | `U32` |
//!
//! The residual crosses as `F32`, so a split group equals the unsplit stage
//! bit for bit, as W7's split test does. The cache rows cross in their
//! native BF16. The last stage samples on the device with the step's
//! settings and returns the token, not logits. Because the CPU executor sends
//! `F32` cache rows and logits, a group runs one backend; a CUDA stage
//! announces [`crate::RUNTIME_CUDA`].
//!
//! A weights directory with [`crate::SYNTHETIC_FILE`] loads a tiny synthetic
//! model in checkpoint form ([`synthetic`]), with V4.1's 40-layer layout,
//! FP8 and FP4 blocks, and a 64-token vocabulary (`t0` to `t63`).

pub mod chain;
pub mod consumer_q;
pub mod dspark;
pub mod synthetic;
pub mod tiers;

use std::{cell::Cell, fmt, fs::File, ops::Range, os::unix::fs::FileExt, path::Path, sync::Arc};

use psionic_backend_cuda::{
    CudaBackend,
    v41_stage::{
        V41EngramHash, V41EngramHostTable, V41HostTensor, V41Sampling, V41SourceRows, V41Stage,
        V41StageConfig, V41StageShape, V41StepInput, V41TensorDtype, V41TensorSource, V41TokenMap,
    },
};
use psionic_models::{
    deepseek_v41_cpu::engram::{CompressedTokenMap, EngramLayout},
    deepseek_v41_stage::{V41GroupManifest, V41StageEntry, V41StageGeometry, stage_contract},
    deepseek_v41_weights::{DeepSeekV41Checkpoint, SafetensorsDtype, V41StageSelection},
    stage_state::{
        CodecLimits, RowRule, Section, SectionKind, StageContract, StageExecutor, StageInbox,
        StageMessage, StageWorker, WireDtype,
    },
};
use psionic_runtime::RuntimeError;
use serde_json::Value;
use tokenizers::Tokenizer;

use crate::{
    Backend, Control, Options, PromptMode, ReferenceAnswer, Refused, SYNTHETIC_FILE, Sampling,
    StageBackend, TextStream, Vocabulary, Weights, check_weights, load_model_for, serve,
};

/// The longest Engram n-gram; the history carries one fewer token.
pub(crate) const MAX_NGRAM: u32 = 4;

/// Why the CUDA executor failed a step.
#[derive(Debug)]
pub struct CudaStageError(pub String);

impl fmt::Display for CudaStageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CudaStageError {}

fn fail(message: impl fmt::Display) -> CudaStageError {
    CudaStageError(message.to_string())
}

/// The wire geometry of a CUDA stage: `F32` residuals, BF16 cache rows.
#[must_use]
pub fn wire_geometry(shape: &V41StageShape) -> V41StageGeometry {
    let g = shape.attention;
    V41StageGeometry {
        streams: shape.hc.streams as u32,
        hidden: shape.hidden as u32,
        head_dim: g.head_dim as u32,
        index_dim: g.index_dim as u32,
        index_top_k: g.index_top_k as u32,
        candidate_blocks: shape.candidate_blocks as u32,
        candidate_block_rows: shape.candidate_block_size as u32,
        max_ngram: MAX_NGRAM,
        vocabulary: shape.vocabulary as u32,
        residual_dtype: WireDtype::F32,
        cache_dtype: WireDtype::Bf16,
    }
}

/// W9's [`StageExecutor`] over W7's [`V41Stage`] on one GPU.
pub struct V41CudaStageExecutor {
    backend: CudaBackend,
    stage: V41Stage,
    geometry: V41StageGeometry,
    index: u16,
    layers: Range<u32>,
    next: Option<StageContract>,
    sampling: Cell<V41Sampling>,
    sampled: Cell<Option<u32>>,
    emit: Cell<bool>,
    /// Qualification: step with per-layer observations.
    observe: bool,
    /// Qualification: the stale-cache negative control continues each new
    /// request after the previous one's positions.
    stale_cache: bool,
    /// The end position of the previous request, and the current request's
    /// offset, for the stale-cache control.
    reached: usize,
    offset: usize,
}

impl V41CudaStageExecutor {
    /// An executor for stage `index` of a loaded stage.
    #[must_use]
    pub fn new(backend: CudaBackend, stage: V41Stage, index: u16) -> Self {
        let geometry = wire_geometry(&stage.config().shape);
        let observe = stage_observes(&stage);
        let layers = stage.config().layers.start as u32..stage.config().layers.end as u32;
        let next = (!stage.config().samples())
            .then(|| stage_contract(&geometry, index + 1, layers.end..layers.end + 1));
        Self {
            backend,
            stage,
            geometry,
            index,
            layers,
            next,
            sampling: Cell::new(V41Sampling::greedy()),
            sampled: Cell::new(None),
            emit: Cell::new(true),
            observe,
            stale_cache: false,
            reached: 0,
            offset: 0,
        }
    }

    /// Turns on the stale-cache negative control (#12405).
    #[must_use]
    pub fn with_stale_cache(mut self) -> Self {
        self.stale_cache = true;
        self
    }

    /// This stage's inbound contract.
    #[must_use]
    pub fn contract(&self) -> StageContract {
        stage_contract(&self.geometry, self.index, self.layers.clone())
    }

    /// The loaded stage.
    #[must_use]
    pub fn stage(&self) -> &V41Stage {
        &self.stage
    }

    /// The stage's CUDA backend.
    #[must_use]
    pub fn backend(&self) -> &CudaBackend {
        &self.backend
    }

    fn output(
        &self,
        out: psionic_backend_cuda::v41_stage::V41StepOutput,
        position: u64,
        tokens: usize,
    ) -> Result<Vec<Section>, CudaStageError> {
        output_sections(self.next.as_ref(), self.geometry, &out, position, tokens).map_err(fail)
    }
}

/// The sections a device stage sends the next stage, from its step output:
/// W9's contract on the CUDA wire. Shared by the CUDA and Metal executors.
pub(crate) fn output_sections(
    next: Option<&StageContract>,
    g: V41StageGeometry,
    out: &psionic_backend_cuda::v41_stage::V41StepOutput,
    position: u64,
    tokens: usize,
) -> Result<Vec<Section>, String> {
    let Some(next) = next else {
        return Ok(Vec::new());
    };
    let t = tokens as u32;
    let mut sections = Vec::with_capacity(next.sections.len());
    for spec in &next.sections {
        let section = match spec.kind {
            SectionKind::Residual => Section::floats(
                spec.kind,
                spec.dtype,
                vec![t, g.streams, g.hidden],
                0,
                &out.residual,
            ),
            SectionKind::PreMix => {
                Section::floats(spec.kind, spec.dtype, vec![t, g.streams], 0, &out.pre_mix)
            }
            SectionKind::SourceKv { layer } | SectionKind::SourceIndexKeys { layer } => {
                let keys = matches!(spec.kind, SectionKind::SourceIndexKeys { .. });
                let width = if keys { g.index_dim } else { g.head_dim };
                let rows = out
                    .source_rows
                    .as_ref()
                    .filter(|r| r.layer == layer as usize);
                let (first_row, words): (u64, &[u16]) = match rows {
                    Some(rows) if keys => (
                        rows.first_row as u64,
                        rows.index_keys
                            .as_deref()
                            .ok_or_else(|| format!("no index keys of KV source {layer}"))?,
                    ),
                    Some(rows) => (rows.first_row as u64, &rows.kv),
                    None => {
                        // No new rows this step.
                        let ratio = match spec.rows {
                            RowRule::Append { ratio } => u64::from(ratio.max(1)),
                            _ => 1,
                        };
                        (position / ratio, &[])
                    }
                };
                if spec.dtype != WireDtype::Bf16 {
                    return Err("device cache rows cross as BF16".into());
                }
                Ok(Section {
                    kind: spec.kind,
                    dtype: WireDtype::Bf16,
                    shape: vec![(words.len() / width as usize) as u32, width],
                    first_row,
                    data: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
                })
            }
            SectionKind::IndexSelection { layer } => {
                let rows = out
                    .selection
                    .as_ref()
                    .filter(|(l, _)| *l == layer as usize)
                    .ok_or_else(|| format!("no selection of layer {layer}"))?;
                Section::i32s(spec.kind, vec![t, g.index_top_k], &rows.1)
            }
            SectionKind::Candidates { .. } => {
                let blocks = out
                    .candidates
                    .as_ref()
                    .ok_or_else(|| String::from("no layer-20 candidates to forward"))?;
                // `-1` after the kept blocks is W9's `u32::MAX` pad.
                let ids: Vec<u32> = blocks.iter().map(|b| *b as u32).collect();
                Section::u32s(spec.kind, vec![t, g.candidate_blocks], &ids)
            }
            SectionKind::EngramHistory => {
                let rows = match spec.rows {
                    RowRule::Fixed(rows) => rows as usize,
                    _ => (MAX_NGRAM - 1) as usize,
                };
                let history = &out.engram_history;
                let mut values = vec![u32::MAX; rows.saturating_sub(history.len())];
                values.extend(history.iter().skip(history.len().saturating_sub(rows)));
                Section::u32s(spec.kind, vec![rows as u32], &values)
            }
            SectionKind::SourceValue { .. } | SectionKind::Logits => {
                return Err(format!("V4.1 stages don't send {}", spec.kind));
            }
        };
        sections.push(section.map_err(|e| e.to_string())?);
    }
    Ok(sections)
}

/// What a device stage takes from an inbound message on the CUDA wire.
pub(crate) struct Inbound {
    pub(crate) residual: Option<Vec<f32>>,
    pub(crate) pre_mix: Option<Vec<f32>>,
    pub(crate) selection: Option<Vec<i32>>,
    pub(crate) candidates: Option<Vec<i32>>,
    pub(crate) rows: Option<V41SourceRows>,
}

/// Reads a message's sections for a device stage. Shared by the CUDA and
/// Metal executors.
pub(crate) fn inbound(message: &StageMessage) -> Result<Inbound, String> {
    let (mut residual, mut pre_mix, mut selection, mut candidates) = (None, None, None, None);
    let mut rows: Option<V41SourceRows> = None;
    for section in &message.sections {
        match section.kind {
            SectionKind::Residual => residual = Some(section.to_f32().map_err(|e| e.to_string())?),
            SectionKind::PreMix => pre_mix = Some(section.to_f32().map_err(|e| e.to_string())?),
            SectionKind::SourceKv { layer } | SectionKind::SourceIndexKeys { layer } => {
                let words = bf16_words(section)?;
                let entry = rows.get_or_insert_with(|| V41SourceRows {
                    layer: layer as usize,
                    first_row: section.first_row as usize,
                    kv: Vec::new(),
                    index_keys: None,
                });
                if entry.layer != layer as usize || entry.first_row as u64 != section.first_row {
                    return Err("KV rows and index keys of different sources".into());
                }
                if matches!(section.kind, SectionKind::SourceKv { .. }) {
                    entry.kv = words;
                } else {
                    entry.index_keys = Some(words);
                }
            }
            SectionKind::IndexSelection { .. } => {
                selection = Some(section.to_i32().map_err(|e| e.to_string())?);
            }
            SectionKind::Candidates { .. } => {
                candidates = Some(
                    section
                        .to_u32()
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .map(|b| b as i32)
                        .collect::<Vec<i32>>(),
                );
            }
            // The stage keeps its own n-gram history from the tokens.
            SectionKind::EngramHistory | SectionKind::SourceValue { .. } | SectionKind::Logits => {}
        }
    }
    Ok(Inbound {
        residual,
        pre_mix,
        selection,
        candidates,
        rows,
    })
}

fn bf16_words(section: &Section) -> Result<Vec<u16>, String> {
    if section.dtype != WireDtype::Bf16 {
        return Err(format!(
            "{} arrived as {:?}; a device group sends BF16 cache rows",
            section.kind, section.dtype
        ));
    }
    Ok(section
        .data
        .chunks_exact(2)
        .map(|w| u16::from_le_bytes([w[0], w[1]]))
        .collect())
}

impl StageExecutor for V41CudaStageExecutor {
    type Error = CudaStageError;

    fn step(&mut self, message: &StageMessage) -> Result<Vec<Section>, Self::Error> {
        self.sampled.set(None);
        let Inbound {
            residual,
            pre_mix,
            selection,
            candidates,
            rows,
        } = inbound(message).map_err(fail)?;
        let mut position =
            usize::try_from(message.position).map_err(|_| fail("position out of range"))?;
        if self.stale_cache {
            // The deliberate defect: a new request keeps the last one's cache.
            if position == 0 {
                self.offset = self.reached;
            }
            position += self.offset;
            self.reached = position + message.tokens.len();
        }
        let input = V41StepInput {
            position,
            tokens: &message.tokens,
            residual: residual.as_deref(),
            pre_mix: pre_mix.as_deref(),
            source_rows: rows.as_ref(),
            selection: selection.as_deref(),
            candidates: candidates.as_deref(),
            sampling: self.sampling.get(),
        };
        let out = self
            .stage
            .step_with_output(&self.backend, &input, self.observe, self.emit.get())
            .map_err(|e| fail(format!("the CUDA stage failed: {e}")))?;
        self.sampled.set(None);
        if self.next.is_none() && self.emit.get() {
            let token = out
                .token
                .ok_or_else(|| fail("the last stage sampled no token"))?;
            self.sampled.set(Some(token));
        }
        self.output(out, message.position, message.tokens.len())
    }

    fn reset(&mut self, _request_id: &str) {
        // A request restarts at position 0, which clears the device state.
        self.sampled.set(None);
    }
}

impl StageBackend for V41CudaStageExecutor {
    fn geometry(&self) -> V41StageGeometry {
        self.geometry
    }

    fn prepare(&self, sampling: Sampling, _step: u64) {
        self.emit.set(true);
        // The device draw is seeded by the seed and the token's position.
        self.sampling.set(V41Sampling {
            temperature: sampling.temperature as f32,
            top_p: sampling.top_p as f32,
            seed: u64::from(sampling.seed),
        });
    }

    fn sampled(&self) -> Option<u32> {
        self.sampled.get()
    }

    fn prepare_output(&self, sampling: Sampling, step: u64, emit: bool) {
        self.prepare(sampling, step);
        self.emit.set(emit);
    }

    fn logits(&self) -> Option<Vec<f32>> {
        self.next
            .is_none()
            .then(|| self.stage.logits().ok())
            .flatten()
    }

    fn observations(&self, tokens: usize) -> Option<Vec<(String, Vec<u32>)>> {
        if !self.observe {
            return None;
        }
        let observed = self.stage.observations(tokens).ok()?;
        Some(
            observed
                .into_iter()
                .flat_map(|(layer, sites)| {
                    sites
                        .into_iter()
                        .map(move |(site, words)| (format!("L{layer}.{site:?}"), words))
                })
                .collect(),
        )
    }
}

fn stage_observes(stage: &V41Stage) -> bool {
    stage.config().observe
}

/// A tensor source with one of the G1 v2 negative controls' weight defects
/// (#12405): layer 3's shared expert output projection with its rows rotated by one
/// 32-row block, or layer 3's router bias rotated by one expert.
pub(crate) struct Controlled<'a> {
    pub(crate) inner: &'a dyn V41TensorSource,
    pub(crate) control: Option<Control>,
}

impl V41TensorSource for Controlled<'_> {
    fn tensor(&self, name: &str) -> Result<V41HostTensor, RuntimeError> {
        match self.control {
            Some(Control::WrongWeights)
                if name == "layers.3.ffn.shared_experts.w2.weight"
                    || name == "layers.3.ffn.shared_experts.w2.scale" =>
            {
                // Rotate the output rows by one 32-row block, with their scales.
                let mut tensor = self.inner.tensor(name)?;
                let rows = tensor.shape.first().copied().unwrap_or(1).max(1);
                let row_bytes = tensor.bytes.len() / rows;
                let block = if name.ends_with(".weight") { 32 } else { 1 };
                let shift = (row_bytes * block) % tensor.bytes.len().max(1);
                tensor.bytes.rotate_left(shift);
                Ok(tensor)
            }
            Some(Control::WrongRouting) if name == "layers.3.ffn.gate.bias" => {
                let mut tensor = self.inner.tensor(name)?;
                let width = tensor.bytes.len() / tensor.shape.iter().product::<usize>().max(1);
                tensor.bytes.rotate_left(width);
                Ok(tensor)
            }
            _ => self.inner.tensor(name),
        }
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// A V4.1 checkpoint as W7's tensor source.
pub struct CheckpointSource<'a>(pub &'a DeepSeekV41Checkpoint);

impl V41TensorSource for CheckpointSource<'_> {
    fn tensor(&self, name: &str) -> Result<V41HostTensor, RuntimeError> {
        let e = |error: &dyn fmt::Display| RuntimeError::Backend(format!("{name}: {error}"));
        let entry = self.0.entry(name).map_err(|x| e(&x))?;
        let dtype = match entry.dtype {
            SafetensorsDtype::F32 => V41TensorDtype::F32,
            SafetensorsDtype::Bf16 => V41TensorDtype::Bf16,
            SafetensorsDtype::F8E4M3 => V41TensorDtype::F8E4M3,
            SafetensorsDtype::F8E8M0 => V41TensorDtype::F8E8M0,
            SafetensorsDtype::I8 => V41TensorDtype::I8,
        };
        Ok(V41HostTensor {
            dtype,
            shape: entry.shape.iter().map(|d| *d as usize).collect(),
            bytes: self.0.read_tensor(name).map_err(|x| e(&x))?,
        })
    }
}

/// Copies one Engram layer's table from the checkpoint into page-locked
/// host memory, in 256 MiB reads.
fn engram_table(
    checkpoint: &DeepSeekV41Checkpoint,
    backend: &mut CudaBackend,
    layer: usize,
) -> Result<V41EngramHostTable, Refused> {
    let fill = |backend: &mut CudaBackend, name: &str| {
        let bad = |e: &dyn fmt::Display| Refused(format!("Engram table {name}: {e}"));
        let entry = checkpoint.entry(name).map_err(|e| bad(&e))?;
        let shard = checkpoint
            .shard_of(name)
            .ok_or_else(|| Refused(format!("Engram table {name} has no shard")))?;
        let file = File::open(checkpoint.root().join(shard)).map_err(|e| bad(&e))?;
        let len = usize::try_from(entry.byte_len).map_err(|e| bad(&e))?;
        let mut host = backend.host_buffer(len).map_err(|e| bad(&e))?;
        let mut buffer = vec![0_u8; (256 << 20).min(len.max(1))];
        let mut done = 0;
        while done < len {
            let n = buffer.len().min(len - done);
            file.read_exact_at(&mut buffer[..n], entry.file_offset + done as u64)
                .map_err(|e| bad(&e))?;
            host.write_bytes_at_offset(done, &buffer[..n])
                .map_err(|e| bad(&e))?;
            done += n;
        }
        Ok(host)
    };
    let weight = format!("layers.{layer}.engram.embed.weight");
    let codes = fill(backend, &weight)?;
    let scales = fill(backend, &format!("layers.{layer}.engram.embed.scale"))?;
    let rows = checkpoint
        .entry(&weight)
        .map_err(|e| Refused(e.to_string()))?
        .shape[0];
    Ok(V41EngramHostTable {
        layer,
        codes,
        scales,
        rows,
    })
}

pub(crate) fn engram_hashes(layout: &EngramLayout) -> Vec<V41EngramHash> {
    layout
        .layers
        .iter()
        .map(|l| V41EngramHash {
            layer: l.layer_id as usize,
            multipliers: l.multipliers,
            primes: l.primes.clone(),
            offsets: l.offsets.clone(),
            rows: l.table_rows,
        })
        .collect()
}

pub(crate) fn token_map(map: &CompressedTokenMap, pad: u32) -> Result<V41TokenMap, Refused> {
    Ok(V41TokenMap {
        ids: map.ids().to_vec(),
        pad: map.get(pad).map_err(|e| Refused(e.to_string()))?,
    })
}

/// What a CUDA stage announces about its device (#12418): its class
/// (`cuda-sm_NN`, from the architecture the kernels were compiled for), the
/// GPU's name and memory, and where the routed experts live.
fn device_info(backend: &CudaBackend, experts: crate::Experts) -> Value {
    let gpu = backend.selected_device();
    serde_json::json!({
        "class": crate::device_class(Backend::Cuda),
        "name": gpu.and_then(|d| d.device_name.clone()),
        "memory_bytes": gpu.and_then(|d| d.memory_capacity_bytes),
        "experts": experts.as_str(),
    })
}

fn cuda_backend() -> Result<CudaBackend, Refused> {
    let backend = CudaBackend::new();
    if backend.selected_device().is_none() || !backend.quantized_kernels_available() {
        return Err(Refused(
            "no CUDA device, or this build has no V4.1 kernels (build it where nvcc is installed)"
                .into(),
        ));
    }
    Ok(backend)
}

/// A model the CUDA stage serves: its text side and how to load its layers.
pub struct CudaModel {
    pub vocabulary: Vocabulary,
    pub digest: String,
    pub eos: u32,
    pub pad: u32,
    kind: CudaWeights,
    /// Keep routed experts in page-locked host memory (`--experts host`,
    /// #12418).
    pub experts_on_host: bool,
    /// Where the routed experts live (`--experts`), and the single-machine
    /// tiers (#12463, #12466). The default loads as before.
    pub experts: crate::Experts,
    pub tiers: crate::HostTiers,
}

enum CudaWeights {
    Synthetic(Box<synthetic::Store>),
    Checkpoint(Box<DeepSeekV41Checkpoint>, Arc<CompressedTokenMap>),
}

impl CudaModel {
    /// Loads the model in `dir`: the synthetic model, or the checkpoint's
    /// shards for `selection` (hashed against the pin).
    ///
    /// # Errors
    ///
    /// Refuses weights the stage can't use.
    pub fn load(dir: &Path, selection: Option<&V41StageSelection>) -> Result<Self, Refused> {
        if dir.join(SYNTHETIC_FILE).is_file() {
            let model = load_model_for(dir, None)?;
            return Ok(Self {
                vocabulary: Vocabulary::Synthetic(synthetic::tokenizer()?),
                digest: model.digest,
                eos: synthetic::EOS,
                pad: synthetic::PAD,
                kind: CudaWeights::Synthetic(Box::new(synthetic::store(synthetic::SEED))),
                experts_on_host: false,
                experts: crate::Experts::Device,
                tiers: crate::HostTiers::default(),
            });
        }
        let model = load_model_for(dir, selection)?;
        let Weights::Checkpoint(checkpoint) = model.weights else {
            return Err(Refused("the weights are not a checkpoint".into()));
        };
        Ok(Self {
            vocabulary: model.vocabulary,
            digest: model.digest,
            eos: model.config.eos_token,
            pad: model.config.pad_token,
            kind: CudaWeights::Checkpoint(checkpoint, model.map),
            experts_on_host: false,
            experts: crate::Experts::Device,
            tiers: crate::HostTiers::default(),
        })
    }

    /// Loads `layers` onto the GPU.
    ///
    /// # Errors
    ///
    /// Refuses missing tensors and device failures.
    pub fn stage(
        &self,
        backend: &mut CudaBackend,
        layers: Range<usize>,
        max_context: usize,
    ) -> Result<V41Stage, Refused> {
        self.stage_for(backend, layers, max_context, false, None)
    }

    /// Loads `layers` for qualification (#12405): with per-layer
    /// observations, or with a negative control's defect.
    ///
    /// # Errors
    ///
    /// Refuses missing tensors and device failures.
    pub fn stage_for(
        &self,
        backend: &mut CudaBackend,
        layers: Range<usize>,
        max_context: usize,
        observe: bool,
        control: Option<Control>,
    ) -> Result<V41Stage, Refused> {
        self.stage_with(backend, layers, max_context, max_context, observe, control)
    }

    /// Loads `layers` onto the GPU with step buffers for `max_step_tokens`
    /// tokens. A group's front sends a prompt as one step, so a serving
    /// stage sizes them for `max_context`.
    ///
    /// # Errors
    ///
    /// Refuses missing tensors and device failures.
    pub fn stage_sized(
        &self,
        backend: &mut CudaBackend,
        layers: Range<usize>,
        max_context: usize,
        max_step_tokens: usize,
    ) -> Result<V41Stage, Refused> {
        self.stage_with(backend, layers, max_context, max_step_tokens, false, None)
    }

    fn stage_with(
        &self,
        backend: &mut CudaBackend,
        layers: Range<usize>,
        max_context: usize,
        max_step_tokens: usize,
        observe: bool,
        control: Option<Control>,
    ) -> Result<V41Stage, Refused> {
        let bad = |e: RuntimeError| Refused(format!("layers {layers:?} can't load: {e}"));
        // `--experts host` alone keeps the old load; a cache or Engram rows
        // from the shards take the tiered one (#12463, #12466).
        let experts = if self.experts == crate::Experts::Device && self.experts_on_host {
            crate::Experts::Host
        } else {
            self.experts
        };
        if self.tiers.any() || experts == crate::Experts::File {
            return self.stage_tiered(
                backend,
                layers,
                max_context,
                max_step_tokens,
                observe,
                control,
                experts,
            );
        }
        match &self.kind {
            CudaWeights::Synthetic(store) => {
                let tables = synthetic::engram_tables(backend, store, &layers).map_err(bad)?;
                V41Stage::load(
                    backend,
                    V41StageConfig {
                        shape: synthetic::shape(),
                        layers: layers.clone(),
                        max_context,
                        max_step_tokens,
                        engram: engram_hashes(&synthetic::layout()),
                        token_map: Some(token_map(&synthetic::token_map()?, self.pad)?),
                        observe,
                        swap_rope_tables: control == Some(Control::WrongRope),
                        experts_on_host: self.experts_on_host,
                    },
                    &Controlled {
                        inner: store.as_ref(),
                        control,
                    },
                    tables,
                )
                .map_err(bad)
            }
            CudaWeights::Checkpoint(checkpoint, map) => {
                let mut tables = Vec::new();
                for layer in [1_usize, 14].into_iter().filter(|l| layers.contains(l)) {
                    tables.push(engram_table(checkpoint, backend, layer)?);
                }
                let layout = EngramLayout::flash().map_err(|e| Refused(e.to_string()))?;
                V41Stage::load(
                    backend,
                    V41StageConfig {
                        shape: V41StageShape::flash(),
                        layers: layers.clone(),
                        max_context,
                        max_step_tokens,
                        engram: engram_hashes(&layout),
                        token_map: Some(token_map(map, self.pad)?),
                        observe,
                        swap_rope_tables: control == Some(Control::WrongRope),
                        experts_on_host: self.experts_on_host,
                    },
                    &Controlled {
                        inner: &CheckpointSource(checkpoint),
                        control,
                    },
                    tables,
                )
                .map_err(bad)
            }
        }
    }
}

impl CudaModel {
    /// Loads `layers` with the host tiers: an expert cache and Engram rows
    /// from the shards.
    #[allow(clippy::too_many_arguments)]
    fn stage_tiered(
        &self,
        backend: &mut CudaBackend,
        layers: Range<usize>,
        max_context: usize,
        max_step_tokens: usize,
        observe: bool,
        control: Option<Control>,
        experts: crate::Experts,
    ) -> Result<V41Stage, Refused> {
        let bad = |e: RuntimeError| Refused(format!("layers {layers:?} can't load: {e}"));
        let (shape, source, tier_source, engram, token_map) = match &self.kind {
            CudaWeights::Synthetic(store) => (
                synthetic::shape(),
                store.as_ref() as &dyn V41TensorSource,
                tiers::TierSource::Synthetic(store.as_ref()),
                engram_hashes(&synthetic::layout()),
                token_map(&synthetic::token_map()?, self.pad)?,
            ),
            CudaWeights::Checkpoint(checkpoint, map) => {
                let layout = EngramLayout::flash().map_err(|e| Refused(e.to_string()))?;
                (
                    V41StageShape::flash(),
                    &CheckpointSource(checkpoint) as &dyn V41TensorSource,
                    tiers::TierSource::Checkpoint(checkpoint),
                    engram_hashes(&layout),
                    token_map(map, self.pad)?,
                )
            }
        };
        let (options, on_host) =
            tiers::stage_options(tier_source, &shape, &layers, experts, &self.tiers)?;
        let tables = if options.engram_rows.is_some() {
            Vec::new()
        } else {
            match &self.kind {
                CudaWeights::Synthetic(store) => {
                    synthetic::engram_tables(backend, store, &layers).map_err(bad)?
                }
                CudaWeights::Checkpoint(checkpoint, _) => {
                    let mut tables = Vec::new();
                    for layer in [1_usize, 14].into_iter().filter(|l| layers.contains(l)) {
                        tables.push(engram_table(checkpoint, backend, layer)?);
                    }
                    tables
                }
            }
        };
        V41Stage::load_with(
            backend,
            V41StageConfig {
                shape,
                layers: layers.clone(),
                max_context,
                max_step_tokens,
                engram,
                token_map: Some(token_map),
                observe,
                swap_rope_tables: control == Some(Control::WrongRope),
                experts_on_host: on_host,
            },
            &Controlled {
                inner: source,
                control,
            },
            tables,
            options,
        )
        .map_err(bad)
    }
}

/// Builds the CUDA executor's worker for one stage.
///
/// # Errors
///
/// Refuses a missing device and weights the layers can't load from.
pub fn cuda_worker(
    model: &CudaModel,
    manifest: &V41GroupManifest,
    entry: &V41StageEntry,
    max_context: usize,
    generation: u64,
) -> Result<StageWorker<V41CudaStageExecutor>, Refused> {
    cuda_worker_for(model, manifest, entry, max_context, generation, false, None)
}

/// [`cuda_worker`] for qualification (#12405): observations and a negative
/// control.
///
/// # Errors
///
/// Refuses a missing device and weights the layers can't load from.
pub fn cuda_worker_for(
    model: &CudaModel,
    manifest: &V41GroupManifest,
    entry: &V41StageEntry,
    max_context: usize,
    generation: u64,
    observe: bool,
    control: Option<Control>,
) -> Result<StageWorker<V41CudaStageExecutor>, Refused> {
    let mut backend = cuda_backend()?;
    let stage = model.stage_for(
        &mut backend,
        entry.first_layer as usize..entry.end_layer as usize,
        max_context,
        observe,
        control,
    )?;
    let mut executor = V41CudaStageExecutor::new(backend, stage, entry.stage);
    if control == Some(Control::StaleCache) {
        executor = executor.with_stale_cache();
    }
    let inbox = StageInbox::new(
        manifest.digest(),
        generation,
        executor.contract(),
        CodecLimits::default(),
    );
    Ok(StageWorker::new(inbox, executor))
}

/// Runs one CUDA stage until the front closes the connection.
///
/// # Errors
///
/// Any refusal to load or serve.
pub fn run_stage(
    options: &Options,
    manifest: &V41GroupManifest,
    entry: &V41StageEntry,
    selection: Option<&V41StageSelection>,
) -> Result<(), Refused> {
    // The backend uses the first visible device: narrow the mask to `--gpu`,
    // counted within any inherited mask. No other thread runs yet.
    let mask = std::env::var("CUDA_VISIBLE_DEVICES").ok();
    let device = crate::select_device(mask.as_deref(), options.gpu)?;
    unsafe { std::env::set_var("CUDA_VISIBLE_DEVICES", device) };
    let mut model = CudaModel::load(&options.weights, selection)?;
    model.experts_on_host = options.experts == crate::Experts::Host;
    model.experts = options.experts;
    model.tiers = options.tiers.clone();
    let model: &'static CudaModel = Box::leak(Box::new(model));
    check_weights(manifest, &model.digest)?;
    if (options.observe || options.control.is_some()) && options.record.is_none() {
        return Err(Refused(
            "--observe and --control are qualification options and need --record".into(),
        ));
    }
    let mut backend = cuda_backend()?;
    let mut loaded = model.stage_with(
        &mut backend,
        entry.first_layer as usize..entry.end_layer as usize,
        options.max_context,
        options.max_step_tokens,
        options.observe,
        options.control,
    )?;
    if options.cuda_fp8_reference {
        if options.record.is_none() || options.tiers.any() {
            return Err(Refused(
                "FP8 diagnostics need --record and ordinary device weights".into(),
            ));
        }
        loaded
            .enable_fp8_reference()
            .map_err(|e| Refused(e.to_string()))?;
    }
    let device = device_info(&backend, options.experts);
    if let Some(plan) = tiers::memory_plan(&loaded) {
        // The single-box plan, once at load, for the operator's log (#12467).
        eprintln!("{}", serde_json::json!({"memory_plan": plan}));
    }
    let mut executor = V41CudaStageExecutor::new(backend, loaded, entry.stage);
    if options.control == Some(Control::StaleCache) {
        executor = executor.with_stale_cache();
    }
    let inbox = StageInbox::new(
        manifest.digest(),
        options.generation,
        executor.contract(),
        CodecLimits::default(),
    );
    let worker = StageWorker::new(inbox, executor);
    let stage = serve::Stage {
        name: options.stage.clone(),
        token: options.token.clone(),
        weights: model.digest.clone(),
        index: entry.stage,
        manifest: manifest.clone(),
        vocabulary: &model.vocabulary,
        eos: model.eos,
        default_token: model.pad,
        front: options.front.clone(),
        listen: options.listen.clone(),
        data_listen: options.data_listen.clone(),
        data_advertise: options.data_advertise.clone(),
        runtime: Backend::Cuda.runtime(),
        profile: options.tiers.profile(if options.cuda_fp8_reference {
            crate::profile_name(Backend::Cuda, None).replacen("cuda-device", "cuda-fp8-mma-tp8", 1)
        } else {
            crate::profile_name(Backend::Cuda, None)
        }),
        device,
        events: options.events.clone(),
        record: options.record.clone(),
        max_context: options.max_context,
        max_step_tokens: options.max_step_tokens,
        prefill_chunk: options.prefill_chunk,
        max_input: options.max_input,
        inbox: serve::InboxBounds::default(),
    };
    serve::serve(stage, worker, options.generation)
}

/// What the unsplit CUDA stage (all 40 layers on one GPU) answers, run as a
/// group would: the prompt as one step, then one token per step, with the
/// device's sampling.
///
/// # Errors
///
/// Any refusal of the model, the device, or the prompt.
pub fn reference(
    dir: &Path,
    prompt: Option<&Value>,
    mode: PromptMode,
    steps: u64,
    sampling: Sampling,
    max_context: usize,
) -> Result<ReferenceAnswer, Refused> {
    let model = CudaModel::load(dir, None)?;
    let mut backend = cuda_backend()?;
    let mut stage = model.stage(&mut backend, 0..40, max_context)?;
    let tokenizer: Arc<Tokenizer> = model.vocabulary.tokenizer();
    let mut input = model.vocabulary.encode(prompt, model.pad, mode)?;
    let mut position = 0;
    let mut text = TextStream::default();
    let mut answer = ReferenceAnswer {
        tokens: Vec::new(),
        text: String::new(),
    };
    let device_sampling = V41Sampling {
        temperature: sampling.temperature as f32,
        top_p: sampling.top_p as f32,
        seed: u64::from(sampling.seed),
    };
    for _ in 0..steps {
        let out = stage
            .step(
                &backend,
                &V41StepInput {
                    position,
                    tokens: &input,
                    residual: None,
                    pre_mix: None,
                    source_rows: None,
                    selection: None,
                    candidates: None,
                    sampling: device_sampling,
                },
                false,
            )
            .map_err(|e| Refused(format!("the CUDA stage failed: {e}")))?;
        let token = out
            .token
            .ok_or_else(|| Refused("the unsplit stage sampled no token".into()))?;
        answer.tokens.push(token);
        answer.text.push_str(&text.push(&tokenizer, token));
        if token == model.eos {
            break;
        }
        position += input.len();
        input = vec![token];
    }
    Ok(answer)
}
