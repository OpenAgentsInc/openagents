//! `openagents inference` — product load path and teach statuses.

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use psionic_gguf::format::ParseError;
use psionic_gguf::{
    AdmitError, admit, format_size, ggml_type_name, load_tokenizer, map_file, parse_path,
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
}

pub enum InferenceExit {
    Failed,
    Usage(String),
}

pub fn run(args: InferenceArgs, json: bool) -> Result<(), InferenceExit> {
    match args.action {
        InferenceAction::Run {
            gguf,
            model: _,
            prompt: _,
            max_tokens: _,
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
            })
        }
        InferenceAction::Doctor => {
            run_doctor(json);
            Ok(())
        }
        InferenceAction::Models | InferenceAction::Status => {
            if json {
                println!("{}", json!({"loaded": false, "models": []}));
            } else {
                println!("No admitted models.");
            }
            Ok(())
        }
        InferenceAction::Add { .. }
        | InferenceAction::Remove { .. }
        | InferenceAction::Serve
        | InferenceAction::Stop
        | InferenceAction::Unload => Err(InferenceExit::Usage(
            "this command is not built yet; use inference run --gguf through map.done".into(),
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
}

struct Printer {
    json: bool,
    teach: bool,
    quiet: bool,
}

impl Printer {
    fn emit(&self, id: &str, message: &str, state: &str, extra: Value) {
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

struct Walk {
    printer: Printer,
    until: Option<String>,
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

    open_and_map(&walk, path, file_size, opts.backend)
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

    let mut _metal_keep = None;
    if want_metal {
        walk.printer
            .ok("map.metal", "Wrapping mmap as Metal shared buffer");
        match psionic_gguf::metal_wrap::wrap_shared(&mapped.mmap) {
            Ok(buf) => _metal_keep = Some(buf),
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
    if walk.hit("map.done") {
        drop(_metal_keep);
        drop(mapped);
        return Ok(());
    }

    walk.printer.emit(
        "build.stop",
        "Stopping: next step is not built yet",
        "ok",
        json!({}),
    );
    if !walk.printer.json {
        eprintln!("             Last completed step: map.done");
        eprintln!("             Next to build: ctx.alloc");
    }
    drop(_metal_keep);
    drop(mapped);
    Ok(())
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
