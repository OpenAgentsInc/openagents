//! `psionic-v41-stage`: one stage of a DeepSeek-V4.1 group.
//!
//! ```text
//! psionic-v41-stage --weights DIR --layers A-B --gpu N --manifest D [--embeddings] [--head]
//!     --stage NAME --generation G --front ADDR --token T
//!     [--backend cpu] [--arithmetic f32|bf16|bf16-tp8] [--max-context N]
//!     [--listen ADDR] [--events FILE]
//! psionic-v41-stage reference --weights DIR [--prompt JSON] [--steps N]
//!     [--temperature T --top-p P --seed S] [--arithmetic A] [--max-context N]
//! psionic-v41-stage manifest --split 0-10,10-20,20-30,30-40
//! psionic-v41-stage synthetic DIR [NAME]
//! ```

use std::{io::Write, path::Path, process::ExitCode};

use psionic_v41_stage::{
    Backend, Options, Refused, Sampling, load_manifest, load_model, parse_arithmetic, parse_layers,
    reference, serve, synthetic_manifest, write_synthetic,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("reference") => run_reference(&args[1..]),
        Some("manifest") => run_manifest(&args[1..]),
        Some("synthetic") => run_synthetic(&args[1..]),
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
    let model: &'static _ = Box::leak(Box::new(load_model(&options.weights)?));
    if !manifest.index_sha256.is_empty()
        && model.digest != format!("sha256:{}", manifest.index_sha256)
    {
        return Err(Refused(format!(
            "the weights are {}; the manifest needs sha256:{}",
            model.digest, manifest.index_sha256
        )));
    }
    let arithmetic = options.arithmetic.unwrap_or(model.arithmetic);
    let stage = serve::Stage {
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
        events: options.events.clone(),
    };
    match options.backend {
        Backend::Cpu => {
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
    }
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
    let model = load_model(Path::new(weights))?;
    let prompt = flag(args, "--prompt")
        .map(|p| serde_json::from_str(p).map_err(|e| Refused(format!("--prompt: {e}"))))
        .transpose()?;
    let sampling = Sampling {
        temperature: number(args, "--temperature", 0.0)?,
        top_p: number(args, "--top-p", 1.0)?,
        seed: number(args, "--seed", 0)?,
    };
    let arithmetic = flag(args, "--arithmetic")
        .map(parse_arithmetic)
        .transpose()?
        .unwrap_or(model.arithmetic);
    let answer = reference(
        &model,
        prompt.as_ref(),
        number(args, "--steps", 8)?,
        sampling,
        arithmetic,
        number(args, "--max-context", 8192)?,
    )?;
    let json = serde_json::to_string(&answer).map_err(|e| Refused(e.to_string()))?;
    let _ = writeln!(std::io::stdout(), "{json}");
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
