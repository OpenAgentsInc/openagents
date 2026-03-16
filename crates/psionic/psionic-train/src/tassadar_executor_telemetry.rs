use std::{collections::BTreeMap, fs, path::Path};

use psionic_data::{TassadarSequenceExample, TassadarSequenceSplit};
use psionic_eval::{
    EvalArtifact, TassadarExecutorLinearBenchmarkReport, build_tassadar_sudoku_v0_sequence_dataset,
};
use psionic_models::{
    TassadarExecutorTransformer, TassadarExecutorTransformerError, TassadarTraceTokenizer, TokenId,
    TokenSequence, TokenizerBoundary,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarExecutorCheckpointState, TassadarExecutorModelArtifact,
    TassadarExecutorReferenceRunBundle, TassadarExecutorRunError, TassadarExecutorTrainingReport,
};

/// Canonical training-telemetry artifact file for the first reference run.
pub const TASSADAR_EXECUTOR_TRAINING_TELEMETRY_FILE: &str = "training_telemetry.json";
/// Canonical exactness-curve artifact file for the first reference run.
pub const TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE: &str = "exactness_curve.json";
/// Canonical trace-divergence artifact file for the first reference run.
pub const TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE: &str = "trace_divergence_report.json";
/// Canonical failure-sample artifact file for the first reference run.
pub const TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE: &str = "failure_samples.json";

const TRAINING_MANIFEST_FILE: &str = "training_manifest.json";
const TRAINING_REPORT_FILE: &str = "training_report.json";
const LINEAR_BENCHMARK_REPORT_FILE: &str = "linear_benchmark_report.json";
const CHECKPOINT_STATE_FILE: &str = "checkpoint_state.json";
const CHECKPOINT_MANIFEST_FILE: &str = "checkpoint_manifest.json";
const MODEL_ARTIFACT_FILE: &str = "model_artifact.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const EXACTNESS_CURVE_BUCKET_COUNT: usize = 256;

/// Per-step telemetry derived from the persisted batch receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTrainingStepTelemetry {
    /// Zero-based global step index.
    pub global_step_index: u32,
    /// Epoch index for the step.
    pub epoch_index: u32,
    /// Frozen batch identifier.
    pub batch_id: String,
    /// Frozen sequence identifiers in the batch.
    pub sequence_ids: Vec<String>,
    /// Mean loss over the step.
    pub mean_loss_milli: i64,
    /// Supervised target tokens consumed by the step.
    pub target_token_count: u32,
    /// Cumulative target-token count through this step.
    pub cumulative_target_token_count: u32,
}

/// Per-epoch telemetry aggregated from the persisted batch receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorEpochTelemetry {
    /// Epoch index.
    pub epoch_index: u32,
    /// Number of steps in the epoch.
    pub step_count: u32,
    /// Mean loss over the epoch in milli-loss units.
    pub mean_loss_milli: i64,
    /// Total target tokens consumed by the epoch.
    pub target_token_count: u32,
}

/// Machine-readable training telemetry tied to the exact run identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTrainingTelemetryReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Frozen training-manifest digest.
    pub training_manifest_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Trained weight digest.
    pub trained_weight_digest: String,
    /// Checkpoint manifest digest tied to the run.
    pub checkpoint_manifest_digest: String,
    /// Per-step telemetry.
    pub step_reports: Vec<TassadarExecutorTrainingStepTelemetry>,
    /// Per-epoch telemetry.
    pub epoch_reports: Vec<TassadarExecutorEpochTelemetry>,
    /// Stable telemetry-report digest.
    pub report_digest: String,
}

impl TassadarExecutorTrainingTelemetryReport {
    fn new(
        run_bundle: &TassadarExecutorReferenceRunBundle,
        training_report: &TassadarExecutorTrainingReport,
        checkpoint_manifest_digest: &str,
    ) -> Self {
        let mut cumulative_target_tokens = 0_u32;
        let step_reports = training_report
            .batch_reports
            .iter()
            .enumerate()
            .map(|(index, batch)| {
                cumulative_target_tokens =
                    cumulative_target_tokens.saturating_add(batch.target_token_count);
                TassadarExecutorTrainingStepTelemetry {
                    global_step_index: index as u32,
                    epoch_index: batch.epoch_index,
                    batch_id: batch.batch_id.clone(),
                    sequence_ids: batch.sequence_ids.clone(),
                    mean_loss_milli: milli(batch.mean_loss),
                    target_token_count: batch.target_token_count,
                    cumulative_target_token_count: cumulative_target_tokens,
                }
            })
            .collect::<Vec<_>>();

        let mut epoch_accumulators = BTreeMap::<u32, (u32, f64, u32)>::new();
        for batch in &training_report.batch_reports {
            let entry = epoch_accumulators
                .entry(batch.epoch_index)
                .or_insert((0, 0.0, 0));
            entry.0 = entry.0.saturating_add(1);
            entry.1 += f64::from(batch.mean_loss) * f64::from(batch.target_token_count);
            entry.2 = entry.2.saturating_add(batch.target_token_count);
        }
        let epoch_reports = epoch_accumulators
            .into_iter()
            .map(
                |(epoch_index, (step_count, weighted_loss_sum, target_token_count))| {
                    let mean_loss = if target_token_count == 0 {
                        0.0
                    } else {
                        (weighted_loss_sum / f64::from(target_token_count)) as f32
                    };
                    TassadarExecutorEpochTelemetry {
                        epoch_index,
                        step_count,
                        mean_loss_milli: milli(mean_loss),
                        target_token_count,
                    }
                },
            )
            .collect::<Vec<_>>();

        let mut report = Self {
            run_id: run_bundle.run_id.clone(),
            dataset_storage_key: run_bundle.dataset_storage_key.clone(),
            dataset_digest: run_bundle.dataset_digest.clone(),
            training_manifest_digest: run_bundle.training_manifest_digest.clone(),
            trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
            trained_weight_digest: run_bundle.trained_weight_digest.clone(),
            checkpoint_manifest_digest: checkpoint_manifest_digest.to_string(),
            step_reports,
            epoch_reports,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_executor_training_telemetry_report|",
            &report,
        );
        report
    }
}

/// One point on an exactness curve over target-token position.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorExactnessCurvePoint {
    /// Inclusive zero-based target-token position where this bucket starts.
    pub target_index_start: u32,
    /// Exclusive zero-based target-token position where this bucket ends.
    pub target_index_end_exclusive: u32,
    /// Number of cases contributing at least one token to this bucket.
    pub evaluated_case_count: u32,
    /// Number of target tokens evaluated inside this bucket.
    pub evaluated_token_count: u32,
    /// Number of exact target tokens inside this bucket.
    pub exact_token_count: u32,
    /// Exact-token rate across all target tokens inside this bucket.
    pub exact_token_rate_bps: u32,
    /// Cases that remained prefix-exact through the full bucket.
    pub exact_prefix_case_count: u32,
    /// Prefix-exact rate through the end of this bucket.
    pub exact_prefix_rate_bps: u32,
}

/// One split-scoped exactness curve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorExactnessCurve {
    /// Split covered by the curve, or `null` for all splits combined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<TassadarSequenceSplit>,
    /// Position-wise curve points.
    pub points: Vec<TassadarExecutorExactnessCurvePoint>,
}

/// Exactness-curve artifact for the persisted run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorExactnessCurveReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Frozen training-manifest digest.
    pub training_manifest_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Checkpoint manifest digest tied to the run.
    pub checkpoint_manifest_digest: String,
    /// All exactness curves.
    pub curves: Vec<TassadarExecutorExactnessCurve>,
    /// Stable report digest.
    pub report_digest: String,
}

/// Divergence summary for one decoded case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTraceDivergenceCase {
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable case identifier.
    pub case_id: String,
    /// Dataset split.
    pub split: TassadarSequenceSplit,
    /// Number of reference target tokens.
    pub target_token_count: u32,
    /// Number of matched target tokens before the first failure.
    pub matched_target_token_count: u32,
    /// Target-token exactness over the whole suffix.
    pub target_token_exactness_bps: u32,
    /// First target-token index where divergence appeared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_divergence_index: Option<u32>,
    /// Symbolic reference token at the first divergence when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_divergence_token: Option<String>,
    /// Symbolic predicted token at the first divergence when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicted_divergence_token: Option<String>,
    /// Whether the full suffix stayed exact.
    pub exact_trace_match: bool,
    /// Whether final outputs matched.
    pub final_output_match: bool,
    /// Whether the halt marker matched.
    pub halt_match: bool,
    /// Reference target digest.
    pub reference_target_digest: String,
    /// Predicted target digest.
    pub predicted_target_digest: String,
}

/// Full divergence report for the persisted run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTraceDivergenceReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Checkpoint manifest digest tied to the run.
    pub checkpoint_manifest_digest: String,
    /// Per-case divergence reports.
    pub case_reports: Vec<TassadarExecutorTraceDivergenceCase>,
    /// Stable report digest.
    pub report_digest: String,
}

/// One failure sample with a small symbolic window around the divergence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorFailureSample {
    /// Stable case identifier.
    pub case_id: String,
    /// Dataset split.
    pub split: TassadarSequenceSplit,
    /// First divergence index when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_divergence_index: Option<u32>,
    /// Target-token exactness for the case.
    pub target_token_exactness_bps: u32,
    /// Whether final outputs matched.
    pub final_output_match: bool,
    /// Whether the halt marker matched.
    pub halt_match: bool,
    /// Reference token ids in the sampled divergence window.
    pub reference_window_token_ids: Vec<u32>,
    /// Predicted token ids in the sampled divergence window.
    pub predicted_window_token_ids: Vec<u32>,
    /// Symbolic reference tokens in the sampled divergence window.
    pub reference_window_tokens: Vec<String>,
    /// Symbolic predicted tokens in the sampled divergence window.
    pub predicted_window_tokens: Vec<String>,
}

/// Failure-sample artifact for later post-run review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorFailureSampleReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Checkpoint manifest digest tied to the run.
    pub checkpoint_manifest_digest: String,
    /// Failure samples sorted from weakest to strongest.
    pub samples: Vec<TassadarExecutorFailureSample>,
    /// Stable report digest.
    pub report_digest: String,
}

/// Errors while materializing telemetry for the persisted Tassadar run.
#[derive(Debug, Error)]
pub enum TassadarExecutorTelemetryError {
    /// Reusing the persisted run surfaces failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Model decode failed during analysis.
    #[error(transparent)]
    Model(#[from] TassadarExecutorTransformerError),
    /// Reading a persisted JSON artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// Path read.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding a persisted JSON artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Path read.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Serializing one telemetry artifact failed.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Writing one telemetry artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// Path written.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct ExampleAnalysis {
    case: TassadarExecutorTraceDivergenceCase,
    reference_target: Vec<TokenId>,
    predicted_target: Vec<TokenId>,
}

/// Generates the canonical telemetry and failure-analysis artifacts for the committed reference run.
pub fn augment_tassadar_reference_run_with_telemetry(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorTelemetryError> {
    let run_bundle: TassadarExecutorReferenceRunBundle = read_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
    )?;
    let _training_manifest: serde_json::Value = read_json(
        output_dir.join(TRAINING_MANIFEST_FILE),
        "tassadar_training_manifest",
    )?;
    let training_report: TassadarExecutorTrainingReport = read_json(
        output_dir.join(TRAINING_REPORT_FILE),
        "tassadar_training_report",
    )?;
    let _benchmark_report: TassadarExecutorLinearBenchmarkReport = read_json(
        output_dir.join(LINEAR_BENCHMARK_REPORT_FILE),
        "tassadar_linear_benchmark_report",
    )?;
    let checkpoint_state: TassadarExecutorCheckpointState = read_json(
        output_dir.join(CHECKPOINT_STATE_FILE),
        "tassadar_checkpoint_state",
    )?;
    let checkpoint_manifest: psionic_datastream::DatastreamManifest = read_json(
        output_dir.join(CHECKPOINT_MANIFEST_FILE),
        "tassadar_checkpoint_manifest",
    )?;
    let _model_artifact: TassadarExecutorModelArtifact = read_json(
        output_dir.join(MODEL_ARTIFACT_FILE),
        "tassadar_model_artifact",
    )?;
    let model = checkpoint_state.materialize_model()?;
    let dataset_bundle =
        build_tassadar_sudoku_v0_sequence_dataset(run_bundle.dataset_version.as_str())
            .map_err(crate::TassadarExecutorTrainingError::from)
            .map_err(TassadarExecutorRunError::from)?;
    let tokenizer = model.tokenizer().clone();
    let analyses = dataset_bundle
        .dataset
        .examples
        .iter()
        .map(|example| analyze_example(&model, &tokenizer, example))
        .collect::<Result<Vec<_>, TassadarExecutorTelemetryError>>()?;
    let checkpoint_manifest_digest = checkpoint_manifest.stable_digest();

    let training_telemetry = TassadarExecutorTrainingTelemetryReport::new(
        &run_bundle,
        &training_report,
        checkpoint_manifest_digest.as_str(),
    );
    let exactness_curve = build_exactness_curve_report(
        &run_bundle,
        checkpoint_manifest_digest.as_str(),
        analyses.as_slice(),
    );
    let trace_divergence = build_trace_divergence_report(
        &run_bundle,
        checkpoint_manifest_digest.as_str(),
        analyses.as_slice(),
    );
    let failure_samples = build_failure_sample_report(
        &run_bundle,
        checkpoint_manifest_digest.as_str(),
        &tokenizer,
        analyses.as_slice(),
    );

    let mut new_artifacts = Vec::new();
    new_artifacts.push(write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_TRAINING_TELEMETRY_FILE,
        "tassadar_training_telemetry",
        &training_telemetry,
    )?);
    new_artifacts.push(write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE,
        "tassadar_exactness_curve",
        &exactness_curve,
    )?);
    new_artifacts.push(write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE,
        "tassadar_trace_divergence_report",
        &trace_divergence,
    )?);
    new_artifacts.push(write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE,
        "tassadar_failure_samples",
        &failure_samples,
    )?);

    let mut artifact_map = run_bundle
        .artifacts
        .iter()
        .cloned()
        .map(|artifact| (artifact.artifact_ref.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    for artifact in new_artifacts {
        artifact_map.insert(artifact.artifact_ref.clone(), artifact);
    }
    let artifacts = artifact_map.into_values().collect::<Vec<_>>();
    let updated_bundle = refresh_bundle_with_artifacts(run_bundle, artifacts);
    write_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
        &updated_bundle,
    )?;
    Ok(updated_bundle)
}

fn build_exactness_curve_report(
    run_bundle: &TassadarExecutorReferenceRunBundle,
    checkpoint_manifest_digest: &str,
    analyses: &[ExampleAnalysis],
) -> TassadarExecutorExactnessCurveReport {
    let splits = [None]
        .into_iter()
        .chain([
            Some(TassadarSequenceSplit::Train),
            Some(TassadarSequenceSplit::Validation),
            Some(TassadarSequenceSplit::Test),
        ])
        .collect::<Vec<_>>();
    let curves = splits
        .into_iter()
        .map(|split| build_curve_for_split(split, analyses))
        .collect::<Vec<_>>();
    let mut report = TassadarExecutorExactnessCurveReport {
        run_id: run_bundle.run_id.clone(),
        dataset_storage_key: run_bundle.dataset_storage_key.clone(),
        dataset_digest: run_bundle.dataset_digest.clone(),
        training_manifest_digest: run_bundle.training_manifest_digest.clone(),
        trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
        checkpoint_manifest_digest: checkpoint_manifest_digest.to_string(),
        curves,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_executor_exactness_curve_report|",
        &report,
    );
    report
}

fn build_curve_for_split(
    split: Option<TassadarSequenceSplit>,
    analyses: &[ExampleAnalysis],
) -> TassadarExecutorExactnessCurve {
    let relevant = analyses
        .iter()
        .filter(|analysis| split.is_none_or(|value| analysis.case.split == value))
        .collect::<Vec<_>>();
    let max_target_tokens = relevant
        .iter()
        .map(|analysis| analysis.reference_target.len())
        .max()
        .unwrap_or(0);
    let bucket_width = max_target_tokens
        .div_ceil(EXACTNESS_CURVE_BUCKET_COUNT)
        .max(1);
    let bucket_count = max_target_tokens.div_ceil(bucket_width);
    let mut points = Vec::with_capacity(bucket_count);
    for bucket_index in 0..bucket_count {
        let start = bucket_index * bucket_width;
        let end = (start + bucket_width).min(max_target_tokens);
        let mut evaluated_case_count = 0_u32;
        let mut evaluated_token_count = 0_u32;
        let mut exact_token_count = 0_u32;
        let mut exact_prefix_case_count = 0_u32;
        for analysis in &relevant {
            if start >= analysis.reference_target.len() {
                continue;
            }
            evaluated_case_count = evaluated_case_count.saturating_add(1);
            for target_index in start..end.min(analysis.reference_target.len()) {
                evaluated_token_count = evaluated_token_count.saturating_add(1);
                if analysis
                    .predicted_target
                    .get(target_index)
                    .is_some_and(|token| *token == analysis.reference_target[target_index])
                {
                    exact_token_count = exact_token_count.saturating_add(1);
                }
            }
            if analysis.case.first_divergence_index.is_none()
                || analysis
                    .case
                    .first_divergence_index
                    .is_some_and(|index| index >= end as u32)
            {
                exact_prefix_case_count = exact_prefix_case_count.saturating_add(1);
            }
        }
        points.push(TassadarExecutorExactnessCurvePoint {
            target_index_start: start as u32,
            target_index_end_exclusive: end as u32,
            evaluated_case_count,
            evaluated_token_count,
            exact_token_count,
            exact_token_rate_bps: rate_bps(exact_token_count, evaluated_token_count),
            exact_prefix_case_count,
            exact_prefix_rate_bps: rate_bps(exact_prefix_case_count, evaluated_case_count),
        });
    }
    TassadarExecutorExactnessCurve { split, points }
}

fn build_trace_divergence_report(
    run_bundle: &TassadarExecutorReferenceRunBundle,
    checkpoint_manifest_digest: &str,
    analyses: &[ExampleAnalysis],
) -> TassadarExecutorTraceDivergenceReport {
    let mut report = TassadarExecutorTraceDivergenceReport {
        run_id: run_bundle.run_id.clone(),
        dataset_storage_key: run_bundle.dataset_storage_key.clone(),
        dataset_digest: run_bundle.dataset_digest.clone(),
        trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
        checkpoint_manifest_digest: checkpoint_manifest_digest.to_string(),
        case_reports: analyses
            .iter()
            .map(|analysis| analysis.case.clone())
            .collect(),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_executor_trace_divergence_report|",
        &report,
    );
    report
}

fn build_failure_sample_report(
    run_bundle: &TassadarExecutorReferenceRunBundle,
    checkpoint_manifest_digest: &str,
    tokenizer: &TassadarTraceTokenizer,
    analyses: &[ExampleAnalysis],
) -> TassadarExecutorFailureSampleReport {
    let mut samples = analyses
        .iter()
        .filter(|analysis| {
            !analysis.case.exact_trace_match
                || !analysis.case.final_output_match
                || !analysis.case.halt_match
        })
        .map(|analysis| failure_sample(tokenizer, analysis))
        .collect::<Vec<_>>();
    samples.sort_by(|left, right| {
        left.target_token_exactness_bps
            .cmp(&right.target_token_exactness_bps)
            .then(
                left.first_divergence_index
                    .cmp(&right.first_divergence_index),
            )
            .then(left.case_id.cmp(&right.case_id))
    });
    let mut report = TassadarExecutorFailureSampleReport {
        run_id: run_bundle.run_id.clone(),
        dataset_storage_key: run_bundle.dataset_storage_key.clone(),
        dataset_digest: run_bundle.dataset_digest.clone(),
        trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
        checkpoint_manifest_digest: checkpoint_manifest_digest.to_string(),
        samples,
        report_digest: String::new(),
    };
    report.report_digest =
        stable_digest(b"psionic_tassadar_executor_failure_sample_report|", &report);
    report
}

fn analyze_example(
    model: &TassadarExecutorTransformer,
    tokenizer: &TassadarTraceTokenizer,
    example: &TassadarSequenceExample,
) -> Result<ExampleAnalysis, TassadarExecutorTelemetryError> {
    let prompt_len = example.metadata.prompt_token_count as usize;
    let prompt = TokenSequence::new(
        example.token_ids[..prompt_len]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>(),
    );
    let reference_target = example.token_ids[prompt_len..]
        .iter()
        .map(|token| TokenId(*token))
        .collect::<Vec<_>>();
    let predicted_target = greedy_decode_target(model, prompt, reference_target.len())?;
    let first_divergence_index =
        first_divergence(reference_target.as_slice(), predicted_target.as_slice());
    let matched_target_token_count =
        first_divergence_index.unwrap_or(reference_target.len() as u32);
    let reference_full = example
        .token_ids
        .iter()
        .map(|token| TokenId(*token))
        .collect::<Vec<_>>();
    let predicted_full = example.token_ids[..prompt_len]
        .iter()
        .map(|token| TokenId(*token))
        .chain(predicted_target.iter().copied())
        .collect::<Vec<_>>();
    let case = TassadarExecutorTraceDivergenceCase {
        sequence_id: example.sequence_id.clone(),
        case_id: example.metadata.case_id.clone(),
        split: example.metadata.split,
        target_token_count: reference_target.len() as u32,
        matched_target_token_count,
        target_token_exactness_bps: rate_bps(
            matched_token_count(reference_target.as_slice(), predicted_target.as_slice()) as u32,
            reference_target.len() as u32,
        ),
        first_divergence_index,
        reference_divergence_token: first_divergence_index
            .and_then(|index| reference_target.get(index as usize))
            .map(|token| token_symbol(tokenizer, *token)),
        predicted_divergence_token: first_divergence_index
            .and_then(|index| predicted_target.get(index as usize))
            .map(|token| token_symbol(tokenizer, *token)),
        exact_trace_match: reference_target == predicted_target,
        final_output_match: tokenizer.extract_output_values(reference_full.as_slice())
            == tokenizer.extract_output_values(predicted_full.as_slice()),
        halt_match: tokenizer.extract_halt_marker(reference_full.as_slice())
            == tokenizer.extract_halt_marker(predicted_full.as_slice()),
        reference_target_digest: stable_digest(
            b"psionic_tassadar_executor_trace_divergence_reference_target|",
            &reference_target
                .iter()
                .map(|token| token.as_u32())
                .collect::<Vec<_>>(),
        ),
        predicted_target_digest: stable_digest(
            b"psionic_tassadar_executor_trace_divergence_predicted_target|",
            &predicted_target
                .iter()
                .map(|token| token.as_u32())
                .collect::<Vec<_>>(),
        ),
    };
    Ok(ExampleAnalysis {
        case,
        reference_target,
        predicted_target,
    })
}

fn failure_sample(
    tokenizer: &TassadarTraceTokenizer,
    analysis: &ExampleAnalysis,
) -> TassadarExecutorFailureSample {
    let center = analysis
        .case
        .first_divergence_index
        .unwrap_or_else(|| analysis.reference_target.len().saturating_sub(1) as u32)
        as usize;
    let start = center.saturating_sub(4);
    let end = center.saturating_add(5).max(start + 1);
    let reference_window = analysis.reference_target
        [start.min(analysis.reference_target.len())..end.min(analysis.reference_target.len())]
        .to_vec();
    let predicted_window = analysis.predicted_target
        [start.min(analysis.predicted_target.len())..end.min(analysis.predicted_target.len())]
        .to_vec();
    TassadarExecutorFailureSample {
        case_id: analysis.case.case_id.clone(),
        split: analysis.case.split,
        first_divergence_index: analysis.case.first_divergence_index,
        target_token_exactness_bps: analysis.case.target_token_exactness_bps,
        final_output_match: analysis.case.final_output_match,
        halt_match: analysis.case.halt_match,
        reference_window_token_ids: reference_window
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        predicted_window_token_ids: predicted_window
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        reference_window_tokens: reference_window
            .iter()
            .map(|token| token_symbol(tokenizer, *token))
            .collect(),
        predicted_window_tokens: predicted_window
            .iter()
            .map(|token| token_symbol(tokenizer, *token))
            .collect(),
    }
}

fn refresh_bundle_with_artifacts(
    mut bundle: TassadarExecutorReferenceRunBundle,
    artifacts: Vec<EvalArtifact>,
) -> TassadarExecutorReferenceRunBundle {
    bundle.artifacts = artifacts;
    bundle.bundle_digest.clear();
    bundle.bundle_digest =
        stable_digest(b"psionic_tassadar_executor_reference_run_bundle|", &bundle);
    bundle
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarExecutorTelemetryError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarExecutorTelemetryError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarExecutorTelemetryError::Deserialize {
        artifact_kind: artifact_kind.to_string(),
        path: path.display().to_string(),
        error,
    })
}

fn write_json_artifact<T>(
    output_dir: &Path,
    relative_path: &str,
    artifact_kind: &str,
    value: &T,
) -> Result<EvalArtifact, TassadarExecutorTelemetryError>
where
    T: Serialize,
{
    let path = output_dir.join(relative_path);
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorTelemetryError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(&path, &bytes).map_err(|error| TassadarExecutorTelemetryError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(EvalArtifact::new(
        artifact_kind,
        relative_path,
        bytes.as_slice(),
    ))
}

fn write_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
    value: &T,
) -> Result<(), TassadarExecutorTelemetryError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorTelemetryError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, &bytes).map_err(|error| TassadarExecutorTelemetryError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn greedy_decode_target(
    model: &TassadarExecutorTransformer,
    prompt: TokenSequence,
    target_token_count: usize,
) -> Result<Vec<TokenId>, TassadarExecutorTransformerError> {
    let mut state = model.start_decode(prompt)?;
    let mut predicted = Vec::with_capacity(target_token_count);
    for _ in 0..target_token_count {
        let next = model.greedy_next_token(&state)?;
        state.prefix.push(next);
        state.next_position = state.next_position.saturating_add(1);
        predicted.push(next);
        if next == model.tokenizer().vocabulary().eos_id() {
            break;
        }
    }
    Ok(predicted)
}

fn first_divergence(reference: &[TokenId], predicted: &[TokenId]) -> Option<u32> {
    for (index, (left, right)) in reference.iter().zip(predicted.iter()).enumerate() {
        if left != right {
            return Some(index as u32);
        }
    }
    if reference.len() == predicted.len() {
        None
    } else {
        Some(reference.len().min(predicted.len()) as u32)
    }
}

fn matched_token_count(reference: &[TokenId], predicted: &[TokenId]) -> usize {
    reference
        .iter()
        .zip(predicted.iter())
        .filter(|(left, right)| left == right)
        .count()
}

fn rate_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        return 10_000;
    }
    ((numerator as f64 / denominator as f64) * 10_000.0).round() as u32
}

fn token_symbol(tokenizer: &TassadarTraceTokenizer, token: TokenId) -> String {
    tokenizer
        .vocabulary()
        .token(token)
        .unwrap_or("<unk>")
        .to_string()
}

fn milli(value: f32) -> i64 {
    (value as f64 * 1000.0).round() as i64
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar executor telemetry value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{
        TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR, execute_tassadar_training_run,
        tassadar_executor_reference_run_config,
    };

    use super::{
        TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE, TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE,
        TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE, TASSADAR_EXECUTOR_TRAINING_TELEMETRY_FILE,
        augment_tassadar_reference_run_with_telemetry,
    };

    #[test]
    fn telemetry_augmentation_writes_analysis_artifacts_and_updates_run_bundle()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        execute_tassadar_training_run(
            temp.path(),
            &tassadar_executor_reference_run_config(),
            Some(psionic_data::TassadarSequenceSplit::Validation),
        )?;
        let bundle = augment_tassadar_reference_run_with_telemetry(temp.path())?;

        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_TRAINING_TELEMETRY_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE)
                .exists()
        );
        assert!(
            bundle
                .artifacts
                .iter()
                .any(|artifact| artifact.artifact_ref == TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE)
        );
        assert!(!bundle.bundle_digest.is_empty());
        Ok(())
    }

    #[test]
    fn reference_run_output_dir_constant_stays_repo_relative() {
        assert_eq!(
            crate::TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR,
            TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR
        );
    }
}
