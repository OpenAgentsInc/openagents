use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{TassadarSequenceDatasetError, TassadarSequenceSplit};
use psionic_eval::{
    TassadarExecutorArchitectureComparisonError, TassadarExecutorArchitectureFamilyReport,
    TassadarSequenceEvalError, build_tassadar_sequence_dataset,
    evaluate_attention_family_for_architecture_comparison,
};
use psionic_models::{
    TassadarExecutorAttentionError, TassadarExecutorAttentionTransformer, TokenId, TokenSequence,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const BOUNDARY_ATTENTION_V4_CHECKPOINT_STATE: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v4/checkpoint_state.json";
const BOUNDARY_ATTENTION_V4_RUN_BUNDLE: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v4/run_bundle.json";

/// Canonical output root for the first bounded learned attention-family run.
pub const TASSADAR_EXECUTOR_ATTENTION_TRAINING_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_training_v1";
/// Canonical output root for the first boundary-first attention-family run.
pub const TASSADAR_EXECUTOR_ATTENTION_BOUNDARY_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v5";
/// Canonical machine-readable training report artifact.
pub const TASSADAR_EXECUTOR_ATTENTION_TRAINING_REPORT_FILE: &str = "training_report.json";
/// Canonical machine-readable validation family report artifact.
pub const TASSADAR_EXECUTOR_ATTENTION_FAMILY_REPORT_FILE: &str = "family_report.json";
/// Canonical persisted checkpoint state artifact.
pub const TASSADAR_EXECUTOR_ATTENTION_CHECKPOINT_STATE_FILE: &str = "checkpoint_state.json";
/// Canonical persisted descriptor artifact.
pub const TASSADAR_EXECUTOR_ATTENTION_MODEL_DESCRIPTOR_FILE: &str = "model_descriptor.json";
/// Canonical top-level run bundle artifact.
pub const TASSADAR_EXECUTOR_ATTENTION_RUN_BUNDLE_FILE: &str = "run_bundle.json";

fn tassadar_progress_updates_enabled() -> bool {
    match std::env::var("OPENAGENTS_TASSADAR_PROGRESS") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "off" | "no")
        }
        Err(_) => !cfg!(test),
    }
}

fn emit_tassadar_progress(message: impl AsRef<str>) {
    if tassadar_progress_updates_enabled() {
        eprintln!("{}", message.as_ref());
    }
}

/// One curriculum stage for bounded attention-family training.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionTrainingStage {
    /// Stable stage identifier.
    pub stage_id: String,
    /// Maximum supervised target tokens for this stage.
    pub target_token_cap: usize,
    /// Number of epochs to spend in this stage.
    pub epoch_count: u32,
    /// Learning-rate scale applied on top of the global base learning rate.
    pub learning_rate_scale: f32,
    /// Explicit loss scale for the first target token.
    pub first_target_loss_scale: f32,
    /// Prefix length that gets elevated early-token loss.
    pub early_token_prefix_len: usize,
    /// Loss scale applied to non-first tokens inside the early prefix.
    pub early_token_loss_scale: f32,
}

impl TassadarExecutorAttentionTrainingStage {
    /// Creates one bounded training stage.
    #[must_use]
    pub fn new(stage_id: impl Into<String>, target_token_cap: usize, epoch_count: u32) -> Self {
        Self {
            stage_id: stage_id.into(),
            target_token_cap: target_token_cap.max(1),
            epoch_count,
            learning_rate_scale: 1.0,
            first_target_loss_scale: 1.0,
            early_token_prefix_len: 1,
            early_token_loss_scale: 1.0,
        }
    }

    /// Applies one learning-rate scale.
    #[must_use]
    pub fn with_learning_rate_scale(mut self, learning_rate_scale: f32) -> Self {
        self.learning_rate_scale = learning_rate_scale.max(0.0);
        self
    }

    /// Applies one first-target loss scale.
    #[must_use]
    pub fn with_first_target_loss_scale(mut self, first_target_loss_scale: f32) -> Self {
        self.first_target_loss_scale = first_target_loss_scale.max(1.0);
        self
    }

    /// Applies one early-prefix weighting policy.
    #[must_use]
    pub fn with_early_prefix_weighting(
        mut self,
        early_token_prefix_len: usize,
        early_token_loss_scale: f32,
    ) -> Self {
        self.early_token_prefix_len = early_token_prefix_len.max(1);
        self.early_token_loss_scale = early_token_loss_scale.max(1.0);
        self
    }
}

/// Fixed bounded training config for the executor-attention research lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionTrainingConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset version to freeze.
    pub dataset_version: String,
    /// Number of bounded optimization epochs.
    pub epochs: u32,
    /// SGD learning rate for the output head.
    pub learning_rate: f32,
    /// Prompt window cap copied from the architecture comparison.
    pub prompt_window_token_cap: usize,
    /// Target-token cap copied from the architecture comparison.
    pub target_token_cap: usize,
    /// Ordered curriculum stages for the training run.
    pub stages: Vec<TassadarExecutorAttentionTrainingStage>,
    /// Optional repo-local checkpoint artifact used to initialize the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_checkpoint_state_ref: Option<String>,
    /// Optional repo-local run bundle reference that explains where the
    /// initialization checkpoint came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_run_bundle_ref: Option<String>,
    /// Whether the shared output head should be trainable.
    #[serde(default = "default_true")]
    pub train_output_head: bool,
    /// Whether the bounded relative-target output-bias adapter should be
    /// trainable.
    #[serde(default)]
    pub train_relative_target_output_bias: bool,
    /// Whether the bounded relative-target hidden-state-conditioned output
    /// projection adapter should be trainable.
    #[serde(default)]
    pub train_relative_target_output_projection: bool,
    /// Whether the bounded previous-token-conditioned relative-target output
    /// bias adapter should be trainable.
    #[serde(default)]
    pub train_relative_target_transition_output_bias: bool,
    /// Learning-rate multiplier applied only to the bounded relative-target
    /// hidden-state-conditioned output projection adapter.
    #[serde(default = "default_one")]
    pub relative_target_output_projection_learning_rate_scale: f32,
    /// Learning-rate multiplier applied only to the bounded previous-token-
    /// conditioned relative-target output bias adapter.
    #[serde(default = "default_one")]
    pub relative_target_transition_output_bias_learning_rate_scale: f32,
}

impl TassadarExecutorAttentionTrainingConfig {
    /// Returns the first bounded attention-family training config.
    #[must_use]
    pub fn reference() -> Self {
        Self {
            run_id: String::from("tassadar-executor-attention-sudoku-v0-train-v1"),
            dataset_version: String::from("train-v0"),
            epochs: 32,
            learning_rate: 0.1,
            prompt_window_token_cap: 256,
            target_token_cap: 32,
            stages: vec![TassadarExecutorAttentionTrainingStage::new(
                "full_suffix",
                32,
                32,
            )],
            initial_checkpoint_state_ref: None,
            initial_run_bundle_ref: None,
            train_output_head: true,
            train_relative_target_output_bias: false,
            train_relative_target_output_projection: false,
            train_relative_target_transition_output_bias: false,
            relative_target_output_projection_learning_rate_scale: 1.0,
            relative_target_transition_output_bias_learning_rate_scale: 1.0,
        }
    }

    /// Returns the first boundary-first attention-family training config.
    #[must_use]
    pub fn boundary_curriculum_reference() -> Self {
        Self {
            run_id: String::from("tassadar-executor-attention-sudoku-v0-boundary-v5"),
            dataset_version: String::from("train-v0"),
            epochs: 32,
            learning_rate: 0.02,
            prompt_window_token_cap: 256,
            target_token_cap: 32,
            stages: vec![
                TassadarExecutorAttentionTrainingStage::new("prompt_to_first_token", 1, 8)
                    .with_learning_rate_scale(1.0)
                    .with_first_target_loss_scale(8.0)
                    .with_early_prefix_weighting(1, 8.0),
                TassadarExecutorAttentionTrainingStage::new("prompt_to_first_2_tokens", 2, 4)
                    .with_learning_rate_scale(1.0)
                    .with_first_target_loss_scale(6.0)
                    .with_early_prefix_weighting(2, 4.0),
                TassadarExecutorAttentionTrainingStage::new("prompt_to_first_4_tokens", 4, 4)
                    .with_learning_rate_scale(1.0)
                    .with_first_target_loss_scale(4.0)
                    .with_early_prefix_weighting(4, 3.0),
                TassadarExecutorAttentionTrainingStage::new("prompt_to_first_8_tokens", 8, 4)
                    .with_learning_rate_scale(1.0)
                    .with_first_target_loss_scale(3.0)
                    .with_early_prefix_weighting(8, 2.0),
                TassadarExecutorAttentionTrainingStage::new("prompt_to_first_16_tokens", 16, 4)
                    .with_learning_rate_scale(1.0)
                    .with_first_target_loss_scale(2.0)
                    .with_early_prefix_weighting(8, 1.75),
                TassadarExecutorAttentionTrainingStage::new("prompt_to_first_32_tokens", 32, 8)
                    .with_learning_rate_scale(1.0)
                    .with_first_target_loss_scale(1.5)
                    .with_early_prefix_weighting(8, 1.25),
            ],
            initial_checkpoint_state_ref: Some(String::from(
                BOUNDARY_ATTENTION_V4_CHECKPOINT_STATE,
            )),
            initial_run_bundle_ref: Some(String::from(BOUNDARY_ATTENTION_V4_RUN_BUNDLE)),
            train_output_head: false,
            train_relative_target_output_bias: false,
            train_relative_target_output_projection: false,
            train_relative_target_transition_output_bias: true,
            relative_target_output_projection_learning_rate_scale: 1.0,
            relative_target_transition_output_bias_learning_rate_scale: 6.0,
        }
    }
}

/// One bounded window used for attention-family training.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BoundedAttentionTrainingWindow {
    prompt: TokenSequence,
    reference_target: Vec<TokenId>,
}

/// Per-epoch bounded attention-family training report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionTrainingEpochReport {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Zero-based epoch index.
    pub epoch_index: u32,
    /// Stable stage identifier.
    pub stage_id: String,
    /// Zero-based epoch index inside the current stage.
    pub stage_epoch_index: u32,
    /// Stage-local target-token cap.
    pub stage_target_token_cap: u32,
    /// Effective learning-rate scale for this epoch.
    pub learning_rate_scale: f32,
    /// Loss scale applied to the first target token.
    pub first_target_loss_scale: f32,
    /// Prefix length that received elevated early-token loss.
    pub early_token_prefix_len: u32,
    /// Loss scale applied to non-first tokens inside the early prefix.
    pub early_token_loss_scale: f32,
    /// Mean cross-entropy over supervised target tokens.
    pub mean_loss: f32,
    /// Number of supervised target tokens.
    pub target_token_count: u32,
    /// Validation report over the same bounded window used by Phase 15.
    pub validation: TassadarExecutorArchitectureFamilyReport,
}

/// Top-level bounded attention-family training report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionTrainingReport {
    /// Frozen config used for the run.
    pub config: TassadarExecutorAttentionTrainingConfig,
    /// Selected best checkpoint identifier.
    pub best_checkpoint_id: String,
    /// Ordered per-epoch reports.
    pub epoch_reports: Vec<TassadarExecutorAttentionTrainingEpochReport>,
    /// Selected validation report.
    pub validation: TassadarExecutorArchitectureFamilyReport,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarExecutorAttentionTrainingReport {
    fn new(
        config: TassadarExecutorAttentionTrainingConfig,
        best_checkpoint_id: String,
        epoch_reports: Vec<TassadarExecutorAttentionTrainingEpochReport>,
        validation: TassadarExecutorArchitectureFamilyReport,
    ) -> Self {
        let mut report = Self {
            config,
            best_checkpoint_id,
            epoch_reports,
            validation,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_executor_attention_training_report|",
            &report,
        );
        report
    }
}

/// Persisted output-head checkpoint for the bounded attention-family trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionCheckpointState {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Frozen trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Frozen trained weight digest.
    pub trained_weight_digest: String,
    /// Trained output projection values.
    pub output_projection: Vec<f32>,
    /// Trained output bias values.
    pub output_bias: Vec<f32>,
    /// Trained bounded relative-target output-bias adapter values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relative_target_output_bias: Vec<f32>,
    /// Trained bounded relative-target hidden-state-conditioned output
    /// projection adapter values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relative_target_output_projection: Vec<f32>,
    /// Trained bounded previous-token-conditioned relative-target output-bias
    /// adapter values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relative_target_transition_output_bias: Vec<f32>,
    /// Stable digest over the persisted state.
    pub state_digest: String,
}

impl TassadarExecutorAttentionCheckpointState {
    fn new(
        checkpoint_id: impl Into<String>,
        run_id: impl Into<String>,
        model: &TassadarExecutorAttentionTransformer,
    ) -> Self {
        let mut checkpoint = Self {
            checkpoint_id: checkpoint_id.into(),
            run_id: run_id.into(),
            trained_model_descriptor_digest: model.descriptor().stable_digest(),
            trained_weight_digest: model.descriptor().weights.digest.clone(),
            output_projection: model.weights().output_projection().to_vec(),
            output_bias: model.weights().output_bias().to_vec(),
            relative_target_output_bias: model.weights().relative_target_output_bias().to_vec(),
            relative_target_output_projection: model
                .weights()
                .relative_target_output_projection()
                .to_vec(),
            relative_target_transition_output_bias: model
                .weights()
                .relative_target_transition_output_bias()
                .to_vec(),
            state_digest: String::new(),
        };
        checkpoint.state_digest = stable_digest(
            b"psionic_tassadar_executor_attention_checkpoint_state|",
            &checkpoint,
        );
        checkpoint
    }

    /// Reconstructs the bounded attention-family model from the checkpoint.
    pub fn materialize_model(
        &self,
    ) -> Result<TassadarExecutorAttentionTransformer, TassadarExecutorAttentionTrainingError> {
        let mut model = TassadarExecutorAttentionTransformer::sudoku_v0();
        if model.weights().output_projection().len() != self.output_projection.len() {
            return Err(
                TassadarExecutorAttentionTrainingError::CheckpointWidthMismatch {
                    tensor: String::from("output_projection"),
                    expected: model.weights().output_projection().len(),
                    actual: self.output_projection.len(),
                },
            );
        }
        if model.weights().output_bias().len() != self.output_bias.len() {
            return Err(
                TassadarExecutorAttentionTrainingError::CheckpointWidthMismatch {
                    tensor: String::from("output_bias"),
                    expected: model.weights().output_bias().len(),
                    actual: self.output_bias.len(),
                },
            );
        }
        if !self.relative_target_output_bias.is_empty()
            && model.weights().relative_target_output_bias().len()
                != self.relative_target_output_bias.len()
        {
            return Err(
                TassadarExecutorAttentionTrainingError::CheckpointWidthMismatch {
                    tensor: String::from("relative_target_output_bias"),
                    expected: model.weights().relative_target_output_bias().len(),
                    actual: self.relative_target_output_bias.len(),
                },
            );
        }
        if !self.relative_target_output_projection.is_empty()
            && model.weights().relative_target_output_projection().len()
                != self.relative_target_output_projection.len()
        {
            return Err(
                TassadarExecutorAttentionTrainingError::CheckpointWidthMismatch {
                    tensor: String::from("relative_target_output_projection"),
                    expected: model.weights().relative_target_output_projection().len(),
                    actual: self.relative_target_output_projection.len(),
                },
            );
        }
        if !self.relative_target_transition_output_bias.is_empty()
            && model
                .weights()
                .relative_target_transition_output_bias()
                .len()
                != self.relative_target_transition_output_bias.len()
        {
            return Err(
                TassadarExecutorAttentionTrainingError::CheckpointWidthMismatch {
                    tensor: String::from("relative_target_transition_output_bias"),
                    expected: model
                        .weights()
                        .relative_target_transition_output_bias()
                        .len(),
                    actual: self.relative_target_transition_output_bias.len(),
                },
            );
        }
        model
            .weights_mut()
            .output_projection_mut()
            .copy_from_slice(self.output_projection.as_slice());
        model
            .weights_mut()
            .output_bias_mut()
            .copy_from_slice(self.output_bias.as_slice());
        if !self.relative_target_output_bias.is_empty() {
            model
                .weights_mut()
                .relative_target_output_bias_mut()
                .copy_from_slice(self.relative_target_output_bias.as_slice());
        }
        if !self.relative_target_output_projection.is_empty() {
            model
                .weights_mut()
                .relative_target_output_projection_mut()
                .copy_from_slice(self.relative_target_output_projection.as_slice());
        }
        if !self.relative_target_transition_output_bias.is_empty() {
            model
                .weights_mut()
                .relative_target_transition_output_bias_mut()
                .copy_from_slice(self.relative_target_transition_output_bias.as_slice());
        }
        model.refresh_after_training();
        Ok(model)
    }
}

/// Top-level persisted bundle for the bounded attention-family training run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionRunBundle {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Relative output directory.
    pub output_dir: String,
    /// Relative training report artifact.
    pub training_report_file: String,
    /// Relative validation family report artifact.
    pub family_report_file: String,
    /// Relative checkpoint artifact.
    pub checkpoint_state_file: String,
    /// Relative model descriptor artifact.
    pub model_descriptor_file: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl TassadarExecutorAttentionRunBundle {
    fn new(run_id: &str, model: &TassadarExecutorAttentionTransformer, output_dir: &Path) -> Self {
        let mut bundle = Self {
            run_id: run_id.to_string(),
            model_id: model.descriptor().model.model_id.clone(),
            output_dir: output_dir.display().to_string(),
            training_report_file: String::from(TASSADAR_EXECUTOR_ATTENTION_TRAINING_REPORT_FILE),
            family_report_file: String::from(TASSADAR_EXECUTOR_ATTENTION_FAMILY_REPORT_FILE),
            checkpoint_state_file: String::from(TASSADAR_EXECUTOR_ATTENTION_CHECKPOINT_STATE_FILE),
            model_descriptor_file: String::from(TASSADAR_EXECUTOR_ATTENTION_MODEL_DESCRIPTOR_FILE),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest =
            stable_digest(b"psionic_tassadar_executor_attention_run_bundle|", &bundle);
        bundle
    }
}

/// Full in-memory outcome of a bounded attention-family training run.
pub struct TassadarExecutorAttentionTrainingOutcome {
    /// Selected best model.
    pub model: TassadarExecutorAttentionTransformer,
    /// Persistable checkpoint state for the best model.
    pub checkpoint: TassadarExecutorAttentionCheckpointState,
    /// Persistable training report.
    pub report: TassadarExecutorAttentionTrainingReport,
}

/// Bounded attention-family training failure.
#[derive(Debug, Error)]
pub enum TassadarExecutorAttentionTrainingError {
    /// Building the bounded sequence dataset failed.
    #[error(transparent)]
    DatasetBuild(#[from] TassadarSequenceEvalError),
    /// Validating the frozen dataset failed.
    #[error(transparent)]
    DatasetValidate(#[from] TassadarSequenceDatasetError),
    /// Validation scoring failed.
    #[error(transparent)]
    Eval(#[from] TassadarExecutorArchitectureComparisonError),
    /// Reading one initialization artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding one initialization artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// File path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Attention-model execution failed during teacher forcing.
    #[error(transparent)]
    Model(#[from] TassadarExecutorAttentionError),
    /// Creating one output directory failed.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Writing one artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// One checkpoint tensor no longer matches the canonical model width.
    #[error("checkpoint tensor `{tensor}` has width mismatch: expected {expected}, found {actual}")]
    CheckpointWidthMismatch {
        /// Tensor name.
        tensor: String,
        /// Expected flattened width.
        expected: usize,
        /// Actual flattened width.
        actual: usize,
    },
    /// The run produced no checkpoint candidates at all.
    #[error("no checkpoint candidates were produced for run `{run_id}`")]
    NoCheckpointCandidates {
        /// Stable run identifier.
        run_id: String,
    },
}

/// Executes the bounded output-head-only attention-family trainer.
pub fn train_tassadar_executor_attention_windowed(
    config: &TassadarExecutorAttentionTrainingConfig,
) -> Result<TassadarExecutorAttentionTrainingOutcome, TassadarExecutorAttentionTrainingError> {
    let bundle = build_tassadar_sequence_dataset(
        psionic_eval::TassadarSequenceWorkload::SudokuV0,
        config.dataset_version.as_str(),
    )?;
    bundle.dataset.validate()?;
    let mut model = TassadarExecutorAttentionTransformer::sudoku_v0();
    if let Some(checkpoint_ref) = &config.initial_checkpoint_state_ref {
        let checkpoint_bytes = fs::read(repo_root().join(checkpoint_ref)).map_err(|error| {
            TassadarExecutorAttentionTrainingError::Read {
                path: checkpoint_ref.clone(),
                error,
            }
        })?;
        let checkpoint: TassadarExecutorAttentionCheckpointState =
            serde_json::from_slice(&checkpoint_bytes).map_err(|error| {
                TassadarExecutorAttentionTrainingError::Deserialize {
                    artifact_kind: String::from("tassadar_executor_attention_checkpoint_state"),
                    path: checkpoint_ref.clone(),
                    error,
                }
            })?;
        model = checkpoint.materialize_model()?;
    }
    let train_examples = bundle
        .dataset
        .split_examples(TassadarSequenceSplit::Train)
        .into_iter()
        .collect::<Vec<_>>();
    let mut epoch_reports = Vec::new();
    let mut best_model = None;
    let mut best_checkpoint = None;
    let mut best_validation = None;
    let mut best_mean_loss = None;
    let total_epochs = total_stage_epochs(config);

    emit_tassadar_progress(format!(
        "tassadar_attention_progress phase=train_start run={} epochs={} stages={} learning_rate={:.6} prompt_cap={} validation_target_cap={} train_examples={} validation_examples={} train_output_head={} train_relative_target_output_bias={} train_relative_target_output_projection={} train_relative_target_transition_output_bias={} projection_lr_scale={:.4} transition_lr_scale={:.4}",
        config.run_id,
        total_epochs,
        config.stages.len(),
        config.learning_rate,
        config.prompt_window_token_cap,
        config.target_token_cap,
        train_examples.len(),
        bundle
            .dataset
            .split_examples(TassadarSequenceSplit::Validation)
            .len(),
        config.train_output_head,
        config.train_relative_target_output_bias,
        config.train_relative_target_output_projection,
        config.train_relative_target_transition_output_bias,
        config.relative_target_output_projection_learning_rate_scale,
        config.relative_target_transition_output_bias_learning_rate_scale,
    ));
    let mut global_epoch_index = 0_u32;

    for (stage_index, stage) in config.stages.iter().enumerate() {
        emit_tassadar_progress(format!(
            "tassadar_attention_progress phase=stage_start run={} stage={}/{} stage_id={} target_cap={} stage_epochs={} learning_rate_scale={:.4} first_target_loss_scale={:.2} early_prefix_len={} early_token_loss_scale={:.2}",
            config.run_id,
            stage_index + 1,
            config.stages.len(),
            stage.stage_id,
            stage.target_token_cap,
            stage.epoch_count,
            stage.learning_rate_scale,
            stage.first_target_loss_scale,
            stage.early_token_prefix_len,
            stage.early_token_loss_scale,
        ));

        for stage_epoch_index in 0..stage.epoch_count {
            let model_width = model.descriptor().config.model_width;
            let vocab_size = model.descriptor().config.vocab_size;
            let mut projection_grad = vec![0.0; model_width * vocab_size];
            let mut bias_grad = vec![0.0; vocab_size];
            let mut relative_target_output_bias_grad =
                vec![0.0; model.weights().relative_target_output_bias().len()];
            let mut relative_target_output_projection_grad =
                vec![0.0; model.weights().relative_target_output_projection().len()];
            let mut relative_target_transition_output_bias_grad = vec![
                0.0;
                model
                    .weights()
                    .relative_target_transition_output_bias()
                    .len()
            ];
            let mut total_loss = 0.0_f32;
            let mut total_target_tokens = 0_u32;
            let effective_learning_rate = config.learning_rate * stage.learning_rate_scale;

            emit_tassadar_progress(format!(
                "tassadar_attention_progress phase=epoch_start run={} epoch={}/{} stage_id={} stage_epoch={}/{} target_cap={} learning_rate={:.6}",
                config.run_id,
                global_epoch_index + 1,
                total_epochs,
                stage.stage_id,
                stage_epoch_index + 1,
                stage.epoch_count,
                stage.target_token_cap,
                effective_learning_rate,
            ));

            for (example_index, example) in train_examples.iter().enumerate() {
                let window = bounded_case_window(
                    example.token_ids.as_slice(),
                    example.metadata.prompt_token_count as usize,
                    config.prompt_window_token_cap,
                    stage.target_token_cap,
                );
                let mut sequence = window.prompt.as_slice().to_vec();
                sequence.extend(window.reference_target.iter().copied());
                let sequence = TokenSequence::new(sequence);
                let forward = model.forward_logits(&sequence)?;
                let prompt_len = window.prompt.len();
                let start_logit_index = prompt_len.saturating_sub(1);
                let end_logit_index = start_logit_index + window.reference_target.len();
                let mut sequence_loss = 0.0_f32;

                for (target_index, logit_index) in (start_logit_index..end_logit_index).enumerate()
                {
                    let loss_scale = loss_scale_for_target_index(stage, target_index);
                    let mut adjusted_logits = forward.logits[logit_index].clone();
                    let previous_token = sequence.as_slice()[logit_index];
                    model.apply_relative_target_output_bias_in_place(
                        adjusted_logits.as_mut_slice(),
                        target_index,
                    );
                    model.apply_relative_target_output_projection_in_place(
                        adjusted_logits.as_mut_slice(),
                        &forward.hidden_states[logit_index],
                        target_index,
                    );
                    model.apply_relative_target_transition_output_bias_in_place(
                        adjusted_logits.as_mut_slice(),
                        Some(previous_token),
                        target_index,
                    );
                    sequence_loss += accumulate_attention_output_step_gradients(
                        &forward.hidden_states[logit_index],
                        adjusted_logits.as_slice(),
                        sequence.as_slice()[logit_index + 1],
                        loss_scale,
                        if config.train_output_head {
                            Some(projection_grad.as_mut_slice())
                        } else {
                            None
                        },
                        if config.train_output_head {
                            Some(bias_grad.as_mut_slice())
                        } else {
                            None
                        },
                        if config.train_relative_target_output_bias {
                            Some(relative_target_output_bias_grad.as_mut_slice())
                        } else {
                            None
                        },
                        if config.train_relative_target_output_projection {
                            Some(relative_target_output_projection_grad.as_mut_slice())
                        } else {
                            None
                        },
                        if config.train_relative_target_transition_output_bias {
                            Some(relative_target_transition_output_bias_grad.as_mut_slice())
                        } else {
                            None
                        },
                        previous_token,
                        target_index,
                        vocab_size,
                    );
                    total_target_tokens = total_target_tokens.saturating_add(1);
                }
                total_loss += sequence_loss;
                emit_tassadar_progress(format!(
                    "tassadar_attention_progress phase=sequence_complete run={} epoch={}/{} stage_id={} sequence={}/{} case_id={} target_tokens={} sequence_mean_loss={:.6}",
                    config.run_id,
                    global_epoch_index + 1,
                    total_epochs,
                    stage.stage_id,
                    example_index + 1,
                    train_examples.len(),
                    example.metadata.case_id,
                    window.reference_target.len(),
                    if window.reference_target.is_empty() {
                        0.0
                    } else {
                        sequence_loss / window.reference_target.len() as f32
                    },
                ));
            }

            if total_target_tokens > 0 {
                let scale = effective_learning_rate / total_target_tokens as f32;
                if config.train_output_head {
                    for (weight, gradient) in model
                        .weights_mut()
                        .output_projection_mut()
                        .iter_mut()
                        .zip(projection_grad.iter())
                    {
                        *weight -= scale * gradient;
                    }
                    for (bias, gradient) in model
                        .weights_mut()
                        .output_bias_mut()
                        .iter_mut()
                        .zip(bias_grad.iter())
                    {
                        *bias -= scale * gradient;
                    }
                }
                if config.train_relative_target_output_bias {
                    for (bias, gradient) in model
                        .weights_mut()
                        .relative_target_output_bias_mut()
                        .iter_mut()
                        .zip(relative_target_output_bias_grad.iter())
                    {
                        *bias -= scale * gradient;
                    }
                }
                if config.train_relative_target_output_projection {
                    let projection_scale =
                        scale * config.relative_target_output_projection_learning_rate_scale;
                    for (weight, gradient) in model
                        .weights_mut()
                        .relative_target_output_projection_mut()
                        .iter_mut()
                        .zip(relative_target_output_projection_grad.iter())
                    {
                        *weight -= projection_scale * gradient;
                    }
                }
                if config.train_relative_target_transition_output_bias {
                    let transition_scale =
                        scale * config.relative_target_transition_output_bias_learning_rate_scale;
                    for (weight, gradient) in model
                        .weights_mut()
                        .relative_target_transition_output_bias_mut()
                        .iter_mut()
                        .zip(relative_target_transition_output_bias_grad.iter())
                    {
                        *weight -= transition_scale * gradient;
                    }
                }
                model.refresh_after_training();
            }

            let validation = evaluate_attention_family_for_architecture_comparison(
                &model,
                &bundle.dataset,
                TassadarSequenceSplit::Validation,
                config.prompt_window_token_cap,
                config.target_token_cap,
            )?;
            let checkpoint_id =
                format!("{}.checkpoint.epoch_{global_epoch_index:04}", config.run_id);
            let mean_loss = if total_target_tokens == 0 {
                0.0
            } else {
                total_loss / total_target_tokens as f32
            };
            emit_tassadar_progress(format!(
                "tassadar_attention_progress phase=epoch_complete run={} epoch={}/{} stage_id={} checkpoint_id={} mean_loss={:.6} first_target_bps={} first_32_bps={} exact_traces={}",
                config.run_id,
                global_epoch_index + 1,
                total_epochs,
                stage.stage_id,
                checkpoint_id,
                mean_loss,
                validation.correctness.first_target_exactness_bps,
                validation.correctness.first_32_token_exactness_bps,
                validation.correctness.exact_trace_case_count,
            ));
            let epoch_report = TassadarExecutorAttentionTrainingEpochReport {
                checkpoint_id: checkpoint_id.clone(),
                epoch_index: global_epoch_index,
                stage_id: stage.stage_id.clone(),
                stage_epoch_index,
                stage_target_token_cap: stage.target_token_cap as u32,
                learning_rate_scale: stage.learning_rate_scale,
                first_target_loss_scale: stage.first_target_loss_scale,
                early_token_prefix_len: stage.early_token_prefix_len as u32,
                early_token_loss_scale: stage.early_token_loss_scale,
                mean_loss,
                target_token_count: total_target_tokens,
                validation: validation.clone(),
            };
            let should_replace_best = best_validation.as_ref().is_none_or(
                |best: &TassadarExecutorArchitectureFamilyReport| {
                    let candidate_rank = attention_correctness_rank(&validation);
                    let best_rank = attention_correctness_rank(best);
                    candidate_rank > best_rank
                        || (candidate_rank == best_rank
                            && best_mean_loss.is_some_and(|best_loss| mean_loss < best_loss))
                },
            );
            if should_replace_best {
                best_model = Some(model.clone());
                best_checkpoint = Some(TassadarExecutorAttentionCheckpointState::new(
                    checkpoint_id.as_str(),
                    config.run_id.as_str(),
                    &model,
                ));
                best_validation = Some(validation.clone());
                best_mean_loss = Some(mean_loss);
            }
            epoch_reports.push(epoch_report);
            global_epoch_index = global_epoch_index.saturating_add(1);
        }
    }

    let best_model = best_model.ok_or_else(|| {
        TassadarExecutorAttentionTrainingError::NoCheckpointCandidates {
            run_id: config.run_id.clone(),
        }
    })?;
    let best_checkpoint = best_checkpoint.ok_or_else(|| {
        TassadarExecutorAttentionTrainingError::NoCheckpointCandidates {
            run_id: config.run_id.clone(),
        }
    })?;
    let best_validation = best_validation.ok_or_else(|| {
        TassadarExecutorAttentionTrainingError::NoCheckpointCandidates {
            run_id: config.run_id.clone(),
        }
    })?;
    let report = TassadarExecutorAttentionTrainingReport::new(
        config.clone(),
        best_checkpoint.checkpoint_id.clone(),
        epoch_reports,
        best_validation,
    );
    Ok(TassadarExecutorAttentionTrainingOutcome {
        model: best_model,
        checkpoint: best_checkpoint,
        report,
    })
}

/// Executes the bounded attention-family trainer and persists the resulting bundle.
pub fn run_tassadar_executor_attention_training(
    output_dir: &Path,
) -> Result<TassadarExecutorAttentionRunBundle, TassadarExecutorAttentionTrainingError> {
    run_tassadar_executor_attention_training_with_config(
        output_dir,
        &TassadarExecutorAttentionTrainingConfig::reference(),
    )
}

/// Executes the boundary-first attention-family trainer and persists the
/// resulting bundle.
pub fn run_tassadar_executor_attention_boundary_training(
    output_dir: &Path,
) -> Result<TassadarExecutorAttentionRunBundle, TassadarExecutorAttentionTrainingError> {
    run_tassadar_executor_attention_training_with_config(
        output_dir,
        &TassadarExecutorAttentionTrainingConfig::boundary_curriculum_reference(),
    )
}

/// Executes one attention-family training config and persists the resulting
/// bundle.
pub fn run_tassadar_executor_attention_training_with_config(
    output_dir: &Path,
    config: &TassadarExecutorAttentionTrainingConfig,
) -> Result<TassadarExecutorAttentionRunBundle, TassadarExecutorAttentionTrainingError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarExecutorAttentionTrainingError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let outcome = train_tassadar_executor_attention_windowed(config)?;
    let run_bundle = TassadarExecutorAttentionRunBundle::new(
        outcome.report.config.run_id.as_str(),
        &outcome.model,
        output_dir,
    );
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_TRAINING_REPORT_FILE),
        &outcome.report,
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_FAMILY_REPORT_FILE),
        &outcome.report.validation,
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_CHECKPOINT_STATE_FILE),
        &outcome.checkpoint,
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_MODEL_DESCRIPTOR_FILE),
        outcome.model.descriptor(),
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_ATTENTION_RUN_BUNDLE_FILE),
        &run_bundle,
    )?;
    Ok(run_bundle)
}

fn total_stage_epochs(config: &TassadarExecutorAttentionTrainingConfig) -> u32 {
    config.stages.iter().map(|stage| stage.epoch_count).sum()
}

fn bounded_case_window(
    token_ids: &[u32],
    prompt_len: usize,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
) -> BoundedAttentionTrainingWindow {
    let prompt_start = prompt_len.saturating_sub(prompt_window_token_cap.max(1));
    let prompt = TokenSequence::new(
        token_ids[prompt_start..prompt_len]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>(),
    );
    let reference_target = token_ids[prompt_len..]
        .iter()
        .take(target_token_cap.max(1))
        .map(|token| TokenId(*token))
        .collect::<Vec<_>>();
    BoundedAttentionTrainingWindow {
        prompt,
        reference_target,
    }
}

fn accumulate_attention_output_step_gradients(
    hidden: &[f32],
    logits: &[f32],
    target_token: TokenId,
    loss_scale: f32,
    projection_grad: Option<&mut [f32]>,
    bias_grad: Option<&mut [f32]>,
    relative_target_output_bias_grad: Option<&mut [f32]>,
    relative_target_output_projection_grad: Option<&mut [f32]>,
    relative_target_transition_output_bias_grad: Option<&mut [f32]>,
    previous_token: TokenId,
    relative_target_index: usize,
    vocab_size: usize,
) -> f32 {
    let probabilities = softmax(logits);
    let target_token_index = target_token.as_u32() as usize;
    let probability = probabilities[target_token_index].max(1e-8);
    let mut projection_grad = projection_grad;
    let mut bias_grad = bias_grad;
    let mut relative_target_output_bias_grad = relative_target_output_bias_grad;
    let mut relative_target_output_projection_grad = relative_target_output_projection_grad;
    let mut relative_target_transition_output_bias_grad =
        relative_target_transition_output_bias_grad;
    let relative_target_bias_offset = relative_target_index * vocab_size;
    let relative_target_projection_offset = relative_target_index * hidden.len() * vocab_size;
    let previous_token_index = previous_token.as_u32() as usize;
    let relative_target_transition_offset =
        (relative_target_index * vocab_size + previous_token_index) * vocab_size;
    for (token_index, probability) in probabilities.iter().enumerate() {
        let delta = (probability - f32::from(token_index == target_token_index)) * loss_scale;
        if let Some(bias_grad) = bias_grad.as_deref_mut() {
            bias_grad[token_index] += delta;
        }
        if let Some(projection_grad) = projection_grad.as_deref_mut() {
            for (hidden_index, hidden_value) in hidden.iter().enumerate() {
                let projection_index = hidden_index * probabilities.len() + token_index;
                projection_grad[projection_index] += hidden_value * delta;
            }
        }
        if let Some(relative_target_output_bias_grad) =
            relative_target_output_bias_grad.as_deref_mut()
        {
            let bias_index = relative_target_bias_offset + token_index;
            if bias_index < relative_target_output_bias_grad.len() {
                relative_target_output_bias_grad[bias_index] += delta;
            }
        }
        if let Some(relative_target_output_projection_grad) =
            relative_target_output_projection_grad.as_deref_mut()
        {
            for (hidden_index, hidden_value) in hidden.iter().enumerate() {
                let projection_index =
                    relative_target_projection_offset + hidden_index * vocab_size + token_index;
                if projection_index < relative_target_output_projection_grad.len() {
                    relative_target_output_projection_grad[projection_index] +=
                        hidden_value * delta;
                }
            }
        }
        if let Some(relative_target_transition_output_bias_grad) =
            relative_target_transition_output_bias_grad.as_deref_mut()
        {
            let transition_index = relative_target_transition_offset + token_index;
            if transition_index < relative_target_transition_output_bias_grad.len() {
                relative_target_transition_output_bias_grad[transition_index] += delta;
            }
        }
    }
    -probability.ln() * loss_scale
}

fn loss_scale_for_target_index(
    stage: &TassadarExecutorAttentionTrainingStage,
    target_index: usize,
) -> f32 {
    if target_index == 0 {
        stage.first_target_loss_scale
    } else if target_index < stage.early_token_prefix_len {
        stage.early_token_loss_scale
    } else {
        1.0
    }
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut exps = logits
        .iter()
        .map(|logit| (logit - max_logit).exp())
        .collect::<Vec<_>>();
    let sum = exps.iter().sum::<f32>().max(1e-8);
    for value in &mut exps {
        *value /= sum;
    }
    exps
}

fn attention_correctness_rank(
    report: &TassadarExecutorArchitectureFamilyReport,
) -> (u32, u32, u32, u32, u32) {
    (
        report.correctness.first_target_exactness_bps,
        report.correctness.first_32_token_exactness_bps,
        report.correctness.first_8_token_exactness_bps,
        report.correctness.exact_trace_case_count,
        report.correctness.aggregate_target_token_exactness_bps,
    )
}

fn write_json<T>(path: PathBuf, value: &T) -> Result<(), TassadarExecutorAttentionTrainingError>
where
    T: Serialize,
{
    let bytes =
        serde_json::to_vec_pretty(value).expect("attention training artifact should serialize");
    fs::write(&path, bytes).map_err(|error| TassadarExecutorAttentionTrainingError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."))
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar attention training value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

const fn default_true() -> bool {
    true
}

const fn default_one() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_EXECUTOR_ATTENTION_RUN_BUNDLE_FILE,
        TASSADAR_EXECUTOR_ATTENTION_TRAINING_REPORT_FILE, TassadarExecutorAttentionTrainingConfig,
        run_tassadar_executor_attention_training, train_tassadar_executor_attention_windowed,
    };

    #[test]
    fn attention_training_reduces_loss_and_writes_bundle() -> Result<(), Box<dyn std::error::Error>>
    {
        let outcome = train_tassadar_executor_attention_windowed(
            &TassadarExecutorAttentionTrainingConfig::reference(),
        )?;
        let first_loss = outcome
            .report
            .epoch_reports
            .first()
            .expect("attention training should emit an epoch report")
            .mean_loss;
        let last_loss = outcome
            .report
            .epoch_reports
            .last()
            .expect("attention training should emit an epoch report")
            .mean_loss;
        assert!(last_loss < first_loss);

        let temp = tempdir()?;
        let bundle = run_tassadar_executor_attention_training(temp.path())?;
        assert!(!bundle.bundle_digest.is_empty());
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_ATTENTION_TRAINING_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_ATTENTION_RUN_BUNDLE_FILE)
                .exists()
        );
        Ok(())
    }
}
