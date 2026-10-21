//! `psionic-v41-stage chain`: a whole group's CUDA stages in one process,
//! one stage per GPU, for measurement (W3, #12390) and complete answers
//! (W2, #12389).
//!
//! ```text
//! psionic-v41-stage chain --weights DIR --split 0-10,10-20,20-30,30-40
//!     [--gpus 0,1,2,3] [--max-context N] [--max-step-tokens N]
//!     [--prompt JSON | --prompt-file FILE [--repeat N]] [--mode thinking|chat]
//!     [--max-tokens N] [--prefill-chunk N] [--temperature T --top-p P --seed S]
//!     [--engram-lookups N] [--load-only] [--runtime LABEL]
//! ```
//!
//! Each stage runs W11's executor behind W9's stage worker on its own
//! thread and GPU, and each hop goes through the stage-state codec with
//! its default limits, as a group's stages do; only the network and Pylon
//! are left out. The run prints one JSON report: each stage's measured
//! device memory (free memory before loading, after loading, and the least
//! seen while running) beside the stage's own allocation ledger, step
//! times, the Engram lookup cost from page-locked host memory, the
//! parameters and bytes read per decode token, and the answer with its
//! token ids and its reasoning and final text as the #12372 adapter splits
//! them.
//!
//! `--prefill-chunk N` uses the group's optional serial chunk schedule,
//! with independent step buffers. Intermediate chunks update state without
//! sampling; the report records the schedule.

use std::{
    fmt::Write as _,
    ops::Range,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Instant,
};

use psionic_backend_cuda::{
    CudaBackend, CudaCommandWait,
    v41_matvec::V41Output,
    v41_mix::V41EngramTable,
    v41_stage::{V41Stage, V41StageMemory, V41StageShape},
};
use psionic_models::{
    deepseek_v41_stage::V41GroupManifest,
    deepseek_v41_weights::{
        DEEPSEEK_V41_FLASH_INDEX_SHA256, DEEPSEEK_V41_FLASH_REPO, DEEPSEEK_V41_FLASH_REVISION,
    },
    stage_state::{CodecLimits, Section, StageInbox, StageMessage, StageWorker, StepKind, encode},
};
use serde_json::{Value, json};

use super::{CudaModel, V41CudaStageExecutor};
use crate::{
    AnswerParser, PromptMode, Refused, Sampling, StageBackend, TextStream, Vocabulary,
    parse_layers, synthetic_manifest,
};

/// Report schema.
pub const CHAIN_SCHEMA: &str = "openagents.deepseek_v41.chain_run.v1";

/// What `chain` was asked to do.
#[derive(Clone, Debug)]
pub struct ChainOptions {
    pub weights: PathBuf,
    pub split: Vec<Range<u32>>,
    pub gpus: Vec<usize>,
    pub max_context: usize,
    pub max_step_tokens: usize,
    pub prompt: Option<Value>,
    pub mode: PromptMode,
    pub max_tokens: u64,
    pub prefill_chunk: Option<usize>,
    pub sampling: Sampling,
    pub engram_lookups: usize,
    pub load_only: bool,
    pub runtime: String,
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn number<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T, Refused> {
    flag(args, name).map_or(Ok(default), |v| {
        v.parse()
            .map_err(|_| Refused(format!("{name} must be a number")))
    })
}

impl ChainOptions {
    /// Parses `chain`'s arguments.
    ///
    /// # Errors
    ///
    /// Refuses missing or malformed values.
    pub fn parse(args: &[String]) -> Result<Self, Refused> {
        let weights = flag(args, "--weights")
            .ok_or_else(|| Refused("--weights is required".into()))?
            .into();
        let split = flag(args, "--split")
            .unwrap_or("0-10,10-20,20-30,30-40")
            .split(',')
            .map(parse_layers)
            .collect::<Result<Vec<_>, _>>()?;
        let gpus = match flag(args, "--gpus") {
            Some(list) => list
                .split(',')
                .map(|g| {
                    g.parse()
                        .map_err(|_| Refused("--gpus takes ordinals".into()))
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => (0..split.len()).collect(),
        };
        if gpus.len() != split.len() {
            return Err(Refused("--gpus names one GPU per stage".into()));
        }
        let max_context = number(args, "--max-context", 8192)?;
        let max_step_tokens = number(args, "--max-step-tokens", max_context)?;
        let prompt = match (flag(args, "--prompt"), flag(args, "--prompt-file")) {
            (Some(_), Some(_)) => {
                return Err(Refused("give --prompt or --prompt-file, not both".into()));
            }
            (Some(json), None) => Some(
                serde_json::from_str::<Value>(json)
                    .map_err(|e| Refused(format!("--prompt: {e}")))?,
            ),
            (None, Some(file)) => {
                let text = std::fs::read_to_string(file)
                    .map_err(|e| Refused(format!("can't read {file}: {e}")))?;
                let repeat = number(args, "--repeat", 1_usize)?.max(1);
                Some(json!([{"role": "user", "content": text.repeat(repeat)}]))
            }
            (None, None) => None,
        };
        let mode = flag(args, "--mode").map_or(Ok(PromptMode::Thinking), PromptMode::parse)?;
        Ok(Self {
            weights,
            split,
            gpus,
            max_context,
            max_step_tokens,
            prompt,
            mode,
            max_tokens: number(args, "--max-tokens", 1024)?,
            prefill_chunk: flag(args, "--prefill-chunk")
                .map(|v| {
                    v.parse()
                        .map_err(|_| Refused("--prefill-chunk must be a number".into()))
                })
                .transpose()?,
            sampling: Sampling {
                temperature: number(args, "--temperature", 0.0)?,
                top_p: number(args, "--top-p", 1.0)?,
                seed: number(args, "--seed", 0)?,
            },
            engram_lookups: number(args, "--engram-lookups", 2000)?,
            load_only: args.iter().any(|a| a == "--load-only"),
            runtime: flag(args, "--runtime").unwrap_or("unlabeled").to_owned(),
        })
    }
}

/// A manifest for the pinned checkpoint with this split, one GPU each.
fn checkpoint_manifest(split: &[Range<u32>], gpus: &[usize]) -> Result<V41GroupManifest, Refused> {
    let mut manifest = synthetic_manifest(split)?;
    manifest.repo = DEEPSEEK_V41_FLASH_REPO.into();
    manifest.revision = DEEPSEEK_V41_FLASH_REVISION.into();
    manifest.index_sha256 = DEEPSEEK_V41_FLASH_INDEX_SHA256.into();
    manifest.memory_source = "chain".into();
    for (stage, gpu) in manifest.stages.iter_mut().zip(gpus) {
        stage.gpu = *gpu as u32;
    }
    Ok(manifest)
}

/// Free and total bytes of the backend's device.
fn free(backend: &CudaBackend) -> Result<(u64, u64), Refused> {
    let info = backend
        .device_memory_info()
        .map_err(|e| Refused(format!("cudaMemGetInfo: {e}")))?
        .ok_or_else(|| Refused("no CUDA device".into()))?;
    Ok((info.free_bytes, info.total_bytes))
}

/// The next value of a `splitmix64` stream.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Times the Engram gather from each of the stage's page-locked tables:
/// `lookups` decode lookups of 24 uniformly random rows, each waited on
/// alone and then all in one submission, and one prefill-sized gather of
/// 24 × 1,024 rows.
fn engram_bench(
    backend: &mut CudaBackend,
    stage: &V41Stage,
    lookups: usize,
) -> Result<Vec<Value>, Refused> {
    const ROWS: usize = 24;
    const PREFILL_TOKENS: usize = 1024;
    let width = stage.config().shape.engram_width;
    let bad = |e: &dyn std::fmt::Display| Refused(format!("the Engram benchmark: {e}"));
    let mut reports = Vec::new();
    for table in stage.engram_tables() {
        let mut seed = 0x0E4A_u64 ^ table.layer as u64;
        let lookups = lookups.max(1);
        let total = lookups.max(PREFILL_TOKENS) * ROWS;
        let ids: Vec<u8> = (0..total)
            .flat_map(|_| ((splitmix(&mut seed) % table.rows) as i64).to_le_bytes())
            .collect();
        let all_ids = backend.byte_buffer(&ids).map_err(|e| bad(&e))?;
        let slot = backend
            .byte_buffer(&vec![0; ROWS * 8])
            .map_err(|e| bad(&e))?;
        let out = backend
            .byte_buffer(&vec![0; total * width * 4])
            .map_err(|e| bad(&e))?;
        let view = V41EngramTable {
            codes: &table.codes,
            scales: &table.scales,
            rows: table.rows,
            width,
        };
        let gather = |backend: &CudaBackend, from: usize, waited: bool| -> Result<(), Refused> {
            let mut s = backend.begin_submission().map_err(|e| bad(&e))?;
            s.copy_buffer_region(&all_ids, from * ROWS * 8, &slot, 0, ROWS * 8)
                .map_err(|e| bad(&e))?;
            s.v41_engram_gather(view, &slot, ROWS, &out, V41Output::Bf16)
                .map_err(|e| bad(&e))?;
            if waited {
                s.commit(CudaCommandWait::Completed).map_err(|e| bad(&e))?;
            }
            Ok(())
        };
        // Warm up the path once.
        gather(backend, 0, true)?;
        let started = Instant::now();
        for i in 0..lookups {
            gather(backend, i, true)?;
        }
        let waited_us = started.elapsed().as_secs_f64() * 1e6 / lookups as f64;
        let started = Instant::now();
        let mut s = backend.begin_submission().map_err(|e| bad(&e))?;
        for i in 0..lookups {
            s.copy_buffer_region(&all_ids, i * ROWS * 8, &slot, 0, ROWS * 8)
                .map_err(|e| bad(&e))?;
            s.v41_engram_gather(view, &slot, ROWS, &out, V41Output::Bf16)
                .map_err(|e| bad(&e))?;
        }
        s.commit(CudaCommandWait::Completed).map_err(|e| bad(&e))?;
        let streamed_us = started.elapsed().as_secs_f64() * 1e6 / lookups as f64;
        let rows = ROWS * PREFILL_TOKENS;
        let started = Instant::now();
        let mut s = backend.begin_submission().map_err(|e| bad(&e))?;
        s.v41_engram_gather(view, &all_ids, rows, &out, V41Output::Bf16)
            .map_err(|e| bad(&e))?;
        s.commit(CudaCommandWait::Completed).map_err(|e| bad(&e))?;
        let prefill_s = started.elapsed().as_secs_f64();
        let row_bytes = width + width / 32;
        reports.push(json!({
            "layer": table.layer,
            "table_rows": table.rows,
            "row_bytes": row_bytes,
            "host_bytes": table.codes.byte_len() + table.scales.byte_len(),
            "rows_per_token": ROWS,
            "bytes_per_token": ROWS * row_bytes,
            "decode_lookups": lookups,
            "decode_us_waited": waited_us,
            "decode_us_in_stream": streamed_us,
            "prefill_rows": rows,
            "prefill_ms": prefill_s * 1e3,
            "prefill_rows_per_s": rows as f64 / prefill_s,
            "prefill_gb_per_s": (rows * row_bytes) as f64 / prefill_s / 1e9,
            "ids": "uniformly random rows (splitmix64), not hashed n-grams",
        }));
    }
    Ok(reports)
}

/// Parameters and bytes a decode token reads on one stage: every weight
/// but the routed experts, `top_k` of each layer's routed experts, the
/// head, and one embedding row.
fn per_token(memory: &V41StageMemory, shape: &V41StageShape) -> Value {
    let (experts, top_k) = (shape.router.experts as u64, shape.router.top_k as u64);
    let mut parameters = 0_u64;
    let mut bytes = 0_u64;
    for l in &memory.layers {
        parameters += l.parameters - l.routed_expert_parameters
            + l.routed_expert_parameters * top_k / experts;
        bytes += (l.weight_bytes - l.routed_expert_bytes) as u64
            + l.routed_expert_bytes as u64 * top_k / experts;
    }
    parameters += memory.head_parameters;
    bytes += memory.head_bytes as u64;
    if memory.embedding_parameters > 0 {
        let row = memory.embedding_parameters / shape.vocabulary as u64;
        parameters += row;
        bytes += memory.embedding_bytes as u64 / shape.vocabulary as u64;
    }
    json!({"parameters": parameters, "weight_bytes": bytes})
}

fn memory_json(memory: &V41StageMemory) -> Value {
    json!({
        "max_context": memory.max_context,
        "max_step_tokens": memory.max_step_tokens,
        "layers": memory.layers.iter().map(|l| json!({
            "layer": l.layer,
            "weight_bytes": l.weight_bytes,
            "routed_expert_bytes": l.routed_expert_bytes,
            "parameters": l.parameters,
            "routed_expert_parameters": l.routed_expert_parameters,
            "state_bytes": l.state_bytes,
        })).collect::<Vec<_>>(),
        "embedding_bytes": memory.embedding_bytes,
        "embedding_parameters": memory.embedding_parameters,
        "head_bytes": memory.head_bytes,
        "head_parameters": memory.head_parameters,
        "constant_bytes": memory.constant_bytes,
        "sources": memory.sources.iter().map(|s| json!({
            "layer": s.layer,
            "ratio": s.ratio,
            "capacity": s.capacity,
            "kv_bytes": s.kv_bytes,
            "index_key_bytes": s.index_key_bytes,
            "compressor_state_bytes": s.compressor_state_bytes,
        })).collect::<Vec<_>>(),
        "selection_bytes": memory.selection_bytes,
        "work_bytes": memory.work_bytes,
        "work_step_bytes": memory.work_step_bytes,
        "engram_host_bytes": memory.engram_host_bytes,
        "device_total": memory.device_total(),
    })
}

enum Command {
    Step {
        bytes: Vec<u8>,
        sampling: Option<Sampling>,
        step: u64,
        emit: bool,
    },
    Report,
}

enum Reply {
    Loaded(Value),
    Stepped {
        sections: Vec<Section>,
        token: Option<u32>,
        ms: f64,
    },
    Report(Value),
    Failed(String),
}

struct StageThread {
    commands: mpsc::Sender<Command>,
    replies: mpsc::Receiver<Reply>,
    handle: thread::JoinHandle<()>,
}

/// Loads one stage on `gpu` and serves commands until the channel closes.
#[allow(clippy::too_many_arguments)]
fn stage_thread(
    weights: &Path,
    manifest: &V41GroupManifest,
    index: usize,
    gpu: usize,
    options: &ChainOptions,
    commands: &mpsc::Receiver<Command>,
    replies: &mpsc::Sender<Reply>,
) -> Result<(), Refused> {
    let entry = manifest.stages[index].clone();
    // Only the stage's shards, hashed against the pin, as a stage machine
    // holds them.
    let model = CudaModel::load(weights, manifest.selection(index).as_ref())?;
    let mut backend = CudaBackend::with_device_index(gpu);
    if backend.selected_device().is_none() || !backend.quantized_kernels_available() {
        return Err(Refused(format!(
            "GPU {gpu} has no CUDA device or V4.1 kernels"
        )));
    }
    let device = backend
        .selected_device()
        .map(|d| d.device_name.clone().unwrap_or_default())
        .unwrap_or_default();
    let (free_before, total) = free(&backend)?;
    let started = Instant::now();
    let stage = model.stage_sized(
        &mut backend,
        entry.first_layer as usize..entry.end_layer as usize,
        options.max_context,
        options.max_step_tokens,
    )?;
    let load_s = started.elapsed().as_secs_f64();
    let (free_loaded, _) = free(&backend)?;
    let engram = engram_bench(&mut backend, &stage, options.engram_lookups)?;
    let (free_bench, _) = free(&backend)?;
    let memory = stage.memory().clone();
    let (weight_bytes, state_bytes) = stage.device_bytes();
    let shape = stage.config().shape;
    let executor = V41CudaStageExecutor::new(backend, stage, entry.stage);
    let inbox = StageInbox::new(
        manifest.digest(),
        1,
        executor.contract(),
        CodecLimits::default(),
    );
    let mut worker = StageWorker::new(inbox, executor);
    let _ = replies.send(Reply::Loaded(json!({
        "stage": index,
        "layers": [entry.first_layer, entry.end_layer],
        "gpu": gpu,
        "device": device,
        "device_total_bytes": total,
        "free_before_load": free_before,
        "free_after_load": free_loaded,
        "free_after_engram_bench": free_bench,
        "used_before_load": total.saturating_sub(free_before),
        "used_by_load": free_before.saturating_sub(free_loaded),
        "load_seconds": load_s,
        "ledger": memory_json(&memory),
        "device_bytes": {"weights": weight_bytes, "state": state_bytes},
        "per_decode_token": per_token(&memory, &shape),
        "engram": engram,
    })));
    let mut free_min = free_bench;
    let mut steps = Vec::new();
    for command in commands {
        match command {
            Command::Step {
                bytes,
                sampling,
                step,
                emit,
            } => {
                if let Some(sampling) = sampling {
                    worker.executor().prepare_output(sampling, step, emit);
                }
                let started = Instant::now();
                let result = worker.step_bytes(&bytes);
                let ms = started.elapsed().as_secs_f64() * 1e3;
                match result {
                    Ok(sections) => {
                        let token = worker.executor().sampled();
                        let (now, _) = free(worker.executor().backend())?;
                        free_min = free_min.min(now);
                        steps.push(ms);
                        let _ = replies.send(Reply::Stepped {
                            sections,
                            token,
                            ms,
                        });
                    }
                    Err(e) => {
                        let _ = replies.send(Reply::Failed(format!("stage {index}: {e}")));
                    }
                }
            }
            Command::Report => {
                let _ = replies.send(Reply::Report(json!({
                    "free_min_while_running": free_min,
                    "peak_used": free_before.saturating_sub(free_min),
                    "peak_used_total": total.saturating_sub(free_min),
                    "step_ms": steps,
                })));
            }
        }
    }
    Ok(())
}

fn summary(values: &[f64]) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
    json!({
        "count": sorted.len(),
        "mean": sorted.iter().sum::<f64>() / sorted.len() as f64,
        "p50": at(0.5),
        "p90": at(0.9),
        "min": sorted[0],
        "max": sorted[sorted.len() - 1],
    })
}

fn recv(thread: &StageThread) -> Result<Reply, Refused> {
    match thread.replies.recv() {
        Ok(Reply::Failed(e)) => Err(Refused(e)),
        Ok(reply) => Ok(reply),
        Err(_) => Err(Refused("a stage thread stopped".into())),
    }
}

/// The tokenizer and template, the end token, and the pad token, without
/// opening the shards.
fn text_side(weights: &Path) -> Result<(Vocabulary, u32, u32), Refused> {
    if weights.join(crate::SYNTHETIC_FILE).is_file() {
        // The CUDA stages' synthetic model, with its own vocabulary.
        let model = CudaModel::load(weights, None)?;
        return Ok((model.vocabulary, model.eos, model.pad));
    }
    let read = |name: &str| {
        std::fs::read(weights.join(name)).map_err(|e| Refused(format!("can't read {name}: {e}")))
    };
    let protocol = psionic_models::deepseek_v41::DeepSeekV41Protocol::from_checkpoint_files(
        &read("tokenizer.json")?,
        &read("tokenizer_config.json")?,
    )
    .map_err(|e| Refused(format!("the checkpoint's tokenizer: {e}")))?;
    let config = psionic_models::deepseek_v41_cpu::transformer::TransformerConfig::flash()
        .map_err(|e| Refused(e.to_string()))?;
    Ok((
        Vocabulary::V41(Box::new(protocol)),
        config.eos_token,
        config.pad_token,
    ))
}

/// Runs `chain` and returns its report.
///
/// # Errors
///
/// Any refusal of a stage, the model, or the prompt.
#[allow(clippy::too_many_lines)]
pub fn run(options: &ChainOptions) -> Result<Value, Refused> {
    let manifest = checkpoint_manifest(&options.split, &options.gpus)?;
    let (vocabulary, eos, pad) = text_side(&options.weights)?;
    let tokenizer = vocabulary.tokenizer();
    let mode = options.mode;
    let prompt_ids = vocabulary.encode(options.prompt.as_ref(), pad, mode)?;
    let mut threads = Vec::new();
    for (index, gpu) in options.gpus.iter().copied().enumerate() {
        let (commands, command_rx) = mpsc::channel();
        let (reply_tx, replies) = mpsc::channel();
        let (weights, manifest, options) =
            (options.weights.clone(), manifest.clone(), options.clone());
        let handle = thread::Builder::new()
            .name(format!("stage-{index}"))
            .spawn(move || {
                if let Err(e) = stage_thread(
                    &weights,
                    &manifest,
                    index,
                    gpu,
                    &options,
                    &command_rx,
                    &reply_tx,
                ) {
                    let _ = reply_tx.send(Reply::Failed(format!("stage {index}: {e}")));
                }
            })
            .map_err(|e| Refused(format!("can't start a stage thread: {e}")))?;
        threads.push(StageThread {
            commands,
            replies,
            handle,
        });
    }
    // Whatever happens, the stage threads finish before the process does:
    // a process that exits while they still hold their devices can crash in
    // the CUDA teardown.
    let driven = (|| -> Result<(Vec<Value>, Value), Refused> {
        let mut loaded = Vec::new();
        for thread in &threads {
            match recv(thread)? {
                Reply::Loaded(report) => loaded.push(report),
                _ => return Err(Refused("a stage answered out of order".into())),
            }
        }
        let mut answer = Value::Null;
        if !options.load_only {
            let last = threads.len() - 1;
            let digest = manifest.digest();
            let mut parser = AnswerParser::new(std::sync::Arc::clone(&tokenizer), eos, mode);
            let mut text = TextStream::default();
            let (mut reasoning, mut content, mut finish) = (String::new(), String::new(), None);
            let (mut tokens, mut raw) = (Vec::new(), String::new());
            let mut token_ms = Vec::new();
            let mut stage_ms: Vec<Vec<f64>> = vec![Vec::new(); threads.len()];
            // The largest encoded message into each stage.
            let mut hop_bytes = vec![0_usize; threads.len()];
            let mut prefill_ms = 0.0;
            let chunk = options.prefill_chunk.unwrap_or(prompt_ids.len()).max(1);
            let mut pending: Vec<Vec<u32>> =
                prompt_ids.chunks(chunk).map(<[u32]>::to_vec).collect();
            pending.reverse();
            let mut position = 0_u64;
            let mut step = 0_u64;
            let mut produced = 0_u64;
            while produced < options.max_tokens {
                let Some(input) = pending.pop() else {
                    break;
                };
                let prefill_part = !pending.is_empty();
                let started = Instant::now();
                let mut message = StageMessage {
                    manifest_digest: digest,
                    generation: 1,
                    request_id: "chain".into(),
                    attempt: 0,
                    stage: 0,
                    layers: manifest.stages[0].layers(),
                    step,
                    kind: if input.len() > 1 {
                        StepKind::Prefill
                    } else {
                        StepKind::Decode
                    },
                    position,
                    tokens: input.clone(),
                    sections: Vec::new(),
                };
                let mut sampled = None;
                for (k, thread) in threads.iter().enumerate() {
                    // Encode without the size limits to measure the message,
                    // then hold it to the codec's default limits as a group does.
                    let unbounded = CodecLimits {
                        max_message_bytes: usize::MAX,
                        max_section_bytes: usize::MAX,
                        max_tokens: usize::MAX,
                        ..CodecLimits::default()
                    };
                    let bytes = encode(&message, &unbounded)
                        .map_err(|e| Refused(format!("hop to stage {k}: {e}")))?;
                    let largest = &mut hop_bytes[k];
                    *largest = (*largest).max(bytes.len());
                    if let Err(e) = encode(&message, &CodecLimits::default()) {
                        return Err(Refused(format!(
                            "hop to stage {k}: {} bytes for {} tokens: {e}",
                            bytes.len(),
                            message.tokens.len()
                        )));
                    }
                    let _ = thread.commands.send(Command::Step {
                        bytes,
                        sampling: (k == last).then_some(options.sampling),
                        step,
                        emit: !prefill_part,
                    });
                    let Reply::Stepped {
                        sections,
                        token,
                        ms,
                    } = recv(thread)?
                    else {
                        return Err(Refused("a stage answered out of order".into()));
                    };
                    stage_ms[k].push(ms);
                    if k == last {
                        sampled = token;
                    } else {
                        let next = &manifest.stages[k + 1];
                        message = message.next_hop((k + 1) as u16, next.layers(), sections);
                    }
                }
                let ms = started.elapsed().as_secs_f64() * 1e3;
                position += input.len() as u64;
                step += 1;
                if prefill_part {
                    // Intermediate chunks update state without running the head.
                    prefill_ms += ms;
                    continue;
                }
                let token =
                    sampled.ok_or_else(|| Refused("the last stage sampled no token".into()))?;
                if input.len() > 1 || produced == 0 {
                    prefill_ms += ms;
                } else {
                    token_ms.push(ms);
                }
                produced += 1;
                tokens.push(token);
                raw.push_str(&text.push(&tokenizer, token));
                if matches!(vocabulary, Vocabulary::V41(_)) {
                    let piece = parser.push(token, produced >= options.max_tokens)?;
                    reasoning.push_str(&piece.reasoning);
                    content.push_str(&piece.content);
                    if piece.finish.is_some() {
                        finish = piece.finish;
                    }
                }
                if token == eos {
                    break;
                }
                pending.push(vec![token]);
            }
            let decode_ms: f64 = token_ms.iter().sum();
            answer = json!({
                "mode": mode.as_str(),
                "prompt_tokens": prompt_ids.len(),
                "prompt_ids": prompt_ids,
                "prefill_chunk": options.prefill_chunk,
                "tokens": tokens.len(),
                "ids": tokens,
                "eos": tokens.last() == Some(&eos),
                "finish": finish,
                "text": raw,
                "reasoning": reasoning,
                "content": content,
                "prefill_ms": prefill_ms,
                "decode_ms_per_token": summary(&token_ms),
                "decode_tokens_per_s": if decode_ms > 0.0 { token_ms.len() as f64 / (decode_ms / 1e3) } else { 0.0 },
                "stage_step_ms": stage_ms.iter().map(|m| summary(m)).collect::<Vec<_>>(),
                "largest_message_bytes_into_stage": hop_bytes,
            });
        }
        let mut stages = Vec::new();
        for (thread, mut report) in threads.iter().zip(loaded) {
            let _ = thread.commands.send(Command::Report);
            if let Reply::Report(run) = recv(thread)? {
                if let Some(map) = report.as_object_mut() {
                    map.insert("run".into(), run);
                }
            }
            stages.push(report);
        }
        Ok((stages, answer))
    })();
    for thread in threads {
        drop(thread.commands);
        let _ = thread.handle.join();
    }
    let (stages, answer) = driven?;
    let mut note = String::new();
    if options.prefill_chunk.is_some() {
        let _ = write!(
            note,
            "serial prefill chunks of at most {} tokens; ",
            options.prefill_chunk.unwrap_or_default()
        );
    }
    note.push_str("stages are threads of one process, one GPU each, with W9's codec between them");
    Ok(json!({
        "schema": CHAIN_SCHEMA,
        "runtime": options.runtime,
        "split": options.split.iter().map(|r| [r.start, r.end]).collect::<Vec<_>>(),
        "max_context": options.max_context,
        "max_step_tokens": options.max_step_tokens,
        "sampling": {
            "temperature": options.sampling.temperature,
            "top_p": options.sampling.top_p,
            "seed": options.sampling.seed,
        },
        "note": note,
        "stages": stages,
        "answer": answer,
    }))
}
