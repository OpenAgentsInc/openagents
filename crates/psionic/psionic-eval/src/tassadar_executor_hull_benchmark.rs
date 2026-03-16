use std::collections::BTreeMap;
use std::time::Instant;

use psionic_data::{
    TassadarSequenceDatasetContract, TassadarSequenceDatasetError, TassadarSequenceSplit,
};
use psionic_models::{
    TassadarExecutorTransformer, TassadarExecutorTransformerDecodeSelection,
    TassadarExecutorTransformerError, TokenId, TokenSequence, TokenizerBoundary,
};
use psionic_runtime::{
    tassadar_sudoku_v0_corpus, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
    TassadarExecutorDecodeMode,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Explicit KV-cache identity for the trained-model linear-scan path over model KV state.
pub const TASSADAR_EXECUTOR_NEURAL_LINEAR_KV_IDENTITY: &str =
    "tassadar.executor_transformer.kv.linear_scan.v1";
/// Explicit KV-cache identity for the trained-model hull-cache path over model KV state.
pub const TASSADAR_EXECUTOR_NEURAL_HULL_KV_IDENTITY: &str =
    "tassadar.executor_transformer.kv.hull_cache.v1";
/// Default per-case target-token cap for the explicit model-KV benchmark.
pub const TASSADAR_EXECUTOR_NEURAL_HULL_BENCHMARK_TARGET_CAP: u32 = 4_096;

/// Per-case trained-model hull benchmark report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorHullBenchmarkCaseReport {
    /// Stable corpus case identifier.
    pub case_id: String,
    /// Split evaluated for this case.
    pub split: TassadarSequenceSplit,
    /// Full CPU-reference target suffix length for this case.
    pub full_target_token_count: u32,
    /// Prefix of the suffix used for explicit model-KV benchmarking.
    pub benchmark_target_token_count: u32,
    /// Machine-legible hull decode selection.
    pub hull_decode_selection: TassadarExecutorTransformerDecodeSelection,
    /// Explicit linear-scan decode elapsed time in milliseconds.
    pub linear_elapsed_ms: u64,
    /// Explicit hull-cache decode elapsed time in milliseconds.
    pub hull_elapsed_ms: u64,
    /// Full direct CPU execution elapsed time in milliseconds.
    pub cpu_full_execution_elapsed_ms: u64,
    /// Explicit linear-scan throughput in benchmark target tokens per second.
    pub linear_tokens_per_second: u32,
    /// Explicit hull-cache throughput in benchmark target tokens per second.
    pub hull_tokens_per_second: u32,
    /// Direct CPU throughput in full target tokens per second.
    pub cpu_full_execution_tokens_per_second: u32,
    /// Whether the explicit linear-scan decode stayed exact over the benchmark window.
    pub linear_exact_prefix_match: bool,
    /// Whether the hull decode stayed exact over the benchmark window.
    pub hull_exact_prefix_match: bool,
    /// Whether the hull decode matched the linear decode over the benchmark window.
    pub hull_matches_linear_prefix: bool,
}

/// Aggregate neural hull benchmark report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorHullBenchmarkReport {
    /// Dataset storage key used for the benchmark.
    pub dataset_storage_key: String,
    /// Dataset digest used for the benchmark.
    pub dataset_digest: String,
    /// Stable trained model descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable trained weight digest.
    pub trained_weight_digest: String,
    /// Stable requested decode mode for the fast path.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Stable explicit linear-scan identity.
    pub linear_kv_identity: String,
    /// Stable hull-cache identity.
    pub hull_kv_identity: String,
    /// Per-case benchmark target cap.
    pub benchmark_target_token_cap: u32,
    /// Aggregate benchmarked target tokens consumed by the neural paths.
    pub benchmarked_target_token_count: u32,
    /// Aggregate full target tokens covered by the direct CPU baseline.
    pub cpu_full_target_token_count: u32,
    /// Number of cases that executed directly on the hull path.
    pub direct_hull_case_count: u32,
    /// Number of cases that fell back from the requested hull path.
    pub hull_fallback_case_count: u32,
    /// Number of cases the model refused to decode.
    pub hull_refusal_case_count: u32,
    /// Aggregate linear-scan throughput in benchmark target tokens per second.
    pub linear_tokens_per_second: u32,
    /// Aggregate hull-cache throughput in benchmark target tokens per second.
    pub hull_tokens_per_second: u32,
    /// Aggregate CPU full-execution throughput in full target tokens per second.
    pub cpu_full_execution_tokens_per_second: u32,
    /// Number of cases exact on the explicit linear-scan path.
    pub linear_exact_prefix_case_count: u32,
    /// Number of cases exact on the hull path.
    pub hull_exact_prefix_case_count: u32,
    /// Number of cases where hull matched linear exactly over the benchmark window.
    pub hull_matches_linear_case_count: u32,
    /// Per-case reports.
    pub case_reports: Vec<TassadarExecutorHullBenchmarkCaseReport>,
}

/// Benchmark failure for the trained neural hull path.
#[derive(Debug, Error)]
pub enum TassadarExecutorHullBenchmarkError {
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
    /// The model refused the requested hull path.
    #[error("model refused hull decode for case `{case_id}`; supported modes: {supported:?}")]
    DecodeRefusal {
        /// Case identifier.
        case_id: String,
        /// Supported modes surfaced by the model.
        supported: Vec<TassadarExecutorDecodeMode>,
    },
}

/// Benchmarks trained-model explicit linear-scan decode against hull-cache decode and full direct CPU execution.
pub fn benchmark_tassadar_executor_neural_hull_decode(
    model: &TassadarExecutorTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split_filter: Option<TassadarSequenceSplit>,
    max_target_tokens_per_case: Option<u32>,
) -> Result<TassadarExecutorHullBenchmarkReport, TassadarExecutorHullBenchmarkError> {
    dataset.validate()?;
    let runtime_programs = tassadar_sudoku_v0_corpus()
        .into_iter()
        .map(|case| (case.validation_case.case_id, case.validation_case.program))
        .collect::<BTreeMap<_, _>>();
    let benchmark_target_token_cap =
        max_target_tokens_per_case.unwrap_or(TASSADAR_EXECUTOR_NEURAL_HULL_BENCHMARK_TARGET_CAP);
    let mut case_reports = Vec::new();
    let mut benchmarked_target_token_count = 0_u64;
    let mut cpu_full_target_token_count = 0_u64;
    let mut total_linear_elapsed_ms = 0_u64;
    let mut total_hull_elapsed_ms = 0_u64;
    let mut total_cpu_elapsed_ms = 0_u64;

    for example in &dataset.examples {
        if split_filter.is_some_and(|split| example.metadata.split != split) {
            continue;
        }
        let Some(program) = runtime_programs.get(example.metadata.case_id.as_str()) else {
            return Err(TassadarExecutorHullBenchmarkError::MissingRuntimeCase {
                case_id: example.metadata.case_id.clone(),
            });
        };

        let prompt_len = example.metadata.prompt_token_count as usize;
        let benchmark_target_token_count = usize::min(
            example.metadata.target_token_count as usize,
            benchmark_target_token_cap as usize,
        );
        let prompt = TokenSequence::new(
            example.token_ids[..prompt_len]
                .iter()
                .map(|token| TokenId(*token))
                .collect::<Vec<_>>(),
        );
        let reference_target = example.token_ids
            [prompt_len..prompt_len + benchmark_target_token_count]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>();

        let linear_started = Instant::now();
        let (linear_target, _) = greedy_decode_target_with_mode(
            model,
            example.metadata.case_id.as_str(),
            prompt.clone(),
            benchmark_target_token_count,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;
        let linear_elapsed_ms = linear_started.elapsed().as_millis() as u64;

        let hull_started = Instant::now();
        let (hull_target, hull_selection) = greedy_decode_target_with_mode(
            model,
            example.metadata.case_id.as_str(),
            prompt,
            benchmark_target_token_count,
            TassadarExecutorDecodeMode::HullCache,
        )?;
        let hull_elapsed_ms = hull_started.elapsed().as_millis() as u64;

        let cpu_started = Instant::now();
        let _execution = TassadarCpuReferenceRunner::for_program(program)?.execute(program)?;
        let cpu_elapsed_ms = cpu_started.elapsed().as_millis() as u64;

        benchmarked_target_token_count =
            benchmarked_target_token_count.saturating_add(benchmark_target_token_count as u64);
        cpu_full_target_token_count = cpu_full_target_token_count
            .saturating_add(u64::from(example.metadata.target_token_count));
        total_linear_elapsed_ms = total_linear_elapsed_ms.saturating_add(linear_elapsed_ms.max(1));
        total_hull_elapsed_ms = total_hull_elapsed_ms.saturating_add(hull_elapsed_ms.max(1));
        total_cpu_elapsed_ms = total_cpu_elapsed_ms.saturating_add(cpu_elapsed_ms.max(1));

        case_reports.push(TassadarExecutorHullBenchmarkCaseReport {
            case_id: example.metadata.case_id.clone(),
            split: example.metadata.split,
            full_target_token_count: example.metadata.target_token_count,
            benchmark_target_token_count: benchmark_target_token_count as u32,
            hull_decode_selection: hull_selection,
            linear_elapsed_ms,
            hull_elapsed_ms,
            cpu_full_execution_elapsed_ms: cpu_elapsed_ms,
            linear_tokens_per_second: tokens_per_second(
                benchmark_target_token_count as u32,
                linear_elapsed_ms,
            ),
            hull_tokens_per_second: tokens_per_second(
                benchmark_target_token_count as u32,
                hull_elapsed_ms,
            ),
            cpu_full_execution_tokens_per_second: tokens_per_second(
                example.metadata.target_token_count,
                cpu_elapsed_ms,
            ),
            linear_exact_prefix_match: linear_target == reference_target,
            hull_exact_prefix_match: hull_target == reference_target,
            hull_matches_linear_prefix: hull_target == linear_target,
        });
    }

    Ok(TassadarExecutorHullBenchmarkReport {
        dataset_storage_key: dataset.storage_key(),
        dataset_digest: dataset.stable_digest(),
        model_descriptor_digest: model.descriptor().stable_digest(),
        trained_weight_digest: model.descriptor().weights.digest.clone(),
        requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
        linear_kv_identity: String::from(TASSADAR_EXECUTOR_NEURAL_LINEAR_KV_IDENTITY),
        hull_kv_identity: String::from(TASSADAR_EXECUTOR_NEURAL_HULL_KV_IDENTITY),
        benchmark_target_token_cap,
        benchmarked_target_token_count: benchmarked_target_token_count as u32,
        cpu_full_target_token_count: cpu_full_target_token_count as u32,
        direct_hull_case_count: case_reports
            .iter()
            .filter(|case| {
                case.hull_decode_selection.effective_decode_mode
                    == Some(TassadarExecutorDecodeMode::HullCache)
            })
            .count() as u32,
        hull_fallback_case_count: case_reports
            .iter()
            .filter(|case| case.hull_decode_selection.fallback_decode_mode.is_some())
            .count() as u32,
        hull_refusal_case_count: case_reports
            .iter()
            .filter(|case| case.hull_decode_selection.effective_decode_mode.is_none())
            .count() as u32,
        linear_tokens_per_second: tokens_per_second(
            benchmarked_target_token_count as u32,
            total_linear_elapsed_ms,
        ),
        hull_tokens_per_second: tokens_per_second(
            benchmarked_target_token_count as u32,
            total_hull_elapsed_ms,
        ),
        cpu_full_execution_tokens_per_second: tokens_per_second(
            cpu_full_target_token_count as u32,
            total_cpu_elapsed_ms,
        ),
        linear_exact_prefix_case_count: case_reports
            .iter()
            .filter(|case| case.linear_exact_prefix_match)
            .count() as u32,
        hull_exact_prefix_case_count: case_reports
            .iter()
            .filter(|case| case.hull_exact_prefix_match)
            .count() as u32,
        hull_matches_linear_case_count: case_reports
            .iter()
            .filter(|case| case.hull_matches_linear_prefix)
            .count() as u32,
        case_reports,
    })
}

fn greedy_decode_target_with_mode(
    model: &TassadarExecutorTransformer,
    case_id: &str,
    prompt: TokenSequence,
    target_token_count: usize,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<
    (Vec<TokenId>, TassadarExecutorTransformerDecodeSelection),
    TassadarExecutorHullBenchmarkError,
> {
    let selection = model.select_decode_mode(requested_decode_mode);
    let Some(effective_decode_mode) = selection.effective_decode_mode else {
        return Err(TassadarExecutorHullBenchmarkError::DecodeRefusal {
            case_id: case_id.to_string(),
            supported: selection.supported_decode_modes,
        });
    };
    let mut state = model.start_decode(prompt)?;
    let mut predicted = Vec::with_capacity(target_token_count);
    for _ in 0..target_token_count {
        let next = model.greedy_next_token_for_mode(&state, effective_decode_mode)?;
        model.push_decoded_token(&mut state, next)?;
        predicted.push(next);
        if next == model.tokenizer().vocabulary().eos_id() {
            break;
        }
    }
    Ok((predicted, selection))
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
    use psionic_runtime::TassadarExecutorDecodeMode;

    use crate::{
        benchmark_tassadar_executor_neural_hull_decode, build_tassadar_sudoku_v0_sequence_dataset,
    };

    #[test]
    fn neural_hull_benchmark_reports_direct_hull_selection_and_window_cap(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let trained = psionic_models::TassadarExecutorTransformer::sudoku_v0();
        let bundle = build_tassadar_sudoku_v0_sequence_dataset("train-v0")?;
        let report = benchmark_tassadar_executor_neural_hull_decode(
            &trained,
            &bundle.dataset,
            Some(TassadarSequenceSplit::Validation),
            Some(64),
        )?;

        assert_eq!(
            report.requested_decode_mode,
            TassadarExecutorDecodeMode::HullCache
        );
        assert_eq!(report.benchmark_target_token_cap, 64);
        assert_eq!(report.case_reports.len(), 2);
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.hull_decode_selection.effective_decode_mode
                == Some(TassadarExecutorDecodeMode::HullCache)));
        Ok(())
    }
}
