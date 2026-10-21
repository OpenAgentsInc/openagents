//! The DeepSeek-V4.1 stage worker that a Pylon stage group starts.
//!
//! A V4.1 group splits the model's 40 layers across stage processes. Pylon's
//! `stage_group` front starts each stage as
//!
//! ```text
//! psionic-v41-stage --weights DIR --layers A-B --gpu N --manifest D
//!     [--embeddings] [--head] --stage NAME --generation G --front ADDR --token T
//! ```
//!
//! and drives it over the front's line-delimited JSON control protocol
//! (`crates/pylon/src/stage_group.rs`). The stage announces runtime
//! [`RUNTIME`], answers health pings and checks, runs one step at a time,
//! and obeys cancellation and generation fences.
//!
//! Stage-to-stage state is W9's [`StageMessage`], encoded with
//! [`psionic_models::stage_state::encode`] and sent as length-prefixed
//! frames to the next stage's loopback listener, whose address the front
//! passes with each step. Each stage admits what it receives through its
//! [`StageInbox`], so stale, duplicate, reordered, cross-request, and
//! wrong-generation state is refused before it is used. Stage 0 turns the
//! conversation into tokens with the V4.1 adapter, and the last stage samples
//! with the front's settings and returns the token ID, its text, and whether
//! it ends the answer.
//!
//! The model half is W9's [`StageExecutor`] trait. [`Backend::Cpu`] runs
//! [`V41CpuStageExecutor`], the CPU port's layer range; [`Backend::Cuda`]
//! (the `cuda` feature) runs W7's device-resident stage, which samples on the
//! device. Each backend announces its own runtime, so a group never mixes
//! them.
//!
//! A checkpoint stage opens only the shards its layers read
//! ([`load_model_for`]) and checks their SHA-256 against the pin.

#[cfg(feature = "cuda")]
pub mod cuda;
pub mod qualify;
pub mod serve;

use std::{
    collections::BTreeMap,
    fmt,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
    task::{Context, Poll, Waker},
};

use psionic_models::{
    deepseek_v41::{
        DeepSeekV41Protocol,
        deepseek_recipe::stream::{
            ChunkGenerator, FinishReason, InferenceChunk, InferenceFinishReason, OutputChunk,
            PromptUsage, StreamError, StreamProcessor,
        },
        deepseek_recipe_core::conversation::Conversation,
        deepseek_recipe_core::messages::InputMessage,
        deepseek_recipe_core::prompt::Prompt,
        deepseek_v41_parsing_options,
    },
    deepseek_v41_cpu::{
        engram::CompressedTokenMap,
        generation::argmax,
        projection::WeightSource,
        quantization::Arithmetic,
        transformer::{CpuTransformer, TransformerConfig},
    },
    deepseek_v41_stage::{
        EngramLocation, GROUP_MANIFEST_SCHEMA, V41_STAGE_PROTOCOL, V41EngramPlacement,
        V41GroupManifest, V41StageEntry, V41StageGeometry, stage_contract,
    },
    deepseek_v41_stage_cpu::{
        LayerRangeRuntime, RangeConfig, V41CpuStageExecutor,
        synthetic::{self, SyntheticWeights},
    },
    deepseek_v41_weights::{CheckpointOpenOptions, DeepSeekV41Checkpoint, V41StageSelection},
    stage_state::{CodecLimits, StageInbox, StageWorker},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokenizers::Tokenizer;
use tokio_stream::Stream;

pub use psionic_models::stage_state::{StageExecutor, StageMessage};

/// The runtime every CPU stage of a V4.1 group announces.
pub const RUNTIME: &str = "psionic-v41-stage/1";
/// The runtime every CUDA stage announces. The CUDA executor samples on the
/// device and sends BF16 cache rows, while the CPU executor sends `F32`, so a
/// group runs one backend: Pylon's front makes a group ready only when every
/// stage announces the same runtime.
pub const RUNTIME_CUDA: &str = "psionic-v41-stage/1+cuda";
/// The control protocol version, as Pylon's `stage_group::PROTOCOL`.
pub const PROTOCOL: u32 = 1;
/// The file that marks a synthetic weights directory.
pub const SYNTHETIC_FILE: &str = "synthetic.json";
/// The schema of [`SYNTHETIC_FILE`].
pub const SYNTHETIC_SCHEMA: &str = "psionic.deepseek_v41.synthetic.v1";
/// The checkpoint index a real weights directory holds.
pub const INDEX_FILE: &str = "model.safetensors.index.json";

/// A stage's refusal to start or to continue, in words for the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

fn refuse<T>(message: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(message.into()))
}

/// Which executor runs the stage's layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// The CPU port, `psionic_models::deepseek_v41_stage_cpu`.
    Cpu,
    /// W7's device-resident stage, `psionic_backend_cuda::v41_stage`
    /// (the `cuda` feature).
    Cuda,
}

impl Backend {
    /// The runtime a stage on this backend announces.
    #[must_use]
    pub const fn runtime(self) -> &'static str {
        match self {
            Self::Cpu => RUNTIME,
            Self::Cuda => RUNTIME_CUDA,
        }
    }

    /// Parses `cpu` or `cuda`.
    ///
    /// # Errors
    ///
    /// Refuses other names, and `cuda` in a build without the `cuda` feature.
    pub fn parse(value: &str) -> Result<Self, Refused> {
        match value {
            "cpu" => Ok(Self::Cpu),
            "cuda" if cfg!(feature = "cuda") => Ok(Self::Cuda),
            "cuda" => refuse(
                "this build has no CUDA stage executor; build psionic-v41-stage with --features cuda",
            ),
            other => refuse(format!("--backend {other} is not cpu or cuda")),
        }
    }
}

/// How stage 0 renders a conversation and the last stage parses the answer:
/// the V4.1 template's thinking mode, which opens the assistant turn in
/// reasoning, or its chat mode, which closes the reasoning at once and
/// answers directly. The oracle's p2 code prompt is chat mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PromptMode {
    /// The default when a request names no mode, as before modes existed.
    #[default]
    Thinking,
    Chat,
}

impl PromptMode {
    /// Every mode a stage serves.
    pub const ALL: [Self; 2] = [Self::Thinking, Self::Chat];

    /// Parses `thinking` or `chat`.
    ///
    /// # Errors
    ///
    /// Refuses other names.
    pub fn parse(value: &str) -> Result<Self, Refused> {
        match value {
            "thinking" => Ok(Self::Thinking),
            "chat" => Ok(Self::Chat),
            other => refuse(format!("the prompt mode {other} is not thinking or chat")),
        }
    }

    /// The mode a step or link frame names in its `mode` field; thinking
    /// when it names none.
    ///
    /// # Errors
    ///
    /// Refuses a mode that isn't text or isn't known.
    pub fn of(message: &Value) -> Result<Self, Refused> {
        match message.get("mode") {
            None | Some(Value::Null) => Ok(Self::Thinking),
            Some(Value::String(mode)) => Self::parse(mode),
            Some(_) => refuse("the prompt mode is not text"),
        }
    }

    /// The mode's name on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Thinking => "thinking",
            Self::Chat => "chat",
        }
    }

    /// Whether the V4.1 template renders the conversation in thinking mode.
    #[must_use]
    pub const fn thinking(self) -> bool {
        matches!(self, Self::Thinking)
    }
}

/// The request sizes a group serves, from its stages' real wire sizes and
/// context capacity. Every stage of a group computes the same budget from
/// the manifest, its backend's wire geometry, and its settings, and
/// announces it in its `hello`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Budget {
    /// Positions one request may fill: its prompt and its answer together.
    pub max_context_tokens: u64,
    /// The most prompt tokens one request may send. The prompt is one
    /// prefill step, so this is the largest prefill every boundary of the
    /// group carries under the codec's limits, below the context and the
    /// operator's `--max-input-tokens`.
    pub max_input_tokens: u64,
    /// The largest prefill the wire alone carries, before the context and
    /// the operator's cap.
    pub max_prefill_tokens: u64,
}

/// Request IDs the budget allows for: the codec's longest.
const BUDGET_REQUEST_ID_BYTES: usize = 128;

/// The budget of a group with `manifest`, whose stages send state in
/// `geometry`, with `max_context` positions per request and an optional
/// operator cap on prompt tokens.
#[must_use]
pub fn budget(
    manifest: &V41GroupManifest,
    geometry: &V41StageGeometry,
    max_context: usize,
    max_input: Option<usize>,
) -> Budget {
    let limits = CodecLimits::default();
    let request_id = BUDGET_REQUEST_ID_BYTES.min(limits.max_request_id_bytes);
    let prefill = manifest
        .stages
        .iter()
        .skip(1)
        .map(|s| stage_contract(geometry, s.stage, s.layers()).max_step_tokens(&limits, request_id))
        .fold(limits.max_tokens as u64, u64::min);
    let context = max_context as u64;
    // One position stays free for the answer's first token.
    let mut input = prefill.min(context.saturating_sub(1));
    if let Some(cap) = max_input {
        input = input.min(cap as u64);
    }
    Budget {
        max_context_tokens: context,
        max_input_tokens: input,
        max_prefill_tokens: prefill,
    }
}

/// Why a request is larger than the group serves. Stage 0 refuses such a
/// request before any stage runs it, and the front fails only that request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestLimit {
    pub reason: String,
    /// The bound the request crossed.
    pub limit: u64,
    /// What the request asked for.
    pub actual: u64,
}

impl Budget {
    /// Checks a request of `input` prompt tokens and up to `output` answer
    /// tokens.
    ///
    /// # Errors
    ///
    /// Names the bound the request crosses.
    pub fn check(&self, input: u64, output: u64) -> Result<(), RequestLimit> {
        if input > self.max_input_tokens {
            return Err(RequestLimit {
                reason: format!(
                    "the prompt is {input} tokens; this group takes at most {}",
                    self.max_input_tokens
                ),
                limit: self.max_input_tokens,
                actual: input,
            });
        }
        let total = input.saturating_add(output.max(1));
        if total > self.max_context_tokens {
            return Err(RequestLimit {
                reason: format!(
                    "the prompt of {input} tokens and up to {} answer tokens need {total} positions; this group has {}",
                    output.max(1),
                    self.max_context_tokens
                ),
                limit: self.max_context_tokens,
                actual: total,
            });
        }
        Ok(())
    }
}

/// What the serving loop needs from an executor beyond W9's trait: a backend
/// that samples on its own device takes the step's settings first and
/// returns the token instead of logits.
pub trait StageBackend: StageExecutor {
    /// The shapes and dtypes this backend sends between stages, from which
    /// the group's [`Budget`] follows.
    fn geometry(&self) -> V41StageGeometry;

    /// The last stage's sampling settings for the next step.
    fn prepare(&self, _sampling: Sampling, _step: u64) {}

    /// The token the last step sampled, when the backend samples itself.
    fn sampled(&self) -> Option<u32> {
        None
    }

    /// The last stage's full-vocabulary logits of the last step's last
    /// token, when the backend keeps them on its device.
    fn logits(&self) -> Option<Vec<f32>> {
        None
    }

    /// Per-layer observations of the last step's tokens (G1 v2 routing
    /// checks), as named 32-bit words, when the stage was loaded to keep them.
    fn observations(&self, _tokens: usize) -> Option<Vec<(String, Vec<u32>)>> {
        None
    }
}

impl StageBackend for V41CpuStageExecutor<'_> {
    fn geometry(&self) -> V41StageGeometry {
        V41CpuStageExecutor::geometry(self)
    }
}

/// A stage's command line.
#[derive(Debug, Clone)]
pub struct Options {
    pub weights: PathBuf,
    pub layers: Range<u32>,
    pub gpu: u32,
    pub manifest: String,
    pub embeddings: bool,
    pub head: bool,
    pub stage: String,
    pub generation: u64,
    pub front: String,
    pub token: String,
    pub backend: Backend,
    /// The CPU arithmetic asked for; always `None` on CUDA, whose only
    /// arithmetic is [`CUDA_PROFILE`].
    pub arithmetic: Option<Arithmetic>,
    pub max_context: usize,
    /// An operator's cap on prompt tokens, below what the wire and the
    /// context allow (`--max-input-tokens`).
    pub max_input: Option<usize>,
    /// Where the stage listens for state from the stage before it.
    pub listen: String,
    /// An optional JSON-lines file of what the stage did, for tests.
    pub events: Option<PathBuf>,
    /// Where to listen for the previous stage's direct link (W12); without
    /// it the stage serves only the front transport.
    pub data_listen: Option<String>,
    /// The link address to announce, such as the machine's internal name.
    pub data_advertise: Option<String>,
    /// Qualification (#12405): where each step's logits and observations
    /// are written. Serving never sets it.
    pub record: Option<PathBuf>,
    /// Qualification: keep per-layer observations of every token.
    pub observe: bool,
    /// Qualification: a deliberate defect the harness must detect.
    pub control: Option<Control>,
}

/// A deliberate defect for the G1 v2 harness's negative controls (#12405).
/// Each must fail the frozen policy; none is reachable without `--control`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// One layer's shared expert output projection has its rows rotated.
    WrongWeights,
    /// One layer's router bias is rotated by one expert.
    WrongRouting,
    /// Window layers use the compressed layers' RoPE frequencies.
    WrongRope,
    /// A request continues on the previous request's cache.
    StaleCache,
}

impl Control {
    /// Parses `--control`.
    ///
    /// # Errors
    ///
    /// Refuses an unknown control.
    pub fn parse(value: &str) -> Result<Self, Refused> {
        match value {
            "wrong-weights" => Ok(Self::WrongWeights),
            "wrong-routing" => Ok(Self::WrongRouting),
            "wrong-rope" => Ok(Self::WrongRope),
            "stale-cache" => Ok(Self::StaleCache),
            other => refuse(format!(
                "--control {other} is not wrong-weights, wrong-routing, wrong-rope, or stale-cache"
            )),
        }
    }

    /// The name `--control` takes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WrongWeights => "wrong-weights",
            Self::WrongRouting => "wrong-routing",
            Self::WrongRope => "wrong-rope",
            Self::StaleCache => "stale-cache",
        }
    }
}

/// Parses `A-B`, end exclusive, as Pylon writes a layer range.
///
/// # Errors
///
/// Refuses anything else.
pub fn parse_layers(value: &str) -> Result<Range<u32>, Refused> {
    let parsed = value
        .split_once('-')
        .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)));
    match parsed {
        Some((a, b)) if a < b && b <= 40 => Ok(a..b),
        _ => refuse(format!(
            "--layers {value} is not a layer range like 0-10 within 0-40"
        )),
    }
}

/// Parses an arithmetic name: `f32`, `bf16`, or `bf16-tp8`.
///
/// # Errors
///
/// Refuses other names.
pub fn parse_arithmetic(value: &str) -> Result<Arithmetic, Refused> {
    match value {
        "f32" => Ok(Arithmetic::Float32),
        "bf16" => Ok(Arithmetic::ReferenceBf16),
        "bf16-tp8" => Ok(Arithmetic::ReferenceBf16Tp8),
        _ => refuse(format!(
            "--arithmetic {value} is not one of f32, bf16, bf16-tp8"
        )),
    }
}

/// The arithmetic the CUDA stage runs: its kernels' own order and rounding
/// (BF16 cache rows, F32 residuals), which none of the CPU port's
/// [`Arithmetic`] settings describe.
pub const CUDA_PROFILE: &str = "device";

/// The name of a CPU arithmetic, as `--arithmetic` takes it.
#[must_use]
pub const fn arithmetic_name(arithmetic: Arithmetic) -> &'static str {
    match arithmetic {
        Arithmetic::Float32 => "f32",
        Arithmetic::ReferenceBf16 => "bf16",
        Arithmetic::ReferenceBf16Tp8 => "bf16-tp8",
    }
}

/// Parses `--arithmetic` for a backend, before any weights load. The CPU
/// port implements `f32`, `bf16`, and `bf16-tp8`; the CUDA stage implements
/// only its own arithmetic, [`CUDA_PROFILE`], so it refuses the CPU names
/// rather than run something other than what was asked for.
///
/// # Errors
///
/// Refuses a name the backend doesn't implement, and unknown names.
pub fn parse_profile(backend: Backend, value: &str) -> Result<Option<Arithmetic>, Refused> {
    match (backend, value) {
        (Backend::Cpu, CUDA_PROFILE) => refuse(format!(
            "--arithmetic {CUDA_PROFILE} is the CUDA stage's arithmetic; the CPU stage takes f32, bf16, or bf16-tp8"
        )),
        (Backend::Cpu, _) => parse_arithmetic(value).map(Some),
        (Backend::Cuda, CUDA_PROFILE) => Ok(None),
        (Backend::Cuda, "f32" | "bf16" | "bf16-tp8") => refuse(format!(
            "the CUDA stage doesn't implement --arithmetic {value}; it runs only `{CUDA_PROFILE}`"
        )),
        (Backend::Cuda, _) => refuse(format!(
            "--arithmetic {value} is not `{CUDA_PROFILE}`, the CUDA stage's arithmetic"
        )),
    }
}

/// The numerical profile a stage announces: `cpu-` and the arithmetic it
/// runs, or `cuda-device` and the architecture its kernels were compiled
/// for.
#[must_use]
pub fn profile_name(backend: Backend, arithmetic: Option<Arithmetic>) -> String {
    match (backend, arithmetic) {
        (Backend::Cpu, Some(a)) => format!("cpu-{}", arithmetic_name(a)),
        (Backend::Cpu, None) => "cpu".into(),
        (Backend::Cuda, _) => match cuda_arch() {
            Some(arch) => format!("cuda-{CUDA_PROFILE}-{arch}"),
            None => format!("cuda-{CUDA_PROFILE}"),
        },
    }
}

/// The architecture the CUDA kernels were compiled for, or `None` in a build
/// without CUDA kernels.
#[must_use]
pub fn cuda_arch() -> Option<&'static str> {
    #[cfg(feature = "cuda")]
    {
        psionic_backend_cuda::COMPILED_CUDA_ARCH
    }
    #[cfg(not(feature = "cuda"))]
    {
        None
    }
}

/// What a stage announces about its own executable: the SHA-256 of the
/// program running and the CUDA architecture its kernels target. Pylon's
/// front compares it across stages, so a replaced worker can't join a group
/// that runs another build.
#[must_use]
pub fn build_identity() -> Value {
    static IDENTITY: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    IDENTITY
        .get_or_init(|| {
            let sha256 = std::env::current_exe()
                .and_then(std::fs::read)
                .map(|bytes| hex::encode(Sha256::digest(bytes)))
                .ok();
            serde_json::json!({
                "sha256": sha256,
                "version": env!("CARGO_PKG_VERSION"),
                "cuda": cfg!(feature = "cuda"),
                "cuda_arch": cuda_arch(),
            })
        })
        .clone()
}

/// The device a stage with `--gpu gpu` runs on, as the one-entry
/// `CUDA_VISIBLE_DEVICES` to give the CUDA runtime.
///
/// `--gpu` is a logical index into the devices this process may see. With no
/// inherited mask it is the physical index. With an inherited mask, such as
/// `CUDA_VISIBLE_DEVICES=2,3` from a scheduler, `--gpu 1` is the mask's
/// second entry (`3`), so stages on one machine never share a device by
/// accident.
///
/// # Errors
///
/// Refuses an index outside the mask and a mask that hides every device.
pub fn select_device(mask: Option<&str>, gpu: u32) -> Result<String, Refused> {
    let Some(mask) = mask else {
        return Ok(gpu.to_string());
    };
    let entries: Vec<&str> = mask.split(',').map(str::trim).collect();
    if entries.iter().any(|e| e.is_empty() || e.starts_with('-')) {
        return refuse(format!(
            "CUDA_VISIBLE_DEVICES={mask} hides every device or has an empty entry"
        ));
    }
    let index = usize::try_from(gpu).unwrap_or(usize::MAX);
    entries.get(index).map(|e| (*e).to_owned()).ok_or_else(|| {
        Refused(format!(
            "--gpu {gpu} is outside CUDA_VISIBLE_DEVICES={mask}, which shows {} device(s); --gpu counts the visible devices from 0",
            entries.len()
        ))
    })
}

impl Options {
    /// Parses a stage's arguments, without the program name.
    ///
    /// # Errors
    ///
    /// Names the missing or invalid argument.
    pub fn parse(args: &[String]) -> Result<Self, Refused> {
        let mut values: BTreeMap<&str, String> = BTreeMap::new();
        let (mut embeddings, mut head, mut observe) = (false, false, false);
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            match flag {
                "--embeddings" => embeddings = true,
                "--head" => head = true,
                "--observe" => observe = true,
                "--weights" | "--layers" | "--gpu" | "--manifest" | "--stage" | "--generation"
                | "--front" | "--token" | "--backend" | "--arithmetic" | "--max-context"
                | "--listen" | "--events" | "--data-listen" | "--data-advertise" | "--record"
                | "--control" | "--max-input-tokens" => {
                    let Some(value) = args.get(i + 1) else {
                        return refuse(format!("{flag} needs a value"));
                    };
                    values.insert(flag, value.clone());
                    i += 1;
                }
                other => return refuse(format!("unknown argument {other}")),
            }
            i += 1;
        }
        let mut take = |flag: &str| {
            values
                .remove(flag)
                .ok_or_else(|| Refused(format!("{flag} is required")))
        };
        let weights = PathBuf::from(take("--weights")?);
        let layers = parse_layers(&take("--layers")?)?;
        let manifest = take("--manifest")?;
        let stage = take("--stage")?;
        let generation = take("--generation")?
            .parse()
            .map_err(|_| Refused("--generation must be a number".into()))?;
        let front = take("--front")?;
        let token = take("--token")?;
        let gpu = match values.remove("--gpu") {
            Some(v) => v
                .parse()
                .map_err(|_| Refused("--gpu must be a number".into()))?,
            None => 0,
        };
        let backend = match values.remove("--backend") {
            None => Backend::Cpu,
            Some(value) => Backend::parse(&value)?,
        };
        let arithmetic = match values.remove("--arithmetic") {
            Some(v) => parse_profile(backend, &v)?,
            None => None,
        };
        let max_context = match values.remove("--max-context") {
            Some(v) => v
                .parse()
                .map_err(|_| Refused("--max-context must be a number".into()))?,
            None => 8192,
        };
        let max_input = values
            .remove("--max-input-tokens")
            .map(|v| {
                v.parse()
                    .map_err(|_| Refused("--max-input-tokens must be a number".into()))
            })
            .transpose()?;
        Ok(Self {
            weights,
            layers,
            gpu,
            manifest,
            embeddings,
            head,
            stage,
            generation,
            front,
            token,
            backend,
            arithmetic,
            max_context,
            max_input,
            listen: values
                .remove("--listen")
                .unwrap_or_else(|| "127.0.0.1:0".into()),
            events: values.remove("--events").map(PathBuf::from),
            data_listen: values.remove("--data-listen"),
            data_advertise: values.remove("--data-advertise"),
            record: values.remove("--record").map(PathBuf::from),
            observe,
            control: values
                .remove("--control")
                .map(|v| Control::parse(&v))
                .transpose()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Weights, tokenizer, and manifest
// ---------------------------------------------------------------------------

/// Where a stage's weights come from.
pub enum Weights {
    /// The tiny synthetic model, for tests and local groups.
    Synthetic(SyntheticWeights),
    /// A V4.1 checkpoint directory.
    Checkpoint(Box<DeepSeekV41Checkpoint>),
}

impl Weights {
    /// The weights as the CPU port reads them.
    #[must_use]
    pub fn source(&self) -> &dyn WeightSource {
        match self {
            Self::Synthetic(w) => w,
            Self::Checkpoint(w) => w.as_ref(),
        }
    }
}

/// How the stage turns a conversation into tokens and tokens into text.
pub enum Vocabulary {
    /// `tok0`–`tok7`: words in the conversation that name a token become
    /// that token, and any other word becomes `tok0`.
    Synthetic(Tokenizer),
    /// The checkpoint's tokenizer and the V4.1 chat template.
    V41(Box<DeepSeekV41Protocol>),
}

impl Vocabulary {
    /// The tokenizer that turns tokens into text.
    #[must_use]
    pub fn tokenizer(&self) -> Arc<Tokenizer> {
        match self {
            Self::Synthetic(t) => Arc::new(t.clone()),
            Self::V41(p) => p.tokenizer(),
        }
    }

    /// Encodes a conversation (`[{"role", "content"}, ...]`) in `mode`.
    /// With no conversation, the prompt is the single token `default`. The
    /// synthetic vocabulary has no template, so its tokens don't depend on
    /// the mode.
    ///
    /// # Errors
    ///
    /// Refuses a malformed conversation or one the template can't render.
    pub fn encode(
        &self,
        prompt: Option<&Value>,
        default: u32,
        mode: PromptMode,
    ) -> Result<Vec<u32>, Refused> {
        let Some(prompt) = prompt else {
            return Ok(vec![default]);
        };
        let turns = prompt
            .as_array()
            .ok_or_else(|| Refused("the prompt is not a list of messages".into()))?;
        let mut messages = Vec::with_capacity(turns.len());
        for turn in turns {
            let role = turn["role"].as_str().unwrap_or_default();
            let content = turn["content"]
                .as_str()
                .ok_or_else(|| Refused("a message has no text content".into()))?;
            messages.push((role.to_owned(), content.to_owned()));
        }
        match self {
            Self::Synthetic(tokenizer) => {
                let mut ids = Vec::new();
                for (_, content) in &messages {
                    for word in content.split_whitespace() {
                        ids.push(tokenizer.token_to_id(word).unwrap_or(0));
                    }
                }
                if ids.is_empty() {
                    ids.push(default);
                }
                Ok(ids)
            }
            Self::V41(protocol) => {
                let mut conversation = Conversation {
                    thinking_mode: mode.thinking(),
                    ..Conversation::default()
                };
                for (role, content) in messages {
                    conversation.messages.push(match role.as_str() {
                        "system" => InputMessage::System {
                            content: Prompt::plain(content),
                            image_sources: Vec::new(),
                            tools: None,
                        },
                        "user" => InputMessage::User {
                            content: Prompt::plain(content),
                            image_sources: Vec::new(),
                        },
                        "assistant" => InputMessage::Assistant {
                            content,
                            reasoning_content: None,
                            tool_calls: None,
                            prefix: false,
                        },
                        other => return refuse(format!("unknown message role {other}")),
                    });
                }
                protocol
                    .encode(&conversation)
                    .map_err(|e| Refused(format!("the prompt can't be encoded: {e}")))
            }
        }
    }
}

/// A loaded model: what every stage of the group shares.
pub struct Model {
    pub weights: Weights,
    pub config: TransformerConfig,
    pub map: Arc<CompressedTokenMap>,
    pub vocabulary: Vocabulary,
    /// `sha256:…` of the checkpoint index, or of the synthetic file.
    pub digest: String,
    /// Default arithmetic: `f32` for the synthetic model, the oracle's
    /// `bf16-tp8` for a checkpoint.
    pub arithmetic: Arithmetic,
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn read(path: &Path) -> Result<Vec<u8>, Refused> {
    std::fs::read(path).map_err(|e| Refused(format!("can't read {}: {e}", path.display())))
}

/// Writes a synthetic weights directory: [`SYNTHETIC_FILE`] with `name`,
/// which only changes the digest.
///
/// # Errors
///
/// Any write error.
pub fn write_synthetic(dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        dir.join(SYNTHETIC_FILE),
        serde_json::to_vec(&serde_json::json!({"schema": SYNTHETIC_SCHEMA, "name": name}))?,
    )
}

/// Loads the weights and tokenizer in `dir`.
///
/// # Errors
///
/// Refuses a directory with neither a synthetic file nor a checkpoint, and
/// any weights or tokenizer the CPU port can't use.
pub fn load_model(dir: &Path) -> Result<Model, Refused> {
    load_model_for(dir, None)
}

/// Loads the weights and tokenizer in `dir` for one stage. A checkpoint
/// opens only the shards `selection` reads and checks their SHA-256 against
/// the pin, so a stage machine holds only its own shards; with no selection
/// every shard is opened (and not hashed), as [`load_model`] does.
///
/// # Errors
///
/// As [`load_model`], plus a selected shard that is missing or doesn't
/// match the pin.
pub fn load_model_for(dir: &Path, selection: Option<&V41StageSelection>) -> Result<Model, Refused> {
    let synthetic_file = dir.join(SYNTHETIC_FILE);
    if synthetic_file.is_file() {
        let bytes = read(&synthetic_file)?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| Refused(format!("{} is not JSON: {e}", synthetic_file.display())))?;
        if value["schema"] != SYNTHETIC_SCHEMA {
            return refuse(format!(
                "{} is not a {SYNTHETIC_SCHEMA} file",
                synthetic_file.display()
            ));
        }
        let model_error = |e: psionic_models::deepseek_v41_cpu::CpuError| Refused(e.to_string());
        return Ok(Model {
            weights: Weights::Synthetic(synthetic::weights().map_err(model_error)?),
            config: synthetic::config().map_err(model_error)?,
            map: synthetic::map().map_err(model_error)?,
            vocabulary: Vocabulary::Synthetic(synthetic::tokenizer().map_err(model_error)?),
            digest: format!("sha256:{}", sha256(&bytes)),
            arithmetic: Arithmetic::Float32,
        });
    }
    let index = dir.join(INDEX_FILE);
    if !index.is_file() {
        return refuse(format!(
            "the weights are missing: {} has neither {SYNTHETIC_FILE} nor {INDEX_FILE}",
            dir.display()
        ));
    }
    let digest = format!("sha256:{}", sha256(&read(&index)?));
    let protocol = DeepSeekV41Protocol::from_checkpoint_files(
        &read(&dir.join("tokenizer.json"))?,
        &read(&dir.join("tokenizer_config.json"))?,
    )
    .map_err(|e| Refused(format!("the checkpoint's tokenizer: {e}")))?;
    let options = CheckpointOpenOptions::pinned_flash()
        .map_err(|e| Refused(format!("the checkpoint pin: {e}")))?;
    let checkpoint = match selection {
        Some(selection) => DeepSeekV41Checkpoint::open_stage(dir, options, selection),
        None => DeepSeekV41Checkpoint::open(dir, options),
    }
    .map_err(|e| {
        Refused(format!(
            "the weights in {} can't be used: {e}",
            dir.display()
        ))
    })?;
    let config = TransformerConfig::flash().map_err(|e| Refused(e.to_string()))?;
    // The Engram hash's compressed vocabulary (99,092), not the output one.
    let map = CompressedTokenMap::build(&protocol.tokenizer(), config.engram.compressed_vocab_size)
        .map_err(|e| Refused(e.to_string()))?;
    Ok(Model {
        weights: Weights::Checkpoint(Box::new(checkpoint)),
        config,
        map: Arc::new(map),
        vocabulary: Vocabulary::V41(Box::new(protocol)),
        digest,
        arithmetic: Arithmetic::ReferenceBf16Tp8,
    })
}

/// A manifest for a local synthetic group with this split; every stage is on
/// host `local`, one GPU ordinal each.
///
/// # Errors
///
/// Refuses a split that doesn't cover layers 0–40 in order.
pub fn synthetic_manifest(split: &[Range<u32>]) -> Result<V41GroupManifest, Refused> {
    let last = split.len().saturating_sub(1);
    let manifest = V41GroupManifest {
        schema: GROUP_MANIFEST_SCHEMA.into(),
        repo: "synthetic".into(),
        revision: "synthetic".into(),
        index_sha256: String::new(),
        protocol_version: V41_STAGE_PROTOCOL,
        memory_source: "synthetic".into(),
        stages: split
            .iter()
            .enumerate()
            .map(|(i, layers)| V41StageEntry {
                stage: i as u16,
                first_layer: layers.start,
                end_layer: layers.end,
                embeddings: i == 0,
                head: i == last,
                host: "local".into(),
                gpu: i as u32,
                gpu_capacity_bytes: 1,
                gpu_weight_bytes: 0,
                gpu_runtime_bytes: 0,
                engram: [1, 14]
                    .into_iter()
                    .filter(|l| layers.contains(l))
                    .map(|layer| V41EngramPlacement {
                        layer,
                        location: EngramLocation::HostMemory,
                        host: "local".into(),
                        bytes: 0,
                    })
                    .collect(),
            })
            .collect(),
    };
    manifest
        .validate()
        .map_err(|e| Refused(format!("the split can't make a manifest: {e}")))?;
    Ok(manifest)
}

fn parse_manifest(bytes: &[u8]) -> Option<V41GroupManifest> {
    let manifest: V41GroupManifest = serde_json::from_slice(bytes).ok()?;
    (manifest.schema == GROUP_MANIFEST_SCHEMA).then_some(manifest)
}

/// Finds the group manifest: `spec` is a manifest file, or `sha256:DIGEST`
/// of a manifest file in the weights directory or its `manifests/` folder.
///
/// # Errors
///
/// Refuses a missing or invalid manifest, or a digest no file has.
pub fn load_manifest(spec: &str, weights: &Path) -> Result<V41GroupManifest, Refused> {
    let manifest = if let Some(digest) = spec.strip_prefix("sha256:") {
        let mut found = None;
        for dir in [weights.to_path_buf(), weights.join("manifests")] {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json")
                    && let Some(manifest) =
                        std::fs::read(&path).ok().and_then(|b| parse_manifest(&b))
                    && manifest.digest_hex() == digest
                {
                    found = Some(manifest);
                }
            }
        }
        found.ok_or_else(|| {
            Refused(format!(
                "no group manifest with digest {digest} is in {} or its manifests folder",
                weights.display()
            ))
        })?
    } else {
        parse_manifest(&read(Path::new(spec))?)
            .ok_or_else(|| Refused(format!("{spec} is not a V4.1 group manifest")))?
    };
    manifest.validate().map_err(|e| Refused(e.to_string()))?;
    Ok(manifest)
}

/// The manifest's entry for this stage's layers, after checking that the
/// stage's flags agree with it.
///
/// # Errors
///
/// Refuses a layer range the manifest doesn't have, and embeddings or head
/// flags that disagree with it.
pub fn find_stage(
    manifest: &V41GroupManifest,
    options: &Options,
) -> Result<V41StageEntry, Refused> {
    let Some(entry) = manifest
        .stages
        .iter()
        .find(|s| s.layers() == options.layers)
    else {
        return refuse(format!(
            "layers {}-{} don't match a stage of the manifest, which splits the model as {}",
            options.layers.start,
            options.layers.end,
            manifest
                .split()
                .iter()
                .map(|r| format!("{}-{}", r.start, r.end))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    if entry.embeddings != options.embeddings || entry.head != options.head {
        return refuse(format!(
            "stage {} of the manifest {} the embeddings and {} the head; the command line disagrees",
            entry.stage,
            if entry.embeddings {
                "holds"
            } else {
                "doesn't hold"
            },
            if entry.head { "holds" } else { "doesn't hold" },
        ));
    }
    Ok(entry.clone())
}

/// Refuses weights other than the ones the manifest names.
///
/// # Errors
///
/// When the manifest names a checkpoint index and `digest` isn't it.
pub fn check_weights(manifest: &V41GroupManifest, digest: &str) -> Result<(), Refused> {
    if !manifest.index_sha256.is_empty() && digest != format!("sha256:{}", manifest.index_sha256) {
        return refuse(format!(
            "the weights are {digest}; the manifest needs sha256:{}",
            manifest.index_sha256
        ));
    }
    Ok(())
}

/// What a stage tells the front and the stages after it.
#[must_use]
pub fn stage_info<'a>(
    options: &Options,
    manifest: &V41GroupManifest,
    entry: &V41StageEntry,
    model: &'a Model,
    backend: Backend,
) -> serve::Stage<'a> {
    serve::Stage {
        name: options.stage.clone(),
        token: options.token.clone(),
        weights: model.digest.clone(),
        index: entry.stage,
        manifest: manifest.clone(),
        vocabulary: &model.vocabulary,
        eos: model.config.eos_token,
        default_token: model.config.pad_token,
        front: options.front.clone(),
        listen: options.listen.clone(),
        data_listen: options.data_listen.clone(),
        data_advertise: options.data_advertise.clone(),
        runtime: backend.runtime(),
        profile: profile_name(
            backend,
            Some(options.arithmetic.unwrap_or(model.arithmetic)),
        ),
        events: options.events.clone(),
        record: options.record.clone(),
        max_context: options.max_context,
        max_input: options.max_input,
        inbox: serve::InboxBounds::default(),
    }
}

/// Builds the CPU executor's worker for one stage.
///
/// # Errors
///
/// Refuses weights the layer range can't run on.
pub fn cpu_worker(
    model: &'static Model,
    manifest: &V41GroupManifest,
    entry: &V41StageEntry,
    arithmetic: Arithmetic,
    max_context: usize,
    generation: u64,
) -> Result<StageWorker<V41CpuStageExecutor<'static>>, Refused> {
    let runtime = LayerRangeRuntime::new(
        model.weights.source(),
        RangeConfig {
            transformer: model.config.clone(),
            layers: entry.first_layer as usize..entry.end_layer as usize,
            max_context,
            arithmetic,
        },
        Arc::clone(&model.map),
    )
    .map_err(|e| Refused(format!("layers {:?} can't run: {e}", entry.layers())))?;
    let executor = V41CpuStageExecutor::new(runtime, entry.stage);
    let inbox = StageInbox::new(
        manifest.digest(),
        generation,
        executor.contract(),
        CodecLimits::default(),
    );
    Ok(StageWorker::new(inbox, executor))
}

// ---------------------------------------------------------------------------
// Sampling and text
// ---------------------------------------------------------------------------

/// The front's sampling settings for one step.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Sampling {
    pub temperature: f64,
    pub top_p: f64,
    pub seed: u32,
}

impl Sampling {
    /// Greedy decoding.
    pub const GREEDY: Self = Self {
        temperature: 0.0,
        top_p: 1.0,
        seed: 0,
    };
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Samples one token. Temperature 0 is greedy (the lowest ID among equal
/// maxima). Otherwise the draw is nucleus sampling at `top_p`, from a
/// generator seeded by the seed and the step, so a run repeats exactly.
///
/// # Errors
///
/// Refuses empty or nonfinite logits and settings out of range.
pub fn sample(logits: &[f32], sampling: Sampling, step: u64) -> Result<u32, Refused> {
    if !(0.0..=2.0).contains(&sampling.temperature)
        || !(sampling.top_p > 0.0 && sampling.top_p <= 1.0)
    {
        return refuse("sampling settings are out of range");
    }
    if sampling.temperature == 0.0 {
        return argmax(logits).map_err(|e| Refused(e.to_string()));
    }
    if logits.is_empty() || logits.iter().any(|x| !x.is_finite()) {
        return refuse("the logits are empty or not finite");
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut order: Vec<(usize, f64)> = logits
        .iter()
        .enumerate()
        .map(|(i, x)| {
            (
                i,
                ((f64::from(*x) - f64::from(max)) / sampling.temperature).exp(),
            )
        })
        .collect();
    let total: f64 = order.iter().map(|(_, p)| p).sum();
    order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut kept = Vec::new();
    let mut mass = 0.0;
    for (i, p) in order {
        let p = p / total;
        kept.push((i, p));
        mass += p;
        if mass >= sampling.top_p {
            break;
        }
    }
    let draw = splitmix((u64::from(sampling.seed) << 32) ^ splitmix(step)) >> 11;
    let mut target = (draw as f64 / (1u64 << 53) as f64) * mass;
    for (i, p) in &kept {
        if target < *p {
            return Ok(*i as u32);
        }
        target -= p;
    }
    Ok(kept.last().map_or(0, |(i, _)| *i as u32))
}

/// Turns sampled tokens into text one token at a time. A token that ends
/// inside a character waits for the next one.
///
/// Each push decodes only a short window: the tokens since the last text
/// came out, plus the tokens of that text for context, so a push costs the
/// same at the first token and the 4,096th (#12412). Its output equals the
/// difference between decoding the whole answer before and after the token.
#[derive(Default)]
pub struct TextStream {
    /// The window: the context tokens, then the tokens not yet emitted.
    ids: Vec<u32>,
    /// The context tokens' text, which the next decode starts with.
    prefix: String,
    /// How many of `ids` are context.
    prefix_index: usize,
    /// Token IDs passed to the tokenizer so far, for the cost tests.
    decoded: usize,
}

impl TextStream {
    /// The text the token adds, possibly empty.
    #[must_use]
    pub fn push(&mut self, tokenizer: &Tokenizer, token: u32) -> String {
        if self.prefix.is_empty() && !self.ids.is_empty() {
            // The context decoded to nothing whole yet, such as a partial
            // character: try again with what it holds now.
            if let Some(prefix) = self.decode(tokenizer)
                && !prefix.ends_with('\u{fffd}')
            {
                self.prefix = prefix;
                self.prefix_index = self.ids.len();
            }
        }
        self.ids.push(token);
        let Some(text) = self.decode(tokenizer) else {
            return String::new();
        };
        if text.len() <= self.prefix.len()
            || text.ends_with('\u{fffd}')
            || !text.starts_with(&self.prefix)
        {
            return String::new();
        }
        let piece = text[self.prefix.len()..].to_owned();
        // The tokens that made this text become the next window's context.
        let context = self.ids.len() - self.prefix_index;
        self.ids.drain(..self.prefix_index);
        self.prefix = self.decode(tokenizer).unwrap_or_default();
        self.prefix_index = context;
        piece
    }

    /// How many token IDs this stream has passed to the tokenizer.
    #[must_use]
    pub fn decoded_ids(&self) -> usize {
        self.decoded
    }

    fn decode(&mut self, tokenizer: &Tokenizer) -> Option<String> {
        self.decoded += self.ids.len();
        tokenizer.decode(&self.ids, false).ok()
    }
}

// ---------------------------------------------------------------------------
// Reference: the unsplit model
// ---------------------------------------------------------------------------

/// What the unsplit model answers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReferenceAnswer {
    pub tokens: Vec<u32>,
    pub text: String,
}

/// Runs the unsplit CPU transformer as a group would: the prompt as one
/// step, then one token per step, up to `steps` tokens or the end of the
/// answer.
///
/// # Errors
///
/// Any refusal of the model or the prompt.
pub fn reference(
    model: &Model,
    prompt: Option<&Value>,
    mode: PromptMode,
    steps: u64,
    sampling: Sampling,
    arithmetic: Arithmetic,
    max_context: usize,
) -> Result<ReferenceAnswer, Refused> {
    let mut transformer = CpuTransformer::new(
        model.weights.source(),
        model.config.clone(),
        Arc::clone(&model.map),
        max_context,
        arithmetic,
    )
    .map_err(|e| Refused(e.to_string()))?;
    let tokenizer = model.vocabulary.tokenizer();
    let mut input = model
        .vocabulary
        .encode(prompt, model.config.pad_token, mode)?;
    let mut text = TextStream::default();
    let mut answer = ReferenceAnswer {
        tokens: Vec::new(),
        text: String::new(),
    };
    for step in 0..steps {
        let logits = transformer
            .forward_tokens(&input)
            .map_err(|e| Refused(e.to_string()))?
            .logits;
        let token = sample(&logits, sampling, step)?;
        answer.tokens.push(token);
        answer.text.push_str(&text.push(&tokenizer, token));
        if token == model.config.eos_token {
            break;
        }
        input = vec![token];
    }
    Ok(answer)
}

// ---------------------------------------------------------------------------
// Answer parsing
// ---------------------------------------------------------------------------

/// Reasoning and final-answer text one token adds, and how the answer
/// ended on its last token.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnswerPiece {
    pub reasoning: String,
    pub content: String,
    /// `stop`, `length`, or the parser's other finish reasons.
    pub finish: Option<String>,
}

struct Collect;

impl ChunkGenerator for Collect {
    type Chunk = OutputChunk;

    async fn generate(&mut self, chunk: OutputChunk) -> Vec<OutputChunk> {
        vec![chunk]
    }
}

/// Runs a stream whose items are all ready, as the recipe's processor is
/// over an in-memory input.
fn ready_items<S: Stream>(stream: S) -> Result<Vec<S::Item>, Refused> {
    let mut stream = std::pin::pin!(stream);
    let mut cx = Context::from_waker(Waker::noop());
    let mut items = Vec::new();
    for _ in 0..1_000_000 {
        match stream.as_mut().poll_next(&mut cx) {
            Poll::Ready(Some(item)) => items.push(item),
            Poll::Ready(None) => return Ok(items),
            Poll::Pending => {}
        }
    }
    refuse("the answer parser didn't finish")
}

/// The #12372 adapter's streaming output parser over the tokens a last
/// stage samples. The checkpoint tokenizer decodes each special token as its
/// own piece, so only the model's `</think>` token ends the reasoning; the
/// same characters written as text stay text.
///
/// One parser runs for the whole answer and takes each token as it comes,
/// so a push costs the same at the first token and the 4,096th. Text the
/// parser holds back, such as a partial character or a possible marker,
/// comes out with the token that completes it, or at the end.
pub struct AnswerParser {
    eos: u32,
    input: Arc<std::sync::Mutex<Fed>>,
    output: std::pin::Pin<Box<dyn Stream<Item = Result<OutputChunk, StreamError>> + Send>>,
    ended: bool,
}

/// Chunks pushed to the parser and not yet read, and whether the answer
/// ended.
#[derive(Default)]
struct Fed {
    chunks: std::collections::VecDeque<InferenceChunk>,
    closed: bool,
}

/// The parser's input: what was pushed so far. Polled with no more input,
/// it is pending until the next push.
struct Feed(Arc<std::sync::Mutex<Fed>>);

impl Stream for Feed {
    type Item = InferenceChunk;

    fn poll_next(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut fed = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match fed.chunks.pop_front() {
            Some(chunk) => Poll::Ready(Some(chunk)),
            None if fed.closed => Poll::Ready(None),
            None => Poll::Pending,
        }
    }
}

impl AnswerParser {
    /// A parser for a V4.1 answer with no tools, rendered in `mode`. In
    /// thinking mode the answer starts in reasoning; in chat mode the prompt
    /// already closed the reasoning, so every token is answer text.
    #[must_use]
    pub fn new(tokenizer: Arc<Tokenizer>, eos: u32, mode: PromptMode) -> Self {
        let conversation = Conversation {
            thinking_mode: mode.thinking(),
            ..Conversation::default()
        };
        let options = deepseek_v41_parsing_options(&conversation, Vec::new());
        let input = Arc::new(std::sync::Mutex::new(Fed::default()));
        if let Ok(mut fed) = input.lock() {
            fed.chunks.push_back(InferenceChunk::Ready {
                system_fingerprint: None,
                prompt_usage: PromptUsage::default(),
            });
        }
        let output = StreamProcessor::new(Collect, options)
            .with_tokenizer(tokenizer)
            .into_protocol_stream(Feed(Arc::clone(&input)));
        Self {
            eos,
            input,
            output: Box::pin(output),
            ended: false,
        }
    }

    /// Adds one sampled token. `last` says the output limit ends the answer
    /// after it.
    ///
    /// # Errors
    ///
    /// Refuses tokens the tokenizer can't decode, and a token after the
    /// answer ended.
    pub fn push(&mut self, token: u32, last: bool) -> Result<AnswerPiece, Refused> {
        if self.ended {
            return refuse("the answer already ended");
        }
        let end = token == self.eos;
        {
            let mut fed = self
                .input
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !end {
                // The end-of-sentence token is the finish, not text.
                fed.chunks.push_back(InferenceChunk::Tokens {
                    token_ids: vec![token],
                    logprobs: None,
                });
            }
            if end || last {
                fed.chunks.push_back(InferenceChunk::Finish {
                    finish_reason: if end {
                        InferenceFinishReason::Stop
                    } else {
                        InferenceFinishReason::Length
                    },
                });
                fed.closed = true;
            }
        }
        self.ended = end || last;
        let mut piece = AnswerPiece::default();
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            match self.output.as_mut().poll_next(&mut cx) {
                Poll::Ready(Some(chunk)) => {
                    match chunk.map_err(|e| Refused(format!("the answer can't be parsed: {e}")))? {
                        OutputChunk::Reasoning { content, .. } => {
                            piece.reasoning.push_str(&content)
                        }
                        OutputChunk::Content { content, .. } => piece.content.push_str(&content),
                        OutputChunk::Finish { finish_reason, .. } => {
                            piece.finish = Some(finish_name(finish_reason).to_owned());
                        }
                        _ => {}
                    }
                }
                // The parser has used every pushed token.
                Poll::Ready(None) | Poll::Pending => return Ok(piece),
            }
        }
    }
}

fn finish_name(reason: FinishReason) -> &'static str {
    match reason {
        FinishReason::Stop | FinishReason::StopSequence => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::ContentFilter => "content_filter",
        FinishReason::EndOfStream => "end_of_stream",
    }
}

#[cfg(test)]
mod parser_tests {
    //! Incremental parsing against parsing the whole answer at once.

    use psionic_models::deepseek_v41::fixture;

    use super::*;

    const EOS: u32 = 1;
    const END_THINKING: u32 = 128_822;

    /// The whole answer in one input, as the parser read it before #12408.
    fn whole(mode: PromptMode, ids: &[u32], last: bool) -> AnswerPiece {
        let tokenizer = fixture::protocol().tokenizer();
        let conversation = Conversation {
            thinking_mode: mode.thinking(),
            ..Conversation::default()
        };
        let end = ids.last() == Some(&EOS);
        let text: Vec<u32> = ids.iter().copied().filter(|id| *id != EOS).collect();
        let mut input = vec![
            InferenceChunk::Ready {
                system_fingerprint: None,
                prompt_usage: PromptUsage::default(),
            },
            InferenceChunk::Tokens {
                token_ids: text,
                logprobs: None,
            },
        ];
        if end || last {
            input.push(InferenceChunk::Finish {
                finish_reason: if end {
                    InferenceFinishReason::Stop
                } else {
                    InferenceFinishReason::Length
                },
            });
        }
        let stream = StreamProcessor::new(
            Collect,
            deepseek_v41_parsing_options(&conversation, Vec::new()),
        )
        .with_tokenizer(tokenizer)
        .into_protocol_stream(tokio_stream::iter(input));
        let mut piece = AnswerPiece::default();
        for chunk in ready_items(stream).unwrap() {
            match chunk.unwrap() {
                OutputChunk::Reasoning { content, .. } => piece.reasoning.push_str(&content),
                OutputChunk::Content { content, .. } => piece.content.push_str(&content),
                OutputChunk::Finish { finish_reason, .. } if end || last => {
                    piece.finish = Some(finish_name(finish_reason).to_owned());
                }
                _ => {}
            }
        }
        piece
    }

    /// The same answer one token at a time.
    fn incremental(mode: PromptMode, ids: &[u32], last: bool) -> AnswerPiece {
        let mut parser = AnswerParser::new(fixture::protocol().tokenizer(), EOS, mode);
        let mut all = AnswerPiece::default();
        for (i, id) in ids.iter().enumerate() {
            let piece = parser.push(*id, last && i + 1 == ids.len()).unwrap();
            all.reasoning.push_str(&piece.reasoning);
            all.content.push_str(&piece.content);
            if piece.finish.is_some() {
                assert!(all.finish.is_none(), "two finishes");
                all.finish = piece.finish;
            }
        }
        all
    }

    #[test]
    fn incremental_parsing_matches_the_whole_answer() {
        let mut cases: Vec<(&str, Vec<u32>, bool)> = [
            ("reasoning_then_final", false),
            ("literal_markers_in_text", false),
            ("split_utf8", false),
            ("truncated_reasoning", true),
            ("no_finish", false),
            ("tool_call_long_arguments", false),
        ]
        .into_iter()
        .map(|(name, last)| (name, fixture::stream_case(name), last))
        .collect();
        // A split character right before the end-of-thinking token, and an
        // answer the output limit cuts inside a character.
        let crab = fixture::stream_case("split_utf8");
        let mut split_then_end = crab[..crab.len() - 1].to_vec();
        split_then_end.push(END_THINKING);
        split_then_end.extend(fixture::stream_case("reasoning_then_final"));
        cases.push(("split_then_end_of_thinking", split_then_end, false));
        cases.push(("cut_inside_a_character", crab[..3].to_vec(), true));
        for mode in PromptMode::ALL {
            for (name, ids, last) in &cases {
                let expected = whole(mode, ids, *last);
                let got = incremental(mode, ids, *last);
                assert_eq!(got, expected, "{name} in {mode:?}");
            }
        }
        // The cases cover both outputs in both modes.
        let thinking = whole(PromptMode::Thinking, &cases[0].1, false);
        assert!(!thinking.reasoning.is_empty() && !thinking.content.is_empty());
        let chat = whole(PromptMode::Chat, &cases[0].1, false);
        assert!(chat.reasoning.is_empty() && !chat.content.is_empty());
    }

    /// The raw text as the stream made it before #12412: every push decodes
    /// the whole answer and emits what follows the text already out.
    fn whole_text(tokenizer: &Tokenizer, ids: &[u32]) -> (Vec<String>, usize) {
        let (mut pieces, mut emitted, mut decoded) = (Vec::new(), String::new(), 0);
        for end in 1..=ids.len() {
            decoded += end;
            let text = tokenizer.decode(&ids[..end], false).unwrap();
            if text.ends_with('\u{fffd}') || !text.starts_with(&emitted) {
                pieces.push(String::new());
                continue;
            }
            pieces.push(text[emitted.len()..].to_owned());
            emitted = text;
        }
        (pieces, decoded)
    }

    fn streamed_text(tokenizer: &Tokenizer, ids: &[u32]) -> (Vec<String>, usize) {
        let mut stream = TextStream::default();
        let pieces = ids.iter().map(|id| stream.push(tokenizer, *id)).collect();
        (pieces, stream.decoded_ids())
    }

    /// Split characters, special tokens, and the same characters written
    /// as text: each token's text is what the whole-answer decode gave it.
    #[test]
    fn the_text_stream_matches_decoding_the_whole_answer() {
        let crab = fixture::stream_case("split_utf8");
        let mut split_then_end = crab[..crab.len() - 1].to_vec();
        split_then_end.push(END_THINKING);
        split_then_end.extend(fixture::stream_case("reasoning_then_final"));
        let cases = [
            fixture::stream_case("reasoning_then_final"),
            fixture::stream_case("literal_markers_in_text"),
            crab.clone(),
            fixture::stream_case("truncated_reasoning"),
            fixture::stream_case("tool_call_long_arguments"),
            split_then_end,
            crab[..3].to_vec(),
        ];
        let v41 = fixture::protocol().tokenizer();
        let synthetic = synthetic::tokenizer().unwrap();
        for (i, ids) in cases.iter().enumerate() {
            let (want, _) = whole_text(&v41, ids);
            let (got, _) = streamed_text(&v41, ids);
            assert_eq!(got, want, "case {i}");
        }
        let all: String = streamed_text(&v41, &crab).0.concat();
        assert!(all.contains("Crab: \u{1f980} and \u{1d518}."), "{all:?}");
        assert!(!all.contains('\u{fffd}'));
        let special = streamed_text(&v41, &[5, END_THINKING, 5]).0.concat();
        assert!(special.contains("</think>"), "{special:?}");
        // The synthetic vocabulary joins its words with spaces.
        let words: Vec<u32> = (2..40).collect();
        assert_eq!(
            streamed_text(&synthetic, &words).0,
            whole_text(&synthetic, &words).0
        );
    }

    /// A 4,096-token answer: the stream passes a bounded window to the
    /// tokenizer at each token, where the whole-answer decode passed
    /// 8,390,656 token IDs in all.
    #[test]
    fn the_text_stream_decodes_a_bounded_window_per_token() {
        let ids = long_answer(4_096);
        let tokenizer = fixture::protocol().tokenizer();
        let (want, before) = whole_text(&tokenizer, &ids);
        let (got, after) = streamed_text(&tokenizer, &ids);
        assert_eq!(got, want);
        assert_eq!(before, 8_390_656);
        assert!(after < 8 * ids.len(), "{after} token IDs decoded");
    }

    /// The reasoning, split characters, and answer cases repeated to `len`
    /// tokens, with no end token.
    fn long_answer(len: usize) -> Vec<u32> {
        let mut unit = fixture::stream_case("split_utf8");
        unit.extend(fixture::stream_case("reasoning_then_final"));
        unit.retain(|id| *id != EOS && *id != END_THINKING);
        unit.into_iter().cycle().take(len).collect()
    }

    /// The last stage's parsing at the output cap, as `Server::run_step`
    /// does it: the raw text stream, then the answer parser, per token.
    /// Run with `--ignored --nocapture` to print the cost before and after
    /// #12412; the timing depends on the machine, so nothing is asserted.
    #[test]
    #[ignore = "prints a timing"]
    fn measure_parsing_at_the_output_cap() {
        let ids = long_answer(4_096);
        let tokenizer = fixture::protocol().tokenizer();
        let parse = |text: &mut dyn FnMut(u32)| {
            let started = std::time::Instant::now();
            let mut parser = AnswerParser::new(Arc::clone(&tokenizer), EOS, PromptMode::Thinking);
            for (i, id) in ids.iter().enumerate() {
                text(*id);
                parser.push(*id, i + 1 == ids.len()).unwrap();
            }
            started.elapsed()
        };
        let mut emitted = String::new();
        let mut seen = Vec::new();
        let before = parse(&mut |id| {
            seen.push(id);
            let text = tokenizer.decode(&seen, false).unwrap();
            if !text.ends_with('\u{fffd}') && text.starts_with(&emitted) {
                emitted = text;
            }
        });
        let mut stream = TextStream::default();
        let after = parse(&mut |id| {
            let _ = stream.push(&tokenizer, id);
        });
        let parser_only = parse(&mut |_| {});
        eprintln!(
            "token IDs decoded: whole-answer text {}, incremental text {}",
            ids.len() * (ids.len() + 1) / 2,
            stream.decoded_ids()
        );
        eprintln!(
            "{} tokens: whole-answer text {before:?}, incremental text {after:?}, parser alone {parser_only:?}",
            ids.len()
        );
    }

    #[test]
    fn a_token_after_the_end_is_refused() {
        let mut parser = AnswerParser::new(fixture::protocol().tokenizer(), EOS, PromptMode::Chat);
        assert_eq!(
            parser.push(EOS, false).unwrap().finish.as_deref(),
            Some("stop")
        );
        assert!(parser.push(5, false).is_err());
    }
}
