use std::time::Instant;

use psionic_models::{AttnResCpuReferenceModel, AttnResExecutionError, TokenSequence};
use psionic_runtime::{
    AttnResHiddenParityReport, AttnResLogitParityReport, AttnResTwoPhaseParityBudget,
    ParityCheckError, compare_attnres_hidden_two_phase_parity,
    compare_attnres_logit_two_phase_parity,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::EvalArtifact;

/// Canonical benchmark reference for the Psionic-owned AttnRes two-phase receipt lane.
pub const ATTNRES_TWO_PHASE_BENCHMARK_REF: &str =
    "benchmark://openagents/psionic/attnres/reference_two_phase_parity";

/// One benchmark input case for AttnRes parity and receipt generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttnResBenchmarkInputCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Ordered input token prefix.
    pub tokens: TokenSequence,
}

impl AttnResBenchmarkInputCase {
    /// Creates one benchmark input case.
    #[must_use]
    pub fn new(case_id: impl Into<String>, tokens: TokenSequence) -> Self {
        Self {
            case_id: case_id.into(),
            tokens,
        }
    }
}

/// Per-case benchmark receipt for standard-vs-two-phase AttnRes execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTwoPhaseBenchmarkCaseReceipt {
    /// Stable case identifier.
    pub case_id: String,
    /// Input token count for the case.
    pub input_token_count: u32,
    /// Standard hidden-path elapsed time in milliseconds.
    pub standard_hidden_elapsed_ms: u64,
    /// Two-phase hidden-path elapsed time in milliseconds.
    pub two_phase_hidden_elapsed_ms: u64,
    /// Standard logits-path elapsed time in milliseconds.
    pub standard_logit_elapsed_ms: u64,
    /// Two-phase logits-path elapsed time in milliseconds.
    pub two_phase_logit_elapsed_ms: u64,
    /// Standard hidden-path throughput.
    pub standard_hidden_tokens_per_second: u32,
    /// Two-phase hidden-path throughput.
    pub two_phase_hidden_tokens_per_second: u32,
    /// Standard logits-path throughput.
    pub standard_logit_tokens_per_second: u32,
    /// Two-phase logits-path throughput.
    pub two_phase_logit_tokens_per_second: u32,
    /// Stable digest over standard hidden output values.
    pub standard_hidden_digest: String,
    /// Stable digest over two-phase hidden output values.
    pub two_phase_hidden_digest: String,
    /// Stable digest over standard logits output values.
    pub standard_logit_digest: String,
    /// Stable digest over two-phase logits output values.
    pub two_phase_logit_digest: String,
    /// Runtime-owned hidden parity report.
    pub hidden_parity: AttnResHiddenParityReport,
    /// Runtime-owned logits parity report.
    pub logit_parity: AttnResLogitParityReport,
}

/// Aggregate machine-readable benchmark receipt for AttnRes standard-vs-two-phase execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTwoPhaseBenchmarkReceipt {
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Stable descriptor digest for the evaluated model.
    pub model_descriptor_digest: String,
    /// Stable weight digest for the evaluated model.
    pub model_weight_digest: String,
    /// Runtime-owned parity budget used by the receipt.
    pub parity_budget: AttnResTwoPhaseParityBudget,
    /// Total token count across all cases.
    pub total_input_tokens: u32,
    /// Aggregate standard hidden-path elapsed time.
    pub total_standard_hidden_elapsed_ms: u64,
    /// Aggregate two-phase hidden-path elapsed time.
    pub total_two_phase_hidden_elapsed_ms: u64,
    /// Aggregate standard logits-path elapsed time.
    pub total_standard_logit_elapsed_ms: u64,
    /// Aggregate two-phase logits-path elapsed time.
    pub total_two_phase_logit_elapsed_ms: u64,
    /// Aggregate standard hidden-path throughput.
    pub standard_hidden_tokens_per_second: u32,
    /// Aggregate two-phase hidden-path throughput.
    pub two_phase_hidden_tokens_per_second: u32,
    /// Aggregate standard logits-path throughput.
    pub standard_logit_tokens_per_second: u32,
    /// Aggregate two-phase logits-path throughput.
    pub two_phase_logit_tokens_per_second: u32,
    /// Per-case benchmark receipts.
    pub cases: Vec<AttnResTwoPhaseBenchmarkCaseReceipt>,
}

impl AttnResTwoPhaseBenchmarkReceipt {
    /// Returns a stable digest over the receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_attnres_two_phase_benchmark_receipt|", self)
    }

    /// Returns the receipt as a Psionic eval artifact.
    #[must_use]
    pub fn as_artifact(&self) -> EvalArtifact {
        let bytes = match serde_json::to_vec_pretty(self) {
            Ok(bytes) => bytes,
            Err(error) => error.to_string().into_bytes(),
        };
        EvalArtifact::new(
            "attnres_two_phase_benchmark_receipt",
            "attnres_two_phase_benchmark_receipt.json",
            &bytes,
        )
    }
}

/// Benchmark receipt generation failure.
#[derive(Debug, Error, PartialEq)]
pub enum AttnResBenchmarkError {
    /// Model execution failed.
    #[error(transparent)]
    Model(#[from] AttnResExecutionError),
    /// Runtime parity comparison failed.
    #[error(transparent)]
    Parity(#[from] ParityCheckError),
}

/// Benchmarks the standard and two-phase AttnRes CPU reference paths and
/// captures the result as one machine-readable receipt.
pub fn benchmark_attnres_two_phase_parity(
    model: &AttnResCpuReferenceModel,
    cases: &[AttnResBenchmarkInputCase],
    parity_budget: AttnResTwoPhaseParityBudget,
) -> Result<AttnResTwoPhaseBenchmarkReceipt, AttnResBenchmarkError> {
    let mut receipts = Vec::with_capacity(cases.len());
    let mut total_input_tokens = 0_u32;
    let mut total_standard_hidden_elapsed_ms = 0_u64;
    let mut total_two_phase_hidden_elapsed_ms = 0_u64;
    let mut total_standard_logit_elapsed_ms = 0_u64;
    let mut total_two_phase_logit_elapsed_ms = 0_u64;

    for case in cases {
        let batch = [case.tokens.clone()];
        let input_token_count = case.tokens.len() as u32;
        total_input_tokens = total_input_tokens.saturating_add(input_token_count);

        let started = Instant::now();
        let standard_hidden = model.forward_hidden(&batch)?;
        let standard_hidden_elapsed_ms = started.elapsed().as_millis() as u64;

        let started = Instant::now();
        let two_phase_hidden = model.forward_two_phase_hidden(&batch)?;
        let two_phase_hidden_elapsed_ms = started.elapsed().as_millis() as u64;

        let started = Instant::now();
        let standard_logits = model.forward(&batch)?;
        let standard_logit_elapsed_ms = started.elapsed().as_millis() as u64;

        let started = Instant::now();
        let two_phase_logits = model.forward_two_phase(&batch)?;
        let two_phase_logit_elapsed_ms = started.elapsed().as_millis() as u64;

        total_standard_hidden_elapsed_ms =
            total_standard_hidden_elapsed_ms.saturating_add(standard_hidden_elapsed_ms.max(1));
        total_two_phase_hidden_elapsed_ms =
            total_two_phase_hidden_elapsed_ms.saturating_add(two_phase_hidden_elapsed_ms.max(1));
        total_standard_logit_elapsed_ms =
            total_standard_logit_elapsed_ms.saturating_add(standard_logit_elapsed_ms.max(1));
        total_two_phase_logit_elapsed_ms =
            total_two_phase_logit_elapsed_ms.saturating_add(two_phase_logit_elapsed_ms.max(1));

        let hidden_parity = compare_attnres_hidden_two_phase_parity(
            standard_hidden.values(),
            two_phase_hidden.values(),
            parity_budget.hidden,
        )?;
        let logit_parity = compare_attnres_logit_two_phase_parity(
            standard_logits.values(),
            two_phase_logits.values(),
            parity_budget.logits,
        )?;

        receipts.push(AttnResTwoPhaseBenchmarkCaseReceipt {
            case_id: case.case_id.clone(),
            input_token_count,
            standard_hidden_elapsed_ms,
            two_phase_hidden_elapsed_ms,
            standard_logit_elapsed_ms,
            two_phase_logit_elapsed_ms,
            standard_hidden_tokens_per_second: tokens_per_second(
                input_token_count,
                standard_hidden_elapsed_ms,
            ),
            two_phase_hidden_tokens_per_second: tokens_per_second(
                input_token_count,
                two_phase_hidden_elapsed_ms,
            ),
            standard_logit_tokens_per_second: tokens_per_second(
                input_token_count,
                standard_logit_elapsed_ms,
            ),
            two_phase_logit_tokens_per_second: tokens_per_second(
                input_token_count,
                two_phase_logit_elapsed_ms,
            ),
            standard_hidden_digest: digest_f32_slice(standard_hidden.values()),
            two_phase_hidden_digest: digest_f32_slice(two_phase_hidden.values()),
            standard_logit_digest: digest_f32_slice(standard_logits.values()),
            two_phase_logit_digest: digest_f32_slice(two_phase_logits.values()),
            hidden_parity,
            logit_parity,
        });
    }

    Ok(AttnResTwoPhaseBenchmarkReceipt {
        benchmark_ref: String::from(ATTNRES_TWO_PHASE_BENCHMARK_REF),
        model_descriptor_digest: model.descriptor().stable_digest(),
        model_weight_digest: model.descriptor().weights.digest.clone(),
        parity_budget,
        total_input_tokens,
        total_standard_hidden_elapsed_ms,
        total_two_phase_hidden_elapsed_ms,
        total_standard_logit_elapsed_ms,
        total_two_phase_logit_elapsed_ms,
        standard_hidden_tokens_per_second: tokens_per_second(
            total_input_tokens,
            total_standard_hidden_elapsed_ms,
        ),
        two_phase_hidden_tokens_per_second: tokens_per_second(
            total_input_tokens,
            total_two_phase_hidden_elapsed_ms,
        ),
        standard_logit_tokens_per_second: tokens_per_second(
            total_input_tokens,
            total_standard_logit_elapsed_ms,
        ),
        two_phase_logit_tokens_per_second: tokens_per_second(
            total_input_tokens,
            total_two_phase_logit_elapsed_ms,
        ),
        cases: receipts,
    })
}

fn tokens_per_second(tokens: u32, elapsed_ms: u64) -> u32 {
    if elapsed_ms == 0 {
        return tokens;
    }
    ((tokens as f64 / elapsed_ms as f64) * 1000.0).round() as u32
}

fn digest_f32_slice(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_attnres_f32_slice|");
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex::encode(hasher.finalize())
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

    use psionic_models::{AttnResCpuReferenceModel, TokenId, TokenSequence};
    use psionic_runtime::AttnResTwoPhaseParityStatus;
    use serde::Deserialize;

    use super::{
        ATTNRES_TWO_PHASE_BENCHMARK_REF, AttnResBenchmarkInputCase,
        AttnResTwoPhaseBenchmarkReceipt, benchmark_attnres_two_phase_parity,
    };

    #[derive(Debug, Deserialize)]
    struct FixtureCaseSet {
        cases: Vec<FixtureCase>,
    }

    #[derive(Debug, Deserialize)]
    struct FixtureCase {
        case_id: String,
        token_ids: Vec<u32>,
    }

    #[test]
    fn attnres_two_phase_benchmark_receipt_is_machine_readable() -> Result<(), Box<dyn Error>> {
        let fixture: FixtureCaseSet = serde_json::from_str(include_str!(
            "../../../fixtures/attnres/two_phase_benchmark_cases.json"
        ))?;
        let cases = fixture
            .cases
            .iter()
            .map(|case| {
                AttnResBenchmarkInputCase::new(
                    case.case_id.clone(),
                    TokenSequence::new(
                        case.token_ids
                            .iter()
                            .map(|token| TokenId(*token))
                            .collect::<Vec<_>>(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        let model = AttnResCpuReferenceModel::seeded(
            "attnres-eval",
            "v0",
            psionic_models::AttnResConfig::new(16, 8, 2)
                .with_num_heads(4)
                .with_vocab_size(64),
        )?;

        let receipt = benchmark_attnres_two_phase_parity(
            &model,
            &cases,
            psionic_runtime::AttnResTwoPhaseParityBudget::default(),
        )?;

        assert_eq!(receipt.benchmark_ref, ATTNRES_TWO_PHASE_BENCHMARK_REF);
        assert_eq!(receipt.cases.len(), cases.len());
        assert!(!receipt.stable_digest().is_empty());
        assert!(
            receipt
                .cases
                .iter()
                .all(|case| case.hidden_parity.status != AttnResTwoPhaseParityStatus::OutsideBudget)
        );
        assert!(
            receipt
                .cases
                .iter()
                .all(|case| case.logit_parity.status != AttnResTwoPhaseParityStatus::OutsideBudget)
        );

        let artifact = receipt.as_artifact();
        assert_eq!(
            artifact.artifact_kind,
            "attnres_two_phase_benchmark_receipt"
        );
        assert_eq!(
            artifact.artifact_ref,
            "attnres_two_phase_benchmark_receipt.json"
        );
        Ok(())
    }

    #[test]
    fn attnres_two_phase_benchmark_receipt_round_trips() -> Result<(), Box<dyn Error>> {
        let receipt = AttnResTwoPhaseBenchmarkReceipt {
            benchmark_ref: String::from(ATTNRES_TWO_PHASE_BENCHMARK_REF),
            model_descriptor_digest: String::from("descriptor"),
            model_weight_digest: String::from("weights"),
            parity_budget: psionic_runtime::AttnResTwoPhaseParityBudget::default(),
            total_input_tokens: 4,
            total_standard_hidden_elapsed_ms: 1,
            total_two_phase_hidden_elapsed_ms: 1,
            total_standard_logit_elapsed_ms: 1,
            total_two_phase_logit_elapsed_ms: 1,
            standard_hidden_tokens_per_second: 4_000,
            two_phase_hidden_tokens_per_second: 4_000,
            standard_logit_tokens_per_second: 4_000,
            two_phase_logit_tokens_per_second: 4_000,
            cases: Vec::new(),
        };
        let encoded = serde_json::to_vec(&receipt)?;
        let decoded: AttnResTwoPhaseBenchmarkReceipt = serde_json::from_slice(&encoded)?;
        assert_eq!(decoded, receipt);
        Ok(())
    }
}
