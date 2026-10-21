//! `psionic-v41-stage`: one stage of a DeepSeek-V4.1 group.
//!
//! ```text
//! psionic-v41-stage --weights DIR --layers A-B --gpu N --manifest D [--embeddings] [--head]
//!     --stage NAME --generation G --front ADDR --token T
//!     [--backend cpu|cuda] [--arithmetic f32|bf16|bf16-tp8|device] [--max-context N]
//!     [--max-input-tokens N] [--listen ADDR] [--data-listen ADDR [--data-advertise ADDR]]
//!     [--events FILE]
//! psionic-v41-stage reference --weights DIR [--prompt JSON] [--mode thinking|chat] [--steps N]
//!     [--temperature T --top-p P --seed S] [--arithmetic A] [--max-context N]
//!     [--backend cpu|cuda]
//! psionic-v41-stage manifest --split 0-10,10-20,20-30,30-40
//! psionic-v41-stage synthetic DIR [NAME]
//! psionic-v41-stage qualify --weights DIR --manifest D --oracle DIR --out DIR [...]
//! psionic-v41-stage qualify-derive --reference R[,R] --control C[,C] --out FILE
//! psionic-v41-stage build-info
//! ```
//!
//! `--arithmetic` takes `f32`, `bf16`, or `bf16-tp8` on the CPU backend and
//! only `device` on CUDA; anything else is refused before weights load.
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
        Some("synthetic") => run_synthetic(&args[1..]),
        Some("qualify") => run_qualify(&args[1..]),
        Some("qualify-derive") => psionic_v41_stage::qualify::run_derive(&args[1..]),
        Some("build-info") => run_build_info(),
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
    if options.observe || options.control.is_some() {
        return Err(Refused(
            "--observe and --control run only on the CUDA backend".into(),
        ));
    }
    let model: &'static _ = Box::leak(Box::new(load_model_for(
        &options.weights,
        selection.as_ref(),
    )?));
    psionic_v41_stage::check_weights(&manifest, &model.digest)?;
    let arithmetic = options.arithmetic.unwrap_or(model.arithmetic);
    let stage = psionic_v41_stage::stage_info(&options, &manifest, &entry, model, Backend::Cpu);
    let worker = psionic_v41_stage::cpu_worker(
        model,
        &manifest,
        &entry,
        arithmetic,
        options.max_context,
        options.generation,
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
    });
    info["profiles"] = serde_json::json!({
        "cpu": ["cpu-f32", "cpu-bf16", "cpu-bf16-tp8"],
        "cuda": cfg!(feature = "cuda").then(|| psionic_v41_stage::profile_name(Backend::Cuda, None)),
    });
    let _ = writeln!(std::io::stdout(), "{info}");
    Ok(())
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
