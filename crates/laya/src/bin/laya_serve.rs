//! `laya-serve`: load one or more laya checkpoints and serve
//! `POST /v1/systemone`.
//!
//! ```text
//! laya-serve --model-dir ~/work/laya-artifacts/english --port 8010
//!
//! laya-serve --bundle-dir ~/work/laya-artifacts --port 8010
//! ```
//!
//! Bundle mode scans the directory for every checkpoint that carries
//! `rl_agent_config.json` + `model.safetensors`, and routes on each
//! request's `model` field.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use candle_core::Device;
use laya::decision::DecisionModel;
use laya::serve::{Admission, MIB, ServeState, Variant, host_memory_budget, router};

struct Args {
    model_dir: Option<PathBuf>,
    bundle_dir: Option<PathBuf>,
    host: String,
    port: u16,
    default: String,
    device: String,
    /// Forwards at once across every variant; `None` keeps the default.
    concurrency: Option<usize>,
    /// Questions one request may carry; `None` keeps the default.
    max_questions: Option<usize>,
    /// Working memory forwards may hold together, in MiB; `None`
    /// measures the host after the weights load.
    memory_budget_mib: Option<usize>,
    /// An alias the default variant also answers to; repeatable.
    aliases: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut model_dir = std::env::var("LAYA_MODEL_DIR").ok();
    let mut bundle_dir = std::env::var("LAYA_BUNDLE_DIR").ok();
    let mut host = "127.0.0.1".to_string();
    let mut port = 8010u16;
    let mut default = String::new();
    let mut device = "cpu".to_string();
    let mut concurrency = None;
    let mut max_questions = None;
    let mut memory_budget_mib = None;
    let mut aliases = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut take = |name: &str| {
            args.next()
                .ok_or_else(|| format!("{name} needs a value after it"))
        };
        match arg.as_str() {
            "--model-dir" => model_dir = Some(take("--model-dir")?),
            "--bundle-dir" => bundle_dir = Some(take("--bundle-dir")?),
            "--host" => host = take("--host")?,
            "--port" => {
                port = take("--port")?
                    .parse()
                    .map_err(|_| "--port must be a whole number".to_string())?
            }
            "--default" => default = take("--default")?,
            "--device" => device = take("--device")?,
            "--concurrency" => {
                concurrency = Some(
                    take("--concurrency")?
                        .parse()
                        .map_err(|_| "--concurrency must be a whole number".to_string())?,
                )
            }
            "--max-questions" => {
                max_questions = Some(
                    take("--max-questions")?
                        .parse()
                        .map_err(|_| "--max-questions must be a whole number".to_string())?,
                )
            }
            "--memory-budget-mib" => {
                memory_budget_mib = Some(
                    take("--memory-budget-mib")?
                        .parse()
                        .map_err(|_| "--memory-budget-mib must be a whole number".to_string())?,
                )
            }
            "--alias" => aliases.push(take("--alias")?),
            "--help" | "-h" => {
                eprintln!(
                    "Serve laya decision models on POST /v1/systemone.\n\
                     \n\
                     Usage:\n  \
                     laya-serve --model-dir DIR [options]\n  \
                     laya-serve --bundle-dir DIR [options]\n\
                     \n\
                     Options:\n  \
                     --model-dir DIR          One checkpoint directory\n  \
                     --bundle-dir DIR         Load every checkpoint in DIR\n  \
                     --default ID             The checkpoint a request gets when it names none\n  \
                     --alias NAME             Another name for the default checkpoint; repeatable\n  \
                     --host H                 Address to listen on (default: 127.0.0.1)\n  \
                     --port P                 Port to listen on (default: 8010)\n  \
                     --device cpu|metal       Where inference runs (default: cpu)\n  \
                     --concurrency N          Most inference passes at once across all checkpoints\n  \
                     --max-questions N        Most questions one request may carry\n  \
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
    if bundle_dir.is_none() && model_dir.is_none() {
        return Err("pass --model-dir or --bundle-dir".to_string());
    }
    Ok(Args {
        model_dir: model_dir.map(PathBuf::from),
        bundle_dir: bundle_dir.map(PathBuf::from),
        host,
        port,
        default,
        device,
        concurrency,
        max_questions,
        memory_budget_mib,
        aliases,
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

/// Load one checkpoint directory under `id`.
fn load_variant(id: &str, dir: &Path, device: &Device) -> Result<Variant, String> {
    eprintln!("laya-serve: loading {id}: {}", dir.display());
    let model = DecisionModel::load(dir, device.clone()).map_err(|e| format!("{id}: {e}"))?;
    let base = model.encoder_id.clone();
    Ok(Variant {
        model,
        model_id: id.to_string(),
        run: dir.display().to_string(),
        base,
    })
}

/// Scan `bundle` for `<variant>/rl_agent_config.json` + `model.safetensors`
/// and load each under its directory name.
fn load_bundle(bundle: &Path, device: &Device) -> Result<Vec<Variant>, String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(bundle)
        .map_err(|e| format!("cannot read the bundle directory {}: {e}", bundle.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_dir()
                && p.join("rl_agent_config.json").exists()
                && p.join("model.safetensors").exists()
        })
        .collect();
    entries.sort();
    if entries.is_empty() {
        return Err(format!(
            "found no checkpoints in {}; each checkpoint is a subdirectory that holds rl_agent_config.json and model.safetensors",
            bundle.display()
        ));
    }
    let mut variants = Vec::new();
    for dir in entries {
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("laya")
            .to_string();
        variants.push(load_variant(&id, &dir, device)?);
    }
    Ok(variants)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("laya-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let device = match device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("laya-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let variants = match &args.bundle_dir {
        Some(bundle) => match load_bundle(bundle, &device) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("laya-serve: {e}");
                return ExitCode::from(1);
            }
        },
        None => {
            let dir = args.model_dir.expect("checked");
            let id = if args.default.is_empty() {
                dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("laya")
                    .to_string()
            } else {
                args.default.clone()
            };
            match load_variant(&id, &dir, &device) {
                Ok(v) => vec![v],
                Err(e) => {
                    eprintln!("laya-serve: {e}");
                    return ExitCode::from(1);
                }
            }
        }
    };
    let default = variants
        .iter()
        .position(|v| v.model_id == args.default)
        .unwrap_or(0);
    eprintln!(
        "laya-serve: loaded {} variants; the default is {}",
        variants.len(),
        variants[default].model_id
    );
    let memory_budget_bytes = match args.memory_budget_mib {
        Some(mib) => mib.saturating_mul(MIB),
        None => match host_memory_budget() {
            Some(bytes) => bytes,
            None => {
                eprintln!(
                    "laya-serve: this host does not report its available memory; set a budget with --memory-budget-mib"
                );
                return ExitCode::from(2);
            }
        },
    };
    let mut admission = Admission {
        memory_budget_bytes,
        ..Admission::default()
    };
    if let Some(n) = args.concurrency {
        admission.concurrency = n;
    }
    if let Some(n) = args.max_questions {
        admission.max_questions = n;
    }
    let state = match ServeState::new(variants, default, args.aliases, args.device, admission) {
        Ok(state) => Arc::new(state),
        Err(e) => {
            eprintln!("laya-serve: {e}");
            return ExitCode::from(2);
        }
    };
    eprintln!(
        "laya-serve: memory budget is {} MiB; at most {} inference passes run at once across all variants",
        state.memory_mib, state.admission.concurrency
    );
    for (variant, share) in state.variants.iter().zip(&state.variant_slots) {
        eprintln!(
            "laya-serve: {}: one inference pass at {} questions needs {} MiB; at most {} run at once",
            variant.model_id, state.admission.max_questions, share.forward_mib, share.limit
        );
    }
    let addr: SocketAddr = match format!("{}:{}", args.host, args.port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("laya-serve: --host and --port do not form a valid listen address: {e}");
            return ExitCode::from(2);
        }
    };
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("laya-serve: cannot listen on {addr}: {e}");
            return ExitCode::from(1);
        }
    };
    eprintln!("laya-serve: listening on http://{addr}");
    match axum::serve(listener, router(state)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("laya-serve: {e}");
            ExitCode::from(1)
        }
    }
}
