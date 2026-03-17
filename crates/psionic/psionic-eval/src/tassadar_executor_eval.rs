use std::collections::BTreeMap;

use psionic_data::{
    TassadarSequenceDatasetContract, TassadarSequenceDatasetError, TassadarSequenceSplit,
};
use psionic_models::{
    TassadarExecutorTransformer, TassadarExecutorTransformerClaimBoundary,
    TassadarExecutorTransformerError, TokenId, TokenSequence, TokenizerBoundary,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Exactness failures surfaced when comparing the trained executor model against CPU-reference truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorEvalFailure {
    /// Generated target tokens diverged from the CPU-reference trace.
    ExactTraceMismatch,
    /// Final output values diverged from the CPU-reference trace.
    FinalOutputMismatch,
    /// Halt marker diverged from the CPU-reference trace.
    HaltMismatch,
}

/// Boundary-first exactness metrics for one decoded suffix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBoundaryMetrics {
    /// Exactness over the first target token.
    pub first_target_exactness_bps: u32,
    /// Exactness over the first eight target tokens.
    pub first_8_token_exactness_bps: u32,
    /// Exactness over the first 32 target tokens.
    pub first_32_token_exactness_bps: u32,
}

/// Per-case exactness report for one trained executor model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorEvalCaseReport {
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable corpus case identifier.
    pub case_id: String,
    /// Token-level exactness over the predicted suffix.
    pub target_token_exactness_bps: u32,
    /// Boundary-first exactness metrics.
    pub boundary: TassadarExecutorBoundaryMetrics,
    /// Number of exact target tokens before the first divergence.
    pub matched_target_token_count: u32,
    /// First target-token index where divergence appeared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_divergence_index: Option<u32>,
    /// Symbolic reference token at the first divergence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_divergence_token: Option<String>,
    /// Symbolic predicted token at the first divergence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicted_divergence_token: Option<String>,
    /// Whether the full predicted suffix matched exactly.
    pub exact_trace_match: bool,
    /// Whether final output values matched exactly.
    pub final_output_match: bool,
    /// Whether the terminal halt marker matched exactly.
    pub halt_match: bool,
    /// Stable digest over the reference target suffix.
    pub reference_target_digest: String,
    /// Stable digest over the predicted target suffix.
    pub predicted_target_digest: String,
    /// Typed failure summary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<TassadarExecutorEvalFailure>,
}

/// Aggregate exactness report for one split.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorEvalReport {
    /// Stable dataset storage key used for evaluation.
    pub dataset_storage_key: String,
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Split that was evaluated.
    pub split: TassadarSequenceSplit,
    /// Explicit model claim boundary.
    pub claim_boundary: TassadarExecutorTransformerClaimBoundary,
    /// Aggregate token-level exactness over the evaluated split.
    pub aggregate_target_token_exactness_bps: u32,
    /// Aggregate exactness over the first target token.
    pub first_target_exactness_bps: u32,
    /// Aggregate exactness over the first eight target tokens.
    pub first_8_token_exactness_bps: u32,
    /// Aggregate exactness over the first 32 target tokens.
    pub first_32_token_exactness_bps: u32,
    /// Number of cases that stayed exact over the full suffix.
    pub exact_trace_case_count: u32,
    /// Number of cases with exact final outputs.
    pub final_output_exact_case_count: u32,
    /// Number of cases with exact halt markers.
    pub halt_exact_case_count: u32,
    /// Per-case reports.
    pub case_reports: Vec<TassadarExecutorEvalCaseReport>,
}

/// Per-case progress facts emitted while a held-out eval is still running.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorEvalCaseProgress {
    /// Split currently being evaluated.
    pub split: TassadarSequenceSplit,
    /// One-based case ordinal inside the evaluated split.
    pub case_index: u32,
    /// Total case count inside the evaluated split.
    pub case_count: u32,
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable corpus case identifier.
    pub case_id: String,
    /// Number of target tokens evaluated for the case.
    pub evaluated_target_token_count: u32,
    /// Number of exact target tokens before the first divergence.
    pub matched_target_token_count: u32,
    /// First target-token index where divergence appeared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_divergence_index: Option<u32>,
    /// Whether the full predicted suffix matched exactly.
    pub exact_trace_match: bool,
    /// Whether final output values matched exactly.
    pub final_output_match: bool,
    /// Whether the terminal halt marker matched exactly.
    pub halt_match: bool,
}

/// Per-case boundary summary emitted as a standalone report artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBoundaryExactnessCaseReport {
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable case identifier.
    pub case_id: String,
    /// First-target exactness.
    pub first_target_exactness_bps: u32,
    /// First-eight-target exactness.
    pub first_8_token_exactness_bps: u32,
    /// First-32-target exactness.
    pub first_32_token_exactness_bps: u32,
    /// Whether the case stayed exact over the full decoded suffix.
    pub exact_trace_match: bool,
}

/// Machine-readable standalone boundary exactness report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBoundaryExactnessReport {
    /// Stable dataset storage key used for evaluation.
    pub dataset_storage_key: String,
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Split that was evaluated.
    pub split: TassadarSequenceSplit,
    /// Explicit model claim boundary.
    pub claim_boundary: TassadarExecutorTransformerClaimBoundary,
    /// Aggregate token-level exactness over the evaluated split.
    pub aggregate_target_token_exactness_bps: u32,
    /// Aggregate exactness over the first target token.
    pub first_target_exactness_bps: u32,
    /// Aggregate exactness over the first eight target tokens.
    pub first_8_token_exactness_bps: u32,
    /// Aggregate exactness over the first 32 target tokens.
    pub first_32_token_exactness_bps: u32,
    /// Number of exact full-suffix cases.
    pub exact_trace_case_count: u32,
    /// Per-case boundary summaries.
    pub case_reports: Vec<TassadarExecutorBoundaryExactnessCaseReport>,
    /// Stable report digest.
    pub report_digest: String,
}

/// One histogram bucket over first-divergence positions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorDivergenceHistogramBucket {
    /// Zero-based first-divergence index for the bucket.
    pub first_divergence_index: u32,
    /// Number of cases that first diverged at this position.
    pub case_count: u32,
}

/// Machine-readable standalone divergence histogram report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorDivergenceHistogramReport {
    /// Stable dataset storage key used for evaluation.
    pub dataset_storage_key: String,
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Split that was evaluated.
    pub split: TassadarSequenceSplit,
    /// Number of cases that stayed exact over the full suffix.
    pub exact_trace_case_count: u32,
    /// Histogram buckets over the first divergence position.
    pub buckets: Vec<TassadarExecutorDivergenceHistogramBucket>,
    /// Stable report digest.
    pub report_digest: String,
}

/// One first-token confusion entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorFirstTokenConfusionEntry {
    /// Symbolic reference token at target index zero.
    pub reference_token: String,
    /// Symbolic predicted token at target index zero.
    pub predicted_token: String,
    /// Number of cases with this confusion pair.
    pub case_count: u32,
}

/// Machine-readable standalone first-token confusion report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorFirstTokenConfusionReport {
    /// Stable dataset storage key used for evaluation.
    pub dataset_storage_key: String,
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Split that was evaluated.
    pub split: TassadarSequenceSplit,
    /// Number of cases that diverged at target token zero.
    pub token_zero_divergence_case_count: u32,
    /// Confusion entries sorted from most frequent to least.
    pub entries: Vec<TassadarExecutorFirstTokenConfusionEntry>,
    /// Stable report digest.
    pub report_digest: String,
}

/// Trained executor exactness evaluation failure.
#[derive(Debug, Error)]
pub enum TassadarExecutorEvalError {
    /// Dataset validation failed.
    #[error(transparent)]
    Dataset(#[from] TassadarSequenceDatasetError),
    /// Model forward/decode failed.
    #[error(transparent)]
    Model(#[from] TassadarExecutorTransformerError),
}

/// Greedily evaluates one trained executor model against the frozen CPU-reference dataset split.
pub fn evaluate_tassadar_executor_transformer(
    model: &TassadarExecutorTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
) -> Result<TassadarExecutorEvalReport, TassadarExecutorEvalError> {
    evaluate_tassadar_executor_transformer_with_target_cap(model, dataset, split, None)
}

/// Greedily evaluates one trained executor model against the frozen CPU-reference dataset split,
/// optionally capping the evaluated suffix length for bounded smoke tests.
pub fn evaluate_tassadar_executor_transformer_with_target_cap(
    model: &TassadarExecutorTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    target_token_cap: Option<usize>,
) -> Result<TassadarExecutorEvalReport, TassadarExecutorEvalError> {
    evaluate_tassadar_executor_transformer_with_target_cap_and_progress(
        model,
        dataset,
        split,
        target_token_cap,
        |_| {},
    )
}

/// Greedily evaluates one trained executor model against the frozen CPU-reference dataset split,
/// optionally capping the evaluated suffix length for bounded smoke tests, while emitting
/// per-case progress facts.
pub fn evaluate_tassadar_executor_transformer_with_target_cap_and_progress<F>(
    model: &TassadarExecutorTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    target_token_cap: Option<usize>,
    mut on_case_complete: F,
) -> Result<TassadarExecutorEvalReport, TassadarExecutorEvalError>
where
    F: FnMut(&TassadarExecutorEvalCaseProgress),
{
    dataset.validate()?;
    let tokenizer = model.tokenizer();
    let mut case_reports = Vec::new();
    let split_examples = dataset.split_examples(split);
    let case_count = split_examples.len() as u32;

    for (case_index, example) in split_examples.into_iter().enumerate() {
        let prompt_len = example.metadata.prompt_token_count as usize;
        let prompt = TokenSequence::new(
            example.token_ids[..prompt_len]
                .iter()
                .map(|token| TokenId(*token))
                .collect::<Vec<_>>(),
        );
        let full_reference_target = example.token_ids[prompt_len..]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>();
        let evaluated_target_len = target_token_cap
            .unwrap_or(full_reference_target.len())
            .min(full_reference_target.len());
        let reference_target = full_reference_target[..evaluated_target_len].to_vec();
        let predicted_target = greedy_decode_target(model, prompt, reference_target.len())?;
        let matched_target_token_count =
            matched_target_token_count(reference_target.as_slice(), predicted_target.as_slice());
        let first_divergence_index =
            first_divergence_index(reference_target.as_slice(), predicted_target.as_slice());
        let predicted_full = example.token_ids[..prompt_len]
            .iter()
            .map(|token| TokenId(*token))
            .chain(predicted_target.iter().copied())
            .collect::<Vec<_>>();
        let full_target_evaluated = evaluated_target_len == full_reference_target.len();
        let reference_full = if full_target_evaluated {
            Some(
                example
                    .token_ids
                    .iter()
                    .map(|token| TokenId(*token))
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let exact_trace_match = full_target_evaluated && predicted_target == reference_target;
        let target_token_exactness_bps =
            suffix_exactness_bps(reference_target.as_slice(), predicted_target.as_slice());
        let boundary = TassadarExecutorBoundaryMetrics {
            first_target_exactness_bps: prefix_exactness_bps(
                reference_target.as_slice(),
                predicted_target.as_slice(),
                1,
            ),
            first_8_token_exactness_bps: prefix_exactness_bps(
                reference_target.as_slice(),
                predicted_target.as_slice(),
                8,
            ),
            first_32_token_exactness_bps: prefix_exactness_bps(
                reference_target.as_slice(),
                predicted_target.as_slice(),
                32,
            ),
        };
        let reference_divergence_token = first_divergence_index.and_then(|index| {
            reference_target
                .get(index as usize)
                .map(|token| symbolic_token(tokenizer, Some(*token)))
        });
        let predicted_divergence_token = first_divergence_index
            .map(|index| symbolic_token(tokenizer, predicted_target.get(index as usize).copied()));
        let final_output_match = reference_full.as_ref().is_some_and(|reference_full| {
            tokenizer.extract_output_values(reference_full.as_slice())
                == tokenizer.extract_output_values(predicted_full.as_slice())
        });
        let halt_match = reference_full.as_ref().is_some_and(|reference_full| {
            tokenizer.extract_halt_marker(reference_full.as_slice())
                == tokenizer.extract_halt_marker(predicted_full.as_slice())
        });
        let mut failures = Vec::new();
        if !exact_trace_match {
            failures.push(TassadarExecutorEvalFailure::ExactTraceMismatch);
        }
        if !final_output_match {
            failures.push(TassadarExecutorEvalFailure::FinalOutputMismatch);
        }
        if !halt_match {
            failures.push(TassadarExecutorEvalFailure::HaltMismatch);
        }
        let case_report = TassadarExecutorEvalCaseReport {
            sequence_id: example.sequence_id.clone(),
            case_id: example.metadata.case_id.clone(),
            target_token_exactness_bps,
            boundary,
            matched_target_token_count,
            first_divergence_index,
            reference_divergence_token,
            predicted_divergence_token,
            exact_trace_match,
            final_output_match,
            halt_match,
            reference_target_digest: stable_digest(
                b"psionic_tassadar_executor_eval_reference_target|",
                &reference_target
                    .iter()
                    .map(|token| token.as_u32())
                    .collect::<Vec<_>>(),
            ),
            predicted_target_digest: stable_digest(
                b"psionic_tassadar_executor_eval_predicted_target|",
                &predicted_target
                    .iter()
                    .map(|token| token.as_u32())
                    .collect::<Vec<_>>(),
            ),
            failures,
        };
        on_case_complete(&TassadarExecutorEvalCaseProgress {
            split,
            case_index: case_index as u32 + 1,
            case_count,
            sequence_id: case_report.sequence_id.clone(),
            case_id: case_report.case_id.clone(),
            evaluated_target_token_count: evaluated_target_len as u32,
            matched_target_token_count: case_report.matched_target_token_count,
            first_divergence_index: case_report.first_divergence_index,
            exact_trace_match: case_report.exact_trace_match,
            final_output_match: case_report.final_output_match,
            halt_match: case_report.halt_match,
        });
        case_reports.push(case_report);
    }

    let case_count = case_reports.len().max(1) as u32;
    Ok(TassadarExecutorEvalReport {
        dataset_storage_key: dataset.storage_key(),
        dataset_digest: dataset.stable_digest(),
        split,
        claim_boundary: model.descriptor().claim_boundary,
        aggregate_target_token_exactness_bps: case_reports
            .iter()
            .map(|case| case.target_token_exactness_bps)
            .sum::<u32>()
            / case_count,
        first_target_exactness_bps: case_reports
            .iter()
            .map(|case| case.boundary.first_target_exactness_bps)
            .sum::<u32>()
            / case_count,
        first_8_token_exactness_bps: case_reports
            .iter()
            .map(|case| case.boundary.first_8_token_exactness_bps)
            .sum::<u32>()
            / case_count,
        first_32_token_exactness_bps: case_reports
            .iter()
            .map(|case| case.boundary.first_32_token_exactness_bps)
            .sum::<u32>()
            / case_count,
        exact_trace_case_count: case_reports
            .iter()
            .filter(|case| case.exact_trace_match)
            .count() as u32,
        final_output_exact_case_count: case_reports
            .iter()
            .filter(|case| case.final_output_match)
            .count() as u32,
        halt_exact_case_count: case_reports.iter().filter(|case| case.halt_match).count() as u32,
        case_reports,
    })
}

/// Builds a standalone boundary exactness report from one eval summary.
#[must_use]
pub fn build_tassadar_executor_boundary_exactness_report(
    report: &TassadarExecutorEvalReport,
) -> TassadarExecutorBoundaryExactnessReport {
    let mut boundary = TassadarExecutorBoundaryExactnessReport {
        dataset_storage_key: report.dataset_storage_key.clone(),
        dataset_digest: report.dataset_digest.clone(),
        split: report.split,
        claim_boundary: report.claim_boundary,
        aggregate_target_token_exactness_bps: report.aggregate_target_token_exactness_bps,
        first_target_exactness_bps: report.first_target_exactness_bps,
        first_8_token_exactness_bps: report.first_8_token_exactness_bps,
        first_32_token_exactness_bps: report.first_32_token_exactness_bps,
        exact_trace_case_count: report.exact_trace_case_count,
        case_reports: report
            .case_reports
            .iter()
            .map(|case| TassadarExecutorBoundaryExactnessCaseReport {
                sequence_id: case.sequence_id.clone(),
                case_id: case.case_id.clone(),
                first_target_exactness_bps: case.boundary.first_target_exactness_bps,
                first_8_token_exactness_bps: case.boundary.first_8_token_exactness_bps,
                first_32_token_exactness_bps: case.boundary.first_32_token_exactness_bps,
                exact_trace_match: case.exact_trace_match,
            })
            .collect(),
        report_digest: String::new(),
    };
    boundary.report_digest = stable_digest(
        b"psionic_tassadar_executor_boundary_exactness_report|",
        &boundary,
    );
    boundary
}

/// Builds a standalone divergence histogram report from one eval summary.
#[must_use]
pub fn build_tassadar_executor_divergence_histogram_report(
    report: &TassadarExecutorEvalReport,
) -> TassadarExecutorDivergenceHistogramReport {
    let mut histogram = BTreeMap::<u32, u32>::new();
    for case in &report.case_reports {
        if let Some(first_divergence_index) = case.first_divergence_index {
            *histogram.entry(first_divergence_index).or_insert(0) += 1;
        }
    }
    let mut divergence = TassadarExecutorDivergenceHistogramReport {
        dataset_storage_key: report.dataset_storage_key.clone(),
        dataset_digest: report.dataset_digest.clone(),
        split: report.split,
        exact_trace_case_count: report.exact_trace_case_count,
        buckets: histogram
            .into_iter()
            .map(
                |(first_divergence_index, case_count)| TassadarExecutorDivergenceHistogramBucket {
                    first_divergence_index,
                    case_count,
                },
            )
            .collect(),
        report_digest: String::new(),
    };
    divergence.report_digest = stable_digest(
        b"psionic_tassadar_executor_divergence_histogram_report|",
        &divergence,
    );
    divergence
}

/// Builds a standalone first-token confusion report from one eval summary.
#[must_use]
pub fn build_tassadar_executor_first_token_confusion_report(
    report: &TassadarExecutorEvalReport,
) -> TassadarExecutorFirstTokenConfusionReport {
    let mut confusion = BTreeMap::<(String, String), u32>::new();
    for case in &report.case_reports {
        if case.first_divergence_index == Some(0) {
            let reference_token = case
                .reference_divergence_token
                .clone()
                .unwrap_or_else(|| String::from("<missing>"));
            let predicted_token = case
                .predicted_divergence_token
                .clone()
                .unwrap_or_else(|| String::from("<missing>"));
            *confusion
                .entry((reference_token, predicted_token))
                .or_insert(0) += 1;
        }
    }
    let mut first_token = TassadarExecutorFirstTokenConfusionReport {
        dataset_storage_key: report.dataset_storage_key.clone(),
        dataset_digest: report.dataset_digest.clone(),
        split: report.split,
        token_zero_divergence_case_count: confusion.values().copied().sum(),
        entries: confusion
            .into_iter()
            .map(|((reference_token, predicted_token), case_count)| {
                TassadarExecutorFirstTokenConfusionEntry {
                    reference_token,
                    predicted_token,
                    case_count,
                }
            })
            .collect(),
        report_digest: String::new(),
    };
    first_token
        .entries
        .sort_by(|left, right| right.case_count.cmp(&left.case_count));
    first_token.report_digest = stable_digest(
        b"psionic_tassadar_executor_first_token_confusion_report|",
        &first_token,
    );
    first_token
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

fn suffix_exactness_bps(reference: &[TokenId], predicted: &[TokenId]) -> u32 {
    if reference.is_empty() {
        return 10_000;
    }
    let matches = reference
        .iter()
        .zip(predicted.iter())
        .filter(|(left, right)| left == right)
        .count();
    ((matches as f64 / reference.len() as f64) * 10_000.0).round() as u32
}

fn prefix_exactness_bps(reference: &[TokenId], predicted: &[TokenId], prefix_len: usize) -> u32 {
    let evaluated_len = reference.len().min(prefix_len);
    if evaluated_len == 0 {
        return 10_000;
    }
    let matches = reference
        .iter()
        .take(evaluated_len)
        .zip(predicted.iter().take(evaluated_len))
        .filter(|(left, right)| left == right)
        .count();
    ((matches as f64 / evaluated_len as f64) * 10_000.0).round() as u32
}

fn matched_target_token_count(reference: &[TokenId], predicted: &[TokenId]) -> u32 {
    reference
        .iter()
        .zip(predicted.iter())
        .take_while(|(left, right)| left == right)
        .count() as u32
}

fn first_divergence_index(reference: &[TokenId], predicted: &[TokenId]) -> Option<u32> {
    let compared = matched_target_token_count(reference, predicted) as usize;
    if compared < reference.len() || compared < predicted.len() {
        Some(compared as u32)
    } else {
        None
    }
}

fn symbolic_token(tokenizer: &impl TokenizerBoundary, token: Option<TokenId>) -> String {
    token
        .and_then(|token_id| tokenizer.vocabulary().token(token_id))
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| String::from("<missing>"))
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("Tassadar executor eval value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_data::TassadarSequenceSplit;

    use crate::build_tassadar_sudoku_v0_sequence_dataset;

    use super::{
        build_tassadar_executor_boundary_exactness_report,
        build_tassadar_executor_divergence_histogram_report,
        build_tassadar_executor_first_token_confusion_report,
        evaluate_tassadar_executor_transformer,
        evaluate_tassadar_executor_transformer_with_target_cap,
    };

    #[test]
    fn executor_eval_reports_trace_output_halt_and_boundary_metrics()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_sudoku_v0_sequence_dataset("train-v0")?;
        let model = psionic_models::TassadarExecutorTransformer::sudoku_v0();
        let report = evaluate_tassadar_executor_transformer(
            &model,
            &bundle.dataset,
            TassadarSequenceSplit::Validation,
        )?;

        assert_eq!(report.split, TassadarSequenceSplit::Validation);
        assert_eq!(report.case_reports.len(), 2);
        assert!(report.first_target_exactness_bps <= 10_000);
        assert!(report.first_8_token_exactness_bps <= 10_000);
        assert!(report.first_32_token_exactness_bps <= 10_000);
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| !case.reference_target_digest.is_empty())
        );
        Ok(())
    }

    #[test]
    fn executor_eval_can_cap_target_suffix_for_smoke_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_sudoku_v0_sequence_dataset("train-v0")?;
        let model = psionic_models::TassadarExecutorTransformer::sudoku_v0();
        let report = evaluate_tassadar_executor_transformer_with_target_cap(
            &model,
            &bundle.dataset,
            TassadarSequenceSplit::Validation,
            Some(8),
        )?;

        assert_eq!(report.case_reports.len(), 2);
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| !case.final_output_match && !case.halt_match)
        );
        Ok(())
    }

    #[test]
    fn executor_eval_builds_boundary_histogram_and_confusion_reports()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_sudoku_v0_sequence_dataset("train-v0")?;
        let model = psionic_models::TassadarExecutorTransformer::sudoku_v0();
        let report = evaluate_tassadar_executor_transformer(
            &model,
            &bundle.dataset,
            TassadarSequenceSplit::Validation,
        )?;
        let boundary = build_tassadar_executor_boundary_exactness_report(&report);
        let histogram = build_tassadar_executor_divergence_histogram_report(&report);
        let confusion = build_tassadar_executor_first_token_confusion_report(&report);

        assert_eq!(boundary.case_reports.len(), report.case_reports.len());
        assert_eq!(histogram.buckets.len(), 1);
        assert_eq!(histogram.buckets[0].first_divergence_index, 0);
        assert_eq!(confusion.token_zero_divergence_case_count, 2);
        assert_eq!(confusion.entries.len(), 1);
        Ok(())
    }
}
