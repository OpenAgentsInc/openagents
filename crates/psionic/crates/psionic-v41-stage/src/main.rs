//! `psionic-v41-stage`: one stage of a DeepSeek-V4.1 group.
//!
//! ```text
//! psionic-v41-stage --weights DIR --layers A-B --gpu N --manifest D [--embeddings] [--head]
//!     --stage NAME --generation G --front ADDR --token T
//!     [--backend cpu|cuda|metal] [--arithmetic f32|bf16|bf16-tp8|device] [--max-context N]
//!     [--max-step-tokens N] [--prefill-chunk N]
//!     [--max-input-tokens N] [--listen ADDR] [--data-listen ADDR [--data-advertise ADDR]]
//!     [--experts device|host] [--wire native|cuda] [--events FILE]
//! psionic-v41-stage reference --weights DIR [--prompt JSON] [--mode thinking|chat] [--steps N]
//!     [--temperature T --top-p P --seed S] [--arithmetic A] [--max-context N]
//!     [--backend cpu|cuda|metal]
//! psionic-v41-stage manifest --split 0-10,10-20,20-30,30-40
//! psionic-v41-stage plan --device HOST:GPU:CLASS:BYTES[:host-experts][:fixed=BYTES] ...
//!     --ram HOST:BYTES ... [--split A-B,...] [--max-context N] [--max-step-tokens N]
//!     [--diagnostic-cpu]
//! psionic-v41-stage synthetic DIR [NAME]
//! psionic-v41-stage qualify --weights DIR --manifest D --oracle DIR --out DIR [...]
//! psionic-v41-stage qualify-derive --rule 2.1|2.2 --reference R[,R] --control C[,C] --out FILE
//! psionic-v41-stage qualify-judge --policy FILE --report R[,R]
//! psionic-v41-stage replay --weights DIR --capture DIR
//! psionic-v41-stage build-info
//! psionic-v41-stage chunk-memory
//! psionic-v41-stage chain --weights DIR [--split A-B,...] [--gpus N,...]
//!     [--max-context N] [--max-step-tokens N] [--prompt JSON | --prompt-file F]
//!     [--mode thinking|chat] [--max-tokens N] [--prefill-chunk N] [--load-only]
//! ```
//!
//! `--arithmetic` takes `f32`, `bf16`, or `bf16-tp8` on the CPU backend and
//! only `device` on CUDA and Metal; anything else is refused before weights
//! load.
//! `build-info` prints the executable's SHA-256, its CUDA architecture, and
//! the profiles it runs, as `scripts/pylon/v41-stage-build.sh` records them.

use std::{io::Write, path::Path, process::ExitCode};

use psionic_v41_stage::{
    Backend, Options, PromptMode, Refused, Sampling, load_manifest, load_model, load_model_for,
    parse_layers, parse_profile, reference, serve, synthetic_manifest, write_synthetic,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("reference") => run_reference(&args[1..]),
        Some("manifest") => run_manifest(&args[1..]),
        Some("plan") => run_plan(&args[1..]),
        Some("synthetic") => run_synthetic(&args[1..]),
        Some("qualify") => run_qualify(&args[1..]),
        Some("qualify-derive") => psionic_v41_stage::qualify::run_derive(&args[1..]),
        Some("qualify-judge") => psionic_v41_stage::qualify::run_judge(&args[1..]),
        Some("build-info") => run_build_info(),
        Some("chain") => run_chain(&args[1..]),
        Some("replay") => run_replay(&args[1..]),
        Some("chunk-memory") if args.len() == 1 => {
            psionic_v41_stage::chunk_memory_profile().map(|v| println!("{v}"))
        }
        _ => run_stage(&args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(refused) => {
            let _ = writeln!(std::io::stderr(), "psionic-v41-stage: {refused}");
            ExitCode::from(2)
        }
    }
}

fn run_stage(args: &[String]) -> Result<(), Refused> {
    let options = Options::parse(args)?;
    let manifest = load_manifest(&options.manifest, &options.weights)?;
    let entry = psionic_v41_stage::find_stage(&manifest, &options)?;
    let selection = manifest.selection(usize::from(entry.stage));
    #[cfg(feature = "cuda")]
    if options.backend == Backend::Cuda {
        return psionic_v41_stage::cuda::run_stage(&options, &manifest, &entry, selection.as_ref());
    }
    #[cfg(all(feature = "metal", target_os = "macos"))]
    if options.backend == Backend::Metal {
        return psionic_v41_stage::metal::run_stage(
            &options,
            &manifest,
            &entry,
            selection.as_ref(),
        );
    }
    if options.control.is_some() {
        return Err(Refused("--control runs only on the CUDA backend".into()));
    }
    let model: &'static _ = Box::leak(Box::new(load_model_for(
        &options.weights,
        selection.as_ref(),
    )?));
    psionic_v41_stage::check_weights(&manifest, &model.digest)?;
    let arithmetic = options.arithmetic.unwrap_or(model.arithmetic);
    let stage = psionic_v41_stage::stage_info(&options, &manifest, &entry, model, Backend::Cpu);
    let worker = psionic_v41_stage::cpu_worker_wired(
        model,
        &manifest,
        &entry,
        arithmetic,
        options.max_context,
        options.generation,
        options.wire,
    )?;
    serve::serve(stage, worker, options.generation)
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

fn run_reference(args: &[String]) -> Result<(), Refused> {
    let weights = flag(args, "--weights").ok_or_else(|| Refused("--weights is required".into()))?;
    let prompt = flag(args, "--prompt")
        .map(|p| serde_json::from_str(p).map_err(|e| Refused(format!("--prompt: {e}"))))
        .transpose()?;
    let sampling = Sampling {
        temperature: number(args, "--temperature", 0.0)?,
        top_p: number(args, "--top-p", 1.0)?,
        seed: number(args, "--seed", 0)?,
    };
    let mode = flag(args, "--mode")
        .map(PromptMode::parse)
        .transpose()?
        .unwrap_or_default();
    let steps = number(args, "--steps", 8)?;
    let max_context = number(args, "--max-context", 8192)?;
    let backend = flag(args, "--backend")
        .map(Backend::parse)
        .transpose()?
        .unwrap_or(Backend::Cpu);
    // Refuse an arithmetic the backend doesn't implement before any load.
    let arithmetic = flag(args, "--arithmetic")
        .map(|value| parse_profile(backend, value))
        .transpose()?
        .flatten();
    let answer = match backend {
        #[cfg(feature = "cuda")]
        Backend::Cuda => psionic_v41_stage::cuda::reference(
            Path::new(weights),
            prompt.as_ref(),
            mode,
            steps,
            sampling,
            max_context,
        )?,
        #[cfg(all(feature = "metal", target_os = "macos"))]
        Backend::Metal => psionic_v41_stage::metal::reference(
            Path::new(weights),
            prompt.as_ref(),
            mode,
            steps,
            sampling,
            max_context,
        )?,
        _ => {
            let model = load_model(Path::new(weights))?;
            let arithmetic = arithmetic.unwrap_or(model.arithmetic);
            reference(
                &model,
                prompt.as_ref(),
                mode,
                steps,
                sampling,
                arithmetic,
                max_context,
            )?
        }
    };
    let json = serde_json::to_string(&answer).map_err(|e| Refused(e.to_string()))?;
    let _ = writeln!(std::io::stdout(), "{json}");
    Ok(())
}

fn run_build_info() -> Result<(), Refused> {
    let mut info = psionic_v41_stage::build_identity();
    info["runtimes"] = serde_json::json!({
        "cpu": Backend::Cpu.runtime(),
        "cuda": cfg!(feature = "cuda").then(|| Backend::Cuda.runtime()),
        "metal": cfg!(all(feature = "metal", target_os = "macos")).then(|| Backend::Metal.runtime()),
    });
    info["profiles"] = serde_json::json!({
        "cpu": ["cpu-f32", "cpu-bf16", "cpu-bf16-tp8"],
        "cuda": cfg!(feature = "cuda").then(|| psionic_v41_stage::profile_name(Backend::Cuda, None)),
        "metal": cfg!(all(feature = "metal", target_os = "macos"))
            .then(|| psionic_v41_stage::profile_name(Backend::Metal, None)),
    });
    let _ = writeln!(std::io::stdout(), "{info}");
    Ok(())
}

#[cfg(all(feature = "metal", target_os = "macos"))]
fn run_replay(args: &[String]) -> Result<(), Refused> {
    let weights = flag(args, "--weights").ok_or_else(|| Refused("--weights is required".into()))?;
    let capture = flag(args, "--capture").ok_or_else(|| Refused("--capture is required".into()))?;
    let report = psionic_v41_stage::metal::replay(Path::new(weights), Path::new(capture))?;
    let _ = writeln!(std::io::stdout(), "{report}");
    Ok(())
}

#[cfg(not(all(feature = "metal", target_os = "macos")))]
fn run_replay(_args: &[String]) -> Result<(), Refused> {
    Err(Refused(
        "replay needs a Mac build with --features metal".into(),
    ))
}

#[cfg(feature = "cuda")]
fn run_chain(args: &[String]) -> Result<(), Refused> {
    let options = psionic_v41_stage::cuda::chain::ChainOptions::parse(args)?;
    let report = psionic_v41_stage::cuda::chain::run(&options)?;
    let json = serde_json::to_string(&report).map_err(|e| Refused(e.to_string()))?;
    let _ = writeln!(std::io::stdout(), "{json}");
    Ok(())
}

#[cfg(not(feature = "cuda"))]
fn run_chain(_args: &[String]) -> Result<(), Refused> {
    Err(Refused("chain needs a build with --features cuda".into()))
}

fn run_manifest(args: &[String]) -> Result<(), Refused> {
    let split = flag(args, "--split").ok_or_else(|| Refused("--split is required".into()))?;
    let split = split
        .split(',')
        .map(parse_layers)
        .collect::<Result<Vec<_>, _>>()?;
    let manifest = synthetic_manifest(&split)?;
    let json = serde_json::to_vec(&manifest).map_err(|e| Refused(e.to_string()))?;
    let _ = std::io::stdout().write_all(&json);
    Ok(())
}

/// Plans a group over devices of different types from W3's measured memory
/// (#12418) and prints its manifest. A device is
/// `HOST:GPU:CLASS:BYTES[:host-experts][:fixed=BYTES]`, where CLASS is
/// `cuda-sm_NN` or `cpu-ARCH`, BYTES the memory the stage may use (a GPU's
/// free memory, or host memory for a CPU stage), `host-experts` keeps a
/// GPU's routed experts in host memory, and `fixed` is the class's measured
/// fixed cost per stage.
///
/// Groups run on GPUs only: a CPU device is refused unless
/// `--diagnostic-cpu` marks the plan as a reference diagnostic (owner,
/// 2026-10-21, #12418). CPU stages stay for unit tests and diagnostics.
fn run_plan(args: &[String]) -> Result<(), Refused> {
    use psionic_models::deepseek_v41_stage::{
        DeviceKind, DeviceSlot, ExpertMemory, HostRam, MeasuredMemory, plan_devices,
    };
    let bad = |what: &str, v: &str| Refused(format!("{what} {v} is not valid"));
    let number = |v: &str| -> Result<u64, Refused> { v.parse().map_err(|_| bad("number", v)) };
    let mut slots = Vec::new();
    let mut hosts = Vec::new();
    let diagnostic = args.iter().any(|a| a == "--diagnostic-cpu");
    let mut i = 0;
    while i < args.len() {
        let value = args.get(i + 1).map(String::as_str).unwrap_or_default();
        match args[i].as_str() {
            "--diagnostic-cpu" => {
                i += 1;
                continue;
            }
            "--device" => {
                let parts: Vec<&str> = value.split(':').collect();
                if parts.len() < 4 {
                    return Err(bad("--device", value));
                }
                let kind = if parts[2].starts_with("cpu") {
                    DeviceKind::Cpu
                } else {
                    DeviceKind::Gpu
                };
                if kind == DeviceKind::Cpu && !diagnostic {
                    return Err(Refused(format!(
                        "--device {value} is a CPU stage; groups run on GPUs only, so pass --diagnostic-cpu for a reference diagnostic"
                    )));
                }
                let mut slot = DeviceSlot {
                    host: parts[0].into(),
                    gpu: number(parts[1])? as u32,
                    class: parts[2].into(),
                    kind,
                    bytes: number(parts[3])?,
                    experts: ExpertMemory::Device,
                    per_stage_bytes: None,
                };
                for extra in &parts[4..] {
                    match extra.split_once('=') {
                        None if *extra == "host-experts" => slot.experts = ExpertMemory::Host,
                        Some(("fixed", n)) => slot.per_stage_bytes = Some(number(n)?),
                        _ => return Err(bad("--device", value)),
                    }
                }
                slots.push(slot);
            }
            "--ram" => {
                let (host, bytes) = value.split_once(':').ok_or_else(|| bad("--ram", value))?;
                hosts.push(HostRam {
                    host: host.into(),
                    bytes: number(bytes)?,
                });
            }
            "--split" | "--max-context" | "--max-step-tokens" => {}
            other => return Err(Refused(format!("plan doesn't take {other}"))),
        }
        i += 2;
    }
    let context = number(flag(args, "--max-context").unwrap_or("8192"))?;
    let step = number(flag(args, "--max-step-tokens").unwrap_or("8192"))?;
    let memory = MeasuredMemory::flash_sized(context, step).map_err(|e| Refused(e.to_string()))?;
    let split = flag(args, "--split")
        .map(|s| {
            s.split(',')
                .map(parse_layers)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let manifest = plan_devices(&memory, &slots, &hosts, split.as_deref())
        .map_err(|e| Refused(e.to_string()))?;
    let json = serde_json::to_vec(&manifest).map_err(|e| Refused(e.to_string()))?;
    let _ = std::io::stdout().write_all(&json);
    Ok(())
}

fn run_synthetic(args: &[String]) -> Result<(), Refused> {
    let dir = args
        .first()
        .ok_or_else(|| Refused("synthetic needs a directory".into()))?;
    write_synthetic(
        Path::new(dir),
        args.get(1).map_or("synthetic", String::as_str),
    )
    .map_err(|e| Refused(format!("can't write {dir}: {e}")))
}

fn run_qualify(args: &[String]) -> Result<(), Refused> {
    let options = psionic_v41_stage::qualify::QualifyOptions::parse(args)?;
    let report = psionic_v41_stage::qualify::run(&options)?;
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        serde_json::json!({"passed": report["passed"], "out": options.out.display().to_string()})
    );
    Ok(())
}
