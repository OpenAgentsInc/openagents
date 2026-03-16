use std::collections::BTreeMap;

use psionic_data::TassadarSequenceSplit;
use psionic_eval::{
    TassadarExecutorEvalError, TassadarExecutorEvalReport, TassadarExecutorLinearBenchmarkError,
    TassadarExecutorLinearBenchmarkReport, TassadarSequenceEvalError, TassadarSequenceWorkload,
    benchmark_tassadar_executor_linear_decode, build_tassadar_sequence_dataset,
    evaluate_tassadar_executor_transformer_with_target_cap,
};
use psionic_models::{
    TassadarExecutorTransformer, TassadarExecutorTransformerError, TokenId, TokenSequence,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarSequenceTrainingError, TassadarSequenceTrainingManifest,
    build_tassadar_sequence_training_manifest,
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

/// One curriculum stage for the trained executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorCurriculumStage {
    /// Stable stage identifier.
    pub stage_id: String,
    /// Max target tokens supervised per example during the stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_train_target_tokens_per_example: Option<usize>,
    /// Number of epochs to spend in the stage.
    pub epochs: u32,
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
        }
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
            curriculum_stages: Vec::new(),
            validate_every_epoch: true,
            select_best_checkpoint_by_boundary: true,
        }
    }

    fn resolved_stages(&self) -> Vec<TassadarExecutorCurriculumStage> {
        let mut stages = self.curriculum_stages.clone();
        stages.push(TassadarExecutorCurriculumStage::new(
            "full_trace_supervision",
            self.max_train_target_tokens_per_example,
            self.epochs,
        ));
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
    let bundle = build_tassadar_sequence_dataset(config.workload, config.dataset_version.as_str())?;
    let manifest = build_tassadar_sequence_training_manifest(
        config.workload,
        config.dataset_version.as_str(),
    )?;
    let mut current_model = match config.workload {
        TassadarSequenceWorkload::SudokuV0 => TassadarExecutorTransformer::sudoku_v0(),
        TassadarSequenceWorkload::Sudoku9x9 => TassadarExecutorTransformer::sudoku_9x9(),
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

    for stage in config.resolved_stages() {
        for stage_epoch_index in 0..stage.epochs {
            current_epoch_batches.clear();
            for batch in &manifest.train_plan.batches {
                let sequence_ids = batch
                    .rows
                    .iter()
                    .flat_map(|row| row.source_sequences.iter())
                    .map(|sequence| sequence.sequence_id.clone())
                    .collect::<Vec<_>>();
                let mut projection_grad = vec![
                    0.0;
                    current_model.descriptor().config.hidden_width()
                        * current_model.descriptor().config.vocab_size
                ];
                let mut bias_grad = vec![0.0; current_model.descriptor().config.vocab_size];
                let mut total_loss = 0.0_f32;
                let mut target_token_count = 0_u32;

                for sequence_id in &sequence_ids {
                    let example = examples_by_id
                        .get(sequence_id.as_str())
                        .expect("frozen train plan should reference known examples");
                    let max_target = stage
                        .max_train_target_tokens_per_example
                        .unwrap_or(example.metadata.target_token_count as usize)
                        .min(example.metadata.target_token_count as usize);
                    let effective_sequence_len =
                        example.metadata.prompt_token_count as usize + max_target;
                    let sequence = TokenSequence::new(
                        example.token_ids[..effective_sequence_len]
                            .iter()
                            .map(|token| TokenId(*token))
                            .collect::<Vec<_>>(),
                    );
                    let forward = current_model.forward_logits(&sequence)?;
                    let start_logit_index =
                        example.metadata.prompt_token_count.saturating_sub(1) as usize;
                    let end_logit_index =
                        (start_logit_index + max_target).min(forward.logits.len());
                    for logit_index in start_logit_index..end_logit_index {
                        let hidden = &forward.hidden_states[logit_index];
                        let logits = &forward.logits[logit_index];
                        let probabilities = softmax(logits.as_slice());
                        let target_token = sequence.as_slice()[logit_index + 1].as_u32() as usize;
                        let probability = probabilities[target_token].max(1e-8);
                        total_loss -= probability.ln();
                        target_token_count = target_token_count.saturating_add(1);

                        for (token_index, probability) in probabilities.iter().enumerate() {
                            let delta = probability - f32::from(token_index == target_token);
                            bias_grad[token_index] += delta;
                            for (hidden_index, hidden_value) in hidden.iter().enumerate() {
                                let projection_index =
                                    hidden_index * probabilities.len() + token_index;
                                projection_grad[projection_index] += hidden_value * delta;
                            }
                        }
                    }
                }

                if target_token_count > 0 {
                    let scale = config.learning_rate / target_token_count as f32;
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
                    current_model.refresh_after_training();
                }

                let batch_report = TassadarExecutorTrainingBatchReport {
                    global_epoch_index,
                    stage_id: stage.stage_id.clone(),
                    stage_epoch_index,
                    stage_max_train_target_tokens_per_example: stage
                        .max_train_target_tokens_per_example,
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
            }

            let evaluation = if config.validate_every_epoch {
                evaluate_tassadar_executor_transformer_with_target_cap(
                    &current_model,
                    &bundle.dataset,
                    TassadarSequenceSplit::Validation,
                    config.max_eval_target_tokens_per_example,
                )?
            } else {
                evaluate_tassadar_executor_transformer_with_target_cap(
                    &current_model,
                    &bundle.dataset,
                    TassadarSequenceSplit::Validation,
                    config.max_eval_target_tokens_per_example,
                )?
            };

            let checkpoint_id =
                format!("{}.checkpoint.epoch_{global_epoch_index:04}", config.run_id);
            let epoch_report = TassadarExecutorTrainingEpochReport {
                checkpoint_id: checkpoint_id.clone(),
                global_epoch_index,
                stage_id: stage.stage_id.clone(),
                stage_epoch_index,
                stage_max_train_target_tokens_per_example: stage
                    .max_train_target_tokens_per_example,
                mean_loss: mean_batch_loss(current_epoch_batches.as_slice()),
                target_token_count: current_epoch_batches
                    .iter()
                    .map(|batch| batch.target_token_count)
                    .sum(),
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
