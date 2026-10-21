//! `--backend metal`: the DeepSeek-V4.1 stage on an Apple GPU
//! (`psionic_backend_metal::v41::stage::V41MetalStage`, #12420) behind W9's
//! [`StageExecutor`].
//!
//! The Metal stage runs the CUDA stage's kernels, ported to Metal, in the
//! same order, and speaks the CUDA wire: `F32` residuals and pre-mixes, BF16
//! KV and indexer-key rows, `I32` selections, and `U32` candidates. A Mac can
//! therefore take a layer range in a group of CUDA stages. It announces
//! [`crate::RUNTIME_METAL`], device class `metal-apple<N>` (the GPU family),
//! and profile `metal-device-apple<N>`, so a manifest entry declares it like
//! any other device class (#12418).
//!
//! Unified memory holds the routed experts and the Engram tables, so
//! `--experts host` doesn't apply.

use std::{cell::Cell, fmt, fs::File, ops::Range, os::unix::fs::FileExt, path::Path, sync::Arc};

use psionic_backend_metal::v41::{
    V41Buffer, V41Metal,
    stage::{
        V41MetalEngramTable, V41MetalStage, V41Sampling, V41StageConfig, V41StageShape,
        V41StepInput, V41TensorSource,
    },
};
use psionic_models::{
    deepseek_v41_cpu::engram::{CompressedTokenMap, EngramLayout},
    deepseek_v41_stage::{V41GroupManifest, V41StageEntry, V41StageGeometry, stage_contract},
    deepseek_v41_weights::{DeepSeekV41Checkpoint, V41StageSelection},
    stage_state::{
        CodecLimits, Section, StageContract, StageExecutor, StageInbox, StageMessage, StageWorker,
    },
};
use serde_json::Value;
use tokenizers::Tokenizer;

use crate::cuda::{
    CheckpointSource, Controlled, engram_hashes, inbound, output_sections, synthetic, token_map,
    wire_geometry,
};
use crate::{
    Backend, Control, Options, PromptMode, ReferenceAnswer, Refused, SYNTHETIC_FILE, Sampling,
    StageBackend, TextStream, Vocabulary, Weights, check_weights, load_model_for, serve,
};

/// Why the Metal executor failed a step.
#[derive(Debug)]
pub struct MetalStageError(pub String);

impl fmt::Display for MetalStageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MetalStageError {}

fn fail(message: impl fmt::Display) -> MetalStageError {
    MetalStageError(message.to_string())
}

/// W9's [`StageExecutor`] over [`V41MetalStage`] on the Mac's GPU.
pub struct V41MetalStageExecutor {
    metal: V41Metal,
    stage: V41MetalStage,
    geometry: V41StageGeometry,
    index: u16,
    layers: Range<u32>,
    next: Option<StageContract>,
    sampling: Cell<V41Sampling>,
    sampled: Cell<Option<u32>>,
    emit: Cell<bool>,
    observe: bool,
    stale_cache: bool,
    reached: usize,
    offset: usize,
}

impl V41MetalStageExecutor {
    /// An executor for stage `index` of a loaded stage.
    #[must_use]
    pub fn new(metal: V41Metal, stage: V41MetalStage, index: u16) -> Self {
        let geometry = wire_geometry(&stage.config().shape);
        let observe = stage.config().observe;
        let layers = stage.config().layers.start as u32..stage.config().layers.end as u32;
        let next = (!stage.config().samples())
            .then(|| stage_contract(&geometry, index + 1, layers.end..layers.end + 1));
        Self {
            metal,
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
    pub fn stage(&self) -> &V41MetalStage {
        &self.stage
    }
}

impl StageExecutor for V41MetalStageExecutor {
    type Error = MetalStageError;

    fn step(&mut self, message: &StageMessage) -> Result<Vec<Section>, Self::Error> {
        self.sampled.set(None);
        let parts = inbound(message).map_err(fail)?;
        let mut position =
            usize::try_from(message.position).map_err(|_| fail("position out of range"))?;
        if self.stale_cache {
            if position == 0 {
                self.offset = self.reached;
            }
            position += self.offset;
            self.reached = position + message.tokens.len();
        }
        let input = V41StepInput {
            position,
            tokens: &message.tokens,
            residual: parts.residual.as_deref(),
            pre_mix: parts.pre_mix.as_deref(),
            source_rows: parts.rows.as_ref(),
            selection: parts.selection.as_deref(),
            candidates: parts.candidates.as_deref(),
            sampling: self.sampling.get(),
        };
        let out = self
            .stage
            .step_with_output(&self.metal, &input, self.observe, self.emit.get())
            .map_err(|e| fail(format!("the Metal stage failed: {e}")))?;
        if self.next.is_none() && self.emit.get() {
            let token = out
                .token
                .ok_or_else(|| fail("the last stage sampled no token"))?;
            self.sampled.set(Some(token));
        }
        output_sections(
            self.next.as_ref(),
            self.geometry,
            &out,
            message.position,
            message.tokens.len(),
        )
        .map_err(fail)
    }

    fn reset(&mut self, _request_id: &str) {
        // A request restarts at position 0, which clears the stage's state.
        self.sampled.set(None);
    }
}

impl StageBackend for V41MetalStageExecutor {
    fn geometry(&self) -> V41StageGeometry {
        self.geometry
    }

    fn prepare(&self, sampling: Sampling, _step: u64) {
        self.emit.set(true);
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
        Some(
            self.stage
                .observations(tokens)
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

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// The device class a Metal stage announces: `metal-` and the GPU family.
#[must_use]
pub fn device_class() -> String {
    match psionic_backend_metal::v41::gpu_family() {
        Some(family) => format!("metal-{family}"),
        None => "metal".into(),
    }
}

/// The numerical profile a Metal stage announces.
#[must_use]
pub fn profile() -> String {
    match psionic_backend_metal::v41::gpu_family() {
        Some(family) => format!("metal-{}-{family}", crate::CUDA_PROFILE),
        None => format!("metal-{}", crate::CUDA_PROFILE),
    }
}

/// What a Metal stage announces about its device: its class, the GPU's name,
/// the memory Metal recommends it keep resident, and unified memory.
fn device_info(metal: &V41Metal) -> Value {
    serde_json::json!({
        "class": device_class(),
        "name": metal.device_name(),
        "memory_bytes": metal.recommended_working_set(),
        "experts": "unified",
    })
}

fn metal_device() -> Result<V41Metal, Refused> {
    V41Metal::new().map_err(|e| Refused(format!("the Metal stage can't start: {e}")))
}

/// Copies an Engram table from the checkpoint into unified memory.
fn engram_table(
    checkpoint: &DeepSeekV41Checkpoint,
    metal: &V41Metal,
    layer: usize,
) -> Result<V41MetalEngramTable, Refused> {
    let fill = |name: &str| -> Result<V41Buffer, Refused> {
        let bad = |e: &dyn fmt::Display| Refused(format!("Engram table {name}: {e}"));
        let entry = checkpoint.entry(name).map_err(|e| bad(&e))?;
        let shard = checkpoint
            .shard_of(name)
            .ok_or_else(|| Refused(format!("Engram table {name} has no shard")))?;
        let file = File::open(checkpoint.root().join(shard)).map_err(|e| bad(&e))?;
        let len = usize::try_from(entry.byte_len).map_err(|e| bad(&e))?;
        let buffer = metal.zeros(len).map_err(|e| bad(&e))?;
        let bytes = buffer.bytes_mut();
        let mut done = 0;
        while done < len {
            let n = (256 << 20).min(len - done);
            file.read_exact_at(&mut bytes[done..done + n], entry.file_offset + done as u64)
                .map_err(|e| bad(&e))?;
            done += n;
        }
        Ok(buffer)
    };
    let weight = format!("layers.{layer}.engram.embed.weight");
    let codes = fill(&weight)?;
    let scales = fill(&format!("layers.{layer}.engram.embed.scale"))?;
    let rows = checkpoint
        .entry(&weight)
        .map_err(|e| Refused(e.to_string()))?
        .shape[0];
    Ok(V41MetalEngramTable {
        layer,
        codes,
        scales,
        rows,
    })
}

/// The synthetic model's Engram tables of `layers`.
fn synthetic_tables(
    metal: &V41Metal,
    store: &synthetic::Store,
    layers: &Range<usize>,
) -> Result<Vec<V41MetalEngramTable>, Refused> {
    let mut out = Vec::new();
    for l in synthetic::layout().layers {
        let layer = l.layer_id as usize;
        if !layers.contains(&layer) {
            continue;
        }
        let bad = |e: &dyn fmt::Display| Refused(format!("Engram table {layer}: {e}"));
        let codes = store
            .tensor(&format!("layers.{layer}.engram.embed.weight"))
            .map_err(|e| bad(&e))?;
        let scales = store
            .tensor(&format!("layers.{layer}.engram.embed.scale"))
            .map_err(|e| bad(&e))?;
        out.push(V41MetalEngramTable {
            layer,
            codes: metal.byte_buffer(&codes.bytes).map_err(|e| bad(&e))?,
            scales: metal.byte_buffer(&scales.bytes).map_err(|e| bad(&e))?,
            rows: l.table_rows,
        });
    }
    Ok(out)
}

enum MetalWeights {
    Synthetic(Box<synthetic::Store>),
    Checkpoint(Box<DeepSeekV41Checkpoint>, Arc<CompressedTokenMap>),
}

/// A model the Metal stage serves: its text side and how to load its layers.
pub struct MetalModel {
    pub vocabulary: Vocabulary,
    pub digest: String,
    pub eos: u32,
    pub pad: u32,
    kind: MetalWeights,
}

impl MetalModel {
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
                kind: MetalWeights::Synthetic(Box::new(synthetic::store(synthetic::SEED))),
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
            kind: MetalWeights::Checkpoint(checkpoint, model.map),
        })
    }

    /// Loads `layers` onto the GPU with step buffers for `max_step_tokens`
    /// tokens, for serving or for qualification (observations, a negative
    /// control).
    ///
    /// # Errors
    ///
    /// Refuses missing tensors and device failures.
    pub fn stage(
        &self,
        metal: &V41Metal,
        layers: Range<usize>,
        max_context: usize,
        max_step_tokens: usize,
        observe: bool,
        control: Option<Control>,
    ) -> Result<V41MetalStage, Refused> {
        let bad = |e: psionic_runtime::RuntimeError| {
            Refused(format!("layers {layers:?} can't load: {e}"))
        };
        match &self.kind {
            MetalWeights::Synthetic(store) => {
                let tables = synthetic_tables(metal, store, &layers)?;
                V41MetalStage::load(
                    metal,
                    V41StageConfig {
                        shape: synthetic::shape(),
                        layers: layers.clone(),
                        max_context,
                        max_step_tokens,
                        engram: engram_hashes(&synthetic::layout()),
                        token_map: Some(token_map(&synthetic::token_map()?, self.pad)?),
                        observe,
                        swap_rope_tables: control == Some(Control::WrongRope),
                        experts_on_host: false,
                    },
                    &Controlled {
                        inner: store.as_ref(),
                        control,
                    },
                    tables,
                )
                .map_err(bad)
            }
            MetalWeights::Checkpoint(checkpoint, map) => {
                let mut tables = Vec::new();
                for layer in [1_usize, 14].into_iter().filter(|l| layers.contains(l)) {
                    tables.push(engram_table(checkpoint, metal, layer)?);
                }
                let layout = EngramLayout::flash().map_err(|e| Refused(e.to_string()))?;
                let config = V41StageConfig {
                    shape: V41StageShape::flash(),
                    layers: layers.clone(),
                    max_context,
                    max_step_tokens,
                    engram: engram_hashes(&layout),
                    token_map: Some(token_map(map, self.pad)?),
                    observe,
                    swap_rope_tables: control == Some(Control::WrongRope),
                    experts_on_host: false,
                };
                V41MetalStage::load(
                    metal,
                    config,
                    &Controlled {
                        inner: &CheckpointSource(checkpoint) as &dyn V41TensorSource,
                        control,
                    },
                    tables,
                )
                .map_err(bad)
            }
        }
    }
}

/// Builds the Metal executor's worker for one stage.
///
/// # Errors
///
/// Refuses a missing GPU and weights the layers can't load from.
pub fn metal_worker(
    model: &MetalModel,
    manifest: &V41GroupManifest,
    entry: &V41StageEntry,
    max_context: usize,
    generation: u64,
) -> Result<StageWorker<V41MetalStageExecutor>, Refused> {
    let metal = metal_device()?;
    let stage = model.stage(
        &metal,
        entry.first_layer as usize..entry.end_layer as usize,
        max_context,
        max_context,
        false,
        None,
    )?;
    let executor = V41MetalStageExecutor::new(metal, stage, entry.stage);
    let inbox = StageInbox::new(
        manifest.digest(),
        generation,
        executor.contract(),
        CodecLimits::default(),
    );
    Ok(StageWorker::new(inbox, executor))
}

/// Runs one Metal stage until the front closes the connection.
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
    let model = MetalModel::load(&options.weights, selection)?;
    let model: &'static MetalModel = Box::leak(Box::new(model));
    check_weights(manifest, &model.digest)?;
    if (options.observe || options.control.is_some()) && options.record.is_none() {
        return Err(Refused(
            "--observe and --control are qualification options and need --record".into(),
        ));
    }
    let metal = metal_device()?;
    let loaded = model.stage(
        &metal,
        entry.first_layer as usize..entry.end_layer as usize,
        options.max_context,
        options.max_step_tokens,
        options.observe,
        options.control,
    )?;
    let device = device_info(&metal);
    let mut executor = V41MetalStageExecutor::new(metal, loaded, entry.stage);
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
        runtime: Backend::Metal.runtime(),
        profile: profile(),
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

/// What the unsplit Metal stage (all 40 layers on the Mac's GPU) answers,
/// run as a group would: the prompt as one step, then one token per step.
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
    let model = MetalModel::load(dir, None)?;
    let metal = metal_device()?;
    let mut stage = model.stage(&metal, 0..40, max_context, max_context, false, None)?;
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
                &metal,
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
            .map_err(|e| Refused(format!("the Metal stage failed: {e}")))?;
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
