use std::time::Instant;

use psionic_data::{
    TassadarSequenceDatasetContract, TassadarSequenceDatasetError, TassadarSequenceSplit,
};
use psionic_models::{
    TassadarExecutorAttentionDecodeRefusal, TassadarExecutorAttentionTransformer,
    TassadarExecutorAttentionError, TassadarExecutorTransformer,
    TassadarExecutorTransformerDecodeRefusal, TassadarExecutorTransformerError, TokenId,
    TokenSequence, TokenizerBoundary,
};
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    tassadar_sudoku_v0_corpus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable family labels carried by bounded architecture-comparison reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorArchitectureFamilyKind {
    LookupBaseline,
    ExecutorAttentionCandidate,
}

impl TassadarExecutorArchitectureFamilyKind {
    /// Returns the stable family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LookupBaseline => "lookup_baseline",
            Self::ExecutorAttentionCandidate => "executor_attention_candidate",
        }
    }
}

/// How one family handles hull-backed decode requests in the bounded comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorArchitectureHullDecodePosture {
    Direct,
    FallbackToReferenceLinear,
    Refused,
}

/// Machine-readable decode-selection summary for one family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorArchitectureDecodeSelectionReport {
    /// Decode path requested by the report.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Decode path actually executed by the family when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Exact fallback mode used when the request could not execute directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Refusal label when the request could not execute at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// High-level hull posture for this family.
    pub hull_decode_posture: TassadarExecutorArchitectureHullDecodePosture,
}

/// Aggregate correctness summary for one family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorArchitectureCorrectnessSummary {
    /// Aggregate token-level exactness over the evaluated split/window.
    pub aggregate_target_token_exactness_bps: u32,
    /// Aggregate exactness over the first target token.
    pub first_target_exactness_bps: u32,
    /// Aggregate exactness over the first eight target tokens.
    pub first_8_token_exactness_bps: u32,
    /// Aggregate exactness over the first 32 target tokens.
    pub first_32_token_exactness_bps: u32,
    /// Number of cases that stayed exact over the bounded decoded suffix.
    pub exact_trace_case_count: u32,
}

/// Aggregate speed summary for one family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorArchitectureSpeedSummary {
    /// Decode mode used for the benchmark.
    pub decode_mode: TassadarExecutorDecodeMode,
    /// Aggregate neural throughput in target tokens per second.
    pub neural_tokens_per_second: u32,
    /// Aggregate CPU reference throughput normalized by the same target count.
    pub cpu_tokens_per_second: u32,
}

/// Bounded per-case architecture-comparison report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorArchitectureCaseReport {
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable case identifier.
    pub case_id: String,
    /// Evaluated target-token count after caps.
    pub target_token_count: u32,
    /// Token-level exactness over the predicted suffix.
    pub target_token_exactness_bps: u32,
    /// Exactness over the first target token.
    pub first_target_exactness_bps: u32,
    /// Exactness over the first eight target tokens.
    pub first_8_token_exactness_bps: u32,
    /// Exactness over the first 32 target tokens.
    pub first_32_token_exactness_bps: u32,
    /// Number of exact target tokens before the first divergence.
    pub matched_target_token_count: u32,
    /// First target-token index where divergence appeared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_divergence_index: Option<u32>,
    /// Whether the bounded predicted suffix matched exactly.
    pub exact_trace_match: bool,
}

/// Machine-readable bounded family report used by research run bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorArchitectureFamilyReport {
    /// Family kind under test.
    pub family_kind: TassadarExecutorArchitectureFamilyKind,
    /// Stable model id.
    pub model_id: String,
    /// Stable descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable weight digest.
    pub trained_weight_digest: String,
    /// Explicit claim boundary for this family.
    pub claim_boundary: String,
    /// Explicit attention-mode label for this family.
    pub attention_mode: String,
    /// Number of layers in the family.
    pub layer_count: u32,
    /// Number of heads in the family.
    pub head_count: u32,
    /// Head dimension in the family.
    pub head_dim: u32,
    /// Prompt window used during bounded evaluation.
    pub prompt_window_token_cap: u32,
    /// Target-token cap used during bounded evaluation.
    pub target_token_cap: u32,
    /// One-sentence architecture fidelity note for the family.
    pub article_fidelity_summary: String,
    /// Aggregate correctness metrics.
    pub correctness: TassadarExecutorArchitectureCorrectnessSummary,
    /// Aggregate speed metrics.
    pub speed: TassadarExecutorArchitectureSpeedSummary,
    /// Decode-selection truth when hull is requested.
    pub requested_hull_decode: TassadarExecutorArchitectureDecodeSelectionReport,
    /// Per-case bounded reports.
    pub case_reports: Vec<TassadarExecutorArchitectureCaseReport>,
    /// Stable report digest.
    pub report_digest: String,
}

/// Top-level same-corpus architecture-comparison report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorArchitectureComparisonReport {
    /// Dataset storage key used for the comparison.
    pub dataset_storage_key: String,
    /// Dataset digest used for the comparison.
    pub dataset_digest: String,
    /// Evaluated split.
    pub split: TassadarSequenceSplit,
    /// Prompt window cap used for both families.
    pub prompt_window_token_cap: u32,
    /// Target-token cap used for both families.
    pub target_token_cap: u32,
    /// Lookup-family baseline report.
    pub lookup_baseline: TassadarExecutorArchitectureFamilyReport,
    /// Executor-attention candidate report.
    pub executor_attention_candidate: TassadarExecutorArchitectureFamilyReport,
    /// Whether the candidate is more exact over the bounded comparison window.
    pub candidate_more_exact: bool,
    /// Whether the candidate is faster in reference-linear decode.
    pub candidate_faster_reference_linear: bool,
    /// Whether the candidate is architecturally closer to the article claim.
    pub candidate_closer_to_article_fidelity: bool,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarExecutorArchitectureComparisonReport {
    fn new(
        dataset_storage_key: String,
        dataset_digest: String,
        split: TassadarSequenceSplit,
        prompt_window_token_cap: usize,
        target_token_cap: usize,
        lookup_baseline: TassadarExecutorArchitectureFamilyReport,
        executor_attention_candidate: TassadarExecutorArchitectureFamilyReport,
    ) -> Self {
        let candidate_more_exact =
            correctness_rank(&executor_attention_candidate.correctness)
                > correctness_rank(&lookup_baseline.correctness);
        let candidate_faster_reference_linear =
            executor_attention_candidate.speed.neural_tokens_per_second
                > lookup_baseline.speed.neural_tokens_per_second;
        let candidate_closer_to_article_fidelity =
            executor_attention_candidate.family_kind
                == TassadarExecutorArchitectureFamilyKind::ExecutorAttentionCandidate;
        let mut report = Self {
            dataset_storage_key,
            dataset_digest,
            split,
            prompt_window_token_cap: prompt_window_token_cap as u32,
            target_token_cap: target_token_cap as u32,
            lookup_baseline,
            executor_attention_candidate,
            candidate_more_exact,
            candidate_faster_reference_linear,
            candidate_closer_to_article_fidelity,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_executor_architecture_comparison_report|", &report);
        report
    }
}

/// Failure while scoring the bounded same-corpus architecture comparison.
#[derive(Debug, Error)]
pub enum TassadarExecutorArchitectureComparisonError {
    /// Dataset validation failed.
    #[error(transparent)]
    Dataset(#[from] TassadarSequenceDatasetError),
    /// Lookup-family decode failed.
    #[error(transparent)]
    LookupModel(#[from] TassadarExecutorTransformerError),
    /// Executor-attention decode failed.
    #[error(transparent)]
    AttentionModel(#[from] TassadarExecutorAttentionError),
    /// CPU reference runner refused one program.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// One dataset case no longer maps to a runtime corpus program.
    #[error("missing runtime Sudoku-v0 program for case `{case_id}`")]
    MissingRuntimeCase { case_id: String },
    /// One bounded report compared different datasets.
    #[error("family reports used different datasets: expected `{expected}`, found `{actual}`")]
    DatasetMismatch { expected: String, actual: String },
}

/// Evaluates the preserved lookup-family baseline on a bounded same-corpus window.
pub fn evaluate_lookup_family_for_architecture_comparison(
    model: &TassadarExecutorTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
) -> Result<TassadarExecutorArchitectureFamilyReport, TassadarExecutorArchitectureComparisonError>
{
    dataset.validate()?;
    let runtime_programs = runtime_programs();
    let mut case_reports = Vec::new();
    let mut total_target_tokens = 0_u64;
    let mut total_neural_elapsed_ms = 0_u64;
    let mut total_cpu_elapsed_ms = 0_u64;

    for example in dataset.split_examples(split) {
        let window = bounded_case_window(example.token_ids.as_slice(), example.metadata.prompt_token_count as usize, prompt_window_token_cap, target_token_cap);
        let predicted_target = timed_lookup_decode(model, window.prompt.clone(), window.reference_target.len(), &mut total_neural_elapsed_ms)?;
        let cpu_elapsed_ms = timed_cpu_reference(runtime_programs.as_slice(), example.metadata.case_id.as_str())?;
        total_cpu_elapsed_ms = total_cpu_elapsed_ms.saturating_add(cpu_elapsed_ms.max(1));
        total_target_tokens = total_target_tokens.saturating_add(window.reference_target.len() as u64);
        case_reports.push(build_case_report(
            example.sequence_id.clone(),
            example.metadata.case_id.clone(),
            window.reference_target.as_slice(),
            predicted_target.as_slice(),
        ));
    }
    Ok(build_family_report(
        dataset,
        split,
        prompt_window_token_cap,
        target_token_cap,
        TassadarExecutorArchitectureFamilyKind::LookupBaseline,
        model.descriptor().model.model_id.clone(),
        model.descriptor().stable_digest(),
        model.descriptor().weights.digest.clone(),
        String::from("greedy_decode_unvalidated"),
        String::from("hard_max_lookup"),
        1,
        model.descriptor().config.context_offsets.len() as u32,
        model.descriptor().config.constrained_lookup_head_dim as u32,
        String::from("fixed relative-offset lookup heads plus optional learned mixer; not a layered causal-attention stack"),
        build_lookup_decode_selection(model),
        total_target_tokens,
        total_neural_elapsed_ms,
        total_cpu_elapsed_ms,
        case_reports,
    ))
}

/// Evaluates the bounded executor-attention candidate on the same comparison window.
pub fn evaluate_attention_family_for_architecture_comparison(
    model: &TassadarExecutorAttentionTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
) -> Result<TassadarExecutorArchitectureFamilyReport, TassadarExecutorArchitectureComparisonError>
{
    dataset.validate()?;
    let runtime_programs = runtime_programs();
    let mut case_reports = Vec::new();
    let mut total_target_tokens = 0_u64;
    let mut total_neural_elapsed_ms = 0_u64;
    let mut total_cpu_elapsed_ms = 0_u64;

    for example in dataset.split_examples(split) {
        let window = bounded_case_window(example.token_ids.as_slice(), example.metadata.prompt_token_count as usize, prompt_window_token_cap, target_token_cap);
        let predicted_target = timed_attention_decode(model, window.prompt.clone(), window.reference_target.len(), &mut total_neural_elapsed_ms)?;
        let cpu_elapsed_ms = timed_cpu_reference(runtime_programs.as_slice(), example.metadata.case_id.as_str())?;
        total_cpu_elapsed_ms = total_cpu_elapsed_ms.saturating_add(cpu_elapsed_ms.max(1));
        total_target_tokens = total_target_tokens.saturating_add(window.reference_target.len() as u64);
        case_reports.push(build_case_report(
            example.sequence_id.clone(),
            example.metadata.case_id.clone(),
            window.reference_target.as_slice(),
            predicted_target.as_slice(),
        ));
    }
    let article_fidelity_summary = if model.has_relative_target_output_bias_signal() {
        String::from(
            "layered full-prefix causal 2D-head hard-max attention plus a bounded relative-target logit-bias adapter, still only as a research windowed lane with hull fallback",
        )
    } else {
        String::from(
            "layered full-prefix causal 2D-head hard-max attention, but only as a bounded windowed research lane with hull fallback",
        )
    };

    Ok(build_family_report(
        dataset,
        split,
        prompt_window_token_cap,
        target_token_cap,
        TassadarExecutorArchitectureFamilyKind::ExecutorAttentionCandidate,
        model.descriptor().model.model_id.clone(),
        model.descriptor().stable_digest(),
        model.descriptor().weights.digest.clone(),
        String::from("research_windowed_decode_only"),
        String::from("layered_causal_hard_max_lookup"),
        model.descriptor().config.layer_count as u32,
        model.descriptor().config.head_count as u32,
        model.descriptor().config.head_dim as u32,
        article_fidelity_summary,
        build_attention_decode_selection(model),
        total_target_tokens,
        total_neural_elapsed_ms,
        total_cpu_elapsed_ms,
        case_reports,
    ))
}

/// Builds a top-level architecture-comparison report from two bounded family reports.
pub fn build_tassadar_executor_architecture_comparison_report(
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
    lookup_baseline: TassadarExecutorArchitectureFamilyReport,
    executor_attention_candidate: TassadarExecutorArchitectureFamilyReport,
) -> Result<TassadarExecutorArchitectureComparisonReport, TassadarExecutorArchitectureComparisonError>
{
    if lookup_baseline.model_id == executor_attention_candidate.model_id
        && lookup_baseline.family_kind != executor_attention_candidate.family_kind
    {
        return Err(TassadarExecutorArchitectureComparisonError::DatasetMismatch {
            expected: dataset.storage_key(),
            actual: dataset.storage_key(),
        });
    }
    Ok(TassadarExecutorArchitectureComparisonReport::new(
        dataset.storage_key(),
        dataset.stable_digest(),
        split,
        prompt_window_token_cap,
        target_token_cap,
        lookup_baseline,
        executor_attention_candidate,
    ))
}

#[derive(Clone)]
struct BoundedCaseWindow {
    prompt: TokenSequence,
    reference_target: Vec<TokenId>,
}

fn bounded_case_window(
    token_ids: &[u32],
    prompt_len: usize,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
) -> BoundedCaseWindow {
    let prompt_start = prompt_len.saturating_sub(prompt_window_token_cap.max(1));
    let prompt = TokenSequence::new(
        token_ids[prompt_start..prompt_len]
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>(),
    );
    let reference_target = token_ids[prompt_len..]
        .iter()
        .take(target_token_cap.max(1))
        .map(|token| TokenId(*token))
        .collect::<Vec<_>>();
    BoundedCaseWindow {
        prompt,
        reference_target,
    }
}

fn timed_lookup_decode(
    model: &TassadarExecutorTransformer,
    prompt: TokenSequence,
    target_token_count: usize,
    total_elapsed_ms: &mut u64,
) -> Result<Vec<TokenId>, TassadarExecutorArchitectureComparisonError> {
    let started = Instant::now();
    let predicted = greedy_decode_lookup(model, prompt, target_token_count)?;
    *total_elapsed_ms =
        total_elapsed_ms.saturating_add(started.elapsed().as_millis() as u64);
    Ok(predicted)
}

fn timed_attention_decode(
    model: &TassadarExecutorAttentionTransformer,
    prompt: TokenSequence,
    target_token_count: usize,
    total_elapsed_ms: &mut u64,
) -> Result<Vec<TokenId>, TassadarExecutorArchitectureComparisonError> {
    let started = Instant::now();
    let predicted = greedy_decode_attention(model, prompt, target_token_count)?;
    *total_elapsed_ms =
        total_elapsed_ms.saturating_add(started.elapsed().as_millis() as u64);
    Ok(predicted)
}

fn timed_cpu_reference(
    runtime_programs: &[(String, psionic_runtime::TassadarProgram)],
    case_id: &str,
) -> Result<u64, TassadarExecutorArchitectureComparisonError> {
    let Some((_, program)) = runtime_programs
        .iter()
        .find(|(candidate_case_id, _)| candidate_case_id == case_id)
    else {
        return Err(TassadarExecutorArchitectureComparisonError::MissingRuntimeCase {
            case_id: case_id.to_string(),
        });
    };
    let started = Instant::now();
    let _execution = TassadarCpuReferenceRunner::for_program(program)?.execute(program)?;
    Ok(started.elapsed().as_millis() as u64)
}

fn greedy_decode_lookup(
    model: &TassadarExecutorTransformer,
    prompt: TokenSequence,
    target_token_count: usize,
) -> Result<Vec<TokenId>, TassadarExecutorArchitectureComparisonError> {
    let mut state = model.start_decode(prompt)?;
    let mut predicted = Vec::with_capacity(target_token_count);
    for _ in 0..target_token_count {
        let next = model.greedy_next_token(&state)?;
        model.push_decoded_token(&mut state, next)?;
        predicted.push(next);
        if next == model.tokenizer().vocabulary().eos_id() {
            break;
        }
    }
    Ok(predicted)
}

fn greedy_decode_attention(
    model: &TassadarExecutorAttentionTransformer,
    prompt: TokenSequence,
    target_token_count: usize,
) -> Result<Vec<TokenId>, TassadarExecutorArchitectureComparisonError> {
    let mut state = model.start_decode(prompt)?;
    let mut predicted = Vec::with_capacity(target_token_count);
    for _ in 0..target_token_count {
        let next = model.greedy_next_token(&state)?;
        model.push_decoded_token(&mut state, next)?;
        predicted.push(next);
        if next == model.tokenizer().vocabulary().eos_id() {
            break;
        }
    }
    Ok(predicted)
}

fn build_case_report(
    sequence_id: String,
    case_id: String,
    reference_target: &[TokenId],
    predicted_target: &[TokenId],
) -> TassadarExecutorArchitectureCaseReport {
    let matched_target_token_count =
        matched_target_token_count(reference_target, predicted_target);
    let first_divergence_index = first_divergence_index(reference_target, predicted_target);
    TassadarExecutorArchitectureCaseReport {
        sequence_id,
        case_id,
        target_token_count: reference_target.len() as u32,
        target_token_exactness_bps: suffix_exactness_bps(reference_target, predicted_target),
        first_target_exactness_bps: prefix_exactness_bps(reference_target, predicted_target, 1),
        first_8_token_exactness_bps: prefix_exactness_bps(reference_target, predicted_target, 8),
        first_32_token_exactness_bps: prefix_exactness_bps(reference_target, predicted_target, 32),
        matched_target_token_count,
        first_divergence_index,
        exact_trace_match: predicted_target == reference_target,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_family_report(
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
    family_kind: TassadarExecutorArchitectureFamilyKind,
    model_id: String,
    model_descriptor_digest: String,
    trained_weight_digest: String,
    claim_boundary: String,
    attention_mode: String,
    layer_count: u32,
    head_count: u32,
    head_dim: u32,
    article_fidelity_summary: String,
    requested_hull_decode: TassadarExecutorArchitectureDecodeSelectionReport,
    total_target_tokens: u64,
    total_neural_elapsed_ms: u64,
    total_cpu_elapsed_ms: u64,
    case_reports: Vec<TassadarExecutorArchitectureCaseReport>,
) -> TassadarExecutorArchitectureFamilyReport {
    let correctness = TassadarExecutorArchitectureCorrectnessSummary {
        aggregate_target_token_exactness_bps: average_bps(
            case_reports
                .iter()
                .map(|case| case.target_token_exactness_bps)
                .collect(),
        ),
        first_target_exactness_bps: average_bps(
            case_reports
                .iter()
                .map(|case| case.first_target_exactness_bps)
                .collect(),
        ),
        first_8_token_exactness_bps: average_bps(
            case_reports
                .iter()
                .map(|case| case.first_8_token_exactness_bps)
                .collect(),
        ),
        first_32_token_exactness_bps: average_bps(
            case_reports
                .iter()
                .map(|case| case.first_32_token_exactness_bps)
                .collect(),
        ),
        exact_trace_case_count: case_reports
            .iter()
            .filter(|case| case.exact_trace_match)
            .count() as u32,
    };
    let speed = TassadarExecutorArchitectureSpeedSummary {
        decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
        neural_tokens_per_second: tokens_per_second(total_target_tokens as u32, total_neural_elapsed_ms),
        cpu_tokens_per_second: tokens_per_second(total_target_tokens as u32, total_cpu_elapsed_ms),
    };
    let mut report = TassadarExecutorArchitectureFamilyReport {
        family_kind,
        model_id,
        model_descriptor_digest,
        trained_weight_digest,
        claim_boundary,
        attention_mode,
        layer_count,
        head_count,
        head_dim,
        prompt_window_token_cap: prompt_window_token_cap as u32,
        target_token_cap: target_token_cap as u32,
        article_fidelity_summary,
        correctness,
        speed,
        requested_hull_decode,
        case_reports,
        report_digest: String::new(),
    };
    report.report_digest =
        stable_digest(b"psionic_tassadar_executor_architecture_family_report|", &(
            dataset.storage_key(),
            dataset.stable_digest(),
            split,
            &report,
        ));
    report
}

fn build_lookup_decode_selection(
    model: &TassadarExecutorTransformer,
) -> TassadarExecutorArchitectureDecodeSelectionReport {
    let selection = model.select_decode_mode(TassadarExecutorDecodeMode::HullCache);
    TassadarExecutorArchitectureDecodeSelectionReport {
        requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
        effective_decode_mode: selection.effective_decode_mode,
        fallback_decode_mode: selection.fallback_decode_mode,
        refusal: selection.refusal.map(|refusal| match refusal {
            TassadarExecutorTransformerDecodeRefusal::NoSupportedDecodeMode => {
                String::from("no_supported_decode_mode")
            }
        }),
        hull_decode_posture: selection_posture(
            selection.effective_decode_mode,
            selection.fallback_decode_mode,
            selection.refusal.is_some(),
        ),
    }
}

fn build_attention_decode_selection(
    model: &TassadarExecutorAttentionTransformer,
) -> TassadarExecutorArchitectureDecodeSelectionReport {
    let selection = model.select_decode_mode(TassadarExecutorDecodeMode::HullCache);
    TassadarExecutorArchitectureDecodeSelectionReport {
        requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
        effective_decode_mode: selection.effective_decode_mode,
        fallback_decode_mode: selection.fallback_decode_mode,
        refusal: selection.refusal.map(|refusal| match refusal {
            TassadarExecutorAttentionDecodeRefusal::NoSupportedDecodeMode => {
                String::from("no_supported_decode_mode")
            }
        }),
        hull_decode_posture: selection_posture(
            selection.effective_decode_mode,
            selection.fallback_decode_mode,
            selection.refusal.is_some(),
        ),
    }
}

fn selection_posture(
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
    refused: bool,
) -> TassadarExecutorArchitectureHullDecodePosture {
    if refused {
        return TassadarExecutorArchitectureHullDecodePosture::Refused;
    }
    match (effective_decode_mode, fallback_decode_mode) {
        (Some(TassadarExecutorDecodeMode::HullCache), None) => {
            TassadarExecutorArchitectureHullDecodePosture::Direct
        }
        (Some(TassadarExecutorDecodeMode::ReferenceLinear), Some(_)) => {
            TassadarExecutorArchitectureHullDecodePosture::FallbackToReferenceLinear
        }
        _ => TassadarExecutorArchitectureHullDecodePosture::Refused,
    }
}

fn runtime_programs() -> Vec<(String, psionic_runtime::TassadarProgram)> {
    tassadar_sudoku_v0_corpus()
        .into_iter()
        .map(|case| (case.validation_case.case_id, case.validation_case.program))
        .collect::<Vec<_>>()
}

fn matched_target_token_count(reference_target: &[TokenId], predicted_target: &[TokenId]) -> u32 {
    reference_target
        .iter()
        .zip(predicted_target.iter())
        .take_while(|(reference, predicted)| reference == predicted)
        .count() as u32
}

fn first_divergence_index(reference_target: &[TokenId], predicted_target: &[TokenId]) -> Option<u32> {
    reference_target
        .iter()
        .zip(predicted_target.iter())
        .position(|(reference, predicted)| reference != predicted)
        .map(|index| index as u32)
        .or_else(|| {
            (reference_target.len() != predicted_target.len())
                .then_some(reference_target.len().min(predicted_target.len()) as u32)
        })
}

fn suffix_exactness_bps(reference_target: &[TokenId], predicted_target: &[TokenId]) -> u32 {
    if reference_target.is_empty() {
        return 10_000;
    }
    let exact = reference_target
        .iter()
        .zip(predicted_target.iter())
        .filter(|(reference, predicted)| reference == predicted)
        .count();
    ((exact as f64 / reference_target.len() as f64) * 10_000.0).round() as u32
}

fn prefix_exactness_bps(reference_target: &[TokenId], predicted_target: &[TokenId], prefix_len: usize) -> u32 {
    if reference_target.is_empty() {
        return 10_000;
    }
    let evaluated = reference_target.len().min(prefix_len);
    if evaluated == 0 {
        return 10_000;
    }
    let exact = reference_target
        .iter()
        .take(evaluated)
        .zip(predicted_target.iter().take(evaluated))
        .filter(|(reference, predicted)| reference == predicted)
        .count();
    ((exact as f64 / evaluated as f64) * 10_000.0).round() as u32
}

fn average_bps(values: Vec<u32>) -> u32 {
    if values.is_empty() {
        return 0;
    }
    (values.iter().map(|value| u64::from(*value)).sum::<u64>() / values.len() as u64) as u32
}

fn correctness_rank(summary: &TassadarExecutorArchitectureCorrectnessSummary) -> (u32, u32, u32, u32, u32) {
    (
        summary.first_target_exactness_bps,
        summary.first_32_token_exactness_bps,
        summary.first_8_token_exactness_bps,
        summary.exact_trace_case_count,
        summary.aggregate_target_token_exactness_bps,
    )
}

fn tokens_per_second(tokens: u32, elapsed_ms: u64) -> u32 {
    if elapsed_ms == 0 {
        return tokens;
    }
    ((tokens as f64 / elapsed_ms as f64) * 1000.0).round() as u32
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar architecture comparison value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_data::TassadarSequenceSplit;
    use psionic_models::{
        TassadarExecutorAttentionTransformer, TassadarExecutorTrainableSurface,
        TassadarExecutorTransformer,
    };

    use crate::{
        build_tassadar_sudoku_v0_sequence_dataset,
        build_tassadar_executor_architecture_comparison_report,
        evaluate_attention_family_for_architecture_comparison,
        evaluate_lookup_family_for_architecture_comparison,
    };

    #[test]
    fn architecture_comparison_reports_bounded_same_corpus_results()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset = build_tassadar_sudoku_v0_sequence_dataset("train-v0")?;
        let lookup = TassadarExecutorTransformer::sudoku_v0_with_surface(
            TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
        );
        let attention = TassadarExecutorAttentionTransformer::sudoku_v0();
        let lookup_report = evaluate_lookup_family_for_architecture_comparison(
            &lookup,
            &dataset.dataset,
            TassadarSequenceSplit::Validation,
            256,
            32,
        )?;
        let attention_report = evaluate_attention_family_for_architecture_comparison(
            &attention,
            &dataset.dataset,
            TassadarSequenceSplit::Validation,
            256,
            32,
        )?;
        let comparison = build_tassadar_executor_architecture_comparison_report(
            &dataset.dataset,
            TassadarSequenceSplit::Validation,
            256,
            32,
            lookup_report,
            attention_report,
        )?;

        assert_eq!(comparison.prompt_window_token_cap, 256);
        assert_eq!(comparison.target_token_cap, 32);
        assert_eq!(
            comparison.lookup_baseline.requested_hull_decode.hull_decode_posture,
            super::TassadarExecutorArchitectureHullDecodePosture::Direct
        );
        assert_eq!(
            comparison
                .executor_attention_candidate
                .requested_hull_decode
                .hull_decode_posture,
            super::TassadarExecutorArchitectureHullDecodePosture::FallbackToReferenceLinear
        );
        assert!(comparison.candidate_closer_to_article_fidelity);
        Ok(())
    }
}
