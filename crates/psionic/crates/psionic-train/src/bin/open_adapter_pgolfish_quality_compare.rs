use std::{
    env, fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    open_adapter_pgolfish_config_with_batch_size, open_adapter_pgolfish_samples,
    FixedBudgetTrainingRun, OpenAdapterExecutionConfig, OpenAdapterPgolfishSampleSplit,
    OpenAdapterTrainingExecutionBackend, PortableModelBundle, TrainingLoopBudget,
    OPEN_ADAPTER_CUDA_BACKEND_LABEL, OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
    OPEN_ADAPTER_PGOLFISH_BATCH_SIZE,
};
use serde::{Deserialize, Serialize};

const REPORT_SCHEMA_VERSION: &str = "psionic.open_adapter_pgolfish_quality_compare.v1";
const DEFAULT_OUTPUT_ROOT: &str = "/tmp/psionic_open_adapter_pgolfish_quality_compare";
const M5_REPORT_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/report.json";
const M5_BUNDLE_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors";
const CUDA_REPORT_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_cuda_parallel_20260327/report.json";
const CUDA_BUNDLE_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_cuda_parallel_20260327/portable_bundle.safetensors";
const ADMITTED_HOME_SUMMARY_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260327e/tailrun_admitted_home_run_summary.json";

#[derive(Clone, Debug)]
struct Args {
    output_root: PathBuf,
    evaluator_backend_label: String,
    m5_report_path: PathBuf,
    m5_bundle_path: PathBuf,
    cuda_report_path: PathBuf,
    cuda_bundle_path: PathBuf,
    admitted_home_summary_path: PathBuf,
    batch_size: usize,
}

#[derive(Clone, Debug, Serialize)]
struct QualityComparisonReport {
    schema_version: String,
    evaluator_backend_label: String,
    output_root: String,
    profile: PgolfishQualityProfile,
    evaluated_artifacts: Vec<EvaluatedArtifactReport>,
    admitted_home_run_context: AdmittedHomeRunContext,
    comparison: ComparisonSummary,
    claim_boundary: String,
}

#[derive(Clone, Debug, Serialize)]
struct PgolfishQualityProfile {
    hidden_size: usize,
    vocab_size: usize,
    lora_rank: usize,
    batch_size: usize,
    heldout_sample_count: usize,
    heldout_split: String,
    mostly_under_pgolf_constraints: Vec<String>,
    explicit_non_pgolf_boundaries: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct EvaluatedArtifactReport {
    artifact_id: String,
    artifact_kind: String,
    source_report_path: String,
    bundle_path: String,
    training_host: String,
    training_backend_label: String,
    training_steps_per_second: f64,
    training_source_tokens_per_second: f64,
    training_initial_mean_loss: f32,
    training_final_mean_loss: f32,
    heldout_mean_loss: f64,
    heldout_bits_per_token: f64,
    heldout_batch_count: usize,
    heldout_sample_count: usize,
    state_dict_digest: String,
}

#[derive(Clone, Debug, Serialize)]
struct AdmittedHomeRunContext {
    source_summary_path: String,
    run_id: String,
    result_classification: String,
    admitted_device_set: Vec<String>,
    accepted_contributions: u64,
    replay_checked_contributions: u64,
    publish_disposition: String,
    promotion_disposition: String,
    aggregate_training_final_mean_loss: f64,
    aggregate_estimated_steps_per_second: f64,
    per_device_contributions: Vec<AdmittedHomeDeviceContribution>,
    boundary_note: String,
}

#[derive(Clone, Debug, Serialize)]
struct AdmittedHomeDeviceContribution {
    node_id: String,
    runtime_role: String,
    execution_backend_label: String,
    estimated_steps_per_second: f64,
    final_mean_loss: f64,
    contribution_share: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ComparisonSummary {
    best_heldout_artifact_id: String,
    best_heldout_mean_loss: f64,
    fastest_same_node_training_artifact_id: String,
    fastest_same_node_steps_per_second: f64,
    same_node_quality_gaps_vs_best: Vec<QualityGap>,
}

#[derive(Clone, Debug, Serialize)]
struct QualityGap {
    artifact_id: String,
    heldout_mean_loss_gap: f64,
    heldout_bits_per_token_gap: f64,
}

#[derive(Clone, Debug, Deserialize)]
struct BenchmarkReport {
    host: String,
    backend_label: String,
    retained_run: RetainedRunReport,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedRunReport {
    steps_per_second: f64,
    source_tokens_per_second: f64,
    initial_mean_loss: f32,
    final_mean_loss: f32,
    final_state_dict_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TailrunAdmittedHomeSummary {
    run_id: String,
    result_classification: String,
    admitted_device_set: Vec<String>,
    accepted_contributions: u64,
    replay_checked_contributions: u64,
    publish_disposition: String,
    promotion_disposition: String,
    per_device_contributions: Vec<TailrunContributionSummary>,
}

#[derive(Clone, Debug, Deserialize)]
struct TailrunContributionSummary {
    node_id: String,
    runtime_role: String,
    execution_backend_label: String,
    estimated_steps_per_second: f64,
    final_mean_loss: f64,
    contribution_share: f64,
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

    let same_node_artifacts = vec![
        ArtifactInput {
            artifact_id: String::from("same_node_m5_mlx"),
            artifact_kind: String::from("same_node_retained_bundle"),
            report_path: args.m5_report_path.clone(),
            bundle_path: args.m5_bundle_path.clone(),
        },
        ArtifactInput {
            artifact_id: String::from("same_node_rtx4080_cuda"),
            artifact_kind: String::from("same_node_retained_bundle"),
            report_path: args.cuda_report_path.clone(),
            bundle_path: args.cuda_bundle_path.clone(),
        },
    ];

    let evaluated_artifacts = same_node_artifacts
        .iter()
        .map(|artifact| {
            evaluate_same_node_artifact(
                artifact,
                args.evaluator_backend_label.as_str(),
                args.batch_size,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let admitted_home_run_context =
        admitted_home_run_context(args.admitted_home_summary_path.as_path())?;

    let best_heldout = evaluated_artifacts
        .iter()
        .min_by(|left, right| left.heldout_mean_loss.total_cmp(&right.heldout_mean_loss))
        .ok_or("missing evaluated artifacts")?;
    let fastest_same_node = evaluated_artifacts
        .iter()
        .max_by(|left, right| {
            left.training_steps_per_second
                .total_cmp(&right.training_steps_per_second)
        })
        .ok_or("missing evaluated artifacts")?;
    let comparison = ComparisonSummary {
        best_heldout_artifact_id: best_heldout.artifact_id.clone(),
        best_heldout_mean_loss: best_heldout.heldout_mean_loss,
        fastest_same_node_training_artifact_id: fastest_same_node.artifact_id.clone(),
        fastest_same_node_steps_per_second: fastest_same_node.training_steps_per_second,
        same_node_quality_gaps_vs_best: evaluated_artifacts
            .iter()
            .map(|artifact| QualityGap {
                artifact_id: artifact.artifact_id.clone(),
                heldout_mean_loss_gap: artifact.heldout_mean_loss - best_heldout.heldout_mean_loss,
                heldout_bits_per_token_gap: artifact.heldout_bits_per_token
                    - best_heldout.heldout_bits_per_token,
            })
            .collect(),
    };

    let report = QualityComparisonReport {
        schema_version: String::from(REPORT_SCHEMA_VERSION),
        evaluator_backend_label: args.evaluator_backend_label.clone(),
        output_root: args.output_root.display().to_string(),
        profile: PgolfishQualityProfile {
            hidden_size: psionic_train::OPEN_ADAPTER_PGOLFISH_HIDDEN_SIZE,
            vocab_size: psionic_train::OPEN_ADAPTER_PGOLFISH_VOCAB_SIZE,
            lora_rank: psionic_train::OPEN_ADAPTER_PGOLFISH_LORA_RANK,
            batch_size: args.batch_size,
            heldout_sample_count: psionic_train::OPEN_ADAPTER_PGOLFISH_HOLDOUT_SAMPLE_COUNT,
            heldout_split: String::from(OpenAdapterPgolfishSampleSplit::Holdout.label()),
            mostly_under_pgolf_constraints: vec![
                String::from("bounded 10-minute training reports are compared against one fixed synthetic SP1024-like held-out profile"),
                String::from("the same hidden width, vocab width, LoRA rank, and tokenizer digest used in the admitted short-run benchmark are reused for evaluation"),
                String::from("quality is reported as held-out cross-entropy loss and bits-per-token instead of raw throughput alone"),
            ],
            explicit_non_pgolf_boundaries: vec![
                String::from("this lane evaluates open-adapter retained bundles, not full Parameter Golf challenge submission artifacts"),
                String::from("the admitted mixed-device swarm row is summary-backed training quality only because that run still has not emitted the same inferable promoted or near-equivalent runtime bundle path as the same-node M5 artifact"),
                String::from("no claim is made that the current held-out synthetic supervision set is a public contest dataset or exact PGOLF leaderboard protocol"),
            ],
        },
        evaluated_artifacts,
        admitted_home_run_context,
        comparison,
        claim_boundary: String::from(
            "This report honestly compares retained same-node home-device artifacts on one shared held-out PGOLF-ish profile and then places the admitted mixed-device Tailnet run beside them using its retained training summary only. It does not claim exact Parameter Golf parity, a promoted inference bundle for the mixed-device run, or open-internet decentralized training.",
        ),
    };

    let report_path = args.output_root.join("quality_report.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "open-adapter PGOLF-ish quality comparison completed: report={} best={} heldout_loss={:.6}",
        report_path.display(),
        report.comparison.best_heldout_artifact_id,
        report.comparison.best_heldout_mean_loss,
    );
    Ok(())
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut output_root = PathBuf::from(DEFAULT_OUTPUT_ROOT);
    let mut evaluator_backend_label = default_evaluator_backend_label();
    let mut m5_report_path = PathBuf::from(M5_REPORT_PATH);
    let mut m5_bundle_path = PathBuf::from(M5_BUNDLE_PATH);
    let mut cuda_report_path = PathBuf::from(CUDA_REPORT_PATH);
    let mut cuda_bundle_path = PathBuf::from(CUDA_BUNDLE_PATH);
    let mut admitted_home_summary_path = PathBuf::from(ADMITTED_HOME_SUMMARY_PATH);
    let mut batch_size = OPEN_ADAPTER_PGOLFISH_BATCH_SIZE;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output-root" => {
                output_root =
                    PathBuf::from(args.next().ok_or("missing value after --output-root")?);
            }
            "--evaluator-backend-label" => {
                evaluator_backend_label = args
                    .next()
                    .ok_or("missing value after --evaluator-backend-label")?;
            }
            "--m5-report" => {
                m5_report_path =
                    PathBuf::from(args.next().ok_or("missing value after --m5-report")?);
            }
            "--m5-bundle" => {
                m5_bundle_path =
                    PathBuf::from(args.next().ok_or("missing value after --m5-bundle")?);
            }
            "--cuda-report" => {
                cuda_report_path =
                    PathBuf::from(args.next().ok_or("missing value after --cuda-report")?);
            }
            "--cuda-bundle" => {
                cuda_bundle_path =
                    PathBuf::from(args.next().ok_or("missing value after --cuda-bundle")?);
            }
            "--admitted-home-summary" => {
                admitted_home_summary_path = PathBuf::from(
                    args.next()
                        .ok_or("missing value after --admitted-home-summary")?,
                );
            }
            "--batch-size" => {
                batch_size = args
                    .next()
                    .ok_or("missing value after --batch-size")?
                    .parse()?;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: open_adapter_pgolfish_quality_compare [--output-root <path>] [--evaluator-backend-label <label>] [--m5-report <path>] [--m5-bundle <path>] [--cuda-report <path>] [--cuda-bundle <path>] [--admitted-home-summary <path>] [--batch-size <size>]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    Ok(Args {
        output_root,
        evaluator_backend_label,
        m5_report_path,
        m5_bundle_path,
        cuda_report_path,
        cuda_bundle_path,
        admitted_home_summary_path,
        batch_size,
    })
}

#[derive(Clone, Debug)]
struct ArtifactInput {
    artifact_id: String,
    artifact_kind: String,
    report_path: PathBuf,
    bundle_path: PathBuf,
}

fn evaluate_same_node_artifact(
    artifact: &ArtifactInput,
    evaluator_backend_label: &str,
    batch_size: usize,
) -> Result<EvaluatedArtifactReport, Box<dyn std::error::Error>> {
    let benchmark_report: BenchmarkReport =
        serde_json::from_slice(&fs::read(artifact.report_path.as_path())?)?;
    let bundle_bytes = fs::read(artifact.bundle_path.as_path())?;
    let bundle = PortableModelBundle::import_safetensors(bundle_bytes.as_slice())?;
    let training_groups = bundle.to_training_groups()?;
    let (heldout_mean_loss, heldout_batch_count, heldout_sample_count) =
        evaluate_bundle_on_heldout_profile(
            artifact.artifact_id.as_str(),
            evaluator_backend_label,
            training_groups,
            batch_size,
        )?;

    Ok(EvaluatedArtifactReport {
        artifact_id: artifact.artifact_id.clone(),
        artifact_kind: artifact.artifact_kind.clone(),
        source_report_path: artifact.report_path.display().to_string(),
        bundle_path: artifact.bundle_path.display().to_string(),
        training_host: benchmark_report.host,
        training_backend_label: benchmark_report.backend_label,
        training_steps_per_second: benchmark_report.retained_run.steps_per_second,
        training_source_tokens_per_second: benchmark_report.retained_run.source_tokens_per_second,
        training_initial_mean_loss: benchmark_report.retained_run.initial_mean_loss,
        training_final_mean_loss: benchmark_report.retained_run.final_mean_loss,
        heldout_mean_loss,
        heldout_bits_per_token: heldout_mean_loss / std::f64::consts::LN_2,
        heldout_batch_count,
        heldout_sample_count,
        state_dict_digest: benchmark_report.retained_run.final_state_dict_digest,
    })
}

fn evaluate_bundle_on_heldout_profile(
    artifact_id: &str,
    evaluator_backend_label: &str,
    training_groups: Vec<psionic_train::TrainingParameterGroupState>,
    batch_size: usize,
) -> Result<(f64, usize, usize), Box<dyn std::error::Error>> {
    let config: OpenAdapterExecutionConfig = open_adapter_pgolfish_config_with_batch_size(
        evaluator_backend_label,
        format!("{artifact_id}-heldout-eval"),
        format!(
            "benchmark.open_adapter.pgolfish.{}.heldout_eval",
            artifact_id
        ),
        1,
        batch_size,
    )?;
    let holdout_samples =
        open_adapter_pgolfish_samples("pgolfish-heldout", OpenAdapterPgolfishSampleSplit::Holdout)?;
    let backend = OpenAdapterTrainingExecutionBackend::new(config.clone(), holdout_samples)?;
    let run = FixedBudgetTrainingRun::new(
        config.run_id,
        config.checkpoint_family,
        TrainingLoopBudget::new(1, 1, 1)?,
        training_groups,
    )?;

    let mut weighted_loss = 0.0_f64;
    let mut sample_count = 0_u64;
    for batch_index in 0..backend.batches().len() {
        let record = backend.produce_gradient_batch(&run, batch_index)?;
        weighted_loss += record.mean_loss as f64 * f64::from(record.training_batch.sample_count);
        sample_count += u64::from(record.training_batch.sample_count);
    }

    Ok((
        weighted_loss / sample_count.max(1) as f64,
        backend.batches().len(),
        sample_count as usize,
    ))
}

fn admitted_home_run_context(
    summary_path: &Path,
) -> Result<AdmittedHomeRunContext, Box<dyn std::error::Error>> {
    let summary: TailrunAdmittedHomeSummary = serde_json::from_slice(&fs::read(summary_path)?)?;
    let aggregate_training_final_mean_loss = if summary.per_device_contributions.is_empty() {
        0.0
    } else {
        summary
            .per_device_contributions
            .iter()
            .map(|device| device.final_mean_loss)
            .sum::<f64>()
            / summary.per_device_contributions.len() as f64
    };
    let aggregate_estimated_steps_per_second = summary
        .per_device_contributions
        .iter()
        .map(|device| device.estimated_steps_per_second * device.contribution_share)
        .sum::<f64>();

    Ok(AdmittedHomeRunContext {
        source_summary_path: summary_path.display().to_string(),
        run_id: summary.run_id,
        result_classification: summary.result_classification,
        admitted_device_set: summary.admitted_device_set,
        accepted_contributions: summary.accepted_contributions,
        replay_checked_contributions: summary.replay_checked_contributions,
        publish_disposition: summary.publish_disposition,
        promotion_disposition: summary.promotion_disposition,
        aggregate_training_final_mean_loss,
        aggregate_estimated_steps_per_second,
        per_device_contributions: summary
            .per_device_contributions
            .into_iter()
            .map(|device| AdmittedHomeDeviceContribution {
                node_id: device.node_id,
                runtime_role: device.runtime_role,
                execution_backend_label: device.execution_backend_label,
                estimated_steps_per_second: device.estimated_steps_per_second,
                final_mean_loss: device.final_mean_loss,
                contribution_share: device.contribution_share,
            })
            .collect(),
        boundary_note: String::from(
            "The admitted mixed-device Tailnet run is included here as retained training-summary context only. It still does not have the same inferable promoted or near-equivalent runtime bundle path as the same-node M5 artifact, so this row stays out of the held-out bundle-eval winner table.",
        ),
    })
}

fn default_evaluator_backend_label() -> String {
    if cfg!(target_os = "macos") {
        String::from(OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL)
    } else {
        String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL)
    }
}
