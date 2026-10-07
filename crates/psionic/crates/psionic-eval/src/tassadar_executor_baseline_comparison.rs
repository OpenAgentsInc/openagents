use std::time::Instant;

use psionic_data::{
    TassadarSequenceDatasetContract, TassadarSequenceDatasetError, TassadarSequenceSplit,
};
use psionic_models::{
    TassadarExecutorAttentionDecodeRefusal, TassadarExecutorAttentionError,
    TassadarExecutorAttentionTransformer, TassadarExecutorLongTraceContract,
    TassadarExecutorTransformer, TassadarExecutorTransformerDecodeRefusal,
    TassadarExecutorTransformerError, TokenId, TokenSequence, TokenizerBoundary,
};
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    tassadar_sudoku_v0_corpus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable family labels carried by the learned baseline comparison report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorBaselineFamilyKind {
    HullSpecializedLookup,
    SparseLookupBaseline,
    HybridAttentionBaseline,
    RecurrentWindowedBaseline,
}

impl TassadarExecutorBaselineFamilyKind {
    /// Returns the stable family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HullSpecializedLookup => "hull_specialized_lookup",
            Self::SparseLookupBaseline => "sparse_lookup_baseline",
            Self::HybridAttentionBaseline => "hybrid_attention_baseline",
            Self::RecurrentWindowedBaseline => "recurrent_windowed_baseline",
        }
    }
}

/// Honest training-state identity for one compared family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorBaselineTrainingState {
    SeededTrainableFamily,
    CheckpointInitialized,
}

/// Machine-readable decode selection summary for one family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBaselineDecodeSelectionReport {
    /// Requested decode mode for the family benchmark.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode used by the family when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Fallback mode used when the request could not execute directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Refusal reason when the family could not execute at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// Machine-readable fit summary for one compared family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBaselineFitSummary {
    /// Declared maximum sequence length for the family.
    pub model_max_sequence_tokens: u32,
    /// Maximum full-sequence token count across the shared split.
    pub full_sequence_token_count_max: u32,
    /// Number of shared cases that fit the family without truncation.
    pub full_sequence_fit_case_count: u32,
    /// Maximum bounded prompt-plus-target token count used in the comparison.
    pub bounded_window_total_token_count_max: u32,
    /// Whether the bounded comparison window fits the family for all shared cases.
    pub bounded_window_fits_model_context: bool,
    /// Explicit long-trace contract when the family declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_trace_contract: Option<TassadarExecutorLongTraceContract>,
}

/// Aggregate correctness summary for one compared family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBaselineCorrectnessSummary {
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

/// Aggregate speed summary for one compared family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBaselineSpeedSummary {
    /// Requested decode mode for this family.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Number of cases that executed directly under the requested mode.
    pub direct_case_count: u32,
    /// Number of cases that executed via fallback.
    pub fallback_case_count: u32,
    /// Number of cases that refused the requested mode.
    pub refusal_case_count: u32,
    /// Aggregate neural throughput in target tokens per second.
    pub neural_tokens_per_second: u32,
    /// Aggregate CPU reference throughput normalized by the same target count.
    pub cpu_tokens_per_second: u32,
}

/// Bounded per-case report for one compared family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBaselineCaseReport {
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable case identifier.
    pub case_id: String,
    /// Full target-token count for the source sequence.
    pub full_target_token_count: u32,
    /// Evaluated target-token count after caps.
    pub target_token_count: u32,
    /// Decode selection used for this case.
    pub decode_selection: TassadarExecutorBaselineDecodeSelectionReport,
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
    /// Reference token at the first divergence when one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_divergence_token: Option<String>,
    /// Predicted token at the first divergence when one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicted_divergence_token: Option<String>,
    /// Whether the bounded predicted suffix matched exactly.
    pub exact_trace_match: bool,
}

/// Machine-readable per-family report used by the learned baseline comparison root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBaselineFamilyReport {
    /// Family kind under test.
    pub family_kind: TassadarExecutorBaselineFamilyKind,
    /// Shared model identifier.
    pub model_id: String,
    /// Shared model-family label.
    pub model_family: String,
    /// Stable descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable weight digest.
    pub trained_weight_digest: String,
    /// Honest training-state identity for the family.
    pub training_state: TassadarExecutorBaselineTrainingState,
    /// Machine-readable trainable-surface summary.
    pub training_surface_summary: String,
    /// Explicit claim boundary for the family.
    pub claim_boundary: String,
    /// Explicit attention-mode label for this family.
    pub attention_mode: String,
    /// One-sentence architecture identity note.
    pub architecture_identity: String,
    /// Bounded fit summary.
    pub fit: TassadarExecutorBaselineFitSummary,
    /// Aggregate correctness metrics.
    pub correctness: TassadarExecutorBaselineCorrectnessSummary,
    /// Aggregate speed metrics.
    pub speed: TassadarExecutorBaselineSpeedSummary,
    /// Per-case bounded reports.
    pub case_reports: Vec<TassadarExecutorBaselineCaseReport>,
    /// Stable report digest.
    pub report_digest: String,
}

/// Top-level same-corpus learned baseline comparison report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBaselineComparisonReport {
    /// Dataset storage key used for the comparison.
    pub dataset_storage_key: String,
    /// Dataset digest used for the comparison.
    pub dataset_digest: String,
    /// Evaluated split.
    pub split: TassadarSequenceSplit,
    /// Prompt window cap used for all families.
    pub prompt_window_token_cap: u32,
    /// Target-token cap used for all families.
    pub target_token_cap: u32,
    /// Hull-specialized lookup baseline report.
    pub hull_specialized_lookup: TassadarExecutorBaselineFamilyReport,
    /// Sparse lookup baseline report.
    pub sparse_lookup_baseline: TassadarExecutorBaselineFamilyReport,
    /// Hybrid attention baseline report.
    pub hybrid_attention_baseline: TassadarExecutorBaselineFamilyReport,
    /// Recurrent/windowed lookup baseline report.
    pub recurrent_windowed_baseline: TassadarExecutorBaselineFamilyReport,
    /// Whether the sparse baseline matches hull exactness on the bounded window.
    pub sparse_matches_hull_exactness: bool,
    /// Whether the hybrid baseline matches or beats the hull baseline on bounded exactness.
    pub hybrid_matches_or_beats_hull_exactness: bool,
    /// Whether the recurrent baseline preserves hull exactness on the bounded window.
    pub recurrent_matches_hull_exactness: bool,
    /// Whether the recurrent baseline changes the long-trace contract explicitly.
    pub recurrent_changes_long_trace_contract: bool,
    /// Plain-language summary that keeps the claim boundary explicit.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarExecutorBaselineComparisonReport {
    fn new(
        dataset_storage_key: String,
        dataset_digest: String,
        split: TassadarSequenceSplit,
        prompt_window_token_cap: usize,
        target_token_cap: usize,
        hull_specialized_lookup: TassadarExecutorBaselineFamilyReport,
        sparse_lookup_baseline: TassadarExecutorBaselineFamilyReport,
        hybrid_attention_baseline: TassadarExecutorBaselineFamilyReport,
        recurrent_windowed_baseline: TassadarExecutorBaselineFamilyReport,
    ) -> Self {
        let sparse_matches_hull_exactness =
            correctness_rank(&sparse_lookup_baseline.correctness)
                >= correctness_rank(&hull_specialized_lookup.correctness);
        let hybrid_matches_or_beats_hull_exactness =
            correctness_rank(&hybrid_attention_baseline.correctness)
                >= correctness_rank(&hull_specialized_lookup.correctness);
        let recurrent_matches_hull_exactness =
            correctness_rank(&recurrent_windowed_baseline.correctness)
                >= correctness_rank(&hull_specialized_lookup.correctness);
        let recurrent_changes_long_trace_contract =
            recurrent_windowed_baseline.fit.long_trace_contract
                != hull_specialized_lookup.fit.long_trace_contract;
        let mut report = Self {
            dataset_storage_key,
            dataset_digest,
            split,
            prompt_window_token_cap: prompt_window_token_cap as u32,
            target_token_cap: target_token_cap as u32,
            hull_specialized_lookup,
            sparse_lookup_baseline,
            hybrid_attention_baseline,
            recurrent_windowed_baseline,
            sparse_matches_hull_exactness,
            hybrid_matches_or_beats_hull_exactness,
            recurrent_matches_hull_exactness,
            recurrent_changes_long_trace_contract,
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Same-corpus learned baseline comparison is now explicit on {} {}: hull_first_32_bps={}, sparse_first_32_bps={}, hybrid_first_32_bps={}, recurrent_first_32_bps={}, recurrent_contract_changed={}, all families remain bounded and comparison-only.",
            report.dataset_storage_key,
            format!("{:?}", report.split).to_lowercase(),
            report
                .hull_specialized_lookup
                .correctness
                .first_32_token_exactness_bps,
            report
                .sparse_lookup_baseline
                .correctness
                .first_32_token_exactness_bps,
            report
                .hybrid_attention_baseline
                .correctness
                .first_32_token_exactness_bps,
            report
                .recurrent_windowed_baseline
                .correctness
                .first_32_token_exactness_bps,
            report.recurrent_changes_long_trace_contract,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_executor_baseline_comparison_report|",
            &report,
        );
        report
    }
}

/// Failure while scoring the same-corpus learned baseline comparison.
#[derive(Debug, Error)]
pub enum TassadarExecutorBaselineComparisonError {
    /// Dataset validation failed.
    #[error(transparent)]
    Dataset(#[from] TassadarSequenceDatasetError),
    /// Lookup-family decode failed.
    #[error(transparent)]
    LookupModel(#[from] TassadarExecutorTransformerError),
    /// Hybrid attention decode failed.
    #[error(transparent)]
    AttentionModel(#[from] TassadarExecutorAttentionError),
    /// CPU reference runner refused one program.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// One dataset case no longer maps to a runtime corpus program.
    #[error("missing runtime Sudoku-v0 program for case `{case_id}`")]
    MissingRuntimeCase { case_id: String },
}

/// Evaluates the hull-specialized, sparse, hybrid, and recurrent learned
/// families on the same Sudoku-v0 corpus window.
#[allow(clippy::too_many_arguments)]
pub fn build_tassadar_executor_baseline_comparison_report(
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
    hull_specialized_lookup: &TassadarExecutorTransformer,
    sparse_lookup_baseline: &TassadarExecutorTransformer,
    hybrid_attention_baseline: &TassadarExecutorAttentionTransformer,
    recurrent_windowed_baseline: &TassadarExecutorTransformer,
) -> Result<TassadarExecutorBaselineComparisonReport, TassadarExecutorBaselineComparisonError> {
    dataset.validate()?;
    let hull_specialized_lookup = evaluate_lookup_family(
        hull_specialized_lookup,
        dataset,
        split,
        prompt_window_token_cap,
        target_token_cap,
        TassadarExecutorBaselineFamilyKind::HullSpecializedLookup,
        TassadarExecutorBaselineTrainingState::SeededTrainableFamily,
        TassadarExecutorDecodeMode::HullCache,
        String::from("output_head_embeddings_and_small_learned_mixer"),
        String::from(
            "fixed relative-offset lookup heads with direct hull-cache decode on the bounded Sudoku-v0 workload",
        ),
    )?;
    let sparse_lookup_baseline = evaluate_lookup_family(
        sparse_lookup_baseline,
        dataset,
        split,
        prompt_window_token_cap,
        target_token_cap,
        TassadarExecutorBaselineFamilyKind::SparseLookupBaseline,
        TassadarExecutorBaselineTrainingState::SeededTrainableFamily,
        TassadarExecutorDecodeMode::SparseTopK,
        String::from("output_head_embeddings_and_small_learned_mixer"),
        String::from(
            "fixed relative-offset lookup heads with an explicit direct sparse-top-k decode budget on the same bounded workload",
        ),
    )?;
    let hybrid_attention_baseline = evaluate_attention_family(
        hybrid_attention_baseline,
        dataset,
        split,
        prompt_window_token_cap,
        target_token_cap,
        TassadarExecutorBaselineTrainingState::SeededTrainableFamily,
    )?;
    let recurrent_windowed_baseline = evaluate_lookup_family(
        recurrent_windowed_baseline,
        dataset,
        split,
        prompt_window_token_cap,
        target_token_cap,
        TassadarExecutorBaselineFamilyKind::RecurrentWindowedBaseline,
        TassadarExecutorBaselineTrainingState::SeededTrainableFamily,
        TassadarExecutorDecodeMode::HullCache,
        String::from("output_head_embeddings_and_small_learned_mixer"),
        String::from(
            "same lookup-family executor with an explicit incremental window long-trace contract instead of a flat full-prefix claim",
        ),
    )?;
    Ok(TassadarExecutorBaselineComparisonReport::new(
        dataset.storage_key(),
        dataset.stable_digest(),
        split,
        prompt_window_token_cap,
        target_token_cap,
        hull_specialized_lookup,
        sparse_lookup_baseline,
        hybrid_attention_baseline,
        recurrent_windowed_baseline,
    ))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_lookup_family(
    model: &TassadarExecutorTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
    family_kind: TassadarExecutorBaselineFamilyKind,
    training_state: TassadarExecutorBaselineTrainingState,
    requested_decode_mode: TassadarExecutorDecodeMode,
    training_surface_summary: String,
    architecture_identity: String,
) -> Result<TassadarExecutorBaselineFamilyReport, TassadarExecutorBaselineComparisonError> {
    let runtime_programs = runtime_programs();
    let split_examples = dataset.split_examples(split);
    let mut case_reports = Vec::new();
    let mut total_target_tokens = 0_u64;
    let mut total_neural_elapsed_ms = 0_u64;
    let mut total_cpu_elapsed_ms = 0_u64;

    for example in &split_examples {
        let window = bounded_case_window(
            example.token_ids.as_slice(),
            example.metadata.prompt_token_count as usize,
            prompt_window_token_cap,
            target_token_cap,
        );
        let (predicted_target, decode_selection, elapsed_ms) = timed_lookup_decode(
            model,
            window.prompt.clone(),
            window.reference_target.len(),
            requested_decode_mode,
        )?;
        let cpu_elapsed_ms = timed_cpu_reference(
            runtime_programs.as_slice(),
            example.metadata.case_id.as_str(),
        )?;
        total_target_tokens =
            total_target_tokens.saturating_add(window.reference_target.len() as u64);
        total_neural_elapsed_ms = total_neural_elapsed_ms.saturating_add(elapsed_ms.max(1));
        total_cpu_elapsed_ms = total_cpu_elapsed_ms.saturating_add(cpu_elapsed_ms.max(1));
        case_reports.push(build_case_report(
            model.tokenizer(),
            example.sequence_id.clone(),
            example.metadata.case_id.clone(),
            example.metadata.target_token_count,
            &decode_selection,
            window.reference_target.as_slice(),
            predicted_target.as_slice(),
        ));
    }

    Ok(build_family_report(
        dataset,
        split,
        prompt_window_token_cap,
        target_token_cap,
        split_examples.as_slice(),
        family_kind,
        model.descriptor().model.model_id.clone(),
        model.descriptor().model.family.clone(),
        model.descriptor().stable_digest(),
        model.descriptor().weights.digest.clone(),
        training_state,
        training_surface_summary,
        serde_label(&model.descriptor().claim_boundary),
        serde_label(&model.descriptor().attention_mode),
        architecture_identity,
        model.descriptor().config.max_sequence_tokens as u32,
        Some(model.descriptor().long_trace_contract),
        requested_decode_mode,
        total_target_tokens,
        total_neural_elapsed_ms,
        total_cpu_elapsed_ms,
        case_reports,
    ))
}

fn evaluate_attention_family(
    model: &TassadarExecutorAttentionTransformer,
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
    training_state: TassadarExecutorBaselineTrainingState,
) -> Result<TassadarExecutorBaselineFamilyReport, TassadarExecutorBaselineComparisonError> {
    let runtime_programs = runtime_programs();
    let split_examples = dataset.split_examples(split);
    let mut case_reports = Vec::new();
    let mut total_target_tokens = 0_u64;
    let mut total_neural_elapsed_ms = 0_u64;
    let mut total_cpu_elapsed_ms = 0_u64;

    for example in &split_examples {
        let window = bounded_case_window(
            example.token_ids.as_slice(),
            example.metadata.prompt_token_count as usize,
            prompt_window_token_cap,
            target_token_cap,
        );
        let (predicted_target, decode_selection, elapsed_ms) = timed_attention_decode(
            model,
            window.prompt.clone(),
            window.reference_target.len(),
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;
        let cpu_elapsed_ms = timed_cpu_reference(
            runtime_programs.as_slice(),
            example.metadata.case_id.as_str(),
        )?;
        total_target_tokens =
            total_target_tokens.saturating_add(window.reference_target.len() as u64);
        total_neural_elapsed_ms = total_neural_elapsed_ms.saturating_add(elapsed_ms.max(1));
        total_cpu_elapsed_ms = total_cpu_elapsed_ms.saturating_add(cpu_elapsed_ms.max(1));
        case_reports.push(build_case_report(
            model.tokenizer(),
            example.sequence_id.clone(),
            example.metadata.case_id.clone(),
            example.metadata.target_token_count,
            &decode_selection,
            window.reference_target.as_slice(),
            predicted_target.as_slice(),
        ));
    }

    Ok(build_family_report(
        dataset,
        split,
        prompt_window_token_cap,
        target_token_cap,
        split_examples.as_slice(),
        TassadarExecutorBaselineFamilyKind::HybridAttentionBaseline,
        model.descriptor().model.model_id.clone(),
        model.descriptor().model.family.clone(),
        model.descriptor().stable_digest(),
        model.descriptor().weights.digest.clone(),
        training_state,
        String::from("bounded attention layers plus relative-target adapter surfaces"),
        serde_label(&model.descriptor().claim_boundary),
        serde_label(&model.descriptor().attention_mode),
        String::from(
            "layered full-prefix causal hard-max attention plus bounded target-conditioned adapters; still a bounded hybrid learned baseline rather than a promoted executor",
        ),
        model.descriptor().config.max_sequence_tokens as u32,
        None,
        TassadarExecutorDecodeMode::ReferenceLinear,
        total_target_tokens,
        total_neural_elapsed_ms,
        total_cpu_elapsed_ms,
        case_reports,
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
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<
    (
        Vec<TokenId>,
        TassadarExecutorBaselineDecodeSelectionReport,
        u64,
    ),
    TassadarExecutorBaselineComparisonError,
> {
    let selection = model.select_decode_mode(requested_decode_mode);
    let started = Instant::now();
    let mut state = model.start_decode(prompt)?;
    let mut predicted = Vec::with_capacity(target_token_count);
    if let Some(effective_decode_mode) = selection.effective_decode_mode {
        for _ in 0..target_token_count {
            let next = model.greedy_next_token_for_mode(&state, effective_decode_mode)?;
            model.push_decoded_token(&mut state, next)?;
            predicted.push(next);
            if next == model.tokenizer().vocabulary().eos_id() {
                break;
            }
        }
    }
    Ok((
        predicted,
        TassadarExecutorBaselineDecodeSelectionReport {
            requested_decode_mode,
            effective_decode_mode: selection.effective_decode_mode,
            fallback_decode_mode: selection.fallback_decode_mode,
            refusal: selection.refusal.map(|refusal| match refusal {
                TassadarExecutorTransformerDecodeRefusal::NoSupportedDecodeMode => {
                    String::from("no_supported_decode_mode")
                }
            }),
        },
        started.elapsed().as_millis() as u64,
    ))
}

fn timed_attention_decode(
    model: &TassadarExecutorAttentionTransformer,
    prompt: TokenSequence,
    target_token_count: usize,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<
    (
        Vec<TokenId>,
        TassadarExecutorBaselineDecodeSelectionReport,
        u64,
    ),
    TassadarExecutorBaselineComparisonError,
> {
    let selection = model.select_decode_mode(requested_decode_mode);
    let started = Instant::now();
    let mut state = model.start_decode(prompt)?;
    let mut predicted = Vec::with_capacity(target_token_count);
    if let Some(effective_decode_mode) = selection.effective_decode_mode {
        for _ in 0..target_token_count {
            let next = model.greedy_next_token_for_mode(&state, effective_decode_mode)?;
            model.push_decoded_token(&mut state, next)?;
            predicted.push(next);
            if next == model.tokenizer().vocabulary().eos_id() {
                break;
            }
        }
    }
    Ok((
        predicted,
        TassadarExecutorBaselineDecodeSelectionReport {
            requested_decode_mode,
            effective_decode_mode: selection.effective_decode_mode,
            fallback_decode_mode: selection.fallback_decode_mode,
            refusal: selection.refusal.map(|refusal| match refusal {
                TassadarExecutorAttentionDecodeRefusal::NoSupportedDecodeMode => {
                    String::from("no_supported_decode_mode")
                }
            }),
        },
        started.elapsed().as_millis() as u64,
    ))
}

fn timed_cpu_reference(
    runtime_programs: &[(String, psionic_runtime::TassadarProgram)],
    case_id: &str,
) -> Result<u64, TassadarExecutorBaselineComparisonError> {
    let Some((_, program)) = runtime_programs
        .iter()
        .find(|(candidate_case_id, _)| candidate_case_id == case_id)
    else {
        return Err(
            TassadarExecutorBaselineComparisonError::MissingRuntimeCase {
                case_id: case_id.to_string(),
            },
        );
    };
    let started = Instant::now();
    let _execution = TassadarCpuReferenceRunner::for_program(program)?.execute(program)?;
    Ok(started.elapsed().as_millis() as u64)
}

fn build_case_report(
    tokenizer: &impl TokenizerBoundary,
    sequence_id: String,
    case_id: String,
    full_target_token_count: u32,
    decode_selection: &TassadarExecutorBaselineDecodeSelectionReport,
    reference_target: &[TokenId],
    predicted_target: &[TokenId],
) -> TassadarExecutorBaselineCaseReport {
    let matched_target_token_count = matched_target_token_count(reference_target, predicted_target);
    let first_divergence_index = first_divergence_index(reference_target, predicted_target);
    let reference_divergence_token = first_divergence_index.and_then(|index| {
        reference_target
            .get(index as usize)
            .and_then(|token| tokenizer.vocabulary().token(*token))
            .map(str::to_string)
    });
    let predicted_divergence_token = first_divergence_index.and_then(|index| {
        predicted_target
            .get(index as usize)
            .and_then(|token| tokenizer.vocabulary().token(*token))
            .map(str::to_string)
    });
    TassadarExecutorBaselineCaseReport {
        sequence_id,
        case_id,
        full_target_token_count,
        target_token_count: reference_target.len() as u32,
        decode_selection: decode_selection.clone(),
        target_token_exactness_bps: suffix_exactness_bps(reference_target, predicted_target),
        first_target_exactness_bps: prefix_exactness_bps(reference_target, predicted_target, 1),
        first_8_token_exactness_bps: prefix_exactness_bps(reference_target, predicted_target, 8),
        first_32_token_exactness_bps: prefix_exactness_bps(reference_target, predicted_target, 32),
        matched_target_token_count,
        first_divergence_index,
        reference_divergence_token,
        predicted_divergence_token,
        exact_trace_match: predicted_target == reference_target,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_family_report(
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
    prompt_window_token_cap: usize,
    target_token_cap: usize,
    split_examples: &[&psionic_data::TassadarSequenceExample],
    family_kind: TassadarExecutorBaselineFamilyKind,
    model_id: String,
    model_family: String,
    model_descriptor_digest: String,
    trained_weight_digest: String,
    training_state: TassadarExecutorBaselineTrainingState,
    training_surface_summary: String,
    claim_boundary: String,
    attention_mode: String,
    architecture_identity: String,
    model_max_sequence_tokens: u32,
    long_trace_contract: Option<TassadarExecutorLongTraceContract>,
    requested_decode_mode: TassadarExecutorDecodeMode,
    total_target_tokens: u64,
    total_neural_elapsed_ms: u64,
    total_cpu_elapsed_ms: u64,
    case_reports: Vec<TassadarExecutorBaselineCaseReport>,
) -> TassadarExecutorBaselineFamilyReport {
    let full_sequence_token_count_max = split_examples
        .iter()
        .map(|example| example.metadata.total_token_count)
        .max()
        .unwrap_or(0);
    let full_sequence_fit_case_count = split_examples
        .iter()
        .filter(|example| example.metadata.total_token_count <= model_max_sequence_tokens)
        .count() as u32;
    let bounded_window_total_token_count_max = split_examples
        .iter()
        .map(|example| {
            let prompt_tokens = usize::min(
                example.metadata.prompt_token_count as usize,
                prompt_window_token_cap,
            ) as u32;
            let target_tokens =
                usize::min(example.metadata.target_token_count as usize, target_token_cap) as u32;
            prompt_tokens.saturating_add(target_tokens)
        })
        .max()
        .unwrap_or(0);
    let fit = TassadarExecutorBaselineFitSummary {
        model_max_sequence_tokens,
        full_sequence_token_count_max,
        full_sequence_fit_case_count,
        bounded_window_total_token_count_max,
        bounded_window_fits_model_context: bounded_window_total_token_count_max
            <= model_max_sequence_tokens,
        long_trace_contract,
    };
    let correctness = TassadarExecutorBaselineCorrectnessSummary {
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
    let speed = TassadarExecutorBaselineSpeedSummary {
        requested_decode_mode,
        direct_case_count: case_reports
            .iter()
            .filter(|case| {
                case.decode_selection.effective_decode_mode == Some(requested_decode_mode)
                    && case.decode_selection.fallback_decode_mode.is_none()
            })
            .count() as u32,
        fallback_case_count: case_reports
            .iter()
            .filter(|case| case.decode_selection.fallback_decode_mode.is_some())
            .count() as u32,
        refusal_case_count: case_reports
            .iter()
            .filter(|case| case.decode_selection.effective_decode_mode.is_none())
            .count() as u32,
        neural_tokens_per_second: tokens_per_second(
            total_target_tokens as u32,
            total_neural_elapsed_ms,
        ),
        cpu_tokens_per_second: tokens_per_second(total_target_tokens as u32, total_cpu_elapsed_ms),
    };
    let mut report = TassadarExecutorBaselineFamilyReport {
        family_kind,
        model_id,
        model_family,
        model_descriptor_digest,
        trained_weight_digest,
        training_state,
        training_surface_summary,
        claim_boundary,
        attention_mode,
        architecture_identity,
        fit,
        correctness,
        speed,
        case_reports,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_executor_baseline_family_report|",
        &(
            dataset.storage_key(),
            dataset.stable_digest(),
            split,
            &report,
        ),
    );
    report
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

fn first_divergence_index(
    reference_target: &[TokenId],
    predicted_target: &[TokenId],
) -> Option<u32> {
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

fn prefix_exactness_bps(
    reference_target: &[TokenId],
    predicted_target: &[TokenId],
    prefix_len: usize,
) -> u32 {
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

fn correctness_rank(
    summary: &TassadarExecutorBaselineCorrectnessSummary,
) -> (u32, u32, u32, u32, u32) {
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
    let encoded = serde_json::to_vec(value)
        .expect("Tassadar learned baseline comparison value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn serde_label<T>(value: &T) -> String
where
    T: Serialize,
{
    serde_json::to_string(value)
        .expect("Tassadar baseline label should serialize")
        .trim_matches('"')
        .to_string()
}

#[cfg(test)]
mod tests {
    use psionic_data::TassadarSequenceSplit;
    use psionic_models::{
        TassadarExecutorAttentionTransformer, TassadarExecutorTrainableSurface,
        TassadarExecutorTransformer,
    };
    use psionic_runtime::TassadarExecutorDecodeMode;

    use crate::{
        build_tassadar_executor_baseline_comparison_report,
        build_tassadar_sudoku_v0_sequence_dataset,
    };

    #[test]
    fn learned_baseline_comparison_surfaces_all_four_family_reports()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset = build_tassadar_sudoku_v0_sequence_dataset("train-v0")?;
        let report = build_tassadar_executor_baseline_comparison_report(
            &dataset.dataset,
            TassadarSequenceSplit::Validation,
            256,
            32,
            &TassadarExecutorTransformer::sudoku_v0_with_surface(
                TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
            ),
            &TassadarExecutorTransformer::sudoku_v0_sparse_with_surface(
                TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
            ),
            &TassadarExecutorAttentionTransformer::sudoku_v0(),
            &TassadarExecutorTransformer::sudoku_v0_windowed_with_surface(
                TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
            ),
        )?;

        assert_eq!(
            report.hull_specialized_lookup.speed.requested_decode_mode,
            TassadarExecutorDecodeMode::HullCache
        );
        assert_eq!(
            report.sparse_lookup_baseline.speed.requested_decode_mode,
            TassadarExecutorDecodeMode::SparseTopK
        );
        assert_eq!(
            report.hybrid_attention_baseline.speed.requested_decode_mode,
            TassadarExecutorDecodeMode::ReferenceLinear
        );
        assert!(report.recurrent_changes_long_trace_contract);
        assert_eq!(
            report
                .sparse_lookup_baseline
                .speed
                .direct_case_count,
            report.sparse_lookup_baseline.case_reports.len() as u32
        );
        Ok(())
    }
}
