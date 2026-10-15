//! Tensor-only comparisons for the pinned embedding and digit models.
use clap::{Parser, ValueEnum};
use psionic_inference::{
    EngineKind, Error, Result, Tensors,
    artifacts::file_sha256,
    models::{self, ModelFamily},
};
use serde::Deserialize;
use serde_json::json;
use std::{path::PathBuf, time::Instant};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Family {
    Potion,
    Mnist,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Engine {
    Native,
    Ort,
}
#[derive(Parser)]
struct Args {
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
        Family::Potion => ModelFamily::Potion,
        Family::Mnist => ModelFamily::Mnist,
    };
    let kind = match args.engine {
        Engine::Native => EngineKind::Native,
        Engine::Ort => EngineKind::Ort,
    };
    let fixture = match family {
        ModelFamily::Potion => include_str!("../../fixtures/potion-reference.json"),
        ModelFamily::Mnist => include_str!("../../fixtures/mnist-reference.json"),
    };
    let reference: Reference = serde_json::from_str(fixture)?;
    let startup = Instant::now();
    let mut engine = models::load_engine(
        family,
        kind,
        &args.model_dir,
        args.threads,
        args.ort_library.as_deref(),
        args.ort_sha256.as_deref(),
    )?;
    let load_ms = startup.elapsed().as_secs_f64() * 1000.0;
    let mut max_distance = 0_f32;
    for case in &reference.cases {
        let actual = engine.run(&case.inputs)?;
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
        ModelFamily::Potion => 1e-5,
        ModelFamily::Mnist => 1e-3,
    };
    if max_distance >= tolerance {
        return Err(Error(format!(
            "Reference distance {max_distance} exceeds {tolerance}"
        )));
    }
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
        json!({"schema":"psionic.small-model.benchmark.v1","model":models::manifest(family)?,"execution":engine.identity(),
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
