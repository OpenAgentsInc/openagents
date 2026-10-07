use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{TassadarSequenceExample, TassadarSequenceSplit};
use psionic_eval::{TassadarExecutorArchitectureFamilyReport, build_tassadar_sequence_dataset};
use psionic_models::{
    TassadarExecutorAttentionTransformer, TassadarTraceTokenizer, TokenId, TokenSequence,
    TokenizerBoundary,
};
use psionic_runtime::TassadarClaimClass;
use psionic_train::{
    TASSADAR_EXECUTOR_BEST_CHECKPOINT_MANIFEST_FILE, TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE,
    TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE, TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE,
    TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE, TassadarExecutorBestCheckpointManifest,
    TassadarExecutorCheckpointExactnessPoint, TassadarExecutorExactTraceSample,
    TassadarExecutorExactnessCurve, TassadarExecutorExactnessCurvePoint,
    TassadarExecutorFailureSample, TassadarExecutorPromotionGateFailure,
    TassadarExecutorPromotionGateFailureKind, TassadarExecutorPromotionGateReport,
    TassadarExecutorTraceDivergenceCase,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_EXECUTOR_ATTENTION_CHECKPOINT_STATE_FILE,
    TASSADAR_EXECUTOR_ATTENTION_FAMILY_REPORT_FILE,
    TASSADAR_EXECUTOR_ATTENTION_MODEL_DESCRIPTOR_FILE, TASSADAR_EXECUTOR_ATTENTION_RUN_BUNDLE_FILE,
    TASSADAR_EXECUTOR_ATTENTION_TRAINING_REPORT_FILE, TassadarExecutorAttentionCheckpointState,
    TassadarExecutorAttentionRunBundle, TassadarExecutorAttentionTrainingConfig,
    TassadarExecutorAttentionTrainingError, TassadarExecutorAttentionTrainingReport,
    TassadarExecutorAttentionTrainingStage, run_tassadar_executor_attention_training_with_config,
};

const V9_CHECKPOINT_STATE_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_attention_boundary_v9/checkpoint_state.json";
const V9_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_attention_boundary_v9/run_bundle.json";
const BOOTSTRAP_DIR_NAME: &str = "bootstrap_pc_boundary";
const BOOTSTRAP_RUN_ID: &str = "tassadar-executor-attention-sudoku-v0-promotion-v3-bootstrap";
const PROMOTION_RUN_ID: &str = "tassadar-executor-attention-sudoku-v0-promotion-v3";
const PROMOTION_BUNDLE_FILE: &str = "promotion_bundle.json";
const REQUIRED_FIRST_TARGET_EXACTNESS_BPS: u32 = 10_000;
const REQUIRED_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE: u32 = 9_000;
const REQUIRED_EXACT_TRACE_CASE_COUNT: u32 = 1;
const EXACTNESS_CURVE_BUCKET_COUNT: usize = 256;

const fn default_learned_bounded_claim_class() -> TassadarClaimClass {
    TassadarClaimClass::LearnedBounded
}

/// Canonical output root for the green learned 4x4 promotion bundle.
pub const TASSADAR_EXECUTOR_ATTENTION_PROMOTION_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/sudoku_v0_promotion_v3";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionPromotionRunBundle {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Coarse claim class. The promotion remains bounded even when the
    /// validation gate is green.
    #[serde(default = "default_learned_bounded_claim_class")]
    pub claim_class: TassadarClaimClass,
    /// Relative output directory.
    pub output_dir: String,
    /// Relative bootstrap run bundle used for initialization.
    pub bootstrap_run_bundle_ref: String,
    /// Relative final training report artifact.
    pub training_report_file: String,
    /// Relative final validation family report artifact.
    pub family_report_file: String,
    /// Relative selected checkpoint state artifact.
    pub checkpoint_state_file: String,
    /// Relative selected model descriptor artifact.
    pub model_descriptor_file: String,
    /// Relative best-checkpoint manifest artifact.
    pub best_checkpoint_manifest_file: String,
    /// Relative exactness-curve artifact.
    pub exactness_curve_file: String,
    /// Relative failure-sample artifact.
    pub failure_samples_file: String,
    /// Relative exact-trace-sample artifact.
    pub exact_trace_samples_file: String,
    /// Relative promotion-gate report artifact.
    pub promotion_gate_report_file: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl TassadarExecutorAttentionPromotionRunBundle {
    fn new(
        run_bundle: &TassadarExecutorAttentionRunBundle,
        output_dir: &Path,
        bootstrap_run_bundle_ref: String,
    ) -> Self {
        let mut bundle = Self {
            run_id: run_bundle.run_id.clone(),
            model_id: run_bundle.model_id.clone(),
            claim_class: TassadarClaimClass::LearnedBounded,
            output_dir: repo_relative_path(output_dir),
            bootstrap_run_bundle_ref,
            training_report_file: String::from(TASSADAR_EXECUTOR_ATTENTION_TRAINING_REPORT_FILE),
            family_report_file: String::from(TASSADAR_EXECUTOR_ATTENTION_FAMILY_REPORT_FILE),
            checkpoint_state_file: String::from(TASSADAR_EXECUTOR_ATTENTION_CHECKPOINT_STATE_FILE),
            model_descriptor_file: String::from(TASSADAR_EXECUTOR_ATTENTION_MODEL_DESCRIPTOR_FILE),
            best_checkpoint_manifest_file: String::from(
                TASSADAR_EXECUTOR_BEST_CHECKPOINT_MANIFEST_FILE,
            ),
            exactness_curve_file: String::from(TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE),
            failure_samples_file: String::from(TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE),
            exact_trace_samples_file: String::from(TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE),
            promotion_gate_report_file: String::from(TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_executor_attention_promotion_run_bundle|",
            &bundle,
        );
        bundle
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionPromotionExactnessCurveReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Stable digest of the selected checkpoint state.
    pub checkpoint_state_digest: String,
    /// Validation exactness over persisted checkpoints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoint_curve: Vec<TassadarExecutorCheckpointExactnessPoint>,
    /// Position-wise exactness curves.
    pub curves: Vec<TassadarExecutorExactnessCurve>,
    /// Stable report digest.
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionPromotionFailureSampleReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Stable digest of the selected checkpoint state.
    pub checkpoint_state_digest: String,
    /// Remaining validation misses.
    pub samples: Vec<TassadarExecutorFailureSample>,
    /// Stable report digest.
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionPromotionExactTraceSampleReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Stable digest of the selected checkpoint state.
    pub checkpoint_state_digest: String,
    /// Number of exact validation traces preserved in the artifact.
    pub exact_trace_case_count: u32,
    /// Exact validation trace samples.
    pub samples: Vec<TassadarExecutorExactTraceSample>,
    /// Stable report digest.
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq)]
struct AttentionExampleAnalysis {
    case: TassadarExecutorTraceDivergenceCase,
    prompt_token_count: u32,
    reference_full: Vec<TokenId>,
    reference_target: Vec<TokenId>,
    predicted_target: Vec<TokenId>,
}

#[derive(Debug, Error)]
pub enum TassadarExecutorAttentionPromotionError {
    #[error(transparent)]
    Training(#[from] TassadarExecutorAttentionTrainingError),
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        artifact_kind: String,
        error: serde_json::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(
        "best checkpoint `{checkpoint_id}` is missing from the persisted attention training report"
    )]
    MissingBestCheckpoint { checkpoint_id: String },
}

/// Returns the bootstrap config that solves the `<pc>` boundary from the
/// preserved `v9` checkpoint.
#[must_use]
pub fn tassadar_attention_promotion_bootstrap_reference() -> TassadarExecutorAttentionTrainingConfig
{
    let mut config = TassadarExecutorAttentionTrainingConfig::boundary_curriculum_reference();
    config.run_id = String::from(BOOTSTRAP_RUN_ID);
    config.learning_rate = 0.02;
    config.stages = vec![
        TassadarExecutorAttentionTrainingStage::new("prompt_to_first_8_tokens", 8, 8)
            .with_learning_rate_scale(1.0)
            .with_first_target_loss_scale(12.0)
            .with_early_prefix_weighting(8, 12.0),
    ];
    config.initial_checkpoint_state_ref = Some(String::from(V9_CHECKPOINT_STATE_REF));
    config.initial_run_bundle_ref = Some(String::from(V9_RUN_BUNDLE_REF));
    config.relative_target_output_bias_learning_rate_scale = 128.0;
    config.relative_target_output_projection_learning_rate_scale = 2.0;
    config.relative_target_transition_output_bias_learning_rate_scale = 8.0;
    config.relative_target_trace_schema_output_bias_learning_rate_scale = 32.0;
    config
}

/// Returns the final promotion config that finishes the early learned trace.
#[must_use]
pub fn tassadar_attention_promotion_reference(
    bootstrap_checkpoint_state_ref: impl Into<String>,
    bootstrap_run_bundle_ref: impl Into<String>,
) -> TassadarExecutorAttentionTrainingConfig {
    let mut config = TassadarExecutorAttentionTrainingConfig::boundary_curriculum_reference();
    config.run_id = String::from(PROMOTION_RUN_ID);
    config.learning_rate = 0.05;
    config.stages = vec![
        TassadarExecutorAttentionTrainingStage::new("prompt_to_first_32_tokens", 32, 16)
            .with_learning_rate_scale(1.0)
            .with_first_target_loss_scale(8.0)
            .with_early_prefix_weighting(16, 8.0),
    ];
    config.initial_checkpoint_state_ref = Some(bootstrap_checkpoint_state_ref.into());
    config.initial_run_bundle_ref = Some(bootstrap_run_bundle_ref.into());
    config.train_relative_target_output_projection = false;
    config.train_relative_target_transition_output_bias = false;
    config.relative_target_output_bias_learning_rate_scale = 512.0;
    config.relative_target_trace_schema_output_bias_learning_rate_scale = 256.0;
    config
}

/// Executes the canonical learned 4x4 attention promotion run and persists the
/// promotion artifacts required by issue `#7`.
pub fn run_tassadar_executor_attention_promotion(
    output_dir: &Path,
) -> Result<TassadarExecutorAttentionPromotionRunBundle, TassadarExecutorAttentionPromotionError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarExecutorAttentionPromotionError::Write {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let bootstrap_dir = output_dir.join(BOOTSTRAP_DIR_NAME);
    let _bootstrap_bundle = run_tassadar_executor_attention_training_with_config(
        &bootstrap_dir,
        &tassadar_attention_promotion_bootstrap_reference(),
    )?;
    let bootstrap_checkpoint_ref =
        repo_relative_path(&bootstrap_dir.join(TASSADAR_EXECUTOR_ATTENTION_CHECKPOINT_STATE_FILE));
    let bootstrap_run_bundle_ref =
        repo_relative_path(&bootstrap_dir.join(TASSADAR_EXECUTOR_ATTENTION_RUN_BUNDLE_FILE));
    let _final_bundle = run_tassadar_executor_attention_training_with_config(
        output_dir,
        &tassadar_attention_promotion_reference(
            bootstrap_checkpoint_ref,
            bootstrap_run_bundle_ref.clone(),
        ),
    )?;
    augment_tassadar_executor_attention_promotion_artifacts(output_dir, bootstrap_run_bundle_ref)
}

/// Generates the promotion-review artifacts for one persisted attention run.
pub fn augment_tassadar_executor_attention_promotion_artifacts(
    output_dir: &Path,
    bootstrap_run_bundle_ref: String,
) -> Result<TassadarExecutorAttentionPromotionRunBundle, TassadarExecutorAttentionPromotionError> {
    let run_bundle: TassadarExecutorAttentionRunBundle = read_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_RUN_BUNDLE_FILE),
        "tassadar_executor_attention_run_bundle",
    )?;
    let training_report: TassadarExecutorAttentionTrainingReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_TRAINING_REPORT_FILE),
        "tassadar_executor_attention_training_report",
    )?;
    let family_report: TassadarExecutorArchitectureFamilyReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_FAMILY_REPORT_FILE),
        "tassadar_executor_attention_family_report",
    )?;
    let checkpoint_state: TassadarExecutorAttentionCheckpointState = read_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_CHECKPOINT_STATE_FILE),
        "tassadar_executor_attention_checkpoint_state",
    )?;
    let model = checkpoint_state.materialize_model()?;
    let tokenizer = model.tokenizer().clone();
    let dataset_bundle = build_tassadar_sequence_dataset(
        psionic_eval::TassadarSequenceWorkload::SudokuV0,
        training_report.config.dataset_version.as_str(),
    )
    .map_err(TassadarExecutorAttentionTrainingError::from)?;
    dataset_bundle
        .dataset
        .validate()
        .map_err(TassadarExecutorAttentionTrainingError::from)?;
    let analyses = dataset_bundle
        .dataset
        .examples
        .iter()
        .map(|example| {
            analyze_example(
                &model,
                &tokenizer,
                example,
                family_report.prompt_window_token_cap as usize,
                family_report.target_token_cap as usize,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let best_epoch = training_report
        .epoch_reports
        .iter()
        .find(|epoch| epoch.checkpoint_id == training_report.best_checkpoint_id)
        .ok_or_else(
            || TassadarExecutorAttentionPromotionError::MissingBestCheckpoint {
                checkpoint_id: training_report.best_checkpoint_id.clone(),
            },
        )?;
    let dataset_storage_key = dataset_bundle.dataset.storage_key().to_string();
    let dataset_digest = dataset_bundle.dataset.stable_digest();
    let checkpoint_state_digest = checkpoint_state.state_digest.clone();

    let best_checkpoint_manifest =
        build_best_checkpoint_manifest(&run_bundle, &training_report, &family_report, best_epoch);
    let exactness_curve = build_exactness_curve_report(
        &run_bundle,
        &training_report,
        &family_report,
        checkpoint_state_digest.as_str(),
        dataset_storage_key.as_str(),
        dataset_digest.as_str(),
        analyses.as_slice(),
    );
    let failure_samples = build_failure_sample_report(
        &run_bundle,
        &family_report,
        &tokenizer,
        checkpoint_state_digest.as_str(),
        dataset_storage_key.as_str(),
        dataset_digest.as_str(),
        analyses.as_slice(),
    );
    let exact_trace_samples = build_exact_trace_sample_report(
        &run_bundle,
        &family_report,
        &tokenizer,
        checkpoint_state_digest.as_str(),
        dataset_storage_key.as_str(),
        dataset_digest.as_str(),
        analyses.as_slice(),
    );
    let promotion_gate_report = build_promotion_gate_report(
        &run_bundle,
        &training_report,
        &family_report,
        best_epoch.stage_id.as_str(),
    );
    let promotion_bundle = TassadarExecutorAttentionPromotionRunBundle::new(
        &run_bundle,
        output_dir,
        bootstrap_run_bundle_ref,
    );

    write_json(
        output_dir.join(TASSADAR_EXECUTOR_BEST_CHECKPOINT_MANIFEST_FILE),
        "tassadar_executor_attention_best_checkpoint_manifest",
        &best_checkpoint_manifest,
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE),
        "tassadar_executor_attention_exactness_curve",
        &exactness_curve,
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE),
        "tassadar_executor_attention_failure_samples",
        &failure_samples,
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE),
        "tassadar_executor_attention_exact_trace_samples",
        &exact_trace_samples,
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE),
        "tassadar_executor_attention_promotion_gate_report",
        &promotion_gate_report,
    )?;
    write_json(
        output_dir.join(PROMOTION_BUNDLE_FILE),
        "tassadar_executor_attention_promotion_bundle",
        &promotion_bundle,
    )?;
    Ok(promotion_bundle)
}

fn build_best_checkpoint_manifest(
    run_bundle: &TassadarExecutorAttentionRunBundle,
    training_report: &TassadarExecutorAttentionTrainingReport,
    family_report: &TassadarExecutorArchitectureFamilyReport,
    best_epoch: &crate::TassadarExecutorAttentionTrainingEpochReport,
) -> TassadarExecutorBestCheckpointManifest {
    let mut manifest = TassadarExecutorBestCheckpointManifest {
        run_id: run_bundle.run_id.clone(),
        trainable_surface: String::from(
            "executor_attention_candidate.relative_target_output_bias_plus_trace_schema_bias",
        ),
        checkpoint_id: training_report.best_checkpoint_id.clone(),
        checkpoint_family: String::from("tassadar_executor_attention_checkpoint_state"),
        checkpoint_ref: format!("checkpoint_state://{}", training_report.best_checkpoint_id),
        selection_basis: String::from("attention_validation_correctness_rank_then_mean_loss"),
        selected_stage_id: best_epoch.stage_id.clone(),
        global_epoch_index: best_epoch.epoch_index,
        first_target_exactness_bps: family_report.correctness.first_target_exactness_bps,
        first_32_token_exactness_bps: family_report.correctness.first_32_token_exactness_bps,
        exact_trace_case_count: family_report.correctness.exact_trace_case_count,
        checkpoint_artifact_ref: String::from(TASSADAR_EXECUTOR_ATTENTION_CHECKPOINT_STATE_FILE),
        model_artifact_ref: String::from(TASSADAR_EXECUTOR_ATTENTION_MODEL_DESCRIPTOR_FILE),
        exactness_curve_ref: String::from(TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE),
        failure_samples_ref: String::from(TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE),
        exact_trace_samples_ref: String::from(TASSADAR_EXECUTOR_EXACT_TRACE_SAMPLES_FILE),
        report_digest: String::new(),
    };
    manifest.report_digest = stable_digest(
        b"psionic_tassadar_executor_best_checkpoint_manifest|",
        &manifest,
    );
    manifest
}

fn build_promotion_gate_report(
    run_bundle: &TassadarExecutorAttentionRunBundle,
    training_report: &TassadarExecutorAttentionTrainingReport,
    family_report: &TassadarExecutorArchitectureFamilyReport,
    selected_stage_id: &str,
) -> TassadarExecutorPromotionGateReport {
    let failures = promotion_gate_failures(
        family_report.correctness.first_target_exactness_bps,
        family_report.correctness.first_32_token_exactness_bps,
        family_report.correctness.exact_trace_case_count,
    );
    let mut report = TassadarExecutorPromotionGateReport {
        run_id: run_bundle.run_id.clone(),
        trainable_surface: String::from(
            "executor_attention_candidate.relative_target_output_bias_plus_trace_schema_bias",
        ),
        checkpoint_id: training_report.best_checkpoint_id.clone(),
        selected_stage_id: selected_stage_id.to_string(),
        first_target_exactness_bps: family_report.correctness.first_target_exactness_bps,
        first_32_token_exactness_bps: family_report.correctness.first_32_token_exactness_bps,
        exact_trace_case_count: family_report.correctness.exact_trace_case_count,
        required_first_target_exactness_bps: REQUIRED_FIRST_TARGET_EXACTNESS_BPS,
        required_first_32_token_exactness_bps_strictly_greater_than:
            REQUIRED_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE,
        required_exact_trace_case_count: REQUIRED_EXACT_TRACE_CASE_COUNT,
        passed: failures.is_empty(),
        failures,
        report_digest: String::new(),
    };
    report.report_digest =
        stable_digest(b"psionic_tassadar_executor_promotion_gate_report|", &report);
    report
}

fn build_exactness_curve_report(
    run_bundle: &TassadarExecutorAttentionRunBundle,
    training_report: &TassadarExecutorAttentionTrainingReport,
    family_report: &TassadarExecutorArchitectureFamilyReport,
    checkpoint_state_digest: &str,
    dataset_storage_key: &str,
    dataset_digest: &str,
    analyses: &[AttentionExampleAnalysis],
) -> TassadarExecutorAttentionPromotionExactnessCurveReport {
    let curves = [None]
        .into_iter()
        .chain([
            Some(TassadarSequenceSplit::Train),
            Some(TassadarSequenceSplit::Validation),
            Some(TassadarSequenceSplit::Test),
        ])
        .map(|split| build_curve_for_split(split, analyses))
        .collect::<Vec<_>>();
    let mut report = TassadarExecutorAttentionPromotionExactnessCurveReport {
        run_id: run_bundle.run_id.clone(),
        dataset_storage_key: dataset_storage_key.to_string(),
        dataset_digest: dataset_digest.to_string(),
        trained_model_descriptor_digest: family_report.model_descriptor_digest.clone(),
        checkpoint_state_digest: checkpoint_state_digest.to_string(),
        checkpoint_curve: training_report
            .epoch_reports
            .iter()
            .map(|epoch| TassadarExecutorCheckpointExactnessPoint {
                checkpoint_id: epoch.checkpoint_id.clone(),
                global_epoch_index: epoch.epoch_index,
                stage_id: epoch.stage_id.clone(),
                stage_epoch_index: epoch.stage_epoch_index,
                aggregate_target_token_exactness_bps: epoch
                    .validation
                    .correctness
                    .aggregate_target_token_exactness_bps,
                first_target_exactness_bps: epoch.validation.correctness.first_target_exactness_bps,
                first_8_token_exactness_bps: epoch
                    .validation
                    .correctness
                    .first_8_token_exactness_bps,
                first_32_token_exactness_bps: epoch
                    .validation
                    .correctness
                    .first_32_token_exactness_bps,
                exact_trace_case_count: epoch.validation.correctness.exact_trace_case_count,
                selected_for_export: epoch.checkpoint_id == training_report.best_checkpoint_id,
            })
            .collect(),
        curves,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_executor_attention_promotion_exactness_curve_report|",
        &report,
    );
    report
}

fn build_failure_sample_report(
    run_bundle: &TassadarExecutorAttentionRunBundle,
    family_report: &TassadarExecutorArchitectureFamilyReport,
    tokenizer: &TassadarTraceTokenizer,
    checkpoint_state_digest: &str,
    dataset_storage_key: &str,
    dataset_digest: &str,
    analyses: &[AttentionExampleAnalysis],
) -> TassadarExecutorAttentionPromotionFailureSampleReport {
    let mut samples = analyses
        .iter()
        .filter(|analysis| analysis.case.split == TassadarSequenceSplit::Validation)
        .filter(|analysis| !analysis.case.exact_trace_match)
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
    let mut report = TassadarExecutorAttentionPromotionFailureSampleReport {
        run_id: run_bundle.run_id.clone(),
        dataset_storage_key: dataset_storage_key.to_string(),
        dataset_digest: dataset_digest.to_string(),
        trained_model_descriptor_digest: family_report.model_descriptor_digest.clone(),
        checkpoint_state_digest: checkpoint_state_digest.to_string(),
        samples,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_executor_attention_promotion_failure_sample_report|",
        &report,
    );
    report
}

fn build_exact_trace_sample_report(
    run_bundle: &TassadarExecutorAttentionRunBundle,
    family_report: &TassadarExecutorArchitectureFamilyReport,
    tokenizer: &TassadarTraceTokenizer,
    checkpoint_state_digest: &str,
    dataset_storage_key: &str,
    dataset_digest: &str,
    analyses: &[AttentionExampleAnalysis],
) -> TassadarExecutorAttentionPromotionExactTraceSampleReport {
    let mut samples = analyses
        .iter()
        .filter(|analysis| analysis.case.split == TassadarSequenceSplit::Validation)
        .filter(|analysis| analysis.case.exact_trace_match)
        .map(|analysis| exact_trace_sample(tokenizer, analysis))
        .collect::<Vec<_>>();
    samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let mut report = TassadarExecutorAttentionPromotionExactTraceSampleReport {
        run_id: run_bundle.run_id.clone(),
        dataset_storage_key: dataset_storage_key.to_string(),
        dataset_digest: dataset_digest.to_string(),
        trained_model_descriptor_digest: family_report.model_descriptor_digest.clone(),
        checkpoint_state_digest: checkpoint_state_digest.to_string(),
        exact_trace_case_count: samples.len() as u32,
        samples,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_executor_attention_promotion_exact_trace_sample_report|",
        &report,
    );
    report
}

fn build_curve_for_split(
    split: Option<TassadarSequenceSplit>,
    analyses: &[AttentionExampleAnalysis],
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

fn analyze_example(
    model: &TassadarExecutorAttentionTransformer,
    tokenizer: &TassadarTraceTokenizer,
    example: &TassadarSequenceExample,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
) -> Result<AttentionExampleAnalysis, TassadarExecutorAttentionPromotionError> {
    let full_prompt_len = example.metadata.prompt_token_count as usize;
    let prompt_start = full_prompt_len.saturating_sub(prompt_window_token_cap.max(1));
    let prompt_len = full_prompt_len - prompt_start;
    let prompt = TokenSequence::new(
        example.token_ids[prompt_start..full_prompt_len]
            .iter()
            .copied()
            .map(TokenId)
            .collect::<Vec<_>>(),
    );
    let reference_target = example.token_ids[full_prompt_len..]
        .iter()
        .take(target_token_cap.max(1))
        .copied()
        .map(TokenId)
        .collect::<Vec<_>>();
    let predicted_target = greedy_decode_target(model, prompt, reference_target.len())?;
    let first_divergence_index =
        first_divergence(reference_target.as_slice(), predicted_target.as_slice());
    let matched_target_token_count =
        first_divergence_index.unwrap_or(reference_target.len() as u32);
    let reference_full = example
        .token_ids
        .iter()
        .copied()
        .map(TokenId)
        .collect::<Vec<_>>();
    let predicted_full = example.token_ids[..prompt_len]
        .iter()
        .copied()
        .map(TokenId)
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
            b"psionic_tassadar_executor_attention_reference_target|",
            &reference_target
                .iter()
                .map(|token| token.as_u32())
                .collect::<Vec<_>>(),
        ),
        predicted_target_digest: stable_digest(
            b"psionic_tassadar_executor_attention_predicted_target|",
            &predicted_target
                .iter()
                .map(|token| token.as_u32())
                .collect::<Vec<_>>(),
        ),
    };
    Ok(AttentionExampleAnalysis {
        case,
        prompt_token_count: prompt_len as u32,
        reference_full,
        reference_target,
        predicted_target,
    })
}

fn greedy_decode_target(
    model: &TassadarExecutorAttentionTransformer,
    prompt: TokenSequence,
    target_len: usize,
) -> Result<Vec<TokenId>, TassadarExecutorAttentionPromotionError> {
    let mut state = model
        .start_decode(prompt)
        .map_err(TassadarExecutorAttentionTrainingError::from)?;
    let mut decoded = Vec::with_capacity(target_len);
    for _ in 0..target_len {
        let token = model
            .greedy_next_token(&state)
            .map_err(TassadarExecutorAttentionTrainingError::from)?;
        model
            .push_decoded_token(&mut state, token)
            .map_err(TassadarExecutorAttentionTrainingError::from)?;
        decoded.push(token);
    }
    Ok(decoded)
}

fn failure_sample(
    tokenizer: &TassadarTraceTokenizer,
    analysis: &AttentionExampleAnalysis,
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

fn exact_trace_sample(
    tokenizer: &TassadarTraceTokenizer,
    analysis: &AttentionExampleAnalysis,
) -> TassadarExecutorExactTraceSample {
    TassadarExecutorExactTraceSample {
        sequence_id: analysis.case.sequence_id.clone(),
        case_id: analysis.case.case_id.clone(),
        split: analysis.case.split,
        prompt_token_count: analysis.prompt_token_count,
        target_token_count: analysis.reference_target.len() as u32,
        final_output_values: tokenizer.extract_output_values(analysis.reference_full.as_slice()),
        target_digest: analysis.case.reference_target_digest.clone(),
        target_prefix_tokens: analysis
            .reference_target
            .iter()
            .take(8)
            .map(|token| token_symbol(tokenizer, *token))
            .collect(),
        target_suffix_tokens: analysis
            .reference_target
            .iter()
            .rev()
            .take(8)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|token| token_symbol(tokenizer, token))
            .collect(),
    }
}

fn token_symbol(tokenizer: &TassadarTraceTokenizer, token: TokenId) -> String {
    tokenizer
        .vocabulary()
        .token(token)
        .unwrap_or("<unknown>")
        .to_string()
}

fn matched_token_count(reference: &[TokenId], predicted: &[TokenId]) -> usize {
    reference
        .iter()
        .zip(predicted.iter())
        .filter(|(left, right)| left == right)
        .count()
}

fn first_divergence(reference: &[TokenId], predicted: &[TokenId]) -> Option<u32> {
    reference
        .iter()
        .zip(predicted.iter())
        .position(|(left, right)| left != right)
        .map(|index| index as u32)
}

fn promotion_gate_failures(
    first_target_exactness_bps: u32,
    first_32_token_exactness_bps: u32,
    exact_trace_case_count: u32,
) -> Vec<TassadarExecutorPromotionGateFailure> {
    let mut failures = Vec::new();
    if first_target_exactness_bps < REQUIRED_FIRST_TARGET_EXACTNESS_BPS {
        failures.push(TassadarExecutorPromotionGateFailure {
            kind: TassadarExecutorPromotionGateFailureKind::FirstTargetExactnessBelowThreshold,
            actual: first_target_exactness_bps,
            required: REQUIRED_FIRST_TARGET_EXACTNESS_BPS,
        });
    }
    if first_32_token_exactness_bps <= REQUIRED_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE {
        failures.push(TassadarExecutorPromotionGateFailure {
            kind: TassadarExecutorPromotionGateFailureKind::First32TokenExactnessBelowThreshold,
            actual: first_32_token_exactness_bps,
            required: REQUIRED_FIRST_32_TOKEN_EXACTNESS_BPS_EXCLUSIVE + 1,
        });
    }
    if exact_trace_case_count < REQUIRED_EXACT_TRACE_CASE_COUNT {
        failures.push(TassadarExecutorPromotionGateFailure {
            kind: TassadarExecutorPromotionGateFailureKind::ExactTraceCountBelowThreshold,
            actual: exact_trace_case_count,
            required: REQUIRED_EXACT_TRACE_CASE_COUNT,
        });
    }
    failures
}

fn rate_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        ((u64::from(numerator) * 10_000) / u64::from(denominator)) as u32
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let bytes = serde_json::to_vec(value)
        .expect("attention promotion artifact should serialize for stable digest");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes.as_slice());
    format!("{:x}", hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn repo_relative_path(path: &Path) -> String {
    let root = repo_root();
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarExecutorAttentionPromotionError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarExecutorAttentionPromotionError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarExecutorAttentionPromotionError::Deserialize {
            artifact_kind: artifact_kind.to_string(),
            path: path.display().to_string(),
            error,
        }
    })
}

fn write_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
    value: &T,
) -> Result<(), TassadarExecutorAttentionPromotionError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorAttentionPromotionError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, bytes).map_err(|error| TassadarExecutorAttentionPromotionError::Write {
        path: path.display().to_string(),
        error,
    })
}
