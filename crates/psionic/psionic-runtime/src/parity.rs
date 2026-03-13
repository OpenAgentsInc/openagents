use psionic_core::QuantizationMode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// How strictly two backend observations must match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParityExpectation {
    /// Every compared value must match exactly.
    Exact,
    /// The compared behavior must match semantically even if sampled text or
    /// token choices may differ.
    Semantic,
    /// Numeric drift is allowed inside an explicit budget.
    Numerical,
}

/// Tinygrad-style numeric drift budget using absolute and relative tolerance.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumericDriftBudget {
    /// Maximum absolute delta.
    pub max_abs_delta: f32,
    /// Maximum relative delta against the CPU/reference value.
    pub max_rel_delta: f32,
}

impl NumericDriftBudget {
    /// Creates a numeric drift budget.
    #[must_use]
    pub const fn new(max_abs_delta: f32, max_rel_delta: f32) -> Self {
        Self {
            max_abs_delta,
            max_rel_delta,
        }
    }

    /// Returns whether the actual value stays inside the budget.
    #[must_use]
    pub fn allows(self, expected: f32, actual: f32) -> bool {
        let abs_delta = (expected - actual).abs();
        abs_delta <= self.max_abs_delta + self.max_rel_delta * expected.abs()
    }
}

/// Drift budget for embeddings parity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingParityBudget {
    /// This surface is always numerical today.
    pub expectation: ParityExpectation,
    /// Element-wise numeric budget.
    pub numeric: NumericDriftBudget,
    /// Minimum cosine similarity over the compared vectors.
    pub min_cosine_similarity: f32,
}

/// Drift budget for logits parity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogitParityBudget {
    /// This surface is always numerical today.
    pub expectation: ParityExpectation,
    /// Element-wise numeric budget.
    pub numeric: NumericDriftBudget,
    /// Maximum acceptable rank drift for the CPU top token.
    pub max_top_token_rank_drift: usize,
}

/// Policy for generation parity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationParityBudget {
    /// How token choices must match.
    pub token_choices: ParityExpectation,
    /// How decoded text must match.
    pub output_text: ParityExpectation,
    /// How termination reasons must match.
    pub termination: ParityExpectation,
}

/// Repo-wide backend parity policy.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackendParityPolicy {
    /// CPU remains the numeric reference until a tighter oracle exists.
    pub cpu_is_reference: bool,
    /// Dense embeddings budget.
    pub embeddings_dense: EmbeddingParityBudget,
    /// Quantized embeddings budget.
    pub embeddings_quantized: EmbeddingParityBudget,
    /// Dense logits budget.
    pub logits_dense: LogitParityBudget,
    /// Quantized logits budget.
    pub logits_quantized: LogitParityBudget,
    /// Seeded generation budget.
    pub seeded_generation: GenerationParityBudget,
    /// Unseeded generation budget.
    pub unseeded_generation: GenerationParityBudget,
}

impl Default for BackendParityPolicy {
    fn default() -> Self {
        Self {
            cpu_is_reference: true,
            embeddings_dense: EmbeddingParityBudget {
                expectation: ParityExpectation::Numerical,
                numeric: NumericDriftBudget::new(1.0e-5, 1.0e-5),
                min_cosine_similarity: 0.999_99,
            },
            embeddings_quantized: EmbeddingParityBudget {
                expectation: ParityExpectation::Numerical,
                numeric: NumericDriftBudget::new(2.0e-3, 2.0e-3),
                min_cosine_similarity: 0.999,
            },
            logits_dense: LogitParityBudget {
                expectation: ParityExpectation::Numerical,
                numeric: NumericDriftBudget::new(1.0e-5, 1.0e-5),
                max_top_token_rank_drift: 0,
            },
            logits_quantized: LogitParityBudget {
                expectation: ParityExpectation::Numerical,
                numeric: NumericDriftBudget::new(5.0e-3, 5.0e-3),
                max_top_token_rank_drift: 1,
            },
            seeded_generation: GenerationParityBudget {
                token_choices: ParityExpectation::Exact,
                output_text: ParityExpectation::Exact,
                termination: ParityExpectation::Exact,
            },
            unseeded_generation: GenerationParityBudget {
                token_choices: ParityExpectation::Semantic,
                output_text: ParityExpectation::Semantic,
                termination: ParityExpectation::Exact,
            },
        }
    }
}

impl BackendParityPolicy {
    /// Returns the embeddings budget for the provided quantization mode.
    #[must_use]
    pub fn embedding_budget(self, quantization: QuantizationMode) -> EmbeddingParityBudget {
        if quantization == QuantizationMode::None {
            self.embeddings_dense
        } else {
            self.embeddings_quantized
        }
    }

    /// Returns the logits budget for the provided quantization mode.
    #[must_use]
    pub fn logit_budget(self, quantization: QuantizationMode) -> LogitParityBudget {
        if quantization == QuantizationMode::None {
            self.logits_dense
        } else {
            self.logits_quantized
        }
    }

    /// Returns the generation budget for a seeded or unseeded request.
    #[must_use]
    pub fn generation_budget(self, seeded: bool) -> GenerationParityBudget {
        if seeded {
            self.seeded_generation
        } else {
            self.unseeded_generation
        }
    }
}

/// Parity comparison failure.
#[derive(Debug, Error, PartialEq)]
pub enum ParityCheckError {
    /// Inputs had different lengths.
    #[error("vector length mismatch: expected {expected_len}, actual {actual_len}")]
    LengthMismatch {
        /// Expected length.
        expected_len: usize,
        /// Actual length.
        actual_len: usize,
    },
    /// The compared vectors were empty.
    #[error("cannot compare empty vectors")]
    EmptyVector,
    /// A non-finite value was observed in the reference vector.
    #[error("reference vector contains a non-finite value at index {index}")]
    NonFiniteReference {
        /// Failing index.
        index: usize,
    },
    /// A non-finite value was observed in the candidate vector.
    #[error("candidate vector contains a non-finite value at index {index}")]
    NonFiniteCandidate {
        /// Failing index.
        index: usize,
    },
}

/// Summary over an embeddings parity comparison.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingParitySummary {
    /// Whether the compared vectors stayed inside the budget.
    pub within_budget: bool,
    /// Largest absolute delta observed.
    pub max_abs_delta: f32,
    /// Largest relative delta observed against the CPU/reference value.
    pub max_rel_delta: f32,
    /// Cosine similarity between the vectors.
    pub cosine_similarity: f32,
    /// First failing element when one exists.
    pub first_failing_index: Option<usize>,
}

/// Summary over a logits parity comparison.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogitParitySummary {
    /// Whether the compared vectors stayed inside the budget.
    pub within_budget: bool,
    /// Largest absolute delta observed.
    pub max_abs_delta: f32,
    /// Largest relative delta observed against the CPU/reference value.
    pub max_rel_delta: f32,
    /// CPU/reference top-token index.
    pub expected_top_token: usize,
    /// Candidate top-token index.
    pub actual_top_token: usize,
    /// Rank drift for the CPU/reference top token inside the candidate logits.
    pub top_token_rank_drift: usize,
    /// First failing element when one exists.
    pub first_failing_index: Option<usize>,
}

/// Compares embedding vectors against the configured drift budget.
pub fn compare_embedding_vectors(
    expected: &[f32],
    actual: &[f32],
    budget: EmbeddingParityBudget,
) -> Result<EmbeddingParitySummary, ParityCheckError> {
    compare_numeric_vectors(expected, actual, budget.numeric).map(
        |(within_budget, max_abs_delta, max_rel_delta, first_failing_index)| {
            let cosine_similarity = cosine_similarity(expected, actual);
            EmbeddingParitySummary {
                within_budget: within_budget && cosine_similarity >= budget.min_cosine_similarity,
                max_abs_delta,
                max_rel_delta,
                cosine_similarity,
                first_failing_index,
            }
        },
    )
}

/// Compares logits vectors against the configured drift budget.
pub fn compare_logits(
    expected: &[f32],
    actual: &[f32],
    budget: LogitParityBudget,
) -> Result<LogitParitySummary, ParityCheckError> {
    let (within_numeric_budget, max_abs_delta, max_rel_delta, first_failing_index) =
        compare_numeric_vectors(expected, actual, budget.numeric)?;
    let expected_top_token = argmax(expected)?;
    let actual_top_token = argmax(actual)?;
    let top_token_rank_drift = rank_of_index(actual, expected_top_token)?;
    Ok(LogitParitySummary {
        within_budget: within_numeric_budget
            && top_token_rank_drift <= budget.max_top_token_rank_drift,
        max_abs_delta,
        max_rel_delta,
        expected_top_token,
        actual_top_token,
        top_token_rank_drift,
        first_failing_index,
    })
}

fn compare_numeric_vectors(
    expected: &[f32],
    actual: &[f32],
    budget: NumericDriftBudget,
) -> Result<(bool, f32, f32, Option<usize>), ParityCheckError> {
    if expected.len() != actual.len() {
        return Err(ParityCheckError::LengthMismatch {
            expected_len: expected.len(),
            actual_len: actual.len(),
        });
    }
    if expected.is_empty() {
        return Err(ParityCheckError::EmptyVector);
    }

    let mut within_budget = true;
    let mut max_abs_delta = 0.0_f32;
    let mut max_rel_delta = 0.0_f32;
    let mut first_failing_index = None;

    for (index, (expected_value, actual_value)) in expected.iter().zip(actual.iter()).enumerate() {
        if !expected_value.is_finite() {
            return Err(ParityCheckError::NonFiniteReference { index });
        }
        if !actual_value.is_finite() {
            return Err(ParityCheckError::NonFiniteCandidate { index });
        }

        let abs_delta = (expected_value - actual_value).abs();
        let rel_delta = if *expected_value == 0.0 {
            if abs_delta == 0.0 { 0.0 } else { f32::INFINITY }
        } else {
            abs_delta / expected_value.abs()
        };

        max_abs_delta = max_abs_delta.max(abs_delta);
        max_rel_delta = max_rel_delta.max(rel_delta);

        if !budget.allows(*expected_value, *actual_value) {
            within_budget = false;
            if first_failing_index.is_none() {
                first_failing_index = Some(index);
            }
        }
    }

    Ok((
        within_budget,
        max_abs_delta,
        max_rel_delta,
        first_failing_index,
    ))
}

fn cosine_similarity(expected: &[f32], actual: &[f32]) -> f32 {
    let dot = expected
        .iter()
        .zip(actual.iter())
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let expected_norm = expected
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    let actual_norm = actual.iter().map(|value| value * value).sum::<f32>().sqrt();
    if expected_norm == 0.0 || actual_norm == 0.0 {
        if expected_norm == actual_norm {
            1.0
        } else {
            0.0
        }
    } else {
        dot / (expected_norm * actual_norm)
    }
}

fn argmax(values: &[f32]) -> Result<usize, ParityCheckError> {
    if values.is_empty() {
        return Err(ParityCheckError::EmptyVector);
    }
    let mut best_index = 0_usize;
    let mut best_value = values[0];
    if !best_value.is_finite() {
        return Err(ParityCheckError::NonFiniteReference { index: 0 });
    }
    for (index, value) in values.iter().enumerate().skip(1) {
        if !value.is_finite() {
            return Err(ParityCheckError::NonFiniteReference { index });
        }
        if *value > best_value {
            best_value = *value;
            best_index = index;
        }
    }
    Ok(best_index)
}

fn rank_of_index(values: &[f32], target_index: usize) -> Result<usize, ParityCheckError> {
    if target_index >= values.len() {
        return Err(ParityCheckError::LengthMismatch {
            expected_len: values.len(),
            actual_len: target_index + 1,
        });
    }
    let target_value = values[target_index];
    if !target_value.is_finite() {
        return Err(ParityCheckError::NonFiniteCandidate {
            index: target_index,
        });
    }
    let rank = values
        .iter()
        .enumerate()
        .filter(|(_, value)| value.is_finite() && **value > target_value)
        .count();
    Ok(rank)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic_in_result_fn)]

    use super::*;

    #[test]
    fn default_policy_keeps_cpu_as_reference_and_quantized_budget_looser() {
        let policy = BackendParityPolicy::default();
        assert!(policy.cpu_is_reference);
        assert_eq!(
            policy
                .embedding_budget(QuantizationMode::None)
                .numeric
                .max_abs_delta,
            1.0e-5
        );
        assert_eq!(
            policy
                .embedding_budget(QuantizationMode::GgmlQ4_0)
                .numeric
                .max_abs_delta,
            2.0e-3
        );
        assert_eq!(
            policy
                .logit_budget(QuantizationMode::GgmlQ8_0)
                .max_top_token_rank_drift,
            1
        );
    }

    #[test]
    fn generation_budget_is_exact_when_seeded_and_semantic_when_unseeded() {
        let policy = BackendParityPolicy::default();
        let seeded = policy.generation_budget(true);
        assert_eq!(seeded.token_choices, ParityExpectation::Exact);
        assert_eq!(seeded.output_text, ParityExpectation::Exact);
        assert_eq!(seeded.termination, ParityExpectation::Exact);

        let unseeded = policy.generation_budget(false);
        assert_eq!(unseeded.token_choices, ParityExpectation::Semantic);
        assert_eq!(unseeded.output_text, ParityExpectation::Semantic);
        assert_eq!(unseeded.termination, ParityExpectation::Exact);
    }

    #[test]
    fn embedding_comparison_uses_budget_and_cosine_similarity()
    -> Result<(), Box<dyn std::error::Error>> {
        let budget = BackendParityPolicy::default().embedding_budget(QuantizationMode::None);
        let summary = compare_embedding_vectors(&[1.0, 0.0], &[1.0 - 5.0e-6, 5.0e-6], budget)?;
        assert!(summary.within_budget);
        assert!(summary.cosine_similarity >= budget.min_cosine_similarity);
        Ok(())
    }

    #[test]
    fn logits_comparison_allows_one_rank_drift_for_quantized_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let budget = BackendParityPolicy::default().logit_budget(QuantizationMode::GgmlQ4_0);
        let summary = compare_logits(&[0.502, 0.5, 0.1], &[0.5, 0.502, 0.1], budget)?;
        assert!(summary.within_budget);
        assert_eq!(summary.top_token_rank_drift, 1);
        Ok(())
    }
}
