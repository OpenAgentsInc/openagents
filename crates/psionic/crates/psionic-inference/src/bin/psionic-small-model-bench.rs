//! Tensor-only comparisons for the pinned embedding and digit models.
use clap::{Parser, ValueEnum};
use psionic_inference::{
    DeviceChoice, EngineKind, Error, Result, Tensors,
    artifacts::file_sha256,
    models::{self, ModelFamily},
};
use serde::Deserialize;
use serde_json::json;
use std::{path::PathBuf, time::Instant};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Family {
    Router,
    Potion,
    Mnist,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Engine {
    Native,
    Ort,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Device {
    Cpu,
    Cuda,
    Metal,
    Coreml,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Units {
    All,
    CpuAndGpu,
    CpuAndNeuralEngine,
    CpuOnly,
}
#[derive(Parser)]
struct Args {
    /// Explicit converted ORT-format model for a reduced runtime comparison.
    #[arg(long)]
    ort_model: Option<PathBuf>,
    #[arg(long)]
    ort_model_sha256: Option<String>,
    #[arg(long, default_value = "cpu")]
    device: Device,
    #[arg(long)]
    allow_cpu_fallback: bool,
    #[arg(long)]
    profile: Option<PathBuf>,
    #[arg(long, default_value = "cpu-and-gpu")]
    coreml_units: Units,
    #[arg(long)]
    family: Family,
    #[arg(long)]
    engine: Engine,
    #[arg(long)]
    model_dir: PathBuf,
    #[arg(long)]
    ort_library: Option<PathBuf>,
    #[arg(long)]
    ort_sha256: Option<String>,
    #[arg(long, default_value_t = 4)]
    threads: usize,
    #[arg(long, default_value_t = 1000)]
    samples: usize,
    #[arg(long, default_value_t = 20)]
    warmup: usize,
}
#[derive(Deserialize)]
struct Reference {
    #[serde(alias = "requests")]
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    inputs: Tensors,
    outputs: Tensors,
}
fn run(args: Args) -> Result<serde_json::Value> {
    if args.samples == 0 || args.warmup == 0 {
        return Err(Error("Use positive sample and warm-up counts".into()));
    }
    let family = match args.family {
        Family::Router => None,
        Family::Potion => Some(ModelFamily::Potion),
        Family::Mnist => Some(ModelFamily::Mnist),
    };
    let kind = match args.engine {
        Engine::Native => EngineKind::Native,
        Engine::Ort => EngineKind::Ort,
    };
    let device = match args.device {
        Device::Cpu => DeviceChoice::Cpu,
        Device::Cuda => DeviceChoice::Cuda,
        Device::Metal => DeviceChoice::Metal,
        Device::Coreml => DeviceChoice::CoreMl,
    };
    let fixture = match family {
        None => include_str!("../../fixtures/s1-router-reference.json"),
        Some(ModelFamily::Potion) => include_str!("../../fixtures/potion-reference.json"),
        Some(ModelFamily::Mnist) => include_str!("../../fixtures/mnist-reference.json"),
    };
    let model = if let Some(family) = family {
        serde_json::to_value(models::manifest(family)?)?
    } else {
        serde_json::to_value(psionic_inference::router::pinned_manifest()?)?
    };
    let reference: Reference = serde_json::from_str(fixture)?;
    let startup = Instant::now();
    let mut engine: Box<dyn psionic_inference::ModelEngine> = match kind {
        EngineKind::Native => {
            if let Some(family) = family {
                models::load_native_on(family, &args.model_dir, args.threads, device)?
            } else {
                psionic_inference::router::load_native_on(&args.model_dir, args.threads, device)?
            }
        }
        EngineKind::Ort => {
            #[cfg(feature = "ort")]
            {
                use psionic_inference::ort_engine::{
                    CoreMlUnits, OrtEngine, OrtExecution, OrtOptions,
                };
                if device != DeviceChoice::Cpu && args.profile.is_none() {
                    return Err(Error(
                        "Provide --profile to measure accelerator placement".into(),
                    ));
                }
                let onnx = if let Some(family) = family {
                    let spec = models::manifest(family)?;
                    spec.verify(&args.model_dir, kind)?;
                    spec.onnx
                } else {
                    psionic_inference::router::pinned_manifest()?.verify(&args.model_dir)?;
                    "model.gq.onnx".into()
                };
                let execution = OrtExecution {
                    device,
                    device_id: 0,
                    allow_cpu_fallback: args.allow_cpu_fallback,
                    profile: args.profile.clone(),
                    coreml_units: match args.coreml_units {
                        Units::All => CoreMlUnits::All,
                        Units::CpuAndGpu => CoreMlUnits::CpuAndGpu,
                        Units::CpuAndNeuralEngine => CoreMlUnits::CpuAndNeuralEngine,
                        Units::CpuOnly => CoreMlUnits::CpuOnly,
                    },
                };
                let onnx_path = if let Some(path) = &args.ort_model {
                    if Some(file_sha256(path)?) != args.ort_model_sha256 {
                        return Err(Error("Converted ORT model checksum does not match".into()));
                    }
                    path.clone()
                } else {
                    args.model_dir.join(onnx)
                };
                Box::new(OrtEngine::load_with_execution(
                    &onnx_path,
                    &OrtOptions {
                        library: args
                            .ort_library
                            .clone()
                            .ok_or_else(|| Error("Provide --ort-library".into()))?,
                        library_sha256: args
                            .ort_sha256
                            .clone()
                            .ok_or_else(|| Error("Provide --ort-sha256".into()))?,
                        threads: args.threads,
                    },
                    &execution,
                )?)
            }
            #[cfg(not(feature = "ort"))]
            {
                return Err(Error("ORT is not enabled".into()));
            }
        }
    };
    let load_ms = startup.elapsed().as_secs_f64() * 1000.0;
    let mut max_distance = 0_f32;
    for case in &reference.cases {
        let actual = engine.run(&case.inputs)?;
        if family.is_none() {
            let manifest = psionic_inference::router::pinned_manifest()?;
            let actual_answer = psionic_inference::router::answer(&actual, &manifest)?;
            let expected_answer = psionic_inference::router::answer(&case.outputs, &manifest)?;
            if [
                actual_answer.category.label,
                actual_answer.difficulty.label,
                actual_answer.stakes.label,
            ] != [
                expected_answer.category.label,
                expected_answer.difficulty.label,
                expected_answer.stakes.label,
            ] || actual_answer
                .flags
                .iter()
                .any(|(k, v)| (*v >= 0.5) != (expected_answer.flags[k] >= 0.5))
            {
                return Err(Error("Router decisions differ from reference".into()));
            }
        }
        if matches!(family, Some(ModelFamily::Mnist)) {
            let argmax = |v: &[f32]| {
                v.iter()
                    .enumerate()
                    .max_by(|(i, a), (j, b)| a.total_cmp(b).then(j.cmp(i)))
                    .map(|(i, _)| i)
            };
            if argmax(actual["Plus214_Output_0"].floats()?)
                != argmax(case.outputs["Plus214_Output_0"].floats()?)
            {
                return Err(Error("Digit decision differs from reference".into()));
            }
        }
        for (name, expected) in &case.outputs {
            let output = actual
                .get(name)
                .ok_or_else(|| Error(format!("Missing output {name}")))?;
            if output.shape != expected.shape {
                return Err(Error("Output dimensions changed".into()));
            }
            for (a, b) in output.floats()?.iter().zip(expected.floats()?) {
                max_distance = max_distance.max((a - b).abs());
            }
        }
    }
    let tolerance = match family {
        Some(ModelFamily::Potion) => 1e-5,
        Some(ModelFamily::Mnist) => 1e-3,
        None => 0.01,
    };
    if max_distance >= tolerance {
        return Err(Error(format!(
            "Reference distance {max_distance} exceeds {tolerance}"
        )));
    }
    let placement = engine.finish_profiling()?;
    for i in 0..args.warmup {
        std::hint::black_box(engine.run(&reference.cases[i % reference.cases.len()].inputs)?);
    }
    let start = Instant::now();
    let mut durations = Vec::with_capacity(args.samples);
    for i in 0..args.samples {
        let tick = Instant::now();
        std::hint::black_box(engine.run(&reference.cases[i % reference.cases.len()].inputs)?);
        durations.push(tick.elapsed().as_secs_f64() * 1000.0);
    }
    let elapsed = start.elapsed().as_secs_f64();
    durations.sort_by(f64::total_cmp);
    let percentile =
        |p: f64| durations[((durations.len() as f64 * p).ceil() as usize).saturating_sub(1)];
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let memory = |key: &str| {
        status
            .lines()
            .find(|s| s.starts_with(key))
            .and_then(|s| s.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u64>().ok())
            .map(|v| v * 1024)
    };
    let executable = std::env::current_exe()?;
    Ok(
        json!({"schema":"psionic.small-model.benchmark.v1","model":model,"converted_ort_model_sha256":args.ort_model_sha256,"execution":engine.identity(),"execution_details":engine.execution_report(),"placement_profile":placement,
        "mode":"engine","samples":args.samples,"warmup":args.warmup,"reference_cases":reference.cases.len(),"max_reference_distance":max_distance,
        "load_ms":load_ms,"p50_ms":percentile(0.50),"p95_ms":percentile(0.95),"p99_ms":percentile(0.99),
        "requests_per_second":args.samples as f64/elapsed,"timings_ms":durations,
        "resident_bytes":memory("VmRSS:"),"peak_resident_bytes":memory("VmHWM:"),"memory_models_loaded":true,
        "binary_sha256":file_sha256(&executable)?,"binary_bytes":executable.metadata()?.len(),"build_profile":if cfg!(debug_assertions){"development"}else{"release"},
        "source_commit":command("git",&["rev-parse","HEAD"]),"source_dirty":command("git",&["status","--porcelain"]),
        "rustc":command("rustc",&["--version"]),"platform":command("uname",&["-a"]),"cpu":std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|s|s.lines().find(|s|s.starts_with("model name")).map(str::to_owned)),
        "command":std::env::args().collect::<Vec<_>>(),"remaining_coverage":["macOS device measurements","embedding task-quality evaluation","digit recognition on product images"]}),
    )
}
fn command(name: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(name)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
}
fn main() {
    match run(Args::parse()) {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
