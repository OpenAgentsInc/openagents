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
    /// Number of deterministic epochs.
    pub epochs: u32,
    /// SGD learning rate for the output projection.
    pub learning_rate: f32,
    /// Optional cap over target tokens consumed from each train example.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_train_target_tokens_per_example: Option<usize>,
    /// Optional cap over target tokens evaluated from each validation example.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_eval_target_tokens_per_example: Option<usize>,
}

impl TassadarExecutorTrainingConfig {
    /// Returns the bounded reference config used by the first honest tests.
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
        }
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
    /// Zero-based epoch index.
    pub epoch_index: u32,
    /// Stable batch identifier from the frozen packing manifest.
    pub batch_id: String,
    /// Stable source sequence identifiers in the batch.
    pub sequence_ids: Vec<String>,
    /// Mean next-token loss over the batch.
    pub mean_loss: f32,
    /// Number of supervised target tokens consumed.
    pub target_token_count: u32,
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
    /// Per-batch training receipts.
    pub batch_reports: Vec<TassadarExecutorTrainingBatchReport>,
    /// Aggregate validation report against CPU-reference truth.
    pub evaluation: TassadarExecutorEvalReport,
    /// Stable digest over the training report.
    pub report_digest: String,
}

impl TassadarExecutorTrainingReport {
    fn new(
        config: TassadarExecutorTrainingConfig,
        manifest: &TassadarSequenceTrainingManifest,
        model: &TassadarExecutorTransformer,
        batch_reports: Vec<TassadarExecutorTrainingBatchReport>,
        evaluation: TassadarExecutorEvalReport,
    ) -> Self {
        let mut report = Self {
            config,
            training_manifest_digest: manifest.manifest_digest.clone(),
            trained_model_descriptor_digest: model.descriptor().stable_digest(),
            trained_weight_digest: model.descriptor().weights.digest.clone(),
            batch_reports,
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
    /// Trained model after bounded next-token updates.
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
    let mut model = match config.workload {
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

    for epoch_index in 0..config.epochs {
        for batch in &manifest.train_plan.batches {
            let sequence_ids = batch
                .rows
                .iter()
                .flat_map(|row| row.source_sequences.iter())
                .map(|sequence| sequence.sequence_id.clone())
                .collect::<Vec<_>>();
            let mut projection_grad = vec![
                0.0;
                model.descriptor().config.hidden_width()
                    * model.descriptor().config.vocab_size
            ];
            let mut bias_grad = vec![0.0; model.descriptor().config.vocab_size];
            let mut total_loss = 0.0_f32;
            let mut target_token_count = 0_u32;

            for sequence_id in &sequence_ids {
                let example = examples_by_id
                    .get(sequence_id.as_str())
                    .expect("frozen train plan should reference known examples");
                let max_target = config
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
                let forward = model.forward_logits(&sequence)?;
                let start_logit_index =
                    example.metadata.prompt_token_count.saturating_sub(1) as usize;
                let end_logit_index = (start_logit_index + max_target).min(forward.logits.len());
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
                            let projection_index = hidden_index * probabilities.len() + token_index;
                            projection_grad[projection_index] += hidden_value * delta;
                        }
                    }
                }
            }

            if target_token_count > 0 {
                let scale = config.learning_rate / target_token_count as f32;
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
                model.refresh_after_training();
            }

            batch_reports.push(TassadarExecutorTrainingBatchReport {
                epoch_index,
                batch_id: batch.batch_id.clone(),
                sequence_ids,
                mean_loss: if target_token_count == 0 {
                    0.0
                } else {
                    total_loss / target_token_count as f32
                },
                target_token_count,
            });
        }
    }

    let evaluation = evaluate_tassadar_executor_transformer_with_target_cap(
        &model,
        &bundle.dataset,
        TassadarSequenceSplit::Validation,
        config.max_eval_target_tokens_per_example,
    )?;
    let report = TassadarExecutorTrainingReport::new(
        config.clone(),
        &manifest,
        &model,
        batch_reports,
        evaluation,
    );
    Ok(TassadarExecutorTrainingOutcome { model, report })
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
        assert_eq!(outcome.report.evaluation.case_reports.len(), 2);
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

        assert!(!outcome.report.batch_reports.is_empty());
        assert_eq!(outcome.report.evaluation.case_reports.len(), 1);
        assert_eq!(
            outcome.report.config.workload,
            TassadarSequenceWorkload::Sudoku9x9
        );
        Ok(())
    }
}
