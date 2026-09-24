//! `kev-serve`: load one or more kev variants and serve
//! `POST /v1/systemone`.
//!
//! ```text
//! kev-serve --adapter-dir ~/work/kev-artifacts/kev-0.5b \
//!           --base-dir   ~/work/kev-artifacts/qwen2.5-0.5b \
//!           --port 8009
//!
//! kev-serve --bundle-dir ~/work/kev-artifacts --port 8009
//! ```
//!
//! Bundle mode scans the directory for every `kev-*` variant that carries
//! `head.safetensors` + `head_meta.json`, resolves its base checkpoint by
//! convention, and routes on each request's `model` field.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use candle_core::{DType, Device};
use kev::decision::DecisionModel;
use kev::lora::LoraConfig;
use kev::serve::{Admission, MIB, ServeState, Variant, host_memory_budget, router};

struct Args {
    adapter_dir: Option<PathBuf>,
    base_dir: Option<PathBuf>,
    bundle_dir: Option<PathBuf>,
    host: String,
    port: u16,
    default: String,
    device: String,
    dtype: String,
    attention: String,
    bucket_size: usize,
    /// Packed tokens one forward may hold; `None` keeps the default bound.
    max_tokens: Option<usize>,
    /// Working memory forwards may hold together, in MiB; `None` measures
    /// the host after the weights load.
    memory_budget_mib: Option<usize>,
}

fn parse_args() -> Result<Args, String> {
    let mut adapter_dir = std::env::var("KEV_ARTIFACT_DIR").ok();
    let mut base_dir = std::env::var("KEV_BASE_DIR").ok();
    let mut bundle_dir = std::env::var("KEV_BUNDLE_DIR").ok();
    let mut host = "127.0.0.1".to_string();
    let mut port = 8009u16;
    let mut default = "kev-latest".to_string();
    let mut device = "cpu".to_string();
    let mut dtype = std::env::var("KEV_DTYPE").unwrap_or_else(|_| "fp32".to_string());
    let mut attention = "eager".to_string();
    let mut bucket_size = 0;
    let mut max_tokens = None;
    let mut memory_budget_mib = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut take = |name: &str| {
            args.next()
                .ok_or_else(|| format!("{name} needs a value after it"))
        };
        match arg.as_str() {
            "--adapter-dir" => adapter_dir = Some(take("--adapter-dir")?),
            "--base-dir" => base_dir = Some(take("--base-dir")?),
            "--bundle-dir" => bundle_dir = Some(take("--bundle-dir")?),
            "--host" => host = take("--host")?,
            "--port" => {
                port = take("--port")?
                    .parse()
                    .map_err(|_| "--port must be a whole number".to_string())?
            }
            "--default" => default = take("--default")?,
            "--device" => device = take("--device")?,
            "--dtype" => dtype = take("--dtype")?,
            "--attention" => attention = take("--attention")?,
            "--bucket-size" => {
                bucket_size = take("--bucket-size")?
                    .parse()
                    .map_err(|_| "--bucket-size must be 0 or 64".to_string())?
            }
            "--max-tokens" => {
                max_tokens = Some(
                    take("--max-tokens")?
                        .parse()
                        .map_err(|_| "--max-tokens must be a whole number".to_string())?,
                )
            }
            "--memory-budget-mib" => {
                memory_budget_mib = Some(
                    take("--memory-budget-mib")?
                        .parse()
                        .map_err(|_| "--memory-budget-mib must be a whole number".to_string())?,
                )
            }
            "--help" | "-h" => {
                eprintln!(
                    "Serve kev decision models on POST /v1/systemone.\n\
                     \n\
                     Usage:\n  \
                     kev-serve --adapter-dir DIR --base-dir DIR [options]\n  \
                     kev-serve --bundle-dir DIR [options]\n\
                     \n\
                     Options:\n  \
                     --adapter-dir DIR        One variant's adapter and head files\n  \
                     --base-dir DIR           The base checkpoint that variant builds on\n  \
                     --bundle-dir DIR         Load every kev-* variant in DIR\n  \
                     --default ID             The variant a request gets when it names none (default: kev-latest)\n  \
                     --host H                 Address to listen on (default: 127.0.0.1)\n  \
                     --port P                 Port to listen on (default: 8009)\n  \
                     --device cpu|metal       Where inference runs (default: cpu)\n  \
                     --dtype fp32|bf16        Weight precision (default: fp32)\n  \
                     --attention eager|sdpa   Attention implementation; sdpa needs --device metal\n  \
                     --bucket-size 0|64       Pad each request to a multiple of this many tokens (0: no padding)\n  \
                     --max-tokens N           Most tokens one request may pack into\n  \
                     --memory-budget-mib N    Working memory for inference, in MiB (default: measured from the host)"
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "unknown argument `{other}`; run with --help to see the options"
                ));
            }
        }
    }
    if !matches!(attention.as_str(), "eager" | "sdpa") {
        return Err("--attention must be eager or sdpa".to_string());
    }
    if !matches!(bucket_size, 0 | 64) {
        return Err("--bucket-size must be 0 or 64".to_string());
    }
    if attention == "sdpa" && device != "metal" {
        return Err("--attention sdpa requires --device metal".to_string());
    }
    if bundle_dir.is_none() && adapter_dir.is_none() {
        return Err("pass --adapter-dir with --base-dir, or --bundle-dir".to_string());
    }
    if bundle_dir.is_none() && base_dir.is_none() {
        return Err("--adapter-dir also needs --base-dir".to_string());
    }
    Ok(Args {
        adapter_dir: adapter_dir.map(PathBuf::from),
        base_dir: base_dir.map(PathBuf::from),
        bundle_dir: bundle_dir.map(PathBuf::from),
        host,
        port,
        default,
        device,
        dtype,
        attention,
        bucket_size,
        max_tokens,
        memory_budget_mib,
    })
}

fn device(name: &str) -> Result<Device, String> {
    match name {
        "cpu" => Ok(Device::Cpu),
        "metal" => {
            Device::new_metal(0).map_err(|e| format!("the Metal device is unavailable: {e}"))
        }
        other => Err(format!("unknown device `{other}`; use cpu or metal")),
    }
}

fn dtype(name: &str) -> Result<DType, String> {
    match name {
        "fp32" | "f32" => Ok(DType::F32),
        "bf16" => Ok(DType::BF16),
        other => Err(format!("unknown dtype `{other}`; use fp32 or bf16")),
    }
}

/// `Qwen/Qwen3-0.6B-Base` -> `qwen3-0.6b`.
fn base_dir_name(hf_id: &str) -> String {
    let name = hf_id.rsplit('/').next().unwrap_or(hf_id).to_lowercase();
    name.strip_suffix("-base").unwrap_or(&name).to_string()
}

fn read_head_meta(adapter_dir: &Path) -> serde_json::Value {
    std::fs::read_to_string(adapter_dir.join("head_meta.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn lora_rank(adapter_dir: &Path) -> usize {
    std::fs::read_to_string(adapter_dir.join("adapter_config.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<LoraConfig>(&text).ok())
        .map(|c| c.r)
        .unwrap_or(0)
}

/// Load one variant from `adapter_dir` + `base_dir` under `id`.
fn load_variant(
    id: &str,
    adapter_dir: &Path,
    base_dir: &Path,
    device: &Device,
    dtype: DType,
) -> Result<Variant, String> {
    let meta = read_head_meta(adapter_dir);
    let base_id = meta["base"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| base_dir.display().to_string());
    eprintln!(
        "kev-serve: loading {id}: {} + {}",
        base_dir.display(),
        adapter_dir.display()
    );
    let model = DecisionModel::load_with_dtype(base_dir, adapter_dir, device.clone(), dtype)
        .map_err(|e| format!("{id}: {e}"))?;
    let base_revision = meta["base_revision"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_default();
    Ok(Variant {
        model,
        model_id: id.to_string(),
        run: adapter_dir.display().to_string(),
        base: base_id,
        base_revision,
        lora: lora_rank(adapter_dir),
    })
}

/// Scan `bundle` for `<variant>/head.safetensors` and load each, resolving
/// its base as `<bundle>/<base dir name>`.
fn load_bundle(bundle: &Path, device: &Device, dtype: DType) -> Result<Vec<Variant>, String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(bundle)
        .map_err(|e| format!("cannot read the bundle directory {}: {e}", bundle.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && p.join("head.safetensors").exists())
        .collect();
    entries.sort();
    if entries.is_empty() {
        return Err(format!(
            "found no variants in {}; each variant is a subdirectory that holds head.safetensors",
            bundle.display()
        ));
    }
    let mut variants = Vec::new();
    for adapter_dir in entries {
        let id = adapter_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("kev")
            .to_string();
        let meta = read_head_meta(&adapter_dir);
        let base = meta["base"]
            .as_str()
            .map(base_dir_name)
            .map(|name| bundle.join(&name))
            .ok_or_else(|| format!("{id}: head_meta.json has no `base` field, so the base checkpoint cannot be found"))?;
        variants.push(load_variant(&id, &adapter_dir, &base, device, dtype)?);
    }
    Ok(variants)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let device = match device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let dtype = match dtype(&args.dtype) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let mut variants = match &args.bundle_dir {
        Some(bundle) => match load_bundle(bundle, &device, dtype) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("kev-serve: {e}");
                return ExitCode::from(1);
            }
        },
        None => {
            let adapter = args.adapter_dir.expect("checked");
            let base = args.base_dir.expect("checked");
            let id = args.default.clone();
            match load_variant(&id, &adapter, &base, &device, dtype) {
                Ok(v) => vec![v],
                Err(e) => {
                    eprintln!("kev-serve: {e}");
                    return ExitCode::from(1);
                }
            }
        }
    };
    let attention = match args.attention.as_str() {
        "eager" => kev::model::AttentionBackend::Eager,
        "sdpa" => kev::model::AttentionBackend::MetalSdpa,
        _ => {
            eprintln!("kev-serve: --attention must be eager or sdpa");
            return ExitCode::from(2);
        }
    };
    for variant in &mut variants {
        if let Err(error) = variant
            .model
            .backbone
            .set_attention(attention)
            .and_then(|()| variant.model.set_bucket_size(args.bucket_size))
        {
            eprintln!("kev-serve: {error}");
            return ExitCode::from(2);
        }
    }
    let default = variants
        .iter()
        .position(|v| v.model_id == args.default)
        .or_else(|| {
            // `kev-latest` picks the largest loaded variant by convention.
            variants
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    let size = |v: &Variant| {
                        v.model_id
                            .trim_start_matches("kev-")
                            .trim_end_matches('b')
                            .parse::<f64>()
                            .unwrap_or(0.0)
                    };
                    size(a)
                        .partial_cmp(&size(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
        })
        .unwrap_or(0);
    eprintln!(
        "kev-serve: loaded {} variants; the default is {}",
        variants.len(),
        variants[default].model_id
    );
    let memory_budget_bytes = match args.memory_budget_mib {
        Some(mib) => mib.saturating_mul(MIB),
        None => match host_memory_budget() {
            Some(bytes) => bytes,
            None => {
                eprintln!(
                    "kev-serve: this host does not report its available memory; set a budget with --memory-budget-mib"
                );
                return ExitCode::from(2);
            }
        },
    };
    let mut admission = Admission {
        memory_budget_bytes,
        ..Admission::default()
    };
    if let Some(max_tokens) = args.max_tokens {
        admission.max_total_tokens = max_tokens;
        admission.max_attention_bytes = Admission::attention_bytes(max_tokens);
    }
    let state = match ServeState::new(
        variants,
        default,
        vec!["jev-latest".to_string()],
        args.device,
        admission,
    ) {
        Ok(state) => Arc::new(state),
        Err(e) => {
            eprintln!("kev-serve: {e}");
            return ExitCode::from(2);
        }
    };
    eprintln!(
        "kev-serve: memory budget is {} MiB; at most {} inference passes run at once across all variants",
        state.memory_mib, state.admission.concurrency
    );
    for (variant, share) in state.variants.iter().zip(&state.variant_slots) {
        eprintln!(
            "kev-serve: {}: one inference pass at {} tokens needs {} MiB; at most {} run at once",
            variant.model_id, state.admission.max_total_tokens, share.forward_mib, share.limit
        );
    }
    let addr: SocketAddr = match format!("{}:{}", args.host, args.port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("kev-serve: --host and --port do not form a valid listen address: {e}");
            return ExitCode::from(2);
        }
    };
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("kev-serve: cannot listen on {addr}: {e}");
            return ExitCode::from(1);
        }
    };
    eprintln!("kev-serve: listening on http://{addr}");
    match axum::serve(listener, router(state)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            ExitCode::from(1)
        }
    }
}
