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

/// Per-case exactness report for one trained executor model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorEvalCaseReport {
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable corpus case identifier.
    pub case_id: String,
    /// Token-level exactness over the predicted suffix.
    pub target_token_exactness_bps: u32,
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
    /// Number of cases that stayed exact over the full suffix.
    pub exact_trace_case_count: u32,
    /// Number of cases with exact final outputs.
    pub final_output_exact_case_count: u32,
    /// Number of cases with exact halt markers.
    pub halt_exact_case_count: u32,
    /// Per-case reports.
    pub case_reports: Vec<TassadarExecutorEvalCaseReport>,
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
    dataset.validate()?;
    let tokenizer = model.tokenizer();
    let mut case_reports = Vec::new();

    for example in dataset.split_examples(split) {
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
        case_reports.push(TassadarExecutorEvalCaseReport {
            sequence_id: example.sequence_id.clone(),
            case_id: example.metadata.case_id.clone(),
            target_token_exactness_bps,
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
        });
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
        evaluate_tassadar_executor_transformer,
        evaluate_tassadar_executor_transformer_with_target_cap,
    };

    #[test]
    fn executor_eval_reports_trace_output_and_halt_metrics()
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
}
