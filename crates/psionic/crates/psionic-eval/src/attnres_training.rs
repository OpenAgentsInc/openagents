use psionic_models::{AttnResCpuReferenceModel, AttnResExecutionError, AttnResNextTokenSample};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::EvalArtifact;

/// Canonical held-out evaluation reference for the Psionic-owned AttnRes tiny-training lane.
pub const ATTNRES_TINY_TRAINING_EVAL_REF: &str =
    "benchmark://openagents/psionic/attnres/tiny_training_held_out";

/// Per-case held-out comparison report for the AttnRes tiny-training lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTrainingEvalCaseReport {
    /// Stable case identifier.
    pub sample_id: String,
    /// Input token count for the case.
    pub input_token_count: u32,
    /// Supervision target token.
    pub target_token: u32,
    /// Baseline cross-entropy loss at the last position.
    pub baseline_loss: f32,
    /// Trained-model cross-entropy loss at the last position.
    pub trained_loss: f32,
    /// Baseline target probability.
    pub baseline_target_probability: f32,
    /// Trained target probability.
    pub trained_target_probability: f32,
    /// Baseline argmax token at the last position.
    pub baseline_predicted_token: u32,
    /// Trained argmax token at the last position.
    pub trained_predicted_token: u32,
    /// L2 delta between baseline and trained routing weights.
    pub routing_l2_delta: f32,
    /// Stable digest over the baseline routing snapshots.
    pub baseline_routing_digest: String,
    /// Stable digest over the trained routing snapshots.
    pub trained_routing_digest: String,
}

/// Aggregate held-out evaluation report for the AttnRes tiny-training lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTrainingEvalReport {
    /// Canonical evaluation reference.
    pub eval_ref: String,
    /// Stable descriptor digest for the baseline model.
    pub baseline_descriptor_digest: String,
    /// Stable descriptor digest for the trained model.
    pub trained_descriptor_digest: String,
    /// Aggregate baseline loss.
    pub baseline_mean_loss: f32,
    /// Aggregate trained loss.
    pub trained_mean_loss: f32,
    /// Aggregate mean loss delta (`trained - baseline`).
    pub mean_loss_delta: f32,
    /// Aggregate mean routing delta.
    pub mean_routing_l2_delta: f32,
    /// Number of cases whose loss improved.
    pub improved_case_count: u32,
    /// Per-case reports.
    pub cases: Vec<AttnResTrainingEvalCaseReport>,
}

impl AttnResTrainingEvalReport {
    /// Returns a stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_attnres_training_eval_report|", self)
    }

    /// Returns the report as a Psionic eval artifact.
    #[must_use]
    pub fn as_artifact(&self) -> EvalArtifact {
        let bytes = match serde_json::to_vec_pretty(self) {
            Ok(bytes) => bytes,
            Err(error) => error.to_string().into_bytes(),
        };
        EvalArtifact::new(
            "attnres_training_eval_report",
            "attnres_training_eval_report.json",
            &bytes,
        )
    }
}

/// Held-out evaluation failure for the AttnRes tiny-training lane.
#[derive(Debug, Error, PartialEq)]
pub enum AttnResTrainingEvalError {
    /// The sample set was empty.
    #[error("attnres training eval requires at least one sample")]
    EmptySamples,
    /// One sample supplied an empty prefix.
    #[error("attnres training eval sample `{sample_id}` has an empty prefix")]
    EmptyPrefix {
        /// Stable sample identifier.
        sample_id: String,
    },
    /// One target token exceeded the configured vocabulary size.
    #[error(
        "attnres training eval sample `{sample_id}` target token {target_token} exceeds vocab size {vocab_size}"
    )]
    TargetOutOfRange {
        /// Stable sample identifier.
        sample_id: String,
        /// Invalid target token.
        target_token: u32,
        /// Configured vocabulary size.
        vocab_size: usize,
    },
    /// Model execution failed.
    #[error(transparent)]
    Model(#[from] AttnResExecutionError),
}

/// Builds the machine-readable held-out comparison report for the AttnRes
/// tiny-training lane.
pub fn evaluate_attnres_training_shift(
    baseline: &AttnResCpuReferenceModel,
    trained: &AttnResCpuReferenceModel,
    samples: &[AttnResNextTokenSample],
) -> Result<AttnResTrainingEvalReport, AttnResTrainingEvalError> {
    if samples.is_empty() {
        return Err(AttnResTrainingEvalError::EmptySamples);
    }

    let mut cases = Vec::with_capacity(samples.len());
    let mut baseline_loss_sum = 0.0f32;
    let mut trained_loss_sum = 0.0f32;
    let mut routing_delta_sum = 0.0f32;
    let mut improved_case_count = 0u32;

    for sample in samples {
        if sample.input_tokens.is_empty() {
            return Err(AttnResTrainingEvalError::EmptyPrefix {
                sample_id: sample.sample_id.clone(),
            });
        }
        if sample.target_token.as_u32() as usize >= baseline.config().vocab_size {
            return Err(AttnResTrainingEvalError::TargetOutOfRange {
                sample_id: sample.sample_id.clone(),
                target_token: sample.target_token.as_u32(),
                vocab_size: baseline.config().vocab_size,
            });
        }

        let baseline_case = evaluate_case(baseline, sample)?;
        let trained_case = evaluate_case(trained, sample)?;
        let routing_l2_delta = l2_delta(
            baseline_case.routing_weights.as_slice(),
            trained_case.routing_weights.as_slice(),
        );
        if trained_case.loss < baseline_case.loss {
            improved_case_count = improved_case_count.saturating_add(1);
        }
        baseline_loss_sum += baseline_case.loss;
        trained_loss_sum += trained_case.loss;
        routing_delta_sum += routing_l2_delta;
        cases.push(AttnResTrainingEvalCaseReport {
            sample_id: sample.sample_id.clone(),
            input_token_count: sample.input_tokens.len() as u32,
            target_token: sample.target_token.as_u32(),
            baseline_loss: baseline_case.loss,
            trained_loss: trained_case.loss,
            baseline_target_probability: baseline_case.target_probability,
            trained_target_probability: trained_case.target_probability,
            baseline_predicted_token: baseline_case.predicted_token,
            trained_predicted_token: trained_case.predicted_token,
            routing_l2_delta,
            baseline_routing_digest: baseline_case.routing_digest,
            trained_routing_digest: trained_case.routing_digest,
        });
    }

    let case_count = samples.len() as f32;
    let baseline_mean_loss = baseline_loss_sum / case_count;
    let trained_mean_loss = trained_loss_sum / case_count;
    Ok(AttnResTrainingEvalReport {
        eval_ref: String::from(ATTNRES_TINY_TRAINING_EVAL_REF),
        baseline_descriptor_digest: baseline.descriptor().stable_digest(),
        trained_descriptor_digest: trained.descriptor().stable_digest(),
        baseline_mean_loss,
        trained_mean_loss,
        mean_loss_delta: trained_mean_loss - baseline_mean_loss,
        mean_routing_l2_delta: routing_delta_sum / case_count,
        improved_case_count,
        cases,
    })
}

#[derive(Clone, Debug)]
struct EvaluatedCase {
    loss: f32,
    target_probability: f32,
    predicted_token: u32,
    routing_digest: String,
    routing_weights: Vec<f32>,
}

fn evaluate_case(
    model: &AttnResCpuReferenceModel,
    sample: &AttnResNextTokenSample,
) -> Result<EvaluatedCase, AttnResTrainingEvalError> {
    let batch = [sample.input_tokens.clone()];
    let logits = model.forward(&batch)?;
    let (_, diagnostics) = model.forward_hidden_with_diagnostics(&batch)?;
    let last_logits = last_position_logits(&logits);
    let probabilities = softmax(last_logits);
    let target_index = sample.target_token.as_u32() as usize;
    let target_probability = probabilities[target_index].max(f32::EPSILON);
    let predicted_token = probabilities
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map_or(0, |(index, _)| index as u32);
    let routing_weights = diagnostics
        .sublayers
        .iter()
        .flat_map(|snapshot| snapshot.routing_weights.iter().copied())
        .collect::<Vec<_>>();
    Ok(EvaluatedCase {
        loss: -target_probability.ln(),
        target_probability,
        predicted_token,
        routing_digest: stable_digest(b"psionic_attnres_training_routing|", &diagnostics),
        routing_weights,
    })
}

fn last_position_logits<'a>(logits: &'a psionic_models::AttnResTensor3) -> &'a [f32] {
    let width = logits.width();
    let last_position = logits.sequence_length() - 1;
    let offset = last_position * width;
    &logits.values()[offset..offset + width]
}

fn softmax(values: &[f32]) -> Vec<f32> {
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = values
        .iter()
        .map(|value| (*value - max).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(f32::EPSILON);
    exp.into_iter().map(|value| value / sum).collect()
}

fn l2_delta(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let diff = left - right;
            diff * diff
        })
        .sum::<f32>()
        .sqrt()
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::error::Error;

    use psionic_models::{AttnResConfig, AttnResNextTokenSample, TokenId, TokenSequence};
    use serde::Deserialize;

    use super::{
        ATTNRES_TINY_TRAINING_EVAL_REF, AttnResTrainingEvalReport, evaluate_attnres_training_shift,
    };

    #[derive(Debug, Deserialize)]
    struct TinyTrainingFixture {
        config: AttnResConfig,
        held_out_samples: Vec<AttnResNextTokenSample>,
    }

    #[test]
    fn attnres_training_eval_report_is_machine_readable() -> Result<(), Box<dyn Error>> {
        let fixture: TinyTrainingFixture = serde_json::from_str(include_str!(
            "../../../fixtures/attnres/tiny_training_cases.json"
        ))?;
        let baseline = psionic_models::AttnResCpuReferenceModel::seeded(
            "attnres-training-eval",
            "v0",
            fixture.config.clone(),
        )?;
        let trained = psionic_models::AttnResCpuReferenceModel::with_weights(
            baseline.descriptor().model.clone(),
            fixture.config.clone(),
            baseline.weights().with_parameter_overrides(
                &fixture.config,
                &BTreeMap::from([(
                    String::from("lm_head.bias"),
                    vec![0.25; fixture.config.vocab_size],
                )]),
            )?,
        )?;
        let report =
            evaluate_attnres_training_shift(&baseline, &trained, &fixture.held_out_samples)?;
        assert_eq!(report.eval_ref, ATTNRES_TINY_TRAINING_EVAL_REF);
        assert_eq!(report.cases.len(), fixture.held_out_samples.len());
        assert!(!report.stable_digest().is_empty());
        let artifact = report.as_artifact();
        assert_eq!(artifact.artifact_kind, "attnres_training_eval_report");
        Ok(())
    }

    #[test]
    fn attnres_training_eval_report_round_trips() -> Result<(), Box<dyn Error>> {
        let report = AttnResTrainingEvalReport {
            eval_ref: String::from(ATTNRES_TINY_TRAINING_EVAL_REF),
            baseline_descriptor_digest: String::from("baseline"),
            trained_descriptor_digest: String::from("trained"),
            baseline_mean_loss: 1.0,
            trained_mean_loss: 0.5,
            mean_loss_delta: -0.5,
            mean_routing_l2_delta: 0.25,
            improved_case_count: 1,
            cases: vec![super::AttnResTrainingEvalCaseReport {
                sample_id: String::from("case-1"),
                input_token_count: 3,
                target_token: TokenId(3).as_u32(),
                baseline_loss: 1.0,
                trained_loss: 0.5,
                baseline_target_probability: 0.2,
                trained_target_probability: 0.4,
                baseline_predicted_token: TokenId(1).as_u32(),
                trained_predicted_token: TokenId(3).as_u32(),
                routing_l2_delta: 0.25,
                baseline_routing_digest: String::from("baseline-routing"),
                trained_routing_digest: String::from("trained-routing"),
            }],
        };
        let encoded = serde_json::to_vec(&report)?;
        let decoded: AttnResTrainingEvalReport = serde_json::from_slice(&encoded)?;
        assert_eq!(decoded, report);
        Ok(())
    }

    #[test]
    fn attnres_training_eval_rejects_empty_prefix() -> Result<(), Box<dyn Error>> {
        let model = psionic_models::AttnResCpuReferenceModel::seeded(
            "attnres-training-eval-empty",
            "v0",
            AttnResConfig::new(8, 4, 2)
                .with_num_heads(2)
                .with_vocab_size(8),
        )?;
        let error = evaluate_attnres_training_shift(
            &model,
            &model,
            &[AttnResNextTokenSample::new(
                "empty",
                TokenSequence::new(Vec::new()),
                TokenId(1),
            )],
        )
        .expect_err("empty prefix");
        assert!(matches!(
            error,
            super::AttnResTrainingEvalError::EmptyPrefix { .. }
        ));
        Ok(())
    }
}
