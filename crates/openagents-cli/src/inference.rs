//! `openagents inference` — product load path and teach statuses.

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use psionic_gguf::format::ParseError;
use psionic_gguf::metal_wrap::MetalShared;
use psionic_gguf::{
    AdmitError, MappedWeights, admit, format_bar, format_size, ggml_type_name, load_tokenizer,
    map_file, parse_path, plan_caches, render_chat, runtime_n_ctx, should_emit,
};

#[derive(Args, Debug)]
pub struct InferenceArgs {
    #[command(subcommand)]
    pub action: InferenceAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InferenceBackend {
    Auto,
    Metal,
    Cpu,
}

#[derive(Subcommand, Debug)]
pub enum InferenceAction {
    /// Walk the load-and-generate path in this process
    Run {
        #[arg(long, help = "Local GGUF path")]
        gguf: Option<PathBuf>,
        #[arg(long, help = "Admitted store id (later)")]
        model: Option<String>,
        #[arg(long, help = "Prompt for tokenize and generate steps")]
        prompt: Option<String>,
        #[arg(long, help = "Decode budget")]
        max_tokens: Option<u32>,
        #[arg(long, help = "Runtime context length (default 4096 on 27B-class)")]
        ctx: Option<u64>,
        #[arg(long, value_enum, default_value_t = InferenceBackend::Auto)]
        backend: InferenceBackend,
        #[arg(long, default_value_t = true, help = "Teach mode (default on)")]
        #[arg(long = "no-teach", action = clap::ArgAction::SetFalse)]
        teach: bool,
        #[arg(long, help = "One line per phase, no teach explanations")]
        quiet: bool,
        #[arg(long, help = "Print the full status script; do not open a GGUF")]
        preview: bool,
        #[arg(long, help = "Stop after this step succeeds")]
        until: Option<String>,
    },
    /// List admitted GGUFs in the local store
    Models,
    /// Admit a GGUF into the local store
    Add {
        #[arg(help = "Path or allowlisted ref")]
        path: PathBuf,
    },
    /// Delete a store entry
    Remove {
        #[arg(help = "Model id")]
        model: String,
    },
    /// Load then bind OpenAI-compatible HTTP on 127.0.0.1
    Serve,
    /// Show whether a serve child is loaded
    Status,
    /// Stop a serve child
    Stop,
    /// Release in-process mmap and Metal buffers
    Unload,
    /// Backends, store path, Metal/CPU presence
    Doctor,
    /// Time map, ctx, prefill, and generate
    Bench {
        #[arg(long, help = "Local GGUF path")]
        gguf: Option<PathBuf>,
        #[arg(long, help = "Prompt for tokenize and generate")]
        prompt: Option<String>,
        #[arg(long, help = "Decode budget")]
        max_tokens: Option<u32>,
        #[arg(long, help = "Runtime context length")]
        ctx: Option<u64>,
        #[arg(long, value_enum, default_value_t = InferenceBackend::Auto)]
        backend: InferenceBackend,
        #[arg(long, help = "Time local Ollama on the same prompt")]
        compare_ollama: Option<String>,
    },
}

#[derive(Debug)]
pub enum InferenceExit {
    Failed,
    Usage(String),
}

pub fn run(args: InferenceArgs, json: bool) -> Result<(), InferenceExit> {
    match args.action {
        InferenceAction::Run {
            gguf,
            model: _,
            prompt,
            max_tokens,
            ctx,
            backend,
            teach,
            quiet,
            preview,
            until,
        } => {
            let teach_on = teach && !quiet;
            walk_run(WalkOpts {
                gguf,
                backend,
                teach: teach_on,
                quiet,
                preview,
                until,
                json,
                suppress_stderr: false,
                on_step: None,
                prompt,
                max_tokens,
                n_ctx: ctx,
                bench: false,
                compare_ollama: None,
            })
        }
        InferenceAction::Bench {
            gguf,
            prompt,
            max_tokens,
            ctx,
            backend,
            compare_ollama,
        } => walk_run(WalkOpts {
            gguf,
            backend,
            teach: false,
            quiet: true,
            preview: false,
            until: None,
            json,
            suppress_stderr: false,
            on_step: None,
            prompt,
            max_tokens,
            n_ctx: ctx,
            bench: true,
            compare_ollama,
        }),
        InferenceAction::Doctor => {
            run_doctor(json);
            Ok(())
        }
        InferenceAction::Models => {
            if json {
                println!("{}", json!({"loaded": loaded(), "models": []}));
            } else {
                println!("No admitted models.");
            }
            Ok(())
        }
        InferenceAction::Status => {
            print_status(json);
            Ok(())
        }
        InferenceAction::Unload => unload(json, false),
        InferenceAction::Add { .. }
        | InferenceAction::Remove { .. }
        | InferenceAction::Serve
        | InferenceAction::Stop => Err(InferenceExit::Usage(
            "this command is not built yet; use inference run --gguf through gen.done".into(),
        )),
    }
}

struct WalkOpts {
    gguf: Option<PathBuf>,
    backend: InferenceBackend,
    teach: bool,
    quiet: bool,
    preview: bool,
    until: Option<String>,
    json: bool,
    suppress_stderr: bool,
    on_step: Option<std::sync::Arc<dyn Fn(&str, &str, &str) + Send + Sync>>,
    prompt: Option<String>,
    max_tokens: Option<u32>,
    n_ctx: Option<u64>,
    bench: bool,
    compare_ollama: Option<String>,
}

struct Printer {
    json: bool,
    teach: bool,
    quiet: bool,
    suppress_stderr: bool,
    on_step: Option<std::sync::Arc<dyn Fn(&str, &str, &str) + Send + Sync>>,
}

impl Printer {
    fn emit(&self, id: &str, message: &str, state: &str, extra: Value) {
        if let Some(on_step) = &self.on_step {
            on_step(id, message, state);
        }
        if self.suppress_stderr {
            return;
        }
        if self.json {
            let mut obj = extra;
            if !obj.is_object() {
                obj = json!({});
            }
            obj["id"] = json!(id);
            obj["message"] = json!(message);
            obj["state"] = json!(state);
            eprintln!("{obj}");
            return;
        }
        if state == "pending" {
            eprintln!("[{id}] {message:<42} pending");
        } else {
            eprintln!("[{id}] {message}");
            if self.teach && !self.quiet {
                if let Some(note) = explanation(id) {
                    eprintln!("            {note}");
                }
            }
        }
    }

    fn ok(&self, id: &str, message: &str) {
        self.emit(id, message, "ok", json!({}));
    }

    fn ok_extra(&self, id: &str, message: &str, extra: Value) {
        self.emit(id, message, "ok", extra);
    }

    fn skip(&self, id: &str, message: &str) {
        self.emit(id, message, "skip", json!({}));
    }

    fn fail(&self, id: &str, message: &str) {
        self.emit(id, message, "fail", json!({}));
    }

    fn pending(&self, id: &str, message: &str) {
        self.emit(id, message, "pending", json!({}));
    }
}

struct BenchTimes {
    mark: std::time::Instant,
    map_ms: u64,
    ctx_ms: u64,
    prompt_ms: u64,
    prefill_ms: u64,
    gen_ms: u64,
    prompt_tokens: u64,
    generated: u32,
    graph: String,
}

struct Walk {
    printer: Printer,
    until: Option<String>,
    prompt: Option<String>,
    max_tokens: u32,
    n_ctx: Option<u64>,
    bench: bool,
    compare_ollama: Option<String>,
    times: Option<Mutex<BenchTimes>>,
}

impl Walk {
    fn mark(&self, phase: &str) {
        let Some(times) = &self.times else {
            return;
        };
        let Ok(mut t) = times.lock() else {
            return;
        };
        let now = std::time::Instant::now();
        let dt = now.saturating_duration_since(t.mark).as_millis() as u64;
        t.mark = now;
        match phase {
            "map" => t.map_ms = dt,
            "ctx" => t.ctx_ms = dt,
            "prompt" => t.prompt_ms = dt,
            "prefill" => t.prefill_ms = dt,
            "gen" => t.gen_ms = dt,
            _ => {}
        }
    }
}

impl Walk {
    fn hit(&self, id: &str) -> bool {
        self.until.as_deref() == Some(id)
    }
}

fn explanation(id: &str) -> Option<&'static str> {
    match id {
        "gguf.look" => Some("Checking the model store and any --gguf path you passed."),
        _ => None,
    }
}

fn store_path() -> PathBuf {
    dirs_home()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents/inference/models")
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

const PREVIEW_SCRIPT: &[(&str, &str)] = &[
    ("gguf.look", "Looking for GGUF"),
    ("gguf.store", "Checking model store at {store-path}"),
    ("gguf.path", "Checking local path {path}"),
    ("gguf.missing", "GGUF not in store"),
    ("gguf.download", "Downloading GGUF from {source} to {dest}"),
    (
        "gguf.download.progress",
        "Download progress {bytes}/{total} bytes",
    ),
    ("gguf.download.done", "Download complete"),
    ("gguf.hash", "Verifying SHA-256"),
    ("gguf.hash.ok", "Digest matches {digest}"),
    ("gguf.found", "Found GGUF at {path} ({size})"),
    ("gguf.open", "Opening GGUF {path}"),
    ("gguf.load", "Loading GGUF"),
    ("gguf.magic", "Reading magic"),
    ("gguf.magic.ok", "Magic is GGUF"),
    ("gguf.version", "Reading version"),
    ("gguf.version.ok", "Version is {version}"),
    ("gguf.n_tensors", "Reading tensor count"),
    ("gguf.n_tensors.ok", "Tensor count is {n}"),
    ("gguf.n_kv", "Reading key-value count"),
    ("gguf.n_kv.ok", "Key-value count is {n}"),
    ("meta.read", "Reading metadata"),
    ("meta.arch", "Architecture is {arch}"),
    ("meta.ftype", "File type is {ftype}"),
    ("meta.embd", "Hidden width is {n}"),
    ("meta.layers", "Layer count is {n}"),
    ("meta.vocab", "Vocabulary size is {n}"),
    ("meta.ffn", "FFN width is {n}"),
    ("meta.quant", "Quantization is {quant}"),
    ("meta.attn_interval", "Full-attention interval is {n}"),
    ("meta.nextn", "MTP extra layers is {n}"),
    ("meta.ctx", "Trained context length is {n}"),
    ("idx.read", "Reading tensor index"),
    ("idx.ok", "Indexed {n} tensors"),
    ("meta.done", "Metadata ready"),
    ("tok.read", "Reading tokenizer"),
    ("tok.model", "Tokenizer model is {model}"),
    ("tok.tokens", "Loaded {n} tokens"),
    ("tok.merges", "Loaded {n} BPE merges"),
    ("tok.special", "Special tokens bos={bos} eos={eos}"),
    ("tok.done", "Tokenizer ready"),
    (
        "name.translate",
        "Translating Ollama tensor names to llama.cpp names",
    ),
    ("name.check", "Checking required tensor names"),
    ("admit.ok", "Admission passed for {family} digest {digest}"),
    ("store.copy", "Copying GGUF into store {dest}"),
    ("store.manifest", "Wrote manifest {path}"),
    ("admit.done", "Model admitted"),
    ("backend.load", "Loading backends"),
    ("backend.cpu", "CPU backend ready"),
    ("backend.metal", "Metal backend ready"),
    ("backend.pick", "Selected backend is {backend}"),
    ("map.mmap", "Mapping GGUF into memory"),
    ("map.mmap.ok", "mmap size {size}"),
    ("map.tensors", "Creating named tensors"),
    (
        "map.devices",
        "Assigning devices: embeddings on CPU, {n_layers} layers on {backend}, output on {backend}",
    ),
    ("map.metal", "Wrapping mmap as Metal shared buffer"),
    ("map.bind", "Binding tensor data pointers"),
    ("map.unmap", "Unmapping unused header and tail"),
    ("map.done", "Weights ready ({size} mapped)"),
    ("ctx.alloc", "Allocating context"),
    ("ctx.length", "Context length is {n}"),
    (
        "ctx.kv",
        "Allocating KV cache for {n} full-attention layers",
    ),
    (
        "ctx.gdn",
        "Allocating recurrent state for {n} Gated DeltaNet layers",
    ),
    ("ctx.sched", "Graph scheduler ready"),
    ("ctx.done", "Context ready"),
    ("prompt.template", "Applying chat template"),
    ("prompt.tokenize", "Tokenizing prompt"),
    ("prompt.done", "Prompt is {n} tokens"),
    ("prefill.start", "Prefill starting"),
    ("prefill.pos", "Prefill position {i}/{n}"),
    ("prefill.done", "Prefill complete"),
    ("gen.step", "Decode step {i}"),
    ("gen.logits", "Computing logits"),
    ("gen.sample", "Sampling token {id} ({piece})"),
    ("gen.stream", "Streaming {piece}"),
    ("gen.stop.eos", "Stop: end of sequence"),
    ("gen.stop.length", "Stop: token budget {n}"),
    ("gen.stop.cancel", "Stop: cancelled"),
    ("gen.stats", "Generated {n} tokens"),
    ("gen.done", "Inference complete"),
];

fn known_until(id: &str) -> bool {
    id == "script"
        || PREVIEW_SCRIPT.iter().any(|(step, _)| *step == id)
        || matches!(
            id,
            "run.start" | "run.teach" | "run.until" | "run.preview" | "build.stop"
        )
}

fn walk_run(opts: WalkOpts) -> Result<(), InferenceExit> {
    let printer = Printer {
        json: opts.json,
        teach: opts.teach,
        quiet: opts.quiet,
        suppress_stderr: opts.suppress_stderr,
        on_step: opts.on_step,
    };
    if opts.preview || opts.until.as_deref() == Some("script") {
        printer.ok("run.preview", "Preview only; not opening a GGUF");
        for (id, message) in PREVIEW_SCRIPT {
            printer.pending(id, message);
        }
        return Ok(());
    }

    if let Some(id) = opts.until.as_deref() {
        if !known_until(id) {
            return Err(InferenceExit::Usage(format!("unknown --until step {id}")));
        }
    }

    let walk = Walk {
        printer,
        until: opts.until.clone(),
        prompt: opts.prompt.clone(),
        max_tokens: opts.max_tokens.unwrap_or(8),
        n_ctx: opts.n_ctx,
        bench: opts.bench,
        compare_ollama: opts.compare_ollama.clone(),
        times: opts.bench.then(|| {
            Mutex::new(BenchTimes {
                mark: std::time::Instant::now(),
                map_ms: 0,
                ctx_ms: 0,
                prompt_ms: 0,
                prefill_ms: 0,
                gen_ms: 0,
                prompt_tokens: 0,
                generated: 0,
                graph: String::from(psionic_gguf::qwen35::GRAPH_STUB),
            })
        }),
    };
    walk.printer.ok("run.start", "Starting inference run");
    if walk.hit("run.start") {
        return Ok(());
    }
    if opts.teach {
        walk.printer.ok("run.teach", "Teach mode on");
        if walk.hit("run.teach") {
            return Ok(());
        }
    }
    if let Some(id) = &opts.until {
        walk.printer.ok("run.until", &format!("Running until {id}"));
        if walk.hit("run.until") {
            return Ok(());
        }
    }

    walk.printer.ok("gguf.look", "Looking for GGUF");
    if walk.hit("gguf.look") {
        return Ok(());
    }

    walk.printer.skip(
        "gguf.store",
        &format!("Checking model store at {}", store_path().display()),
    );
    if walk.hit("gguf.store") {
        return Ok(());
    }

    let Some(path) = opts.gguf.as_ref() else {
        walk.printer
            .fail("gguf.fail.arg", "No GGUF path given; pass --gguf");
        return Err(InferenceExit::Failed);
    };

    walk.printer.ok(
        "gguf.path",
        &format!("Checking local path {}", path.display()),
    );
    if walk.hit("gguf.path") {
        return Ok(());
    }

    walk.printer.skip("gguf.missing", "GGUF not in store");
    walk.printer
        .skip("gguf.download", "Downloading GGUF from {source} to {dest}");
    walk.printer.skip(
        "gguf.download.progress",
        "Download progress {bytes}/{total} bytes",
    );
    walk.printer.skip("gguf.download.done", "Download complete");
    walk.printer.skip("gguf.hash", "Verifying SHA-256");
    walk.printer.skip("gguf.hash.ok", "Digest matches {digest}");

    if !path.is_file() {
        walk.printer.fail(
            "gguf.fail.missing",
            &format!("GGUF not found at {}", path.display()),
        );
        return Err(InferenceExit::Failed);
    }

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let size_h = format_size(file_size);
    walk.printer.ok_extra(
        "gguf.found",
        &format!("Found GGUF at {} ({size_h})", path.display()),
        json!({"path": path.display().to_string(), "size": file_size}),
    );
    if walk.hit("gguf.found") {
        return Ok(());
    }

    let result = open_and_map(&walk, path, file_size, opts.backend);
    if walk.bench {
        emit_bench_summary(&walk);
    }
    result
}

fn open_and_map(
    walk: &Walk,
    path: &Path,
    file_size: u64,
    backend: InferenceBackend,
) -> Result<(), InferenceExit> {
    walk.printer
        .ok("gguf.open", &format!("Opening GGUF {}", path.display()));
    if walk.hit("gguf.open") {
        return Ok(());
    }
    walk.printer.ok("gguf.load", "Loading GGUF");
    if walk.hit("gguf.load") {
        return Ok(());
    }
    walk.printer.ok("gguf.magic", "Reading magic");
    if walk.hit("gguf.magic") {
        return Ok(());
    }

    let meta = match parse_path(path) {
        Ok(meta) => meta,
        Err(ParseError::Magic { got }) => {
            let shown = got
                .iter()
                .map(|b| {
                    if b.is_ascii_graphic() {
                        (*b as char).to_string()
                    } else {
                        format!("\\x{b:02x}")
                    }
                })
                .collect::<String>();
            walk.printer.fail(
                "gguf.fail.magic",
                &format!("Not a GGUF file (magic is {shown})"),
            );
            return Err(InferenceExit::Failed);
        }
        Err(ParseError::Version(v)) => {
            walk.printer.ok("gguf.magic.ok", "Magic is GGUF");
            walk.printer.ok("gguf.version", "Reading version");
            walk.printer.fail(
                "gguf.fail.version",
                &format!("Unsupported GGUF version {v}"),
            );
            return Err(InferenceExit::Failed);
        }
        Err(ParseError::Truncated(_)) => {
            walk.printer
                .fail("idx.fail", "Tensor index is truncated or corrupt");
            return Err(InferenceExit::Failed);
        }
        Err(ParseError::Trailing) => {
            walk.printer
                .fail("idx.fail", "Tensor index is truncated or corrupt");
            return Err(InferenceExit::Failed);
        }
    };

    walk.printer.ok("gguf.magic.ok", "Magic is GGUF");
    if walk.hit("gguf.magic.ok") {
        return Ok(());
    }
    walk.printer.ok("gguf.version", "Reading version");
    if walk.hit("gguf.version") {
        return Ok(());
    }
    walk.printer
        .ok("gguf.version.ok", &format!("Version is {}", meta.version));
    if walk.hit("gguf.version.ok") {
        return Ok(());
    }

    walk.printer.ok("gguf.n_tensors", "Reading tensor count");
    walk.printer.ok(
        "gguf.n_tensors.ok",
        &format!("Tensor count is {}", meta.n_tensors),
    );
    walk.printer.ok("gguf.n_kv", "Reading key-value count");
    walk.printer
        .ok("gguf.n_kv.ok", &format!("Key-value count is {}", meta.n_kv));
    walk.printer.ok("meta.read", "Reading metadata");

    let arch = meta.architecture().unwrap_or("unknown");
    if arch != "qwen35" {
        walk.printer.fail(
            "meta.fail.arch",
            &format!("Unsupported architecture {arch}"),
        );
        return Err(InferenceExit::Failed);
    }
    walk.printer
        .ok("meta.arch", &format!("Architecture is {arch}"));
    let ftype = meta
        .kv_u64("general.file_type")
        .map(|n| n.to_string())
        .unwrap_or_else(|| {
            meta.tensors
                .first()
                .map(|t| ggml_type_name(t.ggml_type).to_string())
                .unwrap_or_else(|| String::from("unknown"))
        });
    walk.printer
        .ok("meta.ftype", &format!("File type is {ftype}"));
    emit_kv_u64(
        walk,
        "meta.embd",
        "Hidden width is {}",
        "qwen35.embedding_length",
        &meta,
    );
    emit_kv_u64(
        walk,
        "meta.layers",
        "Layer count is {}",
        "qwen35.block_count",
        &meta,
    );
    let vocab = meta
        .get("tokenizer.ggml.tokens")
        .and_then(psionic_gguf::format::GgufValue::as_string_array)
        .map(|t| t.len() as u64)
        .or_else(|| meta.kv_u64("qwen35.vocab_size"));
    if let Some(n) = vocab {
        walk.printer
            .ok("meta.vocab", &format!("Vocabulary size is {n}"));
    } else {
        walk.printer.skip("meta.vocab", "Vocabulary size is {n}");
    }
    emit_kv_u64(
        walk,
        "meta.ffn",
        "FFN width is {}",
        "qwen35.feed_forward_length",
        &meta,
    );
    let quant = meta
        .tensors
        .first()
        .map(|t| ggml_type_name(t.ggml_type))
        .unwrap_or("unknown");
    walk.printer
        .ok("meta.quant", &format!("Quantization is {quant}"));
    emit_kv_u64(
        walk,
        "meta.attn_interval",
        "Full-attention interval is {}",
        "qwen35.full_attention_interval",
        &meta,
    );
    emit_kv_u64(
        walk,
        "meta.nextn",
        "MTP extra layers is {}",
        "qwen35.nextn.predict_layers",
        &meta,
    );
    emit_kv_u64(
        walk,
        "meta.ctx",
        "Trained context length is {}",
        "qwen35.context_length",
        &meta,
    );
    walk.printer.ok("idx.read", "Reading tensor index");
    walk.printer
        .ok("idx.ok", &format!("Indexed {} tensors", meta.tensors.len()));
    walk.printer.ok("meta.done", "Metadata ready");
    if walk.hit("meta.done") {
        return Ok(());
    }

    tokenizer_and_map(walk, path, file_size, backend, &meta)
}

fn emit_kv_u64(walk: &Walk, id: &str, template: &str, key: &str, meta: &psionic_gguf::GgufMeta) {
    if let Some(n) = meta.kv_u64(key) {
        walk.printer.ok(id, &template.replace("{}", &n.to_string()));
    } else {
        walk.printer.skip(id, &template.replace("{}", "{n}"));
    }
}

fn tokenizer_and_map(
    walk: &Walk,
    path: &Path,
    file_size: u64,
    backend: InferenceBackend,
    meta: &psionic_gguf::GgufMeta,
) -> Result<(), InferenceExit> {
    walk.printer.ok("tok.read", "Reading tokenizer");
    match load_tokenizer(meta) {
        Ok(tok) => {
            walk.printer
                .ok("tok.model", &format!("Tokenizer model is {}", tok.model));
            walk.printer
                .ok("tok.tokens", &format!("Loaded {} tokens", tok.n_tokens));
            walk.printer
                .ok("tok.merges", &format!("Loaded {} BPE merges", tok.n_merges));
            walk.printer.ok(
                "tok.special",
                &format!("Special tokens bos={} eos={}", tok.bos, tok.eos),
            );
            walk.printer.ok("tok.done", "Tokenizer ready");
        }
        Err(model) if model != "tokens" && model != "merges" => {
            walk.printer.fail(
                "tok.fail.model",
                &format!("Unsupported tokenizer model {model}"),
            );
            return Err(InferenceExit::Failed);
        }
        Err(_) => {
            walk.printer
                .fail("tok.fail.merges", "Tokenizer merges missing");
            return Err(InferenceExit::Failed);
        }
    }
    if walk.hit("tok.done") {
        return Ok(());
    }

    if psionic_gguf::admit::names_need_translate(meta) {
        walk.printer.ok(
            "name.translate",
            "Translating Ollama tensor names to llama.cpp names",
        );
    } else {
        walk.printer.skip(
            "name.translate",
            "Translating Ollama tensor names to llama.cpp names",
        );
    }
    walk.printer
        .ok("name.check", "Checking required tensor names");

    let header_len = meta.data_offset as usize;
    let mut header = vec![0u8; header_len];
    {
        use std::io::Read;
        let mut f = std::fs::File::open(path).map_err(|e| {
            walk.printer
                .fail("map.fail.mmap", &format!("mmap failed: {e}"));
            InferenceExit::Failed
        })?;
        f.read_exact(&mut header).map_err(|_| {
            walk.printer
                .fail("idx.fail", "Tensor index is truncated or corrupt");
            InferenceExit::Failed
        })?;
    }

    match admit(meta, &header) {
        Ok(adm) => {
            walk.printer.ok(
                "admit.ok",
                &format!("Admission passed for {} digest {}", adm.family, adm.digest),
            );
        }
        Err(AdmitError::Family(arch)) => {
            walk.printer.fail(
                "admit.fail.family",
                &format!("Refusing to alias {arch} as a different family"),
            );
            return Err(InferenceExit::Failed);
        }
        Err(AdmitError::MissingTensor(name)) => {
            walk.printer.fail(
                "admit.fail.tensor",
                &format!("Missing required tensor {name}"),
            );
            return Err(InferenceExit::Failed);
        }
    }
    walk.printer.skip(
        "store.copy",
        &format!("Copying GGUF into store {}", store_path().display()),
    );
    walk.printer.skip(
        "store.manifest",
        &format!(
            "Wrote manifest {}",
            store_path().join("manifest.json").display()
        ),
    );
    walk.printer.ok("admit.done", "Model admitted");
    if walk.hit("admit.done") {
        return Ok(());
    }

    map_weights(walk, path, file_size, backend, meta)
}

fn map_weights(
    walk: &Walk,
    path: &Path,
    file_size: u64,
    backend: InferenceBackend,
    meta: &psionic_gguf::GgufMeta,
) -> Result<(), InferenceExit> {
    walk.printer.ok("backend.load", "Loading backends");
    walk.printer.ok("backend.cpu", "CPU backend ready");
    let metal_compiled = psionic_gguf::metal_wrap::metal_compiled();
    if metal_compiled {
        walk.printer.ok("backend.metal", "Metal backend ready");
    } else {
        walk.printer.skip("backend.metal", "Metal backend ready");
    }

    let want_metal = match backend {
        InferenceBackend::Cpu => false,
        InferenceBackend::Metal => true,
        InferenceBackend::Auto => metal_compiled,
    };
    if matches!(backend, InferenceBackend::Metal) && !metal_compiled {
        walk.printer
            .fail("backend.fail", "No usable backend (Metal is not compiled)");
        return Err(InferenceExit::Failed);
    }

    let picked = if want_metal { "metal" } else { "cpu" };
    walk.printer
        .ok("backend.pick", &format!("Selected backend is {picked}"));

    walk.printer.ok("map.mmap", "Mapping GGUF into memory");
    let mapped = match map_file(path, meta) {
        Ok(m) => m,
        Err(reason) => {
            walk.printer
                .fail("map.fail.mmap", &format!("mmap failed: {reason}"));
            return Err(InferenceExit::Failed);
        }
    };
    let map_size = mapped.file_size;
    walk.printer.ok(
        "map.mmap.ok",
        &format!("mmap size {}", format_size(map_size)),
    );
    walk.printer.ok("map.tensors", "Creating named tensors");
    let n_layers = meta.kv_u64("qwen35.block_count").unwrap_or(0);
    walk.printer.ok(
        "map.devices",
        &format!(
            "Assigning devices: embeddings on CPU, {n_layers} layers on {picked}, output on {picked}"
        ),
    );

    let mut metal_keep = None;
    if want_metal {
        walk.printer
            .ok("map.metal", "Wrapping mmap as Metal shared buffer");
        match psionic_gguf::metal_wrap::wrap_shared(&mapped.mmap) {
            Ok(buf) => metal_keep = Some(Arc::new(buf)),
            Err(reason) => {
                if matches!(backend, InferenceBackend::Metal) {
                    walk.printer.fail(
                        "map.fail.metal",
                        &format!("Metal shared buffer failed: {reason}"),
                    );
                    return Err(InferenceExit::Failed);
                }
                walk.printer
                    .skip("map.metal", "Wrapping mmap as Metal shared buffer");
            }
        }
    } else {
        walk.printer
            .skip("map.metal", "Wrapping mmap as Metal shared buffer");
    }

    walk.printer.ok("map.bind", "Binding tensor data pointers");
    walk.printer
        .ok("map.unmap", "Unmapping unused header and tail");
    let size_h = format_size(file_size);
    walk.printer.ok_extra(
        "map.done",
        &format!("Weights ready ({size_h} mapped)"),
        json!({"size": file_size}),
    );

    let metal_bytes = metal_keep.as_ref().map(|buf| buf.length).unwrap_or(0);
    let resident = mapped.resident_bytes();
    store_loaded(LoadedSession {
        metal: metal_keep,
        mapped,
        mmap_bytes: map_size,
        metal_bytes,
        resident_bytes: resident,
        backend: picked.to_string(),
        path: path.to_path_buf(),
        cache_kv: Vec::new(),
        cache_gdn: Vec::new(),
    });

    if !walk.hit("map.done") {
        emit_memory_lines(&walk.printer);
    }
    walk.mark("map");
    if walk.hit("map.done") {
        return Ok(());
    }

    continue_after_map(walk, meta)
}

fn continue_after_map(walk: &Walk, meta: &psionic_gguf::GgufMeta) -> Result<(), InferenceExit> {
    walk.printer.ok("ctx.alloc", "Allocating context");
    if walk.hit("ctx.alloc") {
        return Ok(());
    }

    let n_ctx = runtime_n_ctx(meta, walk.n_ctx);
    walk.printer
        .ok("ctx.length", &format!("Context length is {n_ctx}"));
    if walk.hit("ctx.length") {
        return Ok(());
    }

    let plan = plan_caches(meta, n_ctx);
    walk.printer.ok(
        "ctx.kv",
        &format!(
            "Allocating KV cache for {} full-attention layers",
            plan.n_full
        ),
    );
    let kv = alloc_cache(walk, plan.kv_bytes, n_ctx)?;
    if walk.hit("ctx.kv") {
        attach_caches(kv, Vec::new());
        return Ok(());
    }

    walk.printer.ok(
        "ctx.gdn",
        &format!(
            "Allocating recurrent state for {} Gated DeltaNet layers",
            plan.n_gdn
        ),
    );
    let gdn = alloc_cache(walk, plan.gdn_bytes, n_ctx)?;
    if walk.hit("ctx.gdn") {
        attach_caches(kv, gdn);
        return Ok(());
    }

    walk.printer.ok("ctx.sched", "Graph scheduler ready");
    if walk.hit("ctx.sched") {
        attach_caches(kv, gdn);
        return Ok(());
    }

    attach_caches(kv, gdn);
    walk.printer.ok("ctx.done", "Context ready");
    walk.mark("ctx");
    emit_memory_lines(&walk.printer);
    if walk.hit("ctx.done") {
        return Ok(());
    }

    continue_prompt(walk, meta)
}

fn alloc_cache(walk: &Walk, bytes: u64, n_ctx: u64) -> Result<Vec<u8>, InferenceExit> {
    if bytes > isize::MAX as u64 {
        walk.printer.fail(
            "ctx.fail.mem",
            &format!("Not enough memory for context {n_ctx}: cache size overflows"),
        );
        return Err(InferenceExit::Failed);
    }
    let mut buf = Vec::new();
    if buf.try_reserve_exact(bytes as usize).is_err() {
        walk.printer.fail(
            "ctx.fail.mem",
            &format!("Not enough memory for context {n_ctx}: reserve {bytes} bytes failed"),
        );
        return Err(InferenceExit::Failed);
    }
    const CHUNK: usize = 64 * 1024 * 1024;
    if bytes as usize > CHUNK {
        let total = bytes;
        let mut done = 0u64;
        while done < total {
            let add = (total - done).min(CHUNK as u64) as usize;
            buf.resize(buf.len() + add, 0);
            done = buf.len() as u64;
            if should_emit(done, total, CHUNK as u64) {
                if let Some(bar) = format_bar("KV", done, total, "B") {
                    walk.printer.ok_extra(
                        "ctx.kv",
                        &bar,
                        json!({
                            "pct": (done.min(total) * 100) / total,
                            "done": done,
                            "total": total,
                            "unit": "B",
                            "label": "KV",
                        }),
                    );
                }
            }
        }
    } else {
        buf.resize(bytes as usize, 0);
    }
    Ok(buf)
}

fn attach_caches(kv: Vec<u8>, gdn: Vec<u8>) {
    if let Ok(mut slot) = holder().lock() {
        if let Some(session) = slot.as_mut() {
            session.cache_kv = kv;
            session.cache_gdn = gdn;
        }
    }
}

fn continue_prompt(walk: &Walk, meta: &psionic_gguf::GgufMeta) -> Result<(), InferenceExit> {
    let Some(prompt) = walk.prompt.as_deref() else {
        walk.printer
            .fail("prompt.fail.empty", "No prompt given; pass --prompt");
        return Err(InferenceExit::Failed);
    };
    if prompt.is_empty() {
        walk.printer
            .fail("prompt.fail.empty", "No prompt given; pass --prompt");
        return Err(InferenceExit::Failed);
    }

    walk.printer.ok("prompt.template", "Applying chat template");
    if walk.hit("prompt.template") {
        return Ok(());
    }
    let template = meta
        .get("tokenizer.chat_template")
        .and_then(psionic_gguf::format::GgufValue::as_str);
    let rendered = render_chat(prompt, template);

    walk.printer.ok("prompt.tokenize", "Tokenizing prompt");
    if walk.hit("prompt.tokenize") {
        return Ok(());
    }
    let tok = match load_tokenizer(meta) {
        Ok(tok) => tok,
        Err(reason) => {
            walk.printer
                .fail("prompt.fail.tok", &format!("Tokenize failed: {reason}"));
            return Err(InferenceExit::Failed);
        }
    };
    let tokens = match tok.encode(&rendered) {
        Ok(ids) => ids,
        Err(reason) => {
            walk.printer
                .fail("prompt.fail.tok", &format!("Tokenize failed: {reason}"));
            return Err(InferenceExit::Failed);
        }
    };
    walk.printer.ok_extra(
        "prompt.done",
        &format!("Prompt is {} tokens", tokens.len()),
        json!({ "n": tokens.len(), "ids": tokens }),
    );
    if let Some(times) = &walk.times {
        if let Ok(mut t) = times.lock() {
            t.prompt_tokens = tokens.len() as u64;
        }
    }
    walk.mark("prompt");
    if walk.hit("prompt.done") {
        return Ok(());
    }

    continue_prefill_gen(walk, meta, &tok, &tokens)
}

fn continue_prefill_gen(
    walk: &Walk,
    meta: &psionic_gguf::GgufMeta,
    tok: &psionic_gguf::TokenizerTables,
    tokens: &[u32],
) -> Result<(), InferenceExit> {
    arm_cancel();
    walk.printer.ok("prefill.start", "Prefill starting");
    if walk.hit("prefill.start") {
        return Ok(());
    }
    let metal = current_metal();
    let _gemm = psionic_gguf::metal_gemm::bind(metal.as_deref());
    if metal.is_some() {
        walk.printer.ok("gemm.metal", "Q8 GEMM on Metal");
    }

    let n = tokens.len() as u64;
    let every = if n > 64 { 32 } else { 1 };
    let width = meta.kv_u64("qwen35.embedding_length").unwrap_or(8) as usize;
    let hybrid = with_mapped(psionic_gguf::qwen35::has_hybrid_graph).unwrap_or(false);
    if hybrid {
        walk.printer
            .ok("graph.hybrid", "Decoder graph is qwen35 hybrid");
    }
    if let Some(times) = &walk.times {
        if let Ok(mut t) = times.lock() {
            t.graph = if hybrid {
                String::from(psionic_gguf::qwen35::GRAPH_HYBRID)
            } else {
                String::from(psionic_gguf::qwen35::GRAPH_STUB)
            };
        }
    }
    let mut decode = if hybrid {
        with_mapped(|mapped| psionic_gguf::qwen35::new_state(meta, mapped))
    } else {
        None
    };
    let mut hidden = vec![0f32; width];
    for (i, token) in tokens.iter().copied().enumerate() {
        if cancelled() {
            walk.printer.ok("gen.stop.cancel", "Stop: cancelled");
            return Err(InferenceExit::Failed);
        }
        let pos = (i as u64) + 1;
        if should_emit(pos, n, every) {
            let short = format!("Prefill position {pos}/{n}");
            let message = if n > 32 {
                format_bar("Prefill", pos, n, "pos").unwrap_or(short)
            } else {
                short
            };
            walk.printer.ok_extra(
                "prefill.pos",
                &message,
                json!({
                    "pct": if n == 0 { 0 } else { (pos * 100) / n },
                    "done": pos,
                    "total": n,
                    "unit": "pos",
                    "label": "Prefill",
                }),
            );
        }
        if let Some(state) = decode.as_mut() {
            match with_mapped(|mapped| {
                psionic_gguf::qwen35::embed_and_forward(mapped, meta, token, state)
            }) {
                Some(Ok(next)) => hidden = next,
                Some(Err(reason)) => {
                    walk.printer
                        .fail("prefill.fail", &format!("Hybrid prefill failed: {reason}"));
                    return Err(InferenceExit::Failed);
                }
                None => {}
            }
        }
    }
    if decode.is_none() {
        hidden =
            with_mapped(|mapped| psionic_gguf::generate::prefill_hidden(mapped, tokens, width))
                .flatten()
                .unwrap_or(hidden);
    }

    walk.printer.ok("prefill.done", "Prefill complete");
    walk.mark("prefill");
    if walk.hit("prefill.done") {
        return Ok(());
    }

    let budget = walk.max_tokens.max(1);
    let mut generated = 0u32;
    let mut last_id = tokens.last().copied().unwrap_or(0);
    for step in 1..=budget {
        if cancelled() {
            walk.printer.ok("gen.stop.cancel", "Stop: cancelled");
            walk.printer
                .ok("gen.stats", &format!("Generated {generated} tokens"));
            return Err(InferenceExit::Failed);
        }
        let short = format!("Decode step {step}");
        let message = if budget > 32 {
            format_bar("Decode", step as u64, budget as u64, "tok").unwrap_or(short)
        } else {
            short
        };
        if should_emit(step as u64, budget as u64, if budget > 64 { 32 } else { 1 }) {
            walk.printer.ok_extra(
                "gen.step",
                &message,
                json!({
                    "pct": (step as u64 * 100) / budget as u64,
                    "done": step,
                    "total": budget,
                    "unit": "tok",
                    "label": "Decode",
                }),
            );
        }
        walk.printer.ok("gen.logits", "Computing logits");
        let sampled =
            with_mapped(|mapped| psionic_gguf::generate::greedy_from_hidden(mapped, &hidden, tok))
                .flatten();
        let Some((id, piece)) = sampled else {
            walk.printer.ok(
                "gen.stop.length",
                &format!("Stop: token budget {generated}"),
            );
            walk.printer
                .ok("gen.stats", &format!("Generated {generated} tokens"));
            walk.printer.ok("gen.done", "Inference complete");
            return Ok(());
        };
        walk.printer
            .ok("gen.sample", &format!("Sampling token {id} ({piece})"));
        walk.printer.ok("gen.stream", &format!("Streaming {piece}"));
        if !walk.printer.suppress_stderr && !walk.bench {
            print!("{piece}");
            let _ = std::io::stdout().flush();
        }
        generated += 1;
        last_id = id;
        if let Some(state) = decode.as_mut() {
            match with_mapped(|mapped| {
                psionic_gguf::qwen35::embed_and_forward(mapped, meta, id, state)
            }) {
                Some(Ok(next)) => hidden = next,
                Some(Err(reason)) => {
                    walk.printer
                        .fail("gen.fail", &format!("Hybrid decode failed: {reason}"));
                    return Err(InferenceExit::Failed);
                }
                None => {}
            }
        } else if let Some(next) =
            with_mapped(|mapped| psionic_gguf::generate::embed_token(mapped, id, width)).flatten()
        {
            hidden = next;
        }
        if id == tok.eos as u32 || id == 248046 || id == 248044 {
            walk.printer.ok("gen.stop.eos", "Stop: end of sequence");
            break;
        }
        if step == budget {
            walk.printer
                .ok("gen.stop.length", &format!("Stop: token budget {budget}"));
        }
        let _ = last_id;
    }
    if !walk.printer.suppress_stderr && !walk.bench {
        println!();
    }
    if let Some(times) = &walk.times {
        if let Ok(mut t) = times.lock() {
            t.generated = generated;
        }
    }
    walk.mark("gen");
    walk.printer
        .ok("gen.stats", &format!("Generated {generated} tokens"));
    walk.printer.ok("gen.done", "Inference complete");
    Ok(())
}

fn emit_bench_summary(walk: &Walk) {
    let times = walk.times.as_ref().and_then(|slot| slot.lock().ok());
    let Some(t) = times else {
        return;
    };
    let tok_per_s = if t.gen_ms == 0 {
        0.0
    } else {
        (t.generated as f64) * 1000.0 / (t.gen_ms as f64)
    };
    let mut doc = json!({
        "engine": "openagents",
        "version": env!("CARGO_PKG_VERSION"),
        "graph": t.graph,
        "map_ms": t.map_ms,
        "ctx_ms": t.ctx_ms,
        "prompt_ms": t.prompt_ms,
        "prefill_ms": t.prefill_ms,
        "gen_ms": t.gen_ms,
        "prompt_tokens": t.prompt_tokens,
        "generated": t.generated,
        "tok_per_s": tok_per_s,
    });
    if let Some(tag) = &walk.compare_ollama {
        match compare_ollama(tag, walk.prompt.as_deref().unwrap_or(""), walk.max_tokens) {
            Ok((ms, tokens)) => {
                let ollama_tps = if ms == 0 {
                    0.0
                } else {
                    (tokens as f64) * 1000.0 / (ms as f64)
                };
                doc["ollama_tag"] = json!(tag);
                doc["ollama_ms"] = json!(ms);
                doc["ollama_generated"] = json!(tokens);
                doc["ollama_tok_per_s"] = json!(ollama_tps);
            }
            Err(reason) => {
                walk.printer
                    .skip("bench.ollama", &format!("Ollama compare skipped: {reason}"));
                doc["ollama_tag"] = json!(tag);
                doc["ollama_skipped"] = json!(true);
            }
        }
    }
    walk.printer.ok("bench.done", "Benchmark complete");
    println!("{doc}");
}

fn compare_ollama(tag: &str, prompt: &str, max_tokens: u32) -> Result<(u64, u32), String> {
    let body = json!({
        "model": tag,
        "prompt": prompt,
        "stream": false,
        "options": {"num_predict": max_tokens, "temperature": 0}
    });
    let started = std::time::Instant::now();
    let value = block_on_detached(async move {
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?
            .post("http://127.0.0.1:11434/api/generate")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        response.json::<Value>().await.map_err(|e| e.to_string())
    })?;
    let ms = started.elapsed().as_millis() as u64;
    let text = value.get("response").and_then(Value::as_str).unwrap_or("");
    let tokens = value
        .get("eval_count")
        .and_then(Value::as_u64)
        .unwrap_or(text.split_whitespace().count() as u64) as u32;
    Ok((ms, tokens))
}

fn block_on_detached<T, F>(fut: F) -> Result<T, String>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, String>> + Send + 'static,
{
    std::thread::Builder::new()
        .name("oa-ollama-compare".into())
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?
                .block_on(fut)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| String::from("ollama compare thread panicked"))?
}

static CANCELLED: AtomicBool = AtomicBool::new(false);

fn cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

fn arm_cancel() {
    CANCELLED.store(false, Ordering::SeqCst);
    #[cfg(unix)]
    {
        unsafe {
            libc::signal(libc::SIGINT, on_sigint as *const () as libc::sighandler_t);
        }
    }
}

#[cfg(unix)]
extern "C" fn on_sigint(_: libc::c_int) {
    CANCELLED.store(true, Ordering::SeqCst);
}

fn current_metal() -> Option<Arc<MetalShared>> {
    holder()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().and_then(|session| session.metal.clone()))
}

fn with_mapped<R>(f: impl FnOnce(&MappedWeights) -> R) -> Option<R> {
    let guard = holder().lock().ok()?;
    let session = guard.as_ref()?;
    Some(f(&session.mapped))
}

struct LoadedSession {
    metal: Option<Arc<MetalShared>>,
    /// Held until unload so the mapping stays resident. Dropped after Metal.
    mapped: MappedWeights,
    mmap_bytes: u64,
    metal_bytes: u64,
    resident_bytes: u64,
    backend: String,
    path: PathBuf,
    cache_kv: Vec<u8>,
    cache_gdn: Vec<u8>,
}

impl Drop for LoadedSession {
    fn drop(&mut self) {
        self.metal.take();
        let _released = self.mapped.file_size;
    }
}

fn holder() -> &'static Mutex<Option<LoadedSession>> {
    static HOLDER: OnceLock<Mutex<Option<LoadedSession>>> = OnceLock::new();
    HOLDER.get_or_init(|| Mutex::new(None))
}

fn store_loaded(session: LoadedSession) {
    if let Ok(mut slot) = holder().lock() {
        *slot = Some(session);
    }
}

fn take_loaded() -> Option<LoadedSession> {
    holder().lock().ok().and_then(|mut slot| slot.take())
}

pub fn loaded() -> bool {
    holder().lock().ok().is_some_and(|slot| slot.is_some())
}

fn rss_bytes() -> u64 {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut system = System::new();
    let pid = Pid::from_u32(std::process::id());
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system
        .process(pid)
        .map(|process| process.memory())
        .unwrap_or(0)
}

pub fn status_document() -> Value {
    let rss = rss_bytes();
    let guard = holder().lock().ok();
    let session = guard.as_ref().and_then(|slot| slot.as_ref());
    match session {
        Some(session) => json!({
            "loaded": true,
            "path": session.path.display().to_string(),
            "backend": session.backend,
            "mmap_bytes": session.mmap_bytes,
            "metal_bytes": session.metal_bytes,
            "rss_bytes": rss,
            "cache_kv_bytes": session.cache_kv.len() as u64,
            "cache_gdn_bytes": session.cache_gdn.len() as u64,
            "mmap_resident_bytes": session.resident_bytes,
        }),
        None => json!({
            "loaded": false,
            "mmap_bytes": 0,
            "metal_bytes": 0,
            "rss_bytes": rss,
            "cache_kv_bytes": 0,
            "cache_gdn_bytes": 0,
        }),
    }
}

fn print_status(json: bool) {
    let doc = status_document();
    if json {
        println!("{doc}");
        return;
    }
    if doc["loaded"].as_bool() == Some(true) {
        println!(
            "Loaded {} ({})",
            doc["path"].as_str().unwrap_or(""),
            doc["backend"].as_str().unwrap_or("")
        );
        println!(
            "mmap resident {} / mapped {}",
            format_size(doc["mmap_resident_bytes"].as_u64().unwrap_or(0)),
            format_size(doc["mmap_bytes"].as_u64().unwrap_or(0))
        );
        let metal = doc["metal_bytes"].as_u64().unwrap_or(0);
        if metal > 0 {
            println!("Metal buffer {}", format_size(metal));
        }
        println!(
            "Process RSS {}",
            format_size(doc["rss_bytes"].as_u64().unwrap_or(0))
        );
        println!(
            "Caches KV {} GDN {}",
            format_size(doc["cache_kv_bytes"].as_u64().unwrap_or(0)),
            format_size(doc["cache_gdn_bytes"].as_u64().unwrap_or(0))
        );
    } else {
        println!("Not loaded.");
        println!(
            "Process RSS {}",
            format_size(doc["rss_bytes"].as_u64().unwrap_or(0))
        );
    }
}

fn emit_memory_lines(printer: &Printer) {
    let doc = status_document();
    let mapped = doc["mmap_bytes"].as_u64().unwrap_or(0);
    let resident = doc["mmap_resident_bytes"].as_u64().unwrap_or(0);
    printer.ok(
        "mem.mmap",
        &format!(
            "mmap resident {} / mapped {}",
            format_size(resident),
            format_size(mapped)
        ),
    );
    let metal = doc["metal_bytes"].as_u64().unwrap_or(0);
    if metal > 0 {
        printer.ok("mem.metal", &format!("Metal buffer {}", format_size(metal)));
    } else {
        printer.skip("mem.metal", "Metal buffer {size}");
    }
    printer.ok(
        "mem.rss",
        &format!(
            "Process RSS {}",
            format_size(doc["rss_bytes"].as_u64().unwrap_or(0))
        ),
    );
    let kv = doc["cache_kv_bytes"].as_u64().unwrap_or(0);
    let gdn = doc["cache_gdn_bytes"].as_u64().unwrap_or(0);
    if kv > 0 || gdn > 0 {
        printer.ok(
            "mem.caches",
            &format!("Caches KV {} GDN {}", format_size(kv), format_size(gdn)),
        );
    } else {
        printer.skip("mem.caches", "Caches KV {kv} GDN {gdn}");
    }
}

pub fn memory_status_line() -> Option<String> {
    let doc = status_document();
    if doc["loaded"].as_bool() != Some(true) {
        return None;
    }
    let mapped = format_size(doc["mmap_bytes"].as_u64().unwrap_or(0));
    let resident = format_size(doc["mmap_resident_bytes"].as_u64().unwrap_or(0));
    let rss = format_size(doc["rss_bytes"].as_u64().unwrap_or(0));
    let metal = doc["metal_bytes"].as_u64().unwrap_or(0);
    if metal > 0 {
        Some(format!(
            "mmap {resident} / {mapped} · Metal {} · RSS {rss}",
            format_size(metal)
        ))
    } else {
        Some(format!("mmap {resident} / {mapped} · RSS {rss}"))
    }
}

fn unload(json: bool, suppress_stderr: bool) -> Result<(), InferenceExit> {
    let printer = Printer {
        json,
        teach: !json,
        quiet: json,
        suppress_stderr,
        on_step: None,
    };
    printer.ok("unload.start", "Unloading weights");
    let Some(session) = take_loaded() else {
        printer.skip("unload.mmap", "Unmapping GGUF");
        printer.skip("unload.metal", "Releasing Metal buffer");
        printer.ok("unload.done", "Weights unloaded");
        return Ok(());
    };
    printer.ok("unload.mmap", "Unmapping GGUF");
    if session.metal.is_some() {
        printer.ok("unload.metal", "Releasing Metal buffer");
    } else {
        printer.skip("unload.metal", "Releasing Metal buffer");
    }
    drop(session);
    printer.ok("unload.done", "Weights unloaded");
    Ok(())
}

/// Load a GGUF in this process through `ctx.done` (Coder `/load`).
pub fn load_gguf(path: PathBuf, json: bool) -> Result<(), InferenceExit> {
    load_gguf_with_steps(path, json, false, None)
}

/// Load with optional step callbacks and no teach dump to stderr.
pub fn load_gguf_with_steps(
    path: PathBuf,
    json: bool,
    suppress_stderr: bool,
    on_step: Option<std::sync::Arc<dyn Fn(&str, &str, &str) + Send + Sync>>,
) -> Result<(), InferenceExit> {
    walk_run(WalkOpts {
        gguf: Some(path),
        backend: InferenceBackend::Auto,
        teach: false,
        quiet: true,
        preview: false,
        until: Some("ctx.done".into()),
        json,
        suppress_stderr,
        on_step,
        prompt: None,
        max_tokens: None,
        n_ctx: None,
        bench: false,
        compare_ollama: None,
    })
}

/// Release in-process weights (Coder `/unload`).
pub fn unload_gguf(json: bool) -> Result<(), InferenceExit> {
    unload(json, true)
}

fn run_doctor(json: bool) {
    let metal = psionic_gguf::metal_wrap::metal_compiled();
    let store = store_path();
    if json {
        println!(
            "{}",
            json!({
                "backends": if metal { vec!["cpu", "metal"] } else { vec!["cpu"] },
                "store": store.display().to_string(),
                "provenance": psionic_gguf::PROVENANCE_PIN,
            })
        );
    } else {
        println!("CPU backend: yes");
        println!("Metal backend: {}", if metal { "yes" } else { "no" });
        println!("Store: {}", store.display());
        println!("Provenance: {}", psionic_gguf::PROVENANCE_PIN);
    }
}

pub fn inspect(path: &Path, json: bool) -> Result<(), InferenceExit> {
    let meta = parse_path(path).map_err(|e| InferenceExit::Usage(e.to_string()))?;
    if json {
        let tensors: Vec<Value> = meta
            .tensors
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "dims": t.dims,
                    "type": ggml_type_name(t.ggml_type),
                })
            })
            .collect();
        println!(
            "{}",
            json!({
                "architecture": meta.architecture(),
                "n_tensors": meta.n_tensors,
                "tensors": tensors,
            })
        );
    } else {
        println!(
            "architecture {}  tensors {}",
            meta.architecture().unwrap_or("?"),
            meta.n_tensors
        );
        for t in &meta.tensors {
            println!("  {} {:?} {}", t.name, t.dims, ggml_type_name(t.ggml_type));
        }
    }
    Ok(())
}

pub fn admit_path(path: &Path, json: bool) -> Result<(), InferenceExit> {
    let meta = parse_path(path).map_err(|e| InferenceExit::Usage(e.to_string()))?;
    let header_len = meta.data_offset as usize;
    let mut header = vec![0u8; header_len];
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| InferenceExit::Usage(e.to_string()))?;
    f.read_exact(&mut header)
        .map_err(|e| InferenceExit::Usage(e.to_string()))?;
    match admit(&meta, &header) {
        Ok(adm) => {
            if json {
                println!(
                    "{}",
                    json!({"family": adm.family, "digest": adm.digest, "ok": true})
                );
            } else {
                println!("admitted {} digest {}", adm.family, adm.digest);
            }
            Ok(())
        }
        Err(err) => Err(InferenceExit::Usage(err.to_string())),
    }
}

#[cfg(test)]
pub(crate) fn serialize_load_tests() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_then_status_then_unload_on_fixture() {
        let _guard = serialize_load_tests();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qwen35.gguf");
        psionic_gguf::write_qwen35_fixture(&path).unwrap();
        load_gguf(path, true).unwrap();
        assert!(loaded());
        let doc = status_document();
        assert_eq!(doc["loaded"], true);
        assert!(doc["mmap_bytes"].as_u64().unwrap() > 0);
        assert!(doc.get("rss_bytes").is_some());
        assert!(
            doc["cache_kv_bytes"].as_u64().unwrap() > 0,
            "ctx attach: {doc}"
        );
        assert!(
            doc["cache_gdn_bytes"].as_u64().unwrap() > 0,
            "gdn attach: {doc}"
        );
        unload_gguf(true).unwrap();
        assert!(!loaded());
        let after = status_document();
        assert_eq!(after["loaded"], false);
        assert_eq!(after["mmap_bytes"], 0);
        assert_eq!(after["metal_bytes"], 0);
    }
}
