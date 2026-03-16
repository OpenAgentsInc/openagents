use std::{collections::BTreeMap, env, time::Instant};

use psionic_data::{TassadarSequenceExample, TassadarSequenceSplit};
use psionic_eval::{
    benchmark_tassadar_executor_linear_decode, build_tassadar_sequence_dataset,
    evaluate_tassadar_executor_transformer_with_target_cap_and_progress, TassadarExecutorEvalError,
    TassadarExecutorEvalReport, TassadarExecutorLinearBenchmarkError,
    TassadarExecutorLinearBenchmarkReport, TassadarSequenceEvalError, TassadarSequenceWorkload,
};
use psionic_models::{
    TassadarExecutorTrainableSurface, TassadarExecutorTransformer,
    TassadarExecutorTransformerError, TokenId, TokenSequence,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_sequence_training_manifest, TassadarSequenceTrainingError,
    TassadarSequenceTrainingManifest,
};

fn default_tassadar_sequence_workload() -> TassadarSequenceWorkload {
    TassadarSequenceWorkload::SudokuV0
}

fn default_validate_every_epoch() -> bool {
    true
}

fn default_select_best_checkpoint_by_boundary() -> bool {
    true
}

fn default_trainable_surface() -> TassadarExecutorTrainableSurface {
    TassadarExecutorTrainableSurface::OutputHeadOnly
}

fn tassadar_progress_updates_enabled() -> bool {
    match env::var("OPENAGENTS_TASSADAR_PROGRESS") {
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

fn stage_prefix_mode_is_teacher_forced(mode: &TassadarExecutorStagePrefixMode) -> bool {
    *mode == TassadarExecutorStagePrefixMode::TeacherForced
}

/// Prefix construction mode for one curriculum stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorStagePrefixMode {
    /// Use the frozen reference prefix for every supervised target token.
    TeacherForced,
    /// Feed the model's own greedy predictions back into the prefix during training.
    GreedyRollout,
}

impl TassadarExecutorStagePrefixMode {
    /// Returns a stable label for reports and audits.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TeacherForced => "teacher_forced",
            Self::GreedyRollout => "greedy_rollout",
        }
    }
}

impl Default for TassadarExecutorStagePrefixMode {
    fn default() -> Self {
        Self::TeacherForced
    }
}

/// One curriculum stage for the trained executor lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorCurriculumStage {
    /// Stable stage identifier.
    pub stage_id: String,
    /// Max target tokens supervised per example during the stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_train_target_tokens_per_example: Option<usize>,
    /// Number of epochs to spend in the stage.
    pub epochs: u32,
    /// Optional multiplier over the base learning rate for this stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_rate_scale: Option<f32>,
    /// Prefix construction mode used while supervising the stage.
    #[serde(default, skip_serializing_if = "stage_prefix_mode_is_teacher_forced")]
    pub prefix_mode: TassadarExecutorStagePrefixMode,
}

impl TassadarExecutorCurriculumStage {
    /// Creates one named curriculum stage.
    #[must_use]
    pub fn new(
        stage_id: impl Into<String>,
        max_train_target_tokens_per_example: Option<usize>,
        epochs: u32,
    ) -> Self {
        Self {
            stage_id: stage_id.into(),
            max_train_target_tokens_per_example,
            epochs,
            learning_rate_scale: None,
            prefix_mode: TassadarExecutorStagePrefixMode::TeacherForced,
        }
    }

    /// Overrides the stage-local learning-rate scale.
    #[must_use]
    pub const fn with_learning_rate_scale(mut self, learning_rate_scale: f32) -> Self {
        self.learning_rate_scale = Some(learning_rate_scale);
        self
    }

    /// Overrides the stage-local prefix construction mode.
    #[must_use]
    pub const fn with_prefix_mode(mut self, prefix_mode: TassadarExecutorStagePrefixMode) -> Self {
        self.prefix_mode = prefix_mode;
        self
    }
}

/// Bounded next-token training config for the first neural executor family.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorTrainingConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset/workload family for the run.
    #[serde(default = "default_tassadar_sequence_workload")]
    pub workload: TassadarSequenceWorkload,
    /// Dataset version to freeze for the run.
    pub dataset_version: String,
    /// Number of deterministic epochs in the terminal full-trace stage.
    pub epochs: u32,
    /// SGD learning rate for the output projection.
    pub learning_rate: f32,
    /// Optional cap over target tokens consumed from each train example in the terminal stage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_train_target_tokens_per_example: Option<usize>,
    /// Optional cap over target tokens evaluated from each validation example.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_eval_target_tokens_per_example: Option<usize>,
    /// Optional multiplier over the base learning rate for the terminal full-trace stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_stage_learning_rate_scale: Option<f32>,
    /// Active trainable surface for the run.
    #[serde(default = "default_trainable_surface")]
    pub trainable_surface: TassadarExecutorTrainableSurface,
    /// Optional boundary curriculum preceding the terminal full-trace stage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curriculum_stages: Vec<TassadarExecutorCurriculumStage>,
    /// Whether validation should execute after every epoch.
    #[serde(default = "default_validate_every_epoch")]
    pub validate_every_epoch: bool,
    /// Whether checkpoint export should select the best epoch by boundary metrics.
    #[serde(default = "default_select_best_checkpoint_by_boundary")]
    pub select_best_checkpoint_by_boundary: bool,
}

impl TassadarExecutorTrainingConfig {
    /// Returns the preserved weak baseline config used by the first honest tests.
    #[must_use]
    pub fn reference() -> Self {
        Self {
            run_id: String::from("tassadar-executor-transformer-train-v0"),
            workload: TassadarSequenceWorkload::SudokuV0,
            dataset_version: String::from("train-v0"),
            epochs: 1,
            learning_rate: 0.05,
            max_train_target_tokens_per_example: Some(256),
            max_eval_target_tokens_per_example: None,
            terminal_stage_learning_rate_scale: None,
            trainable_surface: TassadarExecutorTrainableSurface::OutputHeadOnly,
            curriculum_stages: Vec::new(),
            validate_every_epoch: true,
            select_best_checkpoint_by_boundary: true,
        }
    }

    /// Returns the first boundary-focused multi-stage curriculum config.
    #[must_use]
    pub fn boundary_curriculum_reference() -> Self {
        Self {
            run_id: String::from("tassadar-executor-transformer-sudoku-v0-boundary-v1"),
            workload: TassadarSequenceWorkload::SudokuV0,
            dataset_version: String::from("train-v0"),
            epochs: 1,
            learning_rate: 0.05,
            max_train_target_tokens_per_example: None,
            max_eval_target_tokens_per_example: None,
            terminal_stage_learning_rate_scale: None,
            trainable_surface: TassadarExecutorTrainableSurface::OutputHeadOnly,
            curriculum_stages: vec![
                TassadarExecutorCurriculumStage::new("prompt_to_first_token", Some(1), 1),
                TassadarExecutorCurriculumStage::new("prompt_to_first_2_tokens", Some(2), 1),
                TassadarExecutorCurriculumStage::new("prompt_to_first_4_tokens", Some(4), 1),
                TassadarExecutorCurriculumStage::new("prompt_to_first_8_tokens", Some(8), 1),
                TassadarExecutorCurriculumStage::new("prompt_to_first_16_tokens", Some(16), 1),
                TassadarExecutorCurriculumStage::new("prompt_to_first_32_tokens", Some(32), 1),
            ],
            validate_every_epoch: true,
            select_best_checkpoint_by_boundary: true,
        }
    }

    /// Returns a small 9x9 scale-out smoke config.
    #[must_use]
    pub fn sudoku_9x9_scale_smoke() -> Self {
        Self {
            run_id: String::from("tassadar-executor-transformer-sudoku-9x9-scale-smoke-v0"),
            workload: TassadarSequenceWorkload::Sudoku9x9,
            dataset_version: String::from("scale-v0"),
            epochs: 1,
            learning_rate: 0.05,
            max_train_target_tokens_per_example: Some(8),
            max_eval_target_tokens_per_example: Some(8),
            terminal_stage_learning_rate_scale: None,
            trainable_surface: TassadarExecutorTrainableSurface::OutputHeadOnly,
            curriculum_stages: Vec::new(),
            validate_every_epoch: true,
            select_best_checkpoint_by_boundary: true,
        }
    }

    /// Overrides the trainable surface while preserving the rest of the config.
    #[must_use]
    pub const fn with_trainable_surface(
        mut self,
        trainable_surface: TassadarExecutorTrainableSurface,
    ) -> Self {
        self.trainable_surface = trainable_surface;
        self
    }

    fn resolved_stages(&self) -> Vec<TassadarExecutorCurriculumStage> {
        let mut stages = self.curriculum_stages.clone();
        stages.push(
            TassadarExecutorCurriculumStage::new(
                "full_trace_supervision",
                self.max_train_target_tokens_per_example,
                self.epochs,
            )
            .with_learning_rate_scale(self.terminal_stage_learning_rate_scale.unwrap_or(1.0)),
        );
        stages
    }
}

impl Default for TassadarExecutorTrainingConfig {
    fn default() -> Self {
        Self::reference()
    }
}

/// Per-batch deterministic training receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorTrainingBatchReport {
    /// Zero-based global epoch index across all stages.
    pub global_epoch_index: u32,
    /// Stage identifier active for the batch.
    pub stage_id: String,
    /// Zero-based epoch index inside the stage.
    pub stage_epoch_index: u32,
    /// Frozen cap over target tokens used during the stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_max_train_target_tokens_per_example: Option<usize>,
    /// Effective learning rate used during the stage.
    pub stage_learning_rate: f32,
    /// Prefix construction mode used during the stage.
    pub stage_prefix_mode: TassadarExecutorStagePrefixMode,
    /// Stable batch identifier from the frozen packing manifest.
    pub batch_id: String,
    /// Stable source sequence identifiers in the batch.
    pub sequence_ids: Vec<String>,
    /// Mean next-token loss over the batch.
    pub mean_loss: f32,
    /// Number of supervised target tokens consumed.
    pub target_token_count: u32,
}

/// Per-epoch deterministic training receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorTrainingEpochReport {
    /// Stable checkpoint identifier for the epoch.
    pub checkpoint_id: String,
    /// Zero-based global epoch index across all stages.
    pub global_epoch_index: u32,
    /// Stage identifier active for the epoch.
    pub stage_id: String,
    /// Zero-based epoch index inside the stage.
    pub stage_epoch_index: u32,
    /// Frozen cap over target tokens used during the stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_max_train_target_tokens_per_example: Option<usize>,
    /// Effective learning rate used during the stage.
    pub stage_learning_rate: f32,
    /// Prefix construction mode used during the stage.
    pub stage_prefix_mode: TassadarExecutorStagePrefixMode,
    /// Mean next-token loss over the epoch.
    pub mean_loss: f32,
    /// Number of supervised target tokens consumed.
    pub target_token_count: u32,
    /// Validation report recorded for the checkpoint.
    pub evaluation: TassadarExecutorEvalReport,
}

/// One machine-readable checkpoint ranking entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorCheckpointLeaderboardEntry {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Zero-based global epoch index across all stages.
    pub global_epoch_index: u32,
    /// Stage identifier active for the checkpoint.
    pub stage_id: String,
    /// Zero-based epoch index inside the stage.
    pub stage_epoch_index: u32,
    /// First-target exactness used by the boundary selector.
    pub first_target_exactness_bps: u32,
    /// First-eight-target exactness used by the boundary selector.
    pub first_8_token_exactness_bps: u32,
    /// First-32-target exactness used by the boundary selector.
    pub first_32_token_exactness_bps: u32,
    /// Exact-trace validation case count.
    pub exact_trace_case_count: u32,
    /// Aggregate target-token exactness over validation.
    pub aggregate_target_token_exactness_bps: u32,
    /// Whether this checkpoint won export selection.
    pub selected_for_export: bool,
}

/// Aggregate training report plus validation exactness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorTrainingReport {
    /// Frozen config used for the run.
    pub config: TassadarExecutorTrainingConfig,
    /// Frozen sequence-manifest identity.
    pub training_manifest_digest: String,
    /// Stable descriptor digest for the trained model.
    pub trained_model_descriptor_digest: String,
    /// Stable trained weight digest.
    pub trained_weight_digest: String,
    /// Stable selected checkpoint identifier.
    pub best_checkpoint_id: String,
    /// Explicit checkpoint selection basis.
    pub checkpoint_selection_basis: String,
    /// Per-batch training receipts.
    pub batch_reports: Vec<TassadarExecutorTrainingBatchReport>,
    /// Per-epoch training receipts.
    pub epoch_reports: Vec<TassadarExecutorTrainingEpochReport>,
    /// Boundary-ranked checkpoint leaderboard.
    pub checkpoint_leaderboard: Vec<TassadarExecutorCheckpointLeaderboardEntry>,
    /// Aggregate validation report against CPU-reference truth for the selected checkpoint.
    pub evaluation: TassadarExecutorEvalReport,
    /// Stable digest over the training report.
    pub report_digest: String,
}

impl TassadarExecutorTrainingReport {
    fn new(
        config: TassadarExecutorTrainingConfig,
        manifest: &TassadarSequenceTrainingManifest,
        model: &TassadarExecutorTransformer,
        best_checkpoint_id: String,
        batch_reports: Vec<TassadarExecutorTrainingBatchReport>,
        epoch_reports: Vec<TassadarExecutorTrainingEpochReport>,
        checkpoint_leaderboard: Vec<TassadarExecutorCheckpointLeaderboardEntry>,
        evaluation: TassadarExecutorEvalReport,
    ) -> Self {
        let mut report = Self {
            config,
            training_manifest_digest: manifest.manifest_digest.clone(),
            trained_model_descriptor_digest: model.descriptor().stable_digest(),
            trained_weight_digest: model.descriptor().weights.digest.clone(),
            best_checkpoint_id,
            checkpoint_selection_basis: String::from("boundary_metrics_lexicographic_v1"),
            batch_reports,
            epoch_reports,
            checkpoint_leaderboard,
            evaluation,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_executor_training_report|", &report);
        report
    }
}

/// Full training outcome containing the updated model plus the machine-readable report.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarExecutorTrainingOutcome {
    /// Best selected model after bounded next-token updates.
    pub model: TassadarExecutorTransformer,
    /// Machine-readable report.
    pub report: TassadarExecutorTrainingReport,
}

/// Training failure for the neural executor family.
#[derive(Debug, Error)]
pub enum TassadarExecutorTrainingError {
    /// Tokenized dataset generation failed.
    #[error(transparent)]
    SequenceEval(#[from] TassadarSequenceEvalError),
    /// Sequence manifest generation failed.
    #[error(transparent)]
    SequenceTraining(#[from] TassadarSequenceTrainingError),
    /// Validation exactness evaluation failed.
    #[error(transparent)]
    Eval(#[from] TassadarExecutorEvalError),
    /// Neural linear benchmark failed.
    #[error(transparent)]
    Benchmark(#[from] TassadarExecutorLinearBenchmarkError),
    /// Model forward/decode failed.
    #[error(transparent)]
    Model(#[from] TassadarExecutorTransformerError),
    /// The run did not emit any checkpoint candidates.
    #[error("tassadar executor training run `{run_id}` emitted no checkpoint candidates")]
    NoCheckpointCandidates {
        /// Stable run identifier.
        run_id: String,
    },
}

/// Trains the first neural executor family on one frozen Tassadar token-sequence corpus.
pub fn train_tassadar_executor_transformer(
    config: &TassadarExecutorTrainingConfig,
) -> Result<TassadarExecutorTrainingOutcome, TassadarExecutorTrainingError> {
    let run_started_at = Instant::now();
    let bundle = build_tassadar_sequence_dataset(config.workload, config.dataset_version.as_str())?;
    let manifest = build_tassadar_sequence_training_manifest(
        config.workload,
        config.dataset_version.as_str(),
        config.trainable_surface,
    )?;
    let mut current_model = match config.workload {
        TassadarSequenceWorkload::SudokuV0 => {
            TassadarExecutorTransformer::sudoku_v0_with_surface(config.trainable_surface)
        }
        TassadarSequenceWorkload::Sudoku9x9 => {
            TassadarExecutorTransformer::sudoku_9x9_with_surface(config.trainable_surface)
        }
    };
    let examples_by_id = bundle
        .dataset
        .examples
        .iter()
        .map(|example| (example.sequence_id.clone(), example))
        .collect::<BTreeMap<_, _>>();
    let mut batch_reports = Vec::new();
    let mut epoch_reports = Vec::new();
    let mut current_epoch_batches = Vec::new();
    let mut checkpoint_leaderboard = Vec::new();
    let mut best_model: Option<TassadarExecutorTransformer> = None;
    let mut best_epoch_report: Option<TassadarExecutorTrainingEpochReport> = None;
    let mut global_epoch_index = 0_u32;
    let resolved_stages = config.resolved_stages();
    let total_stage_count = resolved_stages.len();
    let total_epoch_count = resolved_stages
        .iter()
        .map(|stage| stage.epochs)
        .sum::<u32>();
    let batches_per_epoch = manifest.train_plan.batches.len();

    emit_tassadar_progress(format!(
        "tassadar_progress phase=train_start run={} workload={} surface={} stages={} epochs={} train_batches_per_epoch={} train_examples={} validation_examples={} dataset={} elapsed_ms={}",
        config.run_id,
        config.workload.dataset_ref(),
        config.trainable_surface.label(),
        total_stage_count,
        total_epoch_count,
        batches_per_epoch,
        bundle
            .dataset
            .examples
            .iter()
            .filter(|example| example.metadata.split == TassadarSequenceSplit::Train)
            .count(),
        bundle
            .dataset
            .examples
            .iter()
            .filter(|example| example.metadata.split == TassadarSequenceSplit::Validation)
            .count(),
        config.dataset_version,
        run_started_at.elapsed().as_millis(),
    ));

    for (stage_index, stage) in resolved_stages.iter().enumerate() {
        let stage_learning_rate = config.learning_rate * stage.learning_rate_scale.unwrap_or(1.0);
        emit_tassadar_progress(format!(
            "tassadar_progress phase=stage_start run={} stage={}/{} stage_id={} prefix_mode={} epochs={} target_cap={} learning_rate={:.6} global_epoch_start={} elapsed_ms={}",
            config.run_id,
            stage_index + 1,
            total_stage_count,
            stage.stage_id,
            stage.prefix_mode.label(),
            stage.epochs,
            stage
                .max_train_target_tokens_per_example
                .map_or_else(|| String::from("full"), |value| value.to_string()),
            stage_learning_rate,
            global_epoch_index,
            run_started_at.elapsed().as_millis(),
        ));
        for stage_epoch_index in 0..stage.epochs {
            let epoch_started_at = Instant::now();
            current_epoch_batches.clear();
            emit_tassadar_progress(format!(
                "tassadar_progress phase=epoch_start run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} prefix_mode={} target_cap={} learning_rate={:.6} batches={} elapsed_ms={}",
                config.run_id,
                stage.stage_id,
                stage_epoch_index + 1,
                stage.epochs,
                global_epoch_index + 1,
                total_epoch_count,
                stage.prefix_mode.label(),
                stage
                    .max_train_target_tokens_per_example
                    .map_or_else(|| String::from("full"), |value| value.to_string()),
                stage_learning_rate,
                batches_per_epoch,
                run_started_at.elapsed().as_millis(),
            ));
            for (batch_index, batch) in manifest.train_plan.batches.iter().enumerate() {
                let batch_started_at = Instant::now();
                let hidden_width = current_model.descriptor().config.hidden_width();
                let vocab_size = current_model.descriptor().config.vocab_size;
                let sequence_ids = batch
                    .rows
                    .iter()
                    .flat_map(|row| row.source_sequences.iter())
                    .map(|sequence| sequence.sequence_id.clone())
                    .collect::<Vec<_>>();
                let sequence_count = sequence_ids.len();
                let estimated_target_tokens = sequence_ids
                    .iter()
                    .map(|sequence_id| {
                        examples_by_id
                            .get(sequence_id.as_str())
                            .expect("frozen train plan should reference known examples")
                    })
                    .map(|example| stage_target_token_count(example, &stage))
                    .sum::<usize>();
                emit_tassadar_progress(format!(
                    "tassadar_progress phase=batch_start run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} batch={}/{} batch_id={} sequences={} estimated_target_tokens={} elapsed_ms={}",
                    config.run_id,
                    stage.stage_id,
                    stage_epoch_index + 1,
                    stage.epochs,
                    global_epoch_index + 1,
                    total_epoch_count,
                    batch_index + 1,
                    batches_per_epoch,
                    batch.batch_id,
                    sequence_count,
                    estimated_target_tokens,
                    run_started_at.elapsed().as_millis(),
                ));
                let mut projection_grad = vec![0.0; hidden_width * vocab_size];
                let mut bias_grad = vec![0.0; vocab_size];
                let mut token_embedding_grad = config
                    .trainable_surface
                    .trains_token_embeddings()
                    .then(|| vec![0.0; current_model.weights().token_embeddings().len()]);
                let mut position_embedding_grad = config
                    .trainable_surface
                    .trains_position_embeddings()
                    .then(|| vec![0.0; current_model.weights().position_embeddings().len()]);
                let mut mixer_projection_grad = config
                    .trainable_surface
                    .trains_small_learned_mixer()
                    .then(|| vec![0.0; hidden_width * hidden_width]);
                let mut mixer_bias_grad = config
                    .trainable_surface
                    .trains_small_learned_mixer()
                    .then(|| vec![0.0; hidden_width]);
                let mut total_loss = 0.0_f32;
                let mut target_token_count = 0_u32;

                for (sequence_index, sequence_id) in sequence_ids.iter().enumerate() {
                    let example = examples_by_id
                        .get(sequence_id.as_str())
                        .expect("frozen train plan should reference known examples");
                    let stage_target_tokens = stage_target_token_count(example, &stage);
                    emit_tassadar_progress(format!(
                        "tassadar_progress phase=sequence_start run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} batch={}/{} batch_id={} sequence={}/{} sequence_id={} case_id={} prompt_tokens={} target_tokens={} prefix_mode={} batch_ms={} epoch_ms={} elapsed_ms={}",
                        config.run_id,
                        stage.stage_id,
                        stage_epoch_index + 1,
                        stage.epochs,
                        global_epoch_index + 1,
                        total_epoch_count,
                        batch_index + 1,
                        batches_per_epoch,
                        batch.batch_id,
                        sequence_index + 1,
                        sequence_count,
                        example.sequence_id,
                        example.metadata.case_id,
                        example.metadata.prompt_token_count,
                        stage_target_tokens,
                        stage.prefix_mode.label(),
                        batch_started_at.elapsed().as_millis(),
                        epoch_started_at.elapsed().as_millis(),
                        run_started_at.elapsed().as_millis(),
                    ));
                    let (sequence_loss, sequence_target_token_count) =
                        accumulate_sequence_gradients(
                            &current_model,
                            example,
                            &stage,
                            projection_grad.as_mut_slice(),
                            bias_grad.as_mut_slice(),
                            token_embedding_grad.as_deref_mut(),
                            position_embedding_grad.as_deref_mut(),
                            mixer_projection_grad.as_deref_mut(),
                            mixer_bias_grad.as_deref_mut(),
                        )?;
                    total_loss += sequence_loss;
                    target_token_count =
                        target_token_count.saturating_add(sequence_target_token_count);
                    emit_tassadar_progress(format!(
                        "tassadar_progress phase=sequence_complete run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} batch={}/{} batch_id={} sequence={}/{} sequence_id={} case_id={} target_tokens={} sequence_mean_loss={:.6} cumulative_target_tokens={} batch_ms={} epoch_ms={} elapsed_ms={}",
                        config.run_id,
                        stage.stage_id,
                        stage_epoch_index + 1,
                        stage.epochs,
                        global_epoch_index + 1,
                        total_epoch_count,
                        batch_index + 1,
                        batches_per_epoch,
                        batch.batch_id,
                        sequence_index + 1,
                        sequence_count,
                        example.sequence_id,
                        example.metadata.case_id,
                        sequence_target_token_count,
                        if sequence_target_token_count == 0 {
                            0.0
                        } else {
                            sequence_loss / sequence_target_token_count as f32
                        },
                        target_token_count,
                        batch_started_at.elapsed().as_millis(),
                        epoch_started_at.elapsed().as_millis(),
                        run_started_at.elapsed().as_millis(),
                    ));
                }

                if target_token_count > 0 {
                    let scale = stage_learning_rate / target_token_count as f32;
                    for (weight, gradient) in current_model
                        .weights_mut()
                        .output_projection_mut()
                        .iter_mut()
                        .zip(projection_grad.iter())
                    {
                        *weight -= scale * gradient;
                    }
                    for (bias, gradient) in current_model
                        .weights_mut()
                        .output_bias_mut()
                        .iter_mut()
                        .zip(bias_grad.iter())
                    {
                        *bias -= scale * gradient;
                    }
                    if let Some(token_embedding_grad) = token_embedding_grad.as_ref() {
                        for (weight, gradient) in current_model
                            .weights_mut()
                            .token_embeddings_mut()
                            .iter_mut()
                            .zip(token_embedding_grad.iter())
                        {
                            *weight -= scale * gradient;
                        }
                    }
                    if let Some(position_embedding_grad) = position_embedding_grad.as_ref() {
                        for (weight, gradient) in current_model
                            .weights_mut()
                            .position_embeddings_mut()
                            .iter_mut()
                            .zip(position_embedding_grad.iter())
                        {
                            *weight -= scale * gradient;
                        }
                    }
                    if let Some(mixer_projection_grad) = mixer_projection_grad.as_ref() {
                        for (weight, gradient) in current_model
                            .weights_mut()
                            .small_learned_mixer_projection_mut()
                            .iter_mut()
                            .zip(mixer_projection_grad.iter())
                        {
                            *weight -= scale * gradient;
                        }
                    }
                    if let Some(mixer_bias_grad) = mixer_bias_grad.as_ref() {
                        for (weight, gradient) in current_model
                            .weights_mut()
                            .small_learned_mixer_bias_mut()
                            .iter_mut()
                            .zip(mixer_bias_grad.iter())
                        {
                            *weight -= scale * gradient;
                        }
                    }
                    current_model.refresh_after_training();
                }

                let batch_report = TassadarExecutorTrainingBatchReport {
                    global_epoch_index,
                    stage_id: stage.stage_id.clone(),
                    stage_epoch_index,
                    stage_max_train_target_tokens_per_example: stage
                        .max_train_target_tokens_per_example,
                    stage_learning_rate,
                    stage_prefix_mode: stage.prefix_mode,
                    batch_id: batch.batch_id.clone(),
                    sequence_ids,
                    mean_loss: if target_token_count == 0 {
                        0.0
                    } else {
                        total_loss / target_token_count as f32
                    },
                    target_token_count,
                };
                current_epoch_batches.push(batch_report.clone());
                batch_reports.push(batch_report);
                emit_tassadar_progress(format!(
                    "tassadar_progress phase=batch_complete run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} batch={}/{} batch_id={} sequences={} target_tokens={} mean_loss={:.6} batch_ms={} epoch_ms={} elapsed_ms={}",
                    config.run_id,
                    stage.stage_id,
                    stage_epoch_index + 1,
                    stage.epochs,
                    global_epoch_index + 1,
                    total_epoch_count,
                    batch_index + 1,
                    batches_per_epoch,
                    batch.batch_id,
                    sequence_count,
                    target_token_count,
                    if target_token_count == 0 {
                        0.0
                    } else {
                        total_loss / target_token_count as f32
                    },
                    batch_started_at.elapsed().as_millis(),
                    epoch_started_at.elapsed().as_millis(),
                    run_started_at.elapsed().as_millis(),
                ));
            }

            let validation_case_count = bundle
                .dataset
                .split_examples(TassadarSequenceSplit::Validation)
                .len();
            emit_tassadar_progress(format!(
                "tassadar_progress phase=validation_start run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} cases={} target_cap={} elapsed_ms={}",
                config.run_id,
                stage.stage_id,
                stage_epoch_index + 1,
                stage.epochs,
                global_epoch_index + 1,
                total_epoch_count,
                validation_case_count,
                config
                    .max_eval_target_tokens_per_example
                    .map_or_else(|| String::from("full"), |value| value.to_string()),
                run_started_at.elapsed().as_millis(),
            ));
            let validation_started_at = Instant::now();
            let evaluation = if config.validate_every_epoch {
                evaluate_tassadar_executor_transformer_with_target_cap_and_progress(
                    &current_model,
                    &bundle.dataset,
                    TassadarSequenceSplit::Validation,
                    config.max_eval_target_tokens_per_example,
                    |progress| {
                        emit_tassadar_progress(format!(
                            "tassadar_progress phase=validation_case_complete run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} case={}/{} sequence_id={} case_id={} target_tokens={} matched_tokens={} first_divergence={} exact_trace={} final_output_match={} halt_match={} validation_ms={} elapsed_ms={}",
                            config.run_id,
                            stage.stage_id,
                            stage_epoch_index + 1,
                            stage.epochs,
                            global_epoch_index + 1,
                            total_epoch_count,
                            progress.case_index,
                            progress.case_count,
                            progress.sequence_id,
                            progress.case_id,
                            progress.evaluated_target_token_count,
                            progress.matched_target_token_count,
                            progress
                                .first_divergence_index
                                .map_or_else(|| String::from("none"), |value| value.to_string()),
                            progress.exact_trace_match,
                            progress.final_output_match,
                            progress.halt_match,
                            validation_started_at.elapsed().as_millis(),
                            run_started_at.elapsed().as_millis(),
                        ));
                    },
                )?
            } else {
                evaluate_tassadar_executor_transformer_with_target_cap_and_progress(
                    &current_model,
                    &bundle.dataset,
                    TassadarSequenceSplit::Validation,
                    config.max_eval_target_tokens_per_example,
                    |progress| {
                        emit_tassadar_progress(format!(
                            "tassadar_progress phase=validation_case_complete run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} case={}/{} sequence_id={} case_id={} target_tokens={} matched_tokens={} first_divergence={} exact_trace={} final_output_match={} halt_match={} validation_ms={} elapsed_ms={}",
                            config.run_id,
                            stage.stage_id,
                            stage_epoch_index + 1,
                            stage.epochs,
                            global_epoch_index + 1,
                            total_epoch_count,
                            progress.case_index,
                            progress.case_count,
                            progress.sequence_id,
                            progress.case_id,
                            progress.evaluated_target_token_count,
                            progress.matched_target_token_count,
                            progress
                                .first_divergence_index
                                .map_or_else(|| String::from("none"), |value| value.to_string()),
                            progress.exact_trace_match,
                            progress.final_output_match,
                            progress.halt_match,
                            validation_started_at.elapsed().as_millis(),
                            run_started_at.elapsed().as_millis(),
                        ));
                    },
                )?
            };
            emit_tassadar_progress(format!(
                "tassadar_progress phase=validation_complete run={} stage_id={} stage_epoch={}/{} global_epoch={}/{} mean_loss={:.6} target_tokens={} aggregate_bps={} first_target_bps={} first_8_bps={} first_32_bps={} exact_traces={} epoch_ms={} elapsed_ms={}",
                config.run_id,
                stage.stage_id,
                stage_epoch_index + 1,
                stage.epochs,
                global_epoch_index + 1,
                total_epoch_count,
                mean_batch_loss(current_epoch_batches.as_slice()),
                current_epoch_batches
                    .iter()
                    .map(|batch| batch.target_token_count)
                    .sum::<u32>(),
                evaluation.aggregate_target_token_exactness_bps,
                evaluation.first_target_exactness_bps,
                evaluation.first_8_token_exactness_bps,
                evaluation.first_32_token_exactness_bps,
                evaluation.exact_trace_case_count,
                epoch_started_at.elapsed().as_millis(),
                run_started_at.elapsed().as_millis(),
            ));

            let checkpoint_id =
                format!("{}.checkpoint.epoch_{global_epoch_index:04}", config.run_id);
            let epoch_mean_loss = mean_batch_loss(current_epoch_batches.as_slice());
            let epoch_target_token_count = current_epoch_batches
                .iter()
                .map(|batch| batch.target_token_count)
                .sum();
            let epoch_report = TassadarExecutorTrainingEpochReport {
                checkpoint_id: checkpoint_id.clone(),
                global_epoch_index,
                stage_id: stage.stage_id.clone(),
                stage_epoch_index,
                stage_max_train_target_tokens_per_example: stage
                    .max_train_target_tokens_per_example,
                stage_learning_rate,
                stage_prefix_mode: stage.prefix_mode,
                mean_loss: epoch_mean_loss,
                target_token_count: epoch_target_token_count,
                evaluation: evaluation.clone(),
            };
            checkpoint_leaderboard.push(TassadarExecutorCheckpointLeaderboardEntry {
                checkpoint_id: checkpoint_id.clone(),
                global_epoch_index,
                stage_id: stage.stage_id.clone(),
                stage_epoch_index,
                first_target_exactness_bps: evaluation.first_target_exactness_bps,
                first_8_token_exactness_bps: evaluation.first_8_token_exactness_bps,
                first_32_token_exactness_bps: evaluation.first_32_token_exactness_bps,
                exact_trace_case_count: evaluation.exact_trace_case_count,
                aggregate_target_token_exactness_bps: evaluation
                    .aggregate_target_token_exactness_bps,
                selected_for_export: false,
            });
            let should_replace_best = match best_epoch_report.as_ref() {
                None => true,
                Some(best) => {
                    if config.select_best_checkpoint_by_boundary {
                        checkpoint_rank_tuple(&epoch_report.evaluation)
                            > checkpoint_rank_tuple(&best.evaluation)
                    } else {
                        epoch_report.evaluation.aggregate_target_token_exactness_bps
                            > best.evaluation.aggregate_target_token_exactness_bps
                    }
                }
            };
            if should_replace_best {
                best_model = Some(current_model.clone());
                best_epoch_report = Some(epoch_report.clone());
                emit_tassadar_progress(format!(
                    "tassadar_progress phase=best_checkpoint_updated run={} checkpoint_id={} stage_id={} global_epoch={}/{} aggregate_bps={} first_target_bps={} first_8_bps={} first_32_bps={} exact_traces={} elapsed_ms={}",
                    config.run_id,
                    checkpoint_id,
                    stage.stage_id,
                    global_epoch_index + 1,
                    total_epoch_count,
                    evaluation.aggregate_target_token_exactness_bps,
                    evaluation.first_target_exactness_bps,
                    evaluation.first_8_token_exactness_bps,
                    evaluation.first_32_token_exactness_bps,
                    evaluation.exact_trace_case_count,
                    run_started_at.elapsed().as_millis(),
                ));
            }
            epoch_reports.push(epoch_report);
            global_epoch_index = global_epoch_index.saturating_add(1);
        }
    }

    let best_model =
        best_model.ok_or_else(|| TassadarExecutorTrainingError::NoCheckpointCandidates {
            run_id: config.run_id.clone(),
        })?;
    let best_epoch_report =
        best_epoch_report.ok_or_else(|| TassadarExecutorTrainingError::NoCheckpointCandidates {
            run_id: config.run_id.clone(),
        })?;

    checkpoint_leaderboard.sort_by(|left, right| {
        checkpoint_rank_entry(right)
            .cmp(&checkpoint_rank_entry(left))
            .then_with(|| left.global_epoch_index.cmp(&right.global_epoch_index))
    });
    for entry in &mut checkpoint_leaderboard {
        entry.selected_for_export = entry.checkpoint_id == best_epoch_report.checkpoint_id;
    }

    let report = TassadarExecutorTrainingReport::new(
        config.clone(),
        &manifest,
        &best_model,
        best_epoch_report.checkpoint_id,
        batch_reports,
        epoch_reports,
        checkpoint_leaderboard,
        best_epoch_report.evaluation,
    );
    emit_tassadar_progress(format!(
        "tassadar_progress phase=train_complete run={} best_checkpoint={} aggregate_bps={} first_target_bps={} first_8_bps={} first_32_bps={} exact_traces={} total_batches={} total_epochs={} elapsed_ms={}",
        config.run_id,
        report.best_checkpoint_id,
        report.evaluation.aggregate_target_token_exactness_bps,
        report.evaluation.first_target_exactness_bps,
        report.evaluation.first_8_token_exactness_bps,
        report.evaluation.first_32_token_exactness_bps,
        report.evaluation.exact_trace_case_count,
        report.batch_reports.len(),
        report.epoch_reports.len(),
        run_started_at.elapsed().as_millis(),
    ));
    Ok(TassadarExecutorTrainingOutcome {
        model: best_model,
        report,
    })
}

/// Trains the neural executor family and benchmarks its neural linear decode against CPU reference.
pub fn benchmark_trained_tassadar_executor_transformer(
    config: &TassadarExecutorTrainingConfig,
    split_filter: Option<TassadarSequenceSplit>,
) -> Result<TassadarExecutorLinearBenchmarkReport, TassadarExecutorTrainingError> {
    let outcome = train_tassadar_executor_transformer(config)?;
    let bundle = build_tassadar_sequence_dataset(config.workload, config.dataset_version.as_str())?;
    Ok(benchmark_tassadar_executor_linear_decode(
        &outcome.model,
        &bundle.dataset,
        split_filter,
    )?)
}

fn accumulate_sequence_gradients(
    current_model: &TassadarExecutorTransformer,
    example: &TassadarSequenceExample,
    stage: &TassadarExecutorCurriculumStage,
    projection_grad: &mut [f32],
    bias_grad: &mut [f32],
    token_embedding_grad: Option<&mut [f32]>,
    position_embedding_grad: Option<&mut [f32]>,
    mixer_projection_grad: Option<&mut [f32]>,
    mixer_bias_grad: Option<&mut [f32]>,
) -> Result<(f32, u32), TassadarExecutorTrainingError> {
    match stage.prefix_mode {
        TassadarExecutorStagePrefixMode::TeacherForced => {
            accumulate_teacher_forced_sequence_gradients(
                current_model,
                example,
                stage,
                projection_grad,
                bias_grad,
                token_embedding_grad,
                position_embedding_grad,
                mixer_projection_grad,
                mixer_bias_grad,
            )
        }
        TassadarExecutorStagePrefixMode::GreedyRollout => {
            accumulate_greedy_rollout_sequence_gradients(
                current_model,
                example,
                stage,
                projection_grad,
                bias_grad,
                token_embedding_grad,
                position_embedding_grad,
                mixer_projection_grad,
                mixer_bias_grad,
            )
        }
    }
}

fn accumulate_teacher_forced_sequence_gradients(
    current_model: &TassadarExecutorTransformer,
    example: &TassadarSequenceExample,
    stage: &TassadarExecutorCurriculumStage,
    projection_grad: &mut [f32],
    bias_grad: &mut [f32],
    mut token_embedding_grad: Option<&mut [f32]>,
    mut position_embedding_grad: Option<&mut [f32]>,
    mut mixer_projection_grad: Option<&mut [f32]>,
    mut mixer_bias_grad: Option<&mut [f32]>,
) -> Result<(f32, u32), TassadarExecutorTrainingError> {
    let max_target = stage
        .max_train_target_tokens_per_example
        .unwrap_or(example.metadata.target_token_count as usize)
        .min(example.metadata.target_token_count as usize);
    let effective_sequence_len = example.metadata.prompt_token_count as usize + max_target;
    let sequence = TokenSequence::new(
        example.token_ids[..effective_sequence_len]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>(),
    );
    let forward = current_model.forward_logits(&sequence)?;
    let start_logit_index = example.metadata.prompt_token_count.saturating_sub(1) as usize;
    let end_logit_index = (start_logit_index + max_target).min(forward.logits.len());
    let mut total_loss = 0.0_f32;
    let mut target_token_count = 0_u32;
    for logit_index in start_logit_index..end_logit_index {
        total_loss += accumulate_step_gradients(
            current_model,
            &forward.hidden_states[logit_index],
            &forward.source_hidden_states[logit_index],
            &forward.step_contexts[logit_index],
            &forward.logits[logit_index],
            sequence.as_slice()[logit_index + 1],
            projection_grad,
            bias_grad,
            token_embedding_grad.as_deref_mut(),
            position_embedding_grad.as_deref_mut(),
            mixer_projection_grad.as_deref_mut(),
            mixer_bias_grad.as_deref_mut(),
        );
        target_token_count = target_token_count.saturating_add(1);
    }
    Ok((total_loss, target_token_count))
}

fn accumulate_greedy_rollout_sequence_gradients(
    current_model: &TassadarExecutorTransformer,
    example: &TassadarSequenceExample,
    stage: &TassadarExecutorCurriculumStage,
    projection_grad: &mut [f32],
    bias_grad: &mut [f32],
    mut token_embedding_grad: Option<&mut [f32]>,
    mut position_embedding_grad: Option<&mut [f32]>,
    mut mixer_projection_grad: Option<&mut [f32]>,
    mut mixer_bias_grad: Option<&mut [f32]>,
) -> Result<(f32, u32), TassadarExecutorTrainingError> {
    let prompt_token_count = example.metadata.prompt_token_count as usize;
    let max_target = stage
        .max_train_target_tokens_per_example
        .unwrap_or(example.metadata.target_token_count as usize)
        .min(example.metadata.target_token_count as usize);
    let prompt = TokenSequence::new(
        example.token_ids[..prompt_token_count]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>(),
    );
    let reference_target = example.token_ids[prompt_token_count..prompt_token_count + max_target]
        .iter()
        .map(|token| TokenId(*token))
        .collect::<Vec<_>>();
    let mut state = current_model.start_decode(prompt)?;
    let mut total_loss = 0.0_f32;
    let mut target_token_count = 0_u32;
    for target_token in reference_target {
        let step = current_model.decode_step(&state)?;
        total_loss += accumulate_step_gradients(
            current_model,
            step.hidden_state.as_slice(),
            step.source_hidden_state.as_slice(),
            &step.step_context,
            step.logits.as_slice(),
            target_token,
            projection_grad,
            bias_grad,
            token_embedding_grad.as_deref_mut(),
            position_embedding_grad.as_deref_mut(),
            mixer_projection_grad.as_deref_mut(),
            mixer_bias_grad.as_deref_mut(),
        );
        target_token_count = target_token_count.saturating_add(1);
        current_model
            .push_decoded_token(&mut state, greedy_token_from_logits(step.logits.as_slice()))?;
    }
    Ok((total_loss, target_token_count))
}

#[allow(clippy::too_many_arguments)]
fn accumulate_step_gradients(
    current_model: &TassadarExecutorTransformer,
    hidden: &[f32],
    source_hidden: &[f32],
    step_context: &psionic_models::TassadarExecutorTransformerStepContext,
    logits: &[f32],
    target_token: TokenId,
    projection_grad: &mut [f32],
    bias_grad: &mut [f32],
    token_embedding_grad: Option<&mut [f32]>,
    position_embedding_grad: Option<&mut [f32]>,
    mixer_projection_grad: Option<&mut [f32]>,
    mixer_bias_grad: Option<&mut [f32]>,
) -> f32 {
    let hidden_width = current_model.descriptor().config.hidden_width();
    let embedding_dim = current_model.descriptor().config.embedding_dim;
    let probabilities = softmax(logits);
    let target_token_index = target_token.as_u32() as usize;
    let probability = probabilities[target_token_index].max(1e-8);
    let mut hidden_grad = vec![0.0; hidden_width];

    for (token_index, probability) in probabilities.iter().enumerate() {
        let delta = probability - f32::from(token_index == target_token_index);
        bias_grad[token_index] += delta;
        for (hidden_index, hidden_value) in hidden.iter().enumerate() {
            let projection_index = hidden_index * probabilities.len() + token_index;
            projection_grad[projection_index] += hidden_value * delta;
            hidden_grad[hidden_index] +=
                current_model.weights().output_projection()[projection_index] * delta;
        }
    }

    let source_hidden_grad = if current_model
        .trainable_surface()
        .trains_small_learned_mixer()
    {
        let mut source_hidden_grad = hidden_grad.clone();
        let mixer_projection = current_model.weights().small_learned_mixer_projection();
        let mixer_projection_grad =
            mixer_projection_grad.expect("mixer projection grad should exist");
        let mixer_bias_grad = mixer_bias_grad.expect("mixer bias grad should exist");
        for output_index in 0..hidden_width {
            mixer_bias_grad[output_index] += hidden_grad[output_index];
            for input_index in 0..hidden_width {
                let weight_index = input_index * hidden_width + output_index;
                mixer_projection_grad[weight_index] +=
                    source_hidden[input_index] * hidden_grad[output_index];
                source_hidden_grad[input_index] +=
                    mixer_projection[weight_index] * hidden_grad[output_index];
            }
        }
        source_hidden_grad
    } else {
        hidden_grad
    };

    if let Some(token_embedding_grad) = token_embedding_grad {
        for (slot_index, token) in step_context.context_tokens.iter().enumerate() {
            let token_index = token.as_u32() as usize;
            let token_offset = token_index * embedding_dim;
            let hidden_offset = slot_index * embedding_dim;
            for dim in 0..embedding_dim {
                token_embedding_grad[token_offset + dim] += source_hidden_grad[hidden_offset + dim];
            }
        }
    }
    if let Some(position_embedding_grad) = position_embedding_grad {
        let position_index = step_context.position as usize;
        let position_offset = position_index * embedding_dim;
        let hidden_offset = step_context.context_tokens.len() * embedding_dim;
        for dim in 0..embedding_dim {
            position_embedding_grad[position_offset + dim] +=
                source_hidden_grad[hidden_offset + dim];
        }
    }

    -probability.ln()
}

fn greedy_token_from_logits(logits: &[f32]) -> TokenId {
    let (best_index, _) = logits
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.partial_cmp(right).unwrap())
        .expect("vocabulary logits should be non-empty");
    TokenId(best_index as u32)
}

fn stage_target_token_count(
    example: &TassadarSequenceExample,
    stage: &TassadarExecutorCurriculumStage,
) -> usize {
    let full_target_tokens = example.metadata.target_token_count as usize;
    stage
        .max_train_target_tokens_per_example
        .unwrap_or(full_target_tokens)
        .min(full_target_tokens)
}

fn checkpoint_rank_tuple(
    report: &TassadarExecutorEvalReport,
) -> (u32, u32, u32, u32, u32, u32, u32) {
    (
        report.first_target_exactness_bps,
        report.first_8_token_exactness_bps,
        report.first_32_token_exactness_bps,
        report.exact_trace_case_count,
        report.aggregate_target_token_exactness_bps,
        report.final_output_exact_case_count,
        report.halt_exact_case_count,
    )
}

fn checkpoint_rank_entry(
    entry: &TassadarExecutorCheckpointLeaderboardEntry,
) -> (u32, u32, u32, u32, u32) {
    (
        entry.first_target_exactness_bps,
        entry.first_8_token_exactness_bps,
        entry.first_32_token_exactness_bps,
        entry.exact_trace_case_count,
        entry.aggregate_target_token_exactness_bps,
    )
}

fn mean_batch_loss(reports: &[TassadarExecutorTrainingBatchReport]) -> f32 {
    let total_tokens = reports
        .iter()
        .map(|batch| batch.target_token_count)
        .sum::<u32>();
    if total_tokens == 0 {
        return 0.0;
    }
    let weighted_loss = reports
        .iter()
        .map(|batch| f64::from(batch.mean_loss) * f64::from(batch.target_token_count))
        .sum::<f64>();
    (weighted_loss / f64::from(total_tokens)) as f32
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp_values = logits
        .iter()
        .map(|logit| (logit - max_logit).exp())
        .collect::<Vec<_>>();
    let normalizer = exp_values.iter().sum::<f32>().max(1e-8);
    exp_values
        .into_iter()
        .map(|value| value / normalizer)
        .collect()
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar executor train value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_eval::TassadarSequenceWorkload;

    use super::{TassadarExecutorTrainingConfig, train_tassadar_executor_transformer};

    #[test]
    fn next_token_training_runs_against_frozen_sudoku_v0_sequence_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let outcome =
            train_tassadar_executor_transformer(&TassadarExecutorTrainingConfig::reference())?;

        assert!(!outcome.report.batch_reports.is_empty());
        assert!(!outcome.report.epoch_reports.is_empty());
        assert_eq!(outcome.report.evaluation.case_reports.len(), 2);
        assert!(!outcome.report.best_checkpoint_id.is_empty());
        assert!(!outcome.report.checkpoint_leaderboard.is_empty());
        assert!(!outcome.report.trained_model_descriptor_digest.is_empty());
        assert!(!outcome.report.trained_weight_digest.is_empty());
        assert!(!outcome.report.report_digest.is_empty());
        Ok(())
    }

    #[test]
    fn next_token_training_runs_against_frozen_sudoku_9x9_sequence_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let outcome = train_tassadar_executor_transformer(
            &TassadarExecutorTrainingConfig::sudoku_9x9_scale_smoke(),
        )?;

        assert_eq!(
            outcome.report.config.workload,
            TassadarSequenceWorkload::Sudoku9x9
        );
        assert_eq!(outcome.report.evaluation.case_reports.len(), 1);
        Ok(())
    }

    #[test]
    fn boundary_curriculum_reference_config_expands_into_multiple_epochs() {
        let config = TassadarExecutorTrainingConfig::boundary_curriculum_reference();
        let stages = config.resolved_stages();

        assert_eq!(stages.len(), 7);
        assert_eq!(stages[0].max_train_target_tokens_per_example, Some(1));
        assert_eq!(stages[5].max_train_target_tokens_per_example, Some(32));
        assert_eq!(stages[6].stage_id, "full_trace_supervision");
        assert!(stages[6].max_train_target_tokens_per_example.is_none());
    }
}
