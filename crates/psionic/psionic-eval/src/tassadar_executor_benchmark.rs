use std::collections::BTreeMap;
use std::time::Instant;

use psionic_data::{
    TassadarSequenceDatasetContract, TassadarSequenceDatasetError, TassadarSequenceSplit,
};
use psionic_models::{
    TassadarExecutorTransformer, TassadarExecutorTransformerError, TokenId, TokenSequence,
    TokenizerBoundary,
};
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, tassadar_sudoku_v0_corpus,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Explicit KV-cache identity for the current neural linear decode path.
pub const TASSADAR_EXECUTOR_LINEAR_KV_IDENTITY: &str = "tassadar.prefix_recompute.no_kv_cache.v1";

/// Per-case trained-model benchmark report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorLinearBenchmarkCaseReport {
    /// Stable corpus case identifier.
    pub case_id: String,
    /// Split evaluated for this case.
    pub split: TassadarSequenceSplit,
    /// Token suffix length used for neural decode.
    pub target_token_count: u32,
    /// Neural decode elapsed time in milliseconds.
    pub neural_elapsed_ms: u64,
    /// CPU reference execution elapsed time in milliseconds.
    pub cpu_elapsed_ms: u64,
    /// Neural decode throughput in target tokens per second.
    pub neural_tokens_per_second: u32,
    /// CPU reference throughput normalized by the same target token count.
    pub cpu_tokens_per_second: u32,
    /// Whether the full predicted suffix stayed exact.
    pub exact_trace_match: bool,
    /// Whether final outputs matched.
    pub final_output_match: bool,
    /// Whether halt markers matched.
    pub halt_match: bool,
}

/// Aggregate neural-linear benchmark report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorLinearBenchmarkReport {
    /// Dataset storage key used for the benchmark.
    pub dataset_storage_key: String,
    /// Dataset digest used for the benchmark.
    pub dataset_digest: String,
    /// Stable trained model descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable trained weight digest.
    pub trained_weight_digest: String,
    /// Machine-legible decode mode label.
    pub decode_mode: String,
    /// Machine-legible KV-cache identity.
    pub kv_cache_identity: String,
    /// Aggregate neural throughput in target tokens per second.
    pub neural_tokens_per_second: u32,
    /// Aggregate CPU throughput in target tokens per second.
    pub cpu_tokens_per_second: u32,
    /// Per-case benchmark reports.
    pub case_reports: Vec<TassadarExecutorLinearBenchmarkCaseReport>,
}

/// Benchmark failure for the trained neural linear path.
#[derive(Debug, Error)]
pub enum TassadarExecutorLinearBenchmarkError {
    /// Dataset validation failed.
    #[error(transparent)]
    Dataset(#[from] TassadarSequenceDatasetError),
    /// Model forward/decode failed.
    #[error(transparent)]
    Model(#[from] TassadarExecutorTransformerError),
    /// CPU reference runner refused one program.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// One dataset case no longer maps to a runtime corpus program.
    #[error("missing runtime Sudoku-v0 program for case `{case_id}`")]
    MissingRuntimeCase {
        /// Missing case identifier.
        case_id: String,
    },
}

/// Benchmarks the trained executor model's neural linear decode against direct CPU reference execution.
pub fn benchmark_tassadar_executor_linear_decode(
    model: &TassadarExecutorTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split_filter: Option<TassadarSequenceSplit>,
) -> Result<TassadarExecutorLinearBenchmarkReport, TassadarExecutorLinearBenchmarkError> {
    dataset.validate()?;
    let tokenizer = model.tokenizer();
    let runtime_programs = tassadar_sudoku_v0_corpus()
        .into_iter()
        .map(|case| (case.validation_case.case_id, case.validation_case.program))
        .collect::<BTreeMap<_, _>>();
    let mut case_reports = Vec::new();
    let mut total_target_tokens = 0_u64;
    let mut total_neural_elapsed_ms = 0_u64;
    let mut total_cpu_elapsed_ms = 0_u64;

    for example in &dataset.examples {
        if split_filter.is_some_and(|split| example.metadata.split != split) {
            continue;
        }
        let Some(program) = runtime_programs.get(example.metadata.case_id.as_str()) else {
            return Err(TassadarExecutorLinearBenchmarkError::MissingRuntimeCase {
                case_id: example.metadata.case_id.clone(),
            });
        };
        let prompt_len = example.metadata.prompt_token_count as usize;
        let prompt = TokenSequence::new(
            example.token_ids[..prompt_len]
                .iter()
                .map(|token| TokenId(*token))
                .collect::<Vec<_>>(),
        );
        let reference_target = example.token_ids[prompt_len..]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>();

        let neural_started = Instant::now();
        let predicted_target = greedy_decode_target(model, prompt, reference_target.len())?;
        let neural_elapsed_ms = neural_started.elapsed().as_millis() as u64;

        let cpu_started = Instant::now();
        let _execution = TassadarCpuReferenceRunner::for_program(program)?.execute(program)?;
        let cpu_elapsed_ms = cpu_started.elapsed().as_millis() as u64;

        let predicted_full = example.token_ids[..prompt_len]
            .iter()
            .map(|token| TokenId(*token))
            .chain(predicted_target.iter().copied())
            .collect::<Vec<_>>();
        let reference_full = example
            .token_ids
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>();
        let target_token_count = example.metadata.target_token_count;
        total_target_tokens = total_target_tokens.saturating_add(u64::from(target_token_count));
        total_neural_elapsed_ms = total_neural_elapsed_ms.saturating_add(neural_elapsed_ms.max(1));
        total_cpu_elapsed_ms = total_cpu_elapsed_ms.saturating_add(cpu_elapsed_ms.max(1));

        case_reports.push(TassadarExecutorLinearBenchmarkCaseReport {
            case_id: example.metadata.case_id.clone(),
            split: example.metadata.split,
            target_token_count,
            neural_elapsed_ms,
            cpu_elapsed_ms,
            neural_tokens_per_second: tokens_per_second(target_token_count, neural_elapsed_ms),
            cpu_tokens_per_second: tokens_per_second(target_token_count, cpu_elapsed_ms),
            exact_trace_match: predicted_target == reference_target,
            final_output_match: tokenizer.extract_output_values(reference_full.as_slice())
                == tokenizer.extract_output_values(predicted_full.as_slice()),
            halt_match: tokenizer.extract_halt_marker(reference_full.as_slice())
                == tokenizer.extract_halt_marker(predicted_full.as_slice()),
        });
    }

    Ok(TassadarExecutorLinearBenchmarkReport {
        dataset_storage_key: dataset.storage_key(),
        dataset_digest: dataset.stable_digest(),
        model_descriptor_digest: model.descriptor().stable_digest(),
        trained_weight_digest: model.descriptor().weights.digest.clone(),
        decode_mode: String::from("reference_linear"),
        kv_cache_identity: String::from(TASSADAR_EXECUTOR_LINEAR_KV_IDENTITY),
        neural_tokens_per_second: tokens_per_second(
            total_target_tokens as u32,
            total_neural_elapsed_ms,
        ),
        cpu_tokens_per_second: tokens_per_second(total_target_tokens as u32, total_cpu_elapsed_ms),
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

fn tokens_per_second(tokens: u32, elapsed_ms: u64) -> u32 {
    if elapsed_ms == 0 {
        return tokens;
    }
    ((tokens as f64 / elapsed_ms as f64) * 1000.0).round() as u32
}

#[cfg(test)]
mod tests {
    use psionic_data::TassadarSequenceSplit;
    use psionic_models::TassadarExecutorTransformer;

    use crate::{
        benchmark_tassadar_executor_linear_decode, build_tassadar_sudoku_v0_sequence_dataset,
    };

    #[test]
    fn neural_linear_benchmark_reports_decode_identity_and_exactness()
    -> Result<(), Box<dyn std::error::Error>> {
        let trained = TassadarExecutorTransformer::sudoku_v0();
        let bundle = build_tassadar_sudoku_v0_sequence_dataset("train-v0")?;
        let report = benchmark_tassadar_executor_linear_decode(
            &trained,
            &bundle.dataset,
            Some(TassadarSequenceSplit::Validation),
        )?;

        assert_eq!(report.decode_mode, "reference_linear");
        assert_eq!(
            report.kv_cache_identity,
            super::TASSADAR_EXECUTOR_LINEAR_KV_IDENTITY
        );
        assert_eq!(report.case_reports.len(), 2);
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| case.target_token_count > 0)
        );
        Ok(())
    }
}
