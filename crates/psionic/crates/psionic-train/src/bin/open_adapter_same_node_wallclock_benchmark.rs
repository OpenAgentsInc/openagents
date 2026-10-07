use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use psionic_train::{
    first_swarm_open_adapter_sft_request, open_adapter_pgolfish_config_with_batch_size,
    open_adapter_pgolfish_samples, run_open_adapter_sft_export, OpenAdapterExecutionConfig,
    OpenAdapterPgolfishSampleSplit, OpenAdapterSftRunOutcome, OpenAdapterTrainingExecutionBackend,
    PortableModelBundle, PortableTokenizerAssetFormat, PortableTokenizerBinding,
    OPEN_ADAPTER_CUDA_BACKEND_LABEL, OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
    OPEN_ADAPTER_PGOLFISH_BATCH_SIZE,
};
use serde::Serialize;

const REPORT_SCHEMA_VERSION: &str = "psionic.open_adapter_same_node_wallclock_benchmark.v1";
const DEFAULT_TARGET_SECONDS: u64 = 600;
const DEFAULT_CALIBRATION_STEPS: u64 = 12;
const REQUEST_STEP_DURATION_MS: u64 = 25;
const TARGET_UTILIZATION_BPS: u64 = 9_500;
const ELAPSED_CHECK_INTERVAL_STEPS: u64 = 1_024;

#[derive(Clone, Debug, Serialize)]
struct BenchmarkReport {
    schema_version: String,
    host: String,
    backend_label: String,
    logical_device_kind: String,
    logical_device_label: String,
    target_wallclock_seconds: u64,
    calibration: CalibrationReport,
    retained_run: RetainedRunReport,
    improvement: ImprovementReport,
    claim_boundary: String,
}

#[derive(Clone, Debug, Serialize)]
struct CalibrationReport {
    run_id: String,
    max_steps: u64,
    observed_wallclock_ms: u64,
    estimated_step_ms: u64,
    final_mean_loss: f32,
}

#[derive(Clone, Debug, Serialize)]
struct RetainedRunReport {
    run_id: String,
    checkpoint_family: String,
    completed_steps: u64,
    observed_wallclock_ms: u64,
    steps_per_second: f64,
    samples_per_second: f64,
    source_tokens_per_second: f64,
    batch_count: usize,
    sample_count: usize,
    initial_mean_loss: f32,
    final_mean_loss: f32,
    loss_delta: f32,
    final_state_dict_digest: String,
    bundle_artifact_path: String,
}

#[derive(Clone, Debug, Serialize)]
struct ImprovementReport {
    calibration_to_retained_loss_delta: f32,
    retained_vs_calibration_steps_gain: u64,
    retained_vs_calibration_wallclock_gain_ms: i64,
}

#[derive(Clone, Debug)]
struct Args {
    backend_label: String,
    output_root: PathBuf,
    target_seconds: u64,
    calibration_steps: u64,
    batch_size: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    fs::create_dir_all(&args.output_root)?;
    let host = hostname();

    let calibration_config = benchmark_config(
        &args.backend_label,
        format!(
            "same-node-wallclock-calibration-{}",
            short_backend_label(args.backend_label.as_str())
        ),
        format!(
            "benchmark.open_adapter.same_node.{}.calibration",
            short_backend_label(args.backend_label.as_str())
        ),
        args.calibration_steps,
        args.batch_size,
    )?;
    let calibration_samples = benchmark_samples(sample_prefix(args.backend_label.as_str()))?;
    let calibration_backend =
        OpenAdapterTrainingExecutionBackend::new(calibration_config.clone(), calibration_samples)?;
    let calibration_outcome = timed_run(&calibration_backend, REQUEST_STEP_DURATION_MS)?;
    let calibration_elapsed_ms = calibration_outcome.observed_wallclock_ms;
    let estimated_step_ms = ((calibration_elapsed_ms + args.calibration_steps.saturating_sub(1))
        / args.calibration_steps)
        .max(1);

    let retained_config = benchmark_config(
        &args.backend_label,
        format!(
            "same-node-wallclock-retained-{}",
            short_backend_label(args.backend_label.as_str())
        ),
        format!(
            "benchmark.open_adapter.same_node.{}.retained",
            short_backend_label(args.backend_label.as_str())
        ),
        u64::MAX / 4,
        args.batch_size,
    )?;
    let retained_samples = benchmark_samples(sample_prefix(args.backend_label.as_str()))?;
    let retained_backend =
        OpenAdapterTrainingExecutionBackend::new(retained_config.clone(), retained_samples)?;
    let retained_outcome = run_until_target_wallclock(
        &retained_backend,
        Duration::from_millis(
            args.target_seconds
                .saturating_mul(1_000)
                .saturating_mul(TARGET_UTILIZATION_BPS)
                / 10_000,
        ),
        REQUEST_STEP_DURATION_MS,
        &args.output_root,
    )?;
    let batch_count = retained_backend.batches().len();
    let sample_count = retained_backend.provenance().sample_count;
    let source_tokens_per_step = retained_backend
        .batches()
        .iter()
        .flat_map(|batch| batch.samples.iter())
        .map(|sample| u64::from(sample.source_token_count))
        .sum::<u64>();
    let observed_seconds = retained_outcome.observed_wallclock_ms as f64 / 1000.0;
    let initial_mean_loss = retained_outcome.initial_mean_loss;
    let final_mean_loss = retained_outcome.final_mean_loss;

    let report = BenchmarkReport {
        schema_version: String::from(REPORT_SCHEMA_VERSION),
        host,
        backend_label: args.backend_label.clone(),
        logical_device_kind: retained_backend.provenance().logical_device_kind.to_string(),
        logical_device_label: retained_backend.provenance().logical_device_label.clone(),
        target_wallclock_seconds: args.target_seconds,
        calibration: CalibrationReport {
            run_id: calibration_config.run_id,
            max_steps: args.calibration_steps,
            observed_wallclock_ms: calibration_elapsed_ms,
            estimated_step_ms,
            final_mean_loss: calibration_outcome
                .outcome
                .gradient_records
                .last()
                .map(|record| record.mean_loss)
                .unwrap_or_default(),
        },
        retained_run: RetainedRunReport {
            run_id: retained_config.run_id.clone(),
            checkpoint_family: retained_config.checkpoint_family.clone(),
            completed_steps: retained_outcome.completed_steps,
            observed_wallclock_ms: retained_outcome.observed_wallclock_ms,
            steps_per_second: retained_outcome.completed_steps as f64
                / observed_seconds.max(f64::EPSILON),
            samples_per_second: (retained_outcome.completed_steps as f64 * sample_count as f64)
                / observed_seconds.max(f64::EPSILON),
            source_tokens_per_second:
                (retained_outcome.completed_steps as f64 * source_tokens_per_step as f64)
                    / observed_seconds.max(f64::EPSILON),
            batch_count,
            sample_count,
            initial_mean_loss,
            final_mean_loss,
            loss_delta: initial_mean_loss - final_mean_loss,
            final_state_dict_digest: retained_outcome.final_state_dict_digest,
            bundle_artifact_path: retained_outcome.bundle_artifact_path,
        },
        improvement: ImprovementReport {
            calibration_to_retained_loss_delta: calibration_outcome
                .outcome
                .gradient_records
                .last()
                .map(|record| record.mean_loss)
                .unwrap_or_default()
                - final_mean_loss,
            retained_vs_calibration_steps_gain: retained_outcome
                .completed_steps
                .saturating_sub(args.calibration_steps),
            retained_vs_calibration_wallclock_gain_ms: retained_outcome.observed_wallclock_ms as i64
                - calibration_elapsed_ms as i64,
        },
        claim_boundary: String::from(
            "This report measures one same-node Rust-only open-adapter training/export lane on the selected MLX Metal or CUDA backend, then keeps the fixed-budget run alive until most of the requested wallclock budget is consumed. It compares like-for-like backend execution on the repo-owned first-swarm supervision set. It does not claim Parameter Golf challenge parity, full-model dense training, or mixed-backend distributed convergence.",
        ),
    };

    let report_path = args.output_root.join("report.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "open-adapter same-node benchmark completed: report={} backend={} final_loss={:.6} steps={} sps={:.3}",
        report_path.display(),
        report.backend_label,
        report.retained_run.final_mean_loss,
        report.retained_run.completed_steps,
        report.retained_run.steps_per_second,
    );
    Ok(())
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut backend_label = default_backend_label();
    let mut output_root = PathBuf::from("/tmp/psionic_open_adapter_same_node_wallclock_benchmark");
    let mut target_seconds = DEFAULT_TARGET_SECONDS;
    let mut calibration_steps = DEFAULT_CALIBRATION_STEPS;
    let mut batch_size = OPEN_ADAPTER_PGOLFISH_BATCH_SIZE;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--backend-label" => {
                backend_label = args.next().ok_or("missing value after --backend-label")?;
            }
            "--output-root" => {
                output_root =
                    PathBuf::from(args.next().ok_or("missing value after --output-root")?);
            }
            "--target-seconds" => {
                target_seconds = args
                    .next()
                    .ok_or("missing value after --target-seconds")?
                    .parse()?;
            }
            "--calibration-steps" => {
                calibration_steps = args
                    .next()
                    .ok_or("missing value after --calibration-steps")?
                    .parse()?;
            }
            "--batch-size" => {
                batch_size = args
                    .next()
                    .ok_or("missing value after --batch-size")?
                    .parse()?;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: open_adapter_same_node_wallclock_benchmark [--backend-label <label>] [--output-root <path>] [--target-seconds <seconds>] [--calibration-steps <steps>] [--batch-size <size>]"
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!("unknown argument: {other}").into());
            }
        }
    }

    if target_seconds == 0 {
        return Err("target-seconds must be positive".into());
    }
    if calibration_steps == 0 {
        return Err("calibration-steps must be positive".into());
    }

    Ok(Args {
        backend_label,
        output_root,
        target_seconds,
        calibration_steps,
        batch_size,
    })
}

fn benchmark_config(
    backend_label: &str,
    run_id: String,
    checkpoint_family: String,
    max_steps: u64,
    batch_size: usize,
) -> Result<OpenAdapterExecutionConfig, Box<dyn std::error::Error>> {
    Ok(open_adapter_pgolfish_config_with_batch_size(
        backend_label,
        run_id,
        checkpoint_family,
        max_steps,
        batch_size,
    )?)
}

fn timed_run(
    backend: &OpenAdapterTrainingExecutionBackend,
    step_duration_ms: u64,
) -> Result<TimedOutcome, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let started_at_ms = now_ms();
    let outcome = run_open_adapter_sft_export(
        backend,
        &first_swarm_open_adapter_sft_request(
            backend.config().run_id.as_str(),
            "r1",
            started_at_ms,
            step_duration_ms,
        ),
    )?;
    Ok(TimedOutcome {
        observed_wallclock_ms: started.elapsed().as_millis().max(1) as u64,
        outcome,
    })
}

fn run_until_target_wallclock(
    backend: &OpenAdapterTrainingExecutionBackend,
    target_duration: Duration,
    step_duration_ms: u64,
    output_root: &Path,
) -> Result<RetainedTimedOutcome, Box<dyn std::error::Error>> {
    let mut run = backend.initialize_run()?;
    let started_at_ms = now_ms();
    let started = Instant::now();
    let mut completed_steps = 0_u64;
    let mut initial_mean_loss = None;
    let mut final_mean_loss = None;
    let batch_count = backend.batches().len().max(1);

    loop {
        let batch_index = completed_steps as usize % batch_count;
        let logical_started_at_ms =
            started_at_ms + completed_steps.saturating_mul(step_duration_ms);
        let logical_finished_at_ms = logical_started_at_ms + step_duration_ms;
        let (step_input, gradient_record) = backend.produce_step_input(
            &run,
            batch_index,
            logical_started_at_ms,
            logical_finished_at_ms,
        )?;
        if initial_mean_loss.is_none() {
            initial_mean_loss = Some(gradient_record.mean_loss);
        }
        final_mean_loss = Some(gradient_record.mean_loss);
        run.apply_step(step_input)?;
        completed_steps = completed_steps.saturating_add(1);

        if completed_steps % ELAPSED_CHECK_INTERVAL_STEPS == 0
            && started.elapsed() >= target_duration
        {
            break;
        }
    }

    let groups = backend.snapshot_training_groups(&run)?;
    let tokenizer = PortableTokenizerBinding::new(
        backend.config().model.tokenizer.clone(),
        PortableTokenizerAssetFormat::PsionicDigest,
        format!(
            "{}@{}",
            backend.config().model.base_model_id,
            backend.config().model.base_model_revision
        ),
    );
    let bundle = PortableModelBundle::from_training_groups(
        backend.provenance().adapter_family.clone(),
        backend.config().model.base_model_revision.clone(),
        backend.config().checkpoint_family.clone(),
        Some(format!("checkpoint://{}/final", backend.config().run_id)),
        groups.as_slice(),
        tokenizer,
        backend.config().model.tokenizer.template_digest.clone(),
    )?;
    let (bundle_bytes, bundle_receipt) = bundle.export_safetensors()?;
    let bundle_artifact_path = output_root.join("portable_bundle.safetensors");
    fs::write(&bundle_artifact_path, bundle_bytes)?;

    Ok(RetainedTimedOutcome {
        observed_wallclock_ms: started.elapsed().as_millis().max(1) as u64,
        completed_steps,
        initial_mean_loss: initial_mean_loss.unwrap_or_default(),
        final_mean_loss: final_mean_loss.unwrap_or_default(),
        final_state_dict_digest: bundle_receipt.state_dict_digest,
        bundle_artifact_path: bundle_artifact_path.display().to_string(),
    })
}

fn default_backend_label() -> String {
    if cfg!(target_os = "macos") {
        String::from(OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL)
    } else {
        String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL)
    }
}

fn sample_prefix(backend_label: &str) -> &str {
    match backend_label {
        OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL => "pgolfish-mlx",
        OPEN_ADAPTER_CUDA_BACKEND_LABEL => "pgolfish-cuda",
        _ => "pgolfish-generic",
    }
}

fn short_backend_label(backend_label: &str) -> &str {
    match backend_label {
        OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL => "mlx",
        OPEN_ADAPTER_CUDA_BACKEND_LABEL => "cuda",
        _ => "generic",
    }
}

fn hostname() -> String {
    env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            fs::read_to_string("/etc/hostname")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            Path::new("/bin/hostname")
                .exists()
                .then(|| std::process::Command::new("/bin/hostname").output().ok())
                .flatten()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| String::from("unknown"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn benchmark_samples(
    sample_prefix: &str,
) -> Result<Vec<psionic_train::OpenAdapterHiddenStateSample>, Box<dyn std::error::Error>> {
    Ok(open_adapter_pgolfish_samples(
        sample_prefix,
        OpenAdapterPgolfishSampleSplit::Training,
    )?)
}

struct TimedOutcome {
    observed_wallclock_ms: u64,
    outcome: OpenAdapterSftRunOutcome,
}

struct RetainedTimedOutcome {
    observed_wallclock_ms: u64,
    completed_steps: u64,
    initial_mean_loss: f32,
    final_mean_loss: f32,
    final_state_dict_digest: String,
    bundle_artifact_path: String,
}
