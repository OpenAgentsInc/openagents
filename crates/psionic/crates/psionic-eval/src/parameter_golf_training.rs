use psionic_data::{ParameterGolfDataError, ParameterGolfSentencePieceByteLuts};
use psionic_models::{ParameterGolfExecutionError, ParameterGolfReferenceModel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::EvalArtifact;

/// Canonical evaluation reference for the bounded Parameter Golf local-reference lane.
pub const PARAMETER_GOLF_LOCAL_REFERENCE_EVAL_REF: &str =
    "benchmark://openagents/psionic/parameter_golf/local_reference_validation";

/// Machine-readable validation report for one Parameter Golf reference model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfValidationEvalReport {
    /// Canonical evaluation reference.
    pub eval_ref: String,
    /// Stable descriptor digest for the evaluated model.
    pub model_descriptor_digest: String,
    /// Sequence length used by the evaluation batches.
    pub sequence_length: usize,
    /// Batch token budget used while iterating over the validation split.
    pub batch_token_budget: usize,
    /// Number of sequences evaluated.
    pub evaluated_sequence_count: usize,
    /// Number of supervised target tokens.
    pub evaluated_token_count: u64,
    /// Number of accounted bytes under the current SentencePiece LUTs.
    pub evaluated_byte_count: u64,
    /// Mean cross-entropy in natural-log units.
    pub mean_loss: f64,
    /// Tokenizer-agnostic `bits per byte`.
    pub bits_per_byte: f64,
}

impl ParameterGolfValidationEvalReport {
    /// Returns a stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_parameter_golf_validation_eval_report|", self)
    }

    /// Returns the report as a Psionic eval artifact.
    #[must_use]
    pub fn as_artifact(&self) -> EvalArtifact {
        let bytes = match serde_json::to_vec_pretty(self) {
            Ok(bytes) => bytes,
            Err(error) => error.to_string().into_bytes(),
        };
        EvalArtifact::new(
            "parameter_golf_validation_eval_report",
            "parameter_golf_validation_eval_report.json",
            &bytes,
        )
    }
}

/// Parameter Golf validation evaluation failure.
#[derive(Debug, Error)]
pub enum ParameterGolfValidationEvalError {
    /// Validation tokens must contain at least one supervised sequence.
    #[error("parameter golf validation eval requires at least `seq_len + 1` tokens")]
    ValidationTooShort,
    /// Sequence length must be positive.
    #[error("parameter golf validation eval requires `seq_len > 0`")]
    InvalidSequenceLength,
    /// Batch token budget must admit at least one full sequence.
    #[error(
        "parameter golf validation eval requires `batch_token_budget >= seq_len`, found batch_token_budget={batch_token_budget} seq_len={seq_len}"
    )]
    InvalidBatchTokenBudget {
        /// Batch token budget.
        batch_token_budget: usize,
        /// Sequence length.
        seq_len: usize,
    },
    /// Model execution failed.
    #[error(transparent)]
    Model(#[from] ParameterGolfExecutionError),
    /// Byte-accounting failed.
    #[error(transparent)]
    Data(#[from] ParameterGolfDataError),
}

/// Evaluates one Parameter Golf reference model on flat validation tokens.
pub fn evaluate_parameter_golf_validation(
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    seq_len: usize,
    batch_token_budget: usize,
    luts: &ParameterGolfSentencePieceByteLuts,
) -> Result<ParameterGolfValidationEvalReport, ParameterGolfValidationEvalError> {
    if seq_len == 0 {
        return Err(ParameterGolfValidationEvalError::InvalidSequenceLength);
    }
    if batch_token_budget < seq_len {
        return Err(ParameterGolfValidationEvalError::InvalidBatchTokenBudget {
            batch_token_budget,
            seq_len,
        });
    }
    if validation_tokens.len() <= seq_len {
        return Err(ParameterGolfValidationEvalError::ValidationTooShort);
    }
    let total_sequences = (validation_tokens.len() - 1) / seq_len;
    if total_sequences == 0 {
        return Err(ParameterGolfValidationEvalError::ValidationTooShort);
    }
    let batch_sequences = (batch_token_budget / seq_len).max(1);
    let mut total_loss_sum = 0.0_f64;
    let mut total_token_count = 0_u64;
    let mut total_byte_count = 0_u64;

    for batch_seq_start in (0..total_sequences).step_by(batch_sequences) {
        let batch_seq_end = (batch_seq_start + batch_sequences).min(total_sequences);
        let raw_start = batch_seq_start * seq_len;
        let raw_end = batch_seq_end * seq_len + 1;
        let local = &validation_tokens[raw_start..raw_end];
        let input_ids = local[..local.len() - 1]
            .chunks(seq_len)
            .map(|row| row.iter().map(|token| u32::from(*token)).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let target_ids = local[1..]
            .chunks(seq_len)
            .map(|row| row.iter().map(|token| u32::from(*token)).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let batch_loss = f64::from(model.loss(input_ids.as_slice(), target_ids.as_slice())?);
        let batch_token_count = target_ids.iter().map(Vec::len).sum::<usize>() as u64;
        total_loss_sum += batch_loss * batch_token_count as f64;
        total_token_count = total_token_count.saturating_add(batch_token_count);
        total_byte_count = total_byte_count.saturating_add(luts.count_target_bytes(
            input_ids
                .iter()
                .flat_map(|row| row.iter().copied())
                .collect::<Vec<_>>()
                .as_slice(),
            target_ids
                .iter()
                .flat_map(|row| row.iter().copied())
                .collect::<Vec<_>>()
                .as_slice(),
        )?);
    }

    let mean_loss = total_loss_sum / total_token_count.max(1) as f64;
    let bits_per_byte = (mean_loss / std::f64::consts::LN_2)
        * (total_token_count as f64 / total_byte_count.max(1) as f64);
    Ok(ParameterGolfValidationEvalReport {
        eval_ref: String::from(PARAMETER_GOLF_LOCAL_REFERENCE_EVAL_REF),
        model_descriptor_digest: model.descriptor().stable_digest(),
        sequence_length: seq_len,
        batch_token_budget,
        evaluated_sequence_count: total_sequences,
        evaluated_token_count: total_token_count,
        evaluated_byte_count: total_byte_count,
        mean_loss,
        bits_per_byte,
    })
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
    use std::error::Error;

    use psionic_data::{
        ParameterGolfSentencePieceByteLuts, ParameterGolfSentencePieceTokenEntry,
        ParameterGolfSentencePieceTokenKind,
    };

    use super::{
        PARAMETER_GOLF_LOCAL_REFERENCE_EVAL_REF, ParameterGolfValidationEvalReport,
        evaluate_parameter_golf_validation,
    };

    #[test]
    fn parameter_golf_validation_eval_report_is_machine_readable() -> Result<(), Box<dyn Error>> {
        let model = psionic_models::ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let luts = ParameterGolfSentencePieceByteLuts::build(
            model.descriptor().config.vocab_size,
            &[
                ParameterGolfSentencePieceTokenEntry::new(1, "a", ParameterGolfSentencePieceTokenKind::Normal),
                ParameterGolfSentencePieceTokenEntry::new(2, "b", ParameterGolfSentencePieceTokenKind::Normal),
                ParameterGolfSentencePieceTokenEntry::new(3, "c", ParameterGolfSentencePieceTokenKind::Normal),
                ParameterGolfSentencePieceTokenEntry::new(4, "d", ParameterGolfSentencePieceTokenKind::Normal),
            ],
        )?;
        let validation_tokens = vec![1_u16, 2, 3, 4, 1, 2, 3, 4, 1];
        let report = evaluate_parameter_golf_validation(&model, &validation_tokens, 4, 8, &luts)?;
        assert_eq!(report.eval_ref, PARAMETER_GOLF_LOCAL_REFERENCE_EVAL_REF);
        assert_eq!(report.evaluated_sequence_count, 2);
        assert!(report.mean_loss.is_finite());
        assert!(report.bits_per_byte.is_finite());
        assert!(!report.stable_digest().is_empty());
        let artifact = report.as_artifact();
        assert_eq!(artifact.artifact_kind, "parameter_golf_validation_eval_report");
        Ok(())
    }

    #[test]
    fn parameter_golf_validation_eval_report_round_trips() -> Result<(), Box<dyn Error>> {
        let report = ParameterGolfValidationEvalReport {
            eval_ref: String::from(PARAMETER_GOLF_LOCAL_REFERENCE_EVAL_REF),
            model_descriptor_digest: String::from("model"),
            sequence_length: 4,
            batch_token_budget: 8,
            evaluated_sequence_count: 2,
            evaluated_token_count: 8,
            evaluated_byte_count: 8,
            mean_loss: 1.0,
            bits_per_byte: 1.4426950408889634,
        };
        let encoded = serde_json::to_vec(&report)?;
        let decoded: ParameterGolfValidationEvalReport = serde_json::from_slice(&encoded)?;
        assert_eq!(decoded, report);
        Ok(())
    }
}
