//! Reproducible model benchmarks; each invocation runs one engine.

use std::{
    path::PathBuf,
    process::Command,
    sync::{Arc, Barrier},
    time::Instant,
};

use clap::{Parser, ValueEnum};
use psionic_inference::{
    EngineKind, Error, ModelEngine, Result, Tensors,
    router::{RouterTokenizer, answer, file_sha256, load_engine, pinned_manifest},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Engine {
    Native,
    Ort,
}
#[derive(Clone, Copy, Debug, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Engine,
    Text,
}
#[derive(Clone, Copy, Debug, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum Bucket {
    Short,
    Medium,
    Long,
    RouteV1,
}

#[derive(Parser)]
#[command(about = "Compare native Psionic and ONNX Runtime execution")]
struct Args {
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
    #[arg(long, default_value_t = 1)]
    concurrency: usize,
    #[arg(long, default_value_t = 1000)]
    samples: usize,
    #[arg(long, default_value_t = 20)]
    warmup: usize,
    #[arg(long, default_value = "engine")]
    mode: Mode,
    #[arg(long, default_value = "route-v1")]
    bucket: Bucket,
}

#[derive(Clone, Deserialize)]
struct Input {
    id: String,
    request: String,
    context: Option<String>,
}
#[derive(Deserialize)]
struct Corpus {
    rows: Vec<Input>,
}
#[derive(Deserialize)]
struct Reference {
    requests: Vec<ReferenceCase>,
}
#[derive(Deserialize)]
struct ReferenceCase {
    inputs: Tensors,
    outputs: Tensors,
}

struct Worker {
    engine: Box<dyn ModelEngine>,
    inputs: Arc<Vec<(Input, Tensors)>>,
    tokenizer: Arc<RouterTokenizer>,
    mode: Mode,
    samples: usize,
    warmup: usize,
}
struct Timings {
    values: Vec<f64>,
    tokens: usize,
    start: Instant,
    end: Instant,
}

impl Worker {
    fn execute(&mut self, i: usize) -> Result<usize> {
        let (text, prepared) = &self.inputs[i % self.inputs.len()];
        let inputs = match self.mode {
            Mode::Engine => std::borrow::Cow::Borrowed(prepared),
            Mode::Text => std::borrow::Cow::Owned(
                self.tokenizer
                    .prepare(&text.request, text.context.as_deref())?,
            ),
        };
        let output = self.engine.run(&inputs)?;
        match self.mode {
            Mode::Engine => {
                std::hint::black_box(output);
            }
            Mode::Text => {
                std::hint::black_box(answer(&output, &pinned_manifest()?)?);
            }
        }
        Ok(inputs["input_ids"].shape[1])
    }
    fn run(mut self, barrier: Arc<Barrier>) -> Result<Timings> {
        // Release all workers even if warm-up fails, so no peer waits forever.
        let warmup = (0..self.warmup).try_for_each(|i| self.execute(i).map(|_| ()));
        barrier.wait();
        warmup?;
        let start = Instant::now();
        let mut values = Vec::with_capacity(self.samples);
        let mut tokens = 0;
        for i in 0..self.samples {
            let begin = Instant::now();
            tokens += self.execute(i)?;
            values.push(begin.elapsed().as_secs_f64() * 1000.0);
        }
        Ok(Timings {
            values,
            tokens,
            start,
            end: Instant::now(),
        })
    }
}

#[derive(Serialize)]
struct Statistics {
    samples: usize,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    mean_ms: f64,
    requests_per_second: f64,
    tokens_per_second: f64,
    wall_seconds: f64,
}
fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((p * sorted.len() as f64).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1)]
}
fn summarize(timings: Vec<Timings>) -> Statistics {
    let start = timings
        .iter()
        .map(|x| x.start)
        .min()
        .unwrap_or_else(Instant::now);
    let end = timings.iter().map(|x| x.end).max().unwrap_or(start);
    let wall = end.duration_since(start).as_secs_f64();
    let tokens: usize = timings.iter().map(|x| x.tokens).sum();
    let mut values: Vec<f64> = timings.into_iter().flat_map(|x| x.values).collect();
    values.sort_by(f64::total_cmp);
    Statistics {
        samples: values.len(),
        p50_ms: percentile(&values, 0.5),
        p95_ms: percentile(&values, 0.95),
        p99_ms: percentile(&values, 0.99),
        mean_ms: values.iter().sum::<f64>() / values.len() as f64,
        requests_per_second: values.len() as f64 / wall,
        tokens_per_second: tokens as f64 / wall,
        wall_seconds: wall,
    }
}

fn memory() -> serde_json::Value {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        let kib = |field: &str| {
            status
                .lines()
                .find(|s| s.starts_with(field))
                .and_then(|s| s.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u64>().ok())
                .map(|v| v * 1024)
        };
        return serde_json::json!({"resident_bytes":kib("VmRSS:"),"peak_resident_bytes":kib("VmHWM:")});
    }
    #[cfg(target_os = "macos")]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        // SAFETY: getrusage initializes the provided rusage on success.
        let peak = if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
            Some(unsafe { usage.assume_init() }.ru_maxrss)
        } else {
            None
        };
        let resident = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|v| v * 1024);
        return serde_json::json!({"resident_bytes":resident,"peak_resident_bytes":peak});
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    serde_json::json!({"resident_bytes":null,"peak_resident_bytes":null})
}

fn cpu_model() -> Option<String> {
    if cfg!(target_os = "macos") {
        return command("sysctl", &["-n", "machdep.cpu.brand_string"]);
    }
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("model name")
                    .and_then(|s| s.split_once(':'))
                    .map(|(_, v)| v.trim().to_owned())
            })
        })
}

fn command(name: &str, args: &[&str]) -> Option<String> {
    Command::new(name)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().into())
}

fn inputs(bucket: Bucket) -> Result<Vec<Input>> {
    let corpus: Corpus = serde_json::from_str(include_str!("../../fixtures/route-v1-inputs.json"))?;
    Ok(match bucket {
        Bucket::RouteV1 => corpus.rows,
        Bucket::Short => vec![Input {
            id: "short".into(),
            request: "Summarize this thread in three bullets.".into(),
            context: None,
        }],
        Bucket::Medium => vec![Input {
            id: "medium".into(),
            request: format!(
                "Review the following Rust changes and fix the failing tests. {}",
                "fn example() { let answer = 42; } ".repeat(12)
            ),
            context: None,
        }],
        Bucket::Long => vec![Input {
            id: "long".into(),
            request: format!(
                "Review these changes and open a pull request. {}",
                "fn example() { let answer = 42; } ".repeat(150)
            ),
            context: None,
        }],
    })
}

fn run(args: Args, process_start: Instant) -> Result<serde_json::Value> {
    if args.samples == 0
        || args.warmup == 0
        || args.concurrency == 0
        || args.threads == 0
        || args.threads > 256
        || args.threads % args.concurrency != 0
        || args.samples < args.concurrency
    {
        return Err(Error("Use positive sample and warm-up counts, and a total thread count divisible by concurrency".into()));
    }
    let kind = match args.engine {
        Engine::Native => EngineKind::Native,
        Engine::Ort => EngineKind::Ort,
    };
    let tokenizer = Arc::new(RouterTokenizer::load(
        &args.model_dir.join("tokenizer/tokenizer.json"),
    )?);
    let texts = inputs(args.bucket)?;
    let corpus_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(
            &texts
                .iter()
                .map(|x| (&x.id, &x.request, &x.context))
                .collect::<Vec<_>>()
        )?)
    );
    let prepared = Arc::new(
        texts
            .into_iter()
            .map(|input| {
                let tensors = tokenizer.prepare(&input.request, input.context.as_deref())?;
                Ok((input, tensors))
            })
            .collect::<Result<Vec<_>>>()?,
    );
    let counts: Vec<usize> = prepared
        .iter()
        .map(|(_, t)| t["input_ids"].shape[1])
        .collect();
    if matches!(args.bucket, Bucket::Long) && counts.iter().any(|n| *n != 512) {
        return Err(Error("Long benchmark must contain 512-token inputs".into()));
    }
    let load_start = Instant::now();
    let mut engines = Vec::new();
    for _ in 0..args.concurrency {
        engines.push(load_engine(
            kind,
            &args.model_dir,
            args.threads / args.concurrency,
            args.ort_library.as_deref(),
            args.ort_sha256.as_deref(),
        )?);
    }
    let load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    let identity = engines[0].identity().clone();
    let ready_ms = process_start.elapsed().as_secs_f64() * 1000.0;
    let first = Instant::now();
    engines[0].run(&prepared[0].1)?;
    let first_inference_ms = first.elapsed().as_secs_f64() * 1000.0;
    let reference: Reference =
        serde_json::from_str(include_str!("../../fixtures/s1-router-reference.json"))?;
    let manifest = pinned_manifest()?;
    let mut maximum_centered_distance = 0.0_f32;
    for case in reference.requests {
        let actual = engines[0].run(&case.inputs)?;
        let a = answer(&actual, &manifest)?;
        let b = answer(&case.outputs, &manifest)?;
        if [a.category.label, a.difficulty.label, a.stakes.label]
            != [b.category.label, b.difficulty.label, b.stakes.label]
            || a.flags
                .iter()
                .any(|(key, value)| (*value >= 0.5) != (b.flags[key] >= 0.5))
        {
            return Err(Error(
                "Reference decision check failed before benchmarking".into(),
            ));
        }
        for (name, expected) in case.outputs {
            let x = actual[&name].floats()?;
            let y = expected.floats()?;
            let mx = x.iter().sum::<f32>() / x.len() as f32;
            let my = y.iter().sum::<f32>() / y.len() as f32;
            for (x, y) in x.iter().zip(y) {
                maximum_centered_distance = maximum_centered_distance
                    .max(((x - mx) - (y - my)).abs() / manifest.temperatures[&name]);
            }
        }
    }
    if maximum_centered_distance >= 0.01 {
        return Err(Error(
            "Reference probability check failed before benchmarking".into(),
        ));
    }
    let barrier = Arc::new(Barrier::new(args.concurrency));
    let mut workers = Vec::new();
    for (i, engine) in engines.into_iter().enumerate() {
        let worker = Worker {
            engine,
            inputs: prepared.clone(),
            tokenizer: tokenizer.clone(),
            mode: args.mode,
            samples: args.samples / args.concurrency
                + usize::from(i < args.samples % args.concurrency),
            warmup: args.warmup,
        };
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || worker.run(barrier)));
    }
    let timings = workers
        .into_iter()
        .map(|w| {
            w.join()
                .map_err(|_| Error("Benchmark worker stopped unexpectedly".into()))?
        })
        .collect::<Result<Vec<_>>>()?;
    let statistics = summarize(timings);
    let exe = std::env::current_exe()?;
    let model_bytes = manifest.files.values().map(|x| x.bytes).sum::<u64>();
    Ok(serde_json::json!({
        "schema":"psionic.model.benchmark.v1","model":manifest.model,"model_revision":manifest.revision,
        "model_sha256":manifest.files["model.gq.onnx"].sha256,"tokenizer_sha256":manifest.files["tokenizer/tokenizer.json"].sha256,
        "engine":identity,"mode":args.mode,"bucket":args.bucket,"total_threads":args.threads,"concurrency":args.concurrency,
        "warmup_per_worker":args.warmup,"token_counts":counts,"corpus_sha256":corpus_sha256,"statistics":statistics,
        "startup":{"session_load_ms":load_ms,"process_to_ready_ms":ready_ms,"first_inference_ms":first_inference_ms,"artifact_cache":"warm"},
        "memory":memory(),"package":{"executable_bytes":exe.metadata()?.len(),"executable_sha256":file_sha256(&exe)?,
            "ort_library_bytes":args.ort_library.as_ref().and_then(|p|p.metadata().ok()).map(|m|m.len()),"source_model_bytes":model_bytes},
        "reference":{"cases":100,"maximum_centered_distance":maximum_centered_distance},
        "environment":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"cpu":cpu_model(),
            "rustc":command("rustc",&["--version"]),"source_commit":command("git",&["rev-parse","HEAD"]),
            "source_dirty":command("git",&["status","--porcelain"]).map(|s|!s.is_empty()),"ort_feature":cfg!(feature="ort"),
            "command":std::env::args().collect::<Vec<_>>()},
        "target_512_p99_under_20_ms":if matches!(args.bucket,Bucket::Long)&&args.threads==4&&args.concurrency==1 {Some(statistics.p99_ms<20.0)}else{None}
    }))
}

fn main() -> std::process::ExitCode {
    let start = Instant::now();
    match run(Args::parse(), start) {
        Ok(report) => {
            println!("{report}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentiles_use_nearest_rank_and_handle_small_samples() {
        assert_eq!(percentile(&[1.0], 0.99), 1.0);
        let values: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&values, 0.50), 50.0);
        assert_eq!(percentile(&values, 0.99), 99.0);
    }
    #[test]
    fn route_corpus_contains_all_32_rows() {
        assert_eq!(inputs(Bucket::RouteV1).unwrap().len(), 32);
    }
}
