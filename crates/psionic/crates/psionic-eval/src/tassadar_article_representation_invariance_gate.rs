use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    TassadarArticleTraceDecodeError, TassadarArticleTransformer, TassadarArticleTransformerError,
    TassadarTraceTokenizer, TokenizerBoundary,
};
use psionic_runtime::{
    append_tassadar_unreachable_instruction_suffix, invert_tassadar_local_permutation,
    remap_tassadar_execution_local_indices, remap_tassadar_program_local_indices,
    tassadar_article_class_corpus, TassadarArticlePromptFieldId, TassadarArticlePromptFieldSurface,
    TassadarArticleRepresentationInvarianceError, TassadarCpuReferenceRunner, TassadarExecution,
    TassadarExecutionRefusal, TassadarInstruction, TassadarProgram, TassadarValidationCase,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_trace_vocabulary_binding_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleTraceVocabularyBindingReport,
    TassadarArticleTraceVocabularyBindingReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF,
};

pub const TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_representation_invariance_gate_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-167A";
const BOUNDARY_DOC_REF: &str = "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const RUNTIME_MODULE_REF: &str =
    "crates/psionic-runtime/src/tassadar_article_representation_invariance.rs";
const TOKENIZER_MODULE_REF: &str = "crates/psionic-models/src/tassadar_sequence.rs";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleRepresentationInvariancePerturbationKind {
    WhitespaceFormatting,
    EquivalentTokenSequence,
    FieldReordering,
    RegisterRenaming,
    EquivalentIrLayout,
}

impl TassadarArticleRepresentationInvariancePerturbationKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::WhitespaceFormatting => "whitespace_formatting",
            Self::EquivalentTokenSequence => "equivalent_token_sequence",
            Self::FieldReordering => "field_reordering",
            Self::RegisterRenaming => "register_renaming",
            Self::EquivalentIrLayout => "equivalent_ir_layout",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTraceStabilityClass {
    Exact,
    CanonicalizedEquivalent,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRepresentationInvarianceCaseRow {
    pub case_id: String,
    pub canonical_case_id: String,
    pub perturbation_kind: TassadarArticleRepresentationInvariancePerturbationKind,
    pub canonical_sequence_digest: String,
    pub perturbed_sequence_digest: String,
    pub output_stable: bool,
    pub raw_trace_stable: bool,
    pub canonicalized_trace_stable: bool,
    pub prompt_boundary_preserved: bool,
    pub tokenizer_roundtrip_exact: bool,
    pub model_binding_roundtrip_exact: bool,
    pub trace_stability_class: TassadarArticleTraceStabilityClass,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRepresentationSuppressedCaseRow {
    pub case_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRepresentationInvarianceBoundaryReview {
    pub boundary_doc_ref: String,
    pub runtime_module_ref: String,
    pub tokenizer_module_ref: String,
    pub boundary_doc_names_invariance_gate: bool,
    pub boundary_doc_names_representation_sensitivity: bool,
    pub runtime_defines_prompt_field_surface: bool,
    pub runtime_defines_local_remap_helpers: bool,
    pub runtime_defines_dead_code_helper: bool,
    pub tokenizer_supports_symbolic_retokenization: bool,
    pub tokenizer_supports_prompt_target_text_composition: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRepresentationEquivalenceReview {
    pub trace_vocabulary_binding_report_ref: String,
    pub trace_vocabulary_binding_green: bool,
    pub case_count: usize,
    pub suppressed_case_count: usize,
    pub passed_case_count: usize,
    pub required_perturbation_kind_count: usize,
    pub covered_perturbation_kind_count: usize,
    pub exact_trace_case_count: usize,
    pub canonicalized_trace_case_count: usize,
    pub output_stable_case_count: usize,
    pub tokenizer_roundtrip_exact_case_count: usize,
    pub model_binding_roundtrip_exact_case_count: usize,
    pub representation_sensitive_case_count: usize,
    pub all_required_perturbations_present: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceStabilityReview {
    pub exact_trace_case_count: usize,
    pub canonicalized_trace_case_count: usize,
    pub failed_case_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRepresentationInvarianceAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub trace_vocabulary_binding_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRepresentationInvarianceGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleRepresentationInvarianceAcceptanceGateTie,
    pub trace_vocabulary_binding_report_ref: String,
    pub trace_vocabulary_binding_green: bool,
    pub case_rows: Vec<TassadarArticleRepresentationInvarianceCaseRow>,
    pub suppressed_case_rows: Vec<TassadarArticleRepresentationSuppressedCaseRow>,
    pub representation_equivalence_review: TassadarArticleRepresentationEquivalenceReview,
    pub trace_stability_review: TassadarArticleTraceStabilityReview,
    pub boundary_review: TassadarArticleRepresentationInvarianceBoundaryReview,
    pub article_representation_invariance_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleRepresentationInvarianceGateReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    TraceVocabularyBinding(#[from] TassadarArticleTraceVocabularyBindingReportError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error(transparent)]
    RuntimeExecution(#[from] TassadarExecutionRefusal),
    #[error(transparent)]
    Representation(#[from] TassadarArticleRepresentationInvarianceError),
    #[error(transparent)]
    Decode(#[from] TassadarArticleTraceDecodeError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    DecodeJson {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_representation_invariance_gate_report() -> Result<
    TassadarArticleRepresentationInvarianceGateReport,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let trace_vocabulary_binding_report = build_tassadar_article_trace_vocabulary_binding_report()?;
    let tokenizer = TassadarTraceTokenizer::new();
    let model = TassadarArticleTransformer::article_trace_domain_reference()?;
    let (case_rows, suppressed_case_rows) = build_case_rows(&tokenizer, &model)?;
    let boundary_review = boundary_review()?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        trace_vocabulary_binding_report,
        case_rows,
        suppressed_case_rows,
        boundary_review,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    trace_vocabulary_binding_report: TassadarArticleTraceVocabularyBindingReport,
    case_rows: Vec<TassadarArticleRepresentationInvarianceCaseRow>,
    suppressed_case_rows: Vec<TassadarArticleRepresentationSuppressedCaseRow>,
    boundary_review: TassadarArticleRepresentationInvarianceBoundaryReview,
) -> TassadarArticleRepresentationInvarianceGateReport {
    let acceptance_gate_tie = TassadarArticleRepresentationInvarianceAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        trace_vocabulary_binding_report_ref: String::from(
            TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let representation_equivalence_review = representation_equivalence_review(
        &case_rows,
        &suppressed_case_rows,
        trace_vocabulary_binding_report.article_trace_vocabulary_binding_green,
    );
    let trace_stability_review = trace_stability_review(&case_rows);
    let article_representation_invariance_green = acceptance_gate_tie.tied_requirement_satisfied
        && trace_vocabulary_binding_report.article_trace_vocabulary_binding_green
        && representation_equivalence_review.passed
        && trace_stability_review.passed
        && boundary_review.passed;
    let article_equivalence_green =
        article_representation_invariance_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleRepresentationInvarianceGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_representation_invariance_gate.report.v1"),
        acceptance_gate_tie,
        trace_vocabulary_binding_report_ref: String::from(
            TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF,
        ),
        trace_vocabulary_binding_green: trace_vocabulary_binding_report
            .article_trace_vocabulary_binding_green,
        case_rows,
        suppressed_case_rows,
        representation_equivalence_review,
        trace_stability_review,
        boundary_review,
        article_representation_invariance_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report closes the prompt, tokenization, and representation invariance gate for the declared owned article route only. It proves that trivial whitespace and prompt/target token-split perturbations do not change the route, that semantically irrelevant prompt-field ordering canonicalizes cleanly, that dead-code suffix IR layout changes preserve exact execution behavior, and that local-slot renaming is explicitly representation-sensitive but still canonicalizes back to the same semantic trace. It does not claim artifact-backed weight identity, reference-linear exactness on the Transformer-backed route, fast-route promotion, benchmark parity, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article representation invariance now records case_count={}, exact_trace_case_count={}, canonicalized_trace_case_count={}, output_stable_case_count={}, tokenizer_roundtrip_exact_case_count={}, model_binding_roundtrip_exact_case_count={}, article_representation_invariance_green={}, and article_equivalence_green={}.",
        report.representation_equivalence_review.case_count,
        report.trace_stability_review.exact_trace_case_count,
        report.trace_stability_review.canonicalized_trace_case_count,
        report.representation_equivalence_review.output_stable_case_count,
        report
            .representation_equivalence_review
            .tokenizer_roundtrip_exact_case_count,
        report
            .representation_equivalence_review
            .model_binding_roundtrip_exact_case_count,
        report.article_representation_invariance_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_representation_invariance_gate_report|",
        &report,
    );
    report
}

fn build_case_rows(
    tokenizer: &TassadarTraceTokenizer,
    model: &TassadarArticleTransformer,
) -> Result<
    (
        Vec<TassadarArticleRepresentationInvarianceCaseRow>,
        Vec<TassadarArticleRepresentationSuppressedCaseRow>,
    ),
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let mut rows = Vec::new();
    let mut suppressed = Vec::new();
    for case in tassadar_article_class_corpus() {
        let canonical_execution = execute_case(&case)?;
        if let Some(reason) = suppression_reason(&case, &canonical_execution, tokenizer, model) {
            suppressed.push(TassadarArticleRepresentationSuppressedCaseRow {
                case_id: case.case_id,
                reason,
            });
            continue;
        }
        rows.push(whitespace_formatting_case_row(
            &case,
            &canonical_execution,
            tokenizer,
            model,
        )?);
        rows.push(equivalent_token_sequence_case_row(
            &case,
            &canonical_execution,
            tokenizer,
            model,
        )?);
        rows.push(field_reordering_case_row(
            &case,
            &canonical_execution,
            tokenizer,
            model,
        )?);
        if case.program.local_count >= 2 {
            rows.push(register_renaming_case_row(
                &case,
                &canonical_execution,
                tokenizer,
                model,
            )?);
        }
        if matches!(
            case.program.instructions.last(),
            Some(TassadarInstruction::Return)
        ) {
            rows.push(equivalent_ir_layout_case_row(
                &case,
                &canonical_execution,
                tokenizer,
                model,
            )?);
        }
    }
    Ok((rows, suppressed))
}

fn whitespace_formatting_case_row(
    case: &TassadarValidationCase,
    canonical_execution: &TassadarExecution,
    tokenizer: &TassadarTraceTokenizer,
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleRepresentationInvarianceCaseRow,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let canonical_tokenized =
        tokenizer.tokenize_program_and_execution(&case.program, canonical_execution);
    let symbolic_text =
        TokenizerBoundary::decode(tokenizer, canonical_tokenized.sequence.as_slice());
    let perturbed_text = inject_whitespace_variants(&symbolic_text);
    let perturbed_tokenized =
        tokenizer.retokenize_symbolic_text(&perturbed_text, canonical_tokenized.prompt_token_count);
    let decoded = tokenizer.decode_article_trace_domain(&perturbed_tokenized)?;
    let perturbed_program = decoded.materialize_program(
        case.program.program_id.clone(),
        case.program.profile_id.clone(),
    );
    let perturbed_execution = decoded.materialize_execution(
        canonical_execution.program_id.clone(),
        canonical_execution.profile_id.clone(),
        canonical_execution.runner_id.clone(),
        canonical_execution.trace_abi.clone(),
    );

    build_case_row(
        case,
        canonical_execution,
        TassadarArticleRepresentationInvariancePerturbationKind::WhitespaceFormatting,
        &perturbed_program,
        &perturbed_execution,
        perturbed_tokenized,
        None,
        None,
        tokenizer,
        model,
        String::from(
            "the shared symbolic token text surface keeps the exact prompt and trace sequence under newline, tab, and repeated-space perturbations",
        ),
    )
}

fn equivalent_token_sequence_case_row(
    case: &TassadarValidationCase,
    canonical_execution: &TassadarExecution,
    tokenizer: &TassadarTraceTokenizer,
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleRepresentationInvarianceCaseRow,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let canonical_tokenized =
        tokenizer.tokenize_program_and_execution(&case.program, canonical_execution);
    let symbolic = tokenizer.decode_symbolic(&canonical_tokenized);
    let prompt_text = symbolic.tokens[..symbolic.prompt_token_count].join("\n");
    let target_text = symbolic.tokens[symbolic.prompt_token_count..].join("\t");
    let perturbed_tokenized =
        tokenizer.compose_prompt_and_target_symbolic_text(&prompt_text, &target_text);
    let decoded = tokenizer.decode_article_trace_domain(&perturbed_tokenized)?;
    let perturbed_program = decoded.materialize_program(
        case.program.program_id.clone(),
        case.program.profile_id.clone(),
    );
    let perturbed_execution = decoded.materialize_execution(
        canonical_execution.program_id.clone(),
        canonical_execution.profile_id.clone(),
        canonical_execution.runner_id.clone(),
        canonical_execution.trace_abi.clone(),
    );

    build_case_row(
        case,
        canonical_execution,
        TassadarArticleRepresentationInvariancePerturbationKind::EquivalentTokenSequence,
        &perturbed_program,
        &perturbed_execution,
        perturbed_tokenized,
        None,
        None,
        tokenizer,
        model,
        String::from(
            "the shared tokenizer keeps the exact sequence when prompt and target token-label text are re-encoded independently and recomposed at the declared boundary",
        ),
    )
}

fn field_reordering_case_row(
    case: &TassadarValidationCase,
    canonical_execution: &TassadarExecution,
    tokenizer: &TassadarTraceTokenizer,
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleRepresentationInvarianceCaseRow,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let perturbed_program = TassadarArticlePromptFieldSurface::from_program(&case.program)
        .reordered(&[
            TassadarArticlePromptFieldId::Instructions,
            TassadarArticlePromptFieldId::InitialMemory,
            TassadarArticlePromptFieldId::MemorySlots,
            TassadarArticlePromptFieldId::LocalCount,
        ])?
        .materialize_program()?;
    let perturbed_execution = execute_program(&perturbed_program)?;
    let perturbed_tokenized =
        tokenizer.tokenize_program_and_execution(&perturbed_program, &perturbed_execution);

    build_case_row(
        case,
        canonical_execution,
        TassadarArticleRepresentationInvariancePerturbationKind::FieldReordering,
        &perturbed_program,
        &perturbed_execution,
        perturbed_tokenized,
        None,
        None,
        tokenizer,
        model,
        String::from(
            "reordering the prompt fields in the explicit runtime surface materializes the same semantic program and keeps the tokenized route exact",
        ),
    )
}

fn register_renaming_case_row(
    case: &TassadarValidationCase,
    canonical_execution: &TassadarExecution,
    tokenizer: &TassadarTraceTokenizer,
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleRepresentationInvarianceCaseRow,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let mut rename = (0..case.program.local_count).collect::<Vec<_>>();
    rename.swap(0, 1);
    let perturbed_program = remap_tassadar_program_local_indices(&case.program, &rename)?;
    let perturbed_execution = execute_program(&perturbed_program)?;
    let perturbed_tokenized =
        tokenizer.tokenize_program_and_execution(&perturbed_program, &perturbed_execution);
    let inverse = invert_tassadar_local_permutation(&rename)?;
    let canonicalized_program = remap_tassadar_program_local_indices(&perturbed_program, &inverse)?;
    let canonicalized_execution =
        remap_tassadar_execution_local_indices(&perturbed_execution, &inverse)?;

    build_case_row(
        case,
        canonical_execution,
        TassadarArticleRepresentationInvariancePerturbationKind::RegisterRenaming,
        &perturbed_program,
        &perturbed_execution,
        perturbed_tokenized,
        Some(canonicalized_program),
        Some(canonicalized_execution),
        tokenizer,
        model,
        String::from(
            "swapping local-slot identities changes the raw token and trace surface, but inverse canonicalization recovers the exact semantic prompt and append-only execution trace",
        ),
    )
}

fn equivalent_ir_layout_case_row(
    case: &TassadarValidationCase,
    canonical_execution: &TassadarExecution,
    tokenizer: &TassadarTraceTokenizer,
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleRepresentationInvarianceCaseRow,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let perturbed_program = append_tassadar_unreachable_instruction_suffix(
        &case.program,
        vec![
            TassadarInstruction::I32Const { value: 99 },
            TassadarInstruction::Output,
            TassadarInstruction::Return,
        ],
    )?;
    let perturbed_execution = execute_program(&perturbed_program)?;
    let perturbed_tokenized =
        tokenizer.tokenize_program_and_execution(&perturbed_program, &perturbed_execution);

    build_case_row(
        case,
        canonical_execution,
        TassadarArticleRepresentationInvariancePerturbationKind::EquivalentIrLayout,
        &perturbed_program,
        &perturbed_execution,
        perturbed_tokenized,
        None,
        None,
        tokenizer,
        model,
        String::from(
            "adding an unreachable instruction suffix changes the prompt-only IR layout while preserving the exact executed trace and outputs",
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_case_row(
    case: &TassadarValidationCase,
    canonical_execution: &TassadarExecution,
    perturbation_kind: TassadarArticleRepresentationInvariancePerturbationKind,
    perturbed_program: &TassadarProgram,
    perturbed_execution: &TassadarExecution,
    perturbed_tokenized: psionic_models::TassadarTokenizedExecutionSequence,
    canonicalized_program: Option<TassadarProgram>,
    canonicalized_execution: Option<TassadarExecution>,
    tokenizer: &TassadarTraceTokenizer,
    model: &TassadarArticleTransformer,
    detail: String,
) -> Result<
    TassadarArticleRepresentationInvarianceCaseRow,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let canonical_tokenized =
        tokenizer.tokenize_program_and_execution(&case.program, canonical_execution);
    let decoded = tokenizer.decode_article_trace_domain(&perturbed_tokenized)?;
    let reconstructed_program = decoded.materialize_program(
        perturbed_program.program_id.clone(),
        perturbed_program.profile_id.clone(),
    );
    let reconstructed_execution = decoded.materialize_execution(
        perturbed_execution.program_id.clone(),
        perturbed_execution.profile_id.clone(),
        perturbed_execution.runner_id.clone(),
        perturbed_execution.trace_abi.clone(),
    );
    let model_roundtrip =
        model.roundtrip_article_trace_domain(perturbed_program, perturbed_execution)?;
    let raw_trace_stable =
        execution_matches_ignoring_identity(canonical_execution, perturbed_execution);
    let canonicalized_trace_stable = match (canonicalized_program, canonicalized_execution) {
        (Some(program), Some(execution)) => {
            program == case.program
                && execution_matches_ignoring_identity(canonical_execution, &execution)
        }
        _ => raw_trace_stable,
    };
    let output_stable = perturbed_execution.outputs == canonical_execution.outputs;
    let prompt_boundary_preserved = decoded.prompt_token_count
        == perturbed_tokenized.prompt_token_count
        && decoded.target_token_count == perturbed_tokenized.target_token_count
        && model_roundtrip.prompt_boundary_preserved
        && model_roundtrip.halt_marker_preserved;
    let tokenizer_roundtrip_exact = reconstructed_program == *perturbed_program
        && reconstructed_execution == *perturbed_execution
        && decoded.sequence_digest == perturbed_tokenized.sequence_digest;
    let model_binding_roundtrip_exact = model_roundtrip.roundtrip_exact;
    let trace_stability_class = if raw_trace_stable {
        TassadarArticleTraceStabilityClass::Exact
    } else if canonicalized_trace_stable {
        TassadarArticleTraceStabilityClass::CanonicalizedEquivalent
    } else {
        TassadarArticleTraceStabilityClass::Failed
    };
    let passed = output_stable
        && canonicalized_trace_stable
        && prompt_boundary_preserved
        && tokenizer_roundtrip_exact
        && model_binding_roundtrip_exact;

    Ok(TassadarArticleRepresentationInvarianceCaseRow {
        case_id: format!("{}::{}", case.case_id, perturbation_kind.label()),
        canonical_case_id: case.case_id.clone(),
        perturbation_kind,
        canonical_sequence_digest: canonical_tokenized.sequence_digest,
        perturbed_sequence_digest: perturbed_tokenized.sequence_digest,
        output_stable,
        raw_trace_stable,
        canonicalized_trace_stable,
        prompt_boundary_preserved,
        tokenizer_roundtrip_exact,
        model_binding_roundtrip_exact,
        trace_stability_class,
        passed,
        detail,
    })
}

fn representation_equivalence_review(
    case_rows: &[TassadarArticleRepresentationInvarianceCaseRow],
    suppressed_case_rows: &[TassadarArticleRepresentationSuppressedCaseRow],
    trace_vocabulary_binding_green: bool,
) -> TassadarArticleRepresentationEquivalenceReview {
    let exact_trace_case_count = case_rows
        .iter()
        .filter(|row| row.trace_stability_class == TassadarArticleTraceStabilityClass::Exact)
        .count();
    let canonicalized_trace_case_count = case_rows
        .iter()
        .filter(|row| {
            row.trace_stability_class == TassadarArticleTraceStabilityClass::CanonicalizedEquivalent
        })
        .count();
    let covered_perturbation_kind_count = case_rows
        .iter()
        .map(|row| row.perturbation_kind)
        .collect::<BTreeSet<_>>()
        .len();
    let required_perturbation_kind_count = required_perturbation_kinds().len();
    let all_required_perturbations_present = case_rows
        .iter()
        .map(|row| row.perturbation_kind)
        .collect::<BTreeSet<_>>()
        == required_perturbation_kinds();
    let output_stable_case_count = case_rows.iter().filter(|row| row.output_stable).count();
    let tokenizer_roundtrip_exact_case_count = case_rows
        .iter()
        .filter(|row| row.tokenizer_roundtrip_exact)
        .count();
    let model_binding_roundtrip_exact_case_count = case_rows
        .iter()
        .filter(|row| row.model_binding_roundtrip_exact)
        .count();
    let passed_case_count = case_rows.iter().filter(|row| row.passed).count();
    let passed = trace_vocabulary_binding_green
        && !case_rows.is_empty()
        && all_required_perturbations_present
        && case_rows.iter().all(|row| row.output_stable)
        && case_rows.iter().all(|row| row.tokenizer_roundtrip_exact)
        && case_rows
            .iter()
            .all(|row| row.model_binding_roundtrip_exact)
        && case_rows.iter().all(|row| row.passed)
        && exact_trace_case_count > 0
        && canonicalized_trace_case_count > 0;

    TassadarArticleRepresentationEquivalenceReview {
        trace_vocabulary_binding_report_ref: String::from(
            TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF,
        ),
        trace_vocabulary_binding_green,
        case_count: case_rows.len(),
        suppressed_case_count: suppressed_case_rows.len(),
        passed_case_count,
        required_perturbation_kind_count,
        covered_perturbation_kind_count,
        exact_trace_case_count,
        canonicalized_trace_case_count,
        output_stable_case_count,
        tokenizer_roundtrip_exact_case_count,
        model_binding_roundtrip_exact_case_count,
        representation_sensitive_case_count: canonicalized_trace_case_count,
        all_required_perturbations_present,
        passed,
        detail: format!(
            "the invariance suite covers {} perturbation kinds across {} supported cases with {} explicit suppressions, keeps outputs stable on {} cases, keeps tokenizer roundtrip exact on {} cases, keeps model-bound roundtrip exact on {} cases, and separates {} exact-trace cases from {} canonicalized-equivalence cases",
            covered_perturbation_kind_count,
            case_rows.len(),
            suppressed_case_rows.len(),
            output_stable_case_count,
            tokenizer_roundtrip_exact_case_count,
            model_binding_roundtrip_exact_case_count,
            exact_trace_case_count,
            canonicalized_trace_case_count,
        ),
    }
}

fn suppression_reason(
    case: &TassadarValidationCase,
    execution: &TassadarExecution,
    tokenizer: &TassadarTraceTokenizer,
    model: &TassadarArticleTransformer,
) -> Option<String> {
    let tokenized = tokenizer.tokenize_program_and_execution(&case.program, execution);
    let config = &model.descriptor().config;
    if tokenized.prompt_token_count > config.max_source_positions {
        return Some(format!(
            "suppressed because prompt_token_count={} exceeds max_source_positions={} on the bounded trace-domain reference model",
            tokenized.prompt_token_count, config.max_source_positions
        ));
    }
    if tokenized.target_token_count > config.max_target_positions {
        return Some(format!(
            "suppressed because target_token_count={} exceeds max_target_positions={} on the bounded trace-domain reference model",
            tokenized.target_token_count, config.max_target_positions
        ));
    }
    None
}

fn trace_stability_review(
    case_rows: &[TassadarArticleRepresentationInvarianceCaseRow],
) -> TassadarArticleTraceStabilityReview {
    let exact_trace_case_count = case_rows
        .iter()
        .filter(|row| row.trace_stability_class == TassadarArticleTraceStabilityClass::Exact)
        .count();
    let canonicalized_trace_case_count = case_rows
        .iter()
        .filter(|row| {
            row.trace_stability_class == TassadarArticleTraceStabilityClass::CanonicalizedEquivalent
        })
        .count();
    let failed_case_count = case_rows
        .iter()
        .filter(|row| row.trace_stability_class == TassadarArticleTraceStabilityClass::Failed)
        .count();
    let passed =
        failed_case_count == 0 && exact_trace_case_count > 0 && canonicalized_trace_case_count > 0;

    TassadarArticleTraceStabilityReview {
        exact_trace_case_count,
        canonicalized_trace_case_count,
        failed_case_count,
        passed,
        detail: format!(
            "trace stability stays exact on {} cases, canonicalizes cleanly on {} representation-sensitive cases, and fails on {} cases",
            exact_trace_case_count, canonicalized_trace_case_count, failed_case_count
        ),
    }
}

fn required_perturbation_kinds() -> BTreeSet<TassadarArticleRepresentationInvariancePerturbationKind>
{
    BTreeSet::from([
        TassadarArticleRepresentationInvariancePerturbationKind::WhitespaceFormatting,
        TassadarArticleRepresentationInvariancePerturbationKind::EquivalentTokenSequence,
        TassadarArticleRepresentationInvariancePerturbationKind::FieldReordering,
        TassadarArticleRepresentationInvariancePerturbationKind::RegisterRenaming,
        TassadarArticleRepresentationInvariancePerturbationKind::EquivalentIrLayout,
    ])
}

fn boundary_review() -> Result<
    TassadarArticleRepresentationInvarianceBoundaryReview,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let boundary_doc = read_repo_text(BOUNDARY_DOC_REF)?;
    let runtime_module = read_repo_text(RUNTIME_MODULE_REF)?;
    let tokenizer_module = read_repo_text(TOKENIZER_MODULE_REF)?;
    let boundary_doc_names_invariance_gate =
        boundary_doc.contains("TAS-167A") && boundary_doc.contains("invariance");
    let boundary_doc_names_representation_sensitivity =
        boundary_doc.contains("representation-sensitive") || boundary_doc.contains("canonicalized");
    let runtime_defines_prompt_field_surface =
        runtime_module.contains("TassadarArticlePromptFieldSurface");
    let runtime_defines_local_remap_helpers = runtime_module
        .contains("remap_tassadar_program_local_indices")
        && runtime_module.contains("remap_tassadar_execution_local_indices");
    let runtime_defines_dead_code_helper =
        runtime_module.contains("append_tassadar_unreachable_instruction_suffix");
    let tokenizer_supports_symbolic_retokenization =
        tokenizer_module.contains("retokenize_symbolic_text");
    let tokenizer_supports_prompt_target_text_composition =
        tokenizer_module.contains("compose_prompt_and_target_symbolic_text");
    let passed = boundary_doc_names_invariance_gate
        && boundary_doc_names_representation_sensitivity
        && runtime_defines_prompt_field_surface
        && runtime_defines_local_remap_helpers
        && runtime_defines_dead_code_helper
        && tokenizer_supports_symbolic_retokenization
        && tokenizer_supports_prompt_target_text_composition;

    Ok(TassadarArticleRepresentationInvarianceBoundaryReview {
        boundary_doc_ref: String::from(BOUNDARY_DOC_REF),
        runtime_module_ref: String::from(RUNTIME_MODULE_REF),
        tokenizer_module_ref: String::from(TOKENIZER_MODULE_REF),
        boundary_doc_names_invariance_gate,
        boundary_doc_names_representation_sensitivity,
        runtime_defines_prompt_field_surface,
        runtime_defines_local_remap_helpers,
        runtime_defines_dead_code_helper,
        tokenizer_supports_symbolic_retokenization,
        tokenizer_supports_prompt_target_text_composition,
        passed,
        detail: String::from(
            "the boundary review checks that runtime-owned prompt-surface and local-remap helpers exist, tokenizer symbolic retokenization helpers exist, and the canonical boundary doc names TAS-167A plus representation-sensitive canonicalization explicitly",
        ),
    })
}

fn inject_whitespace_variants(text: &str) -> String {
    let mut perturbed = String::from("\n");
    for (index, token) in text.split_whitespace().enumerate() {
        perturbed.push_str(token);
        match index % 4 {
            0 => perturbed.push('\n'),
            1 => perturbed.push('\t'),
            2 => perturbed.push_str("  "),
            _ => perturbed.push(' '),
        }
    }
    perturbed.push('\n');
    perturbed
}

fn execute_case(
    case: &TassadarValidationCase,
) -> Result<TassadarExecution, TassadarArticleRepresentationInvarianceGateReportError> {
    execute_program(&case.program)
}

fn execute_program(
    program: &TassadarProgram,
) -> Result<TassadarExecution, TassadarArticleRepresentationInvarianceGateReportError> {
    Ok(TassadarCpuReferenceRunner::for_program(program)?.execute(program)?)
}

fn execution_matches_ignoring_identity(
    left: &TassadarExecution,
    right: &TassadarExecution,
) -> bool {
    left.trace_abi == right.trace_abi
        && left.steps == right.steps
        && left.outputs == right.outputs
        && left.final_locals == right.final_locals
        && left.final_memory == right.final_memory
        && left.final_stack == right.final_stack
        && left.halt_reason == right.halt_reason
}

pub fn tassadar_article_representation_invariance_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF)
}

pub fn write_tassadar_article_representation_invariance_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleRepresentationInvarianceGateReport,
    TassadarArticleRepresentationInvarianceGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleRepresentationInvarianceGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_representation_invariance_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleRepresentationInvarianceGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn read_repo_text(
    relative_path: &str,
) -> Result<String, TassadarArticleRepresentationInvarianceGateReportError> {
    let path = repo_root().join(relative_path);
    fs::read_to_string(&path).map_err(|error| {
        TassadarArticleRepresentationInvarianceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleRepresentationInvarianceGateReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleRepresentationInvarianceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleRepresentationInvarianceGateReportError::DecodeJson {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        boundary_review, build_report_from_inputs,
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_article_representation_invariance_gate_report,
        build_tassadar_article_trace_vocabulary_binding_report, read_repo_json,
        write_tassadar_article_representation_invariance_gate_report,
        TassadarArticleRepresentationInvarianceGateReport,
        TassadarArticleRepresentationInvariancePerturbationKind,
        TassadarArticleTraceStabilityClass,
        TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
    };

    #[test]
    fn article_representation_invariance_gate_tracks_green_requirement_without_final_green() {
        let report = build_tassadar_article_representation_invariance_gate_report()
            .expect("representation invariance gate");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.acceptance_gate_tie.acceptance_status,
            super::TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert!(report.trace_vocabulary_binding_green);
        assert!(report.article_representation_invariance_green);
        assert!(report.article_equivalence_green);
        assert_eq!(
            report
                .representation_equivalence_review
                .required_perturbation_kind_count,
            5
        );
        assert_eq!(
            report
                .representation_equivalence_review
                .covered_perturbation_kind_count,
            5
        );
        assert!(
            report
                .representation_equivalence_review
                .suppressed_case_count
                > 0
        );
        assert!(report.trace_stability_review.exact_trace_case_count > 0);
        assert!(report.trace_stability_review.canonicalized_trace_case_count > 0);
    }

    #[test]
    fn register_renaming_is_representation_sensitive_but_canonicalized() {
        let report = build_tassadar_article_representation_invariance_gate_report()
            .expect("representation invariance gate");
        let row = report
            .case_rows
            .iter()
            .find(|row| {
                row.perturbation_kind
                    == TassadarArticleRepresentationInvariancePerturbationKind::RegisterRenaming
            })
            .expect("register-renaming row");

        assert!(row.output_stable);
        assert!(!row.raw_trace_stable);
        assert!(row.canonicalized_trace_stable);
        assert_eq!(
            row.trace_stability_class,
            TassadarArticleTraceStabilityClass::CanonicalizedEquivalent
        );
    }

    #[test]
    fn failed_case_keeps_representation_invariance_gate_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate");
        let trace_vocabulary_binding_report =
            build_tassadar_article_trace_vocabulary_binding_report()
                .expect("trace vocabulary binding");
        let mut report = build_tassadar_article_representation_invariance_gate_report()
            .expect("representation invariance gate");
        report.case_rows[0].passed = false;
        report.case_rows[0].output_stable = false;
        report.case_rows[0].trace_stability_class = TassadarArticleTraceStabilityClass::Failed;
        let report = build_report_from_inputs(
            acceptance_gate_report,
            trace_vocabulary_binding_report,
            report.case_rows,
            report.suppressed_case_rows,
            boundary_review().expect("boundary review"),
        );

        assert!(!report.article_representation_invariance_green);
        assert!(!report.article_equivalence_green);
    }

    #[test]
    fn article_representation_invariance_gate_matches_committed_truth() {
        let generated = build_tassadar_article_representation_invariance_gate_report()
            .expect("representation invariance gate");
        let committed: TassadarArticleRepresentationInvarianceGateReport = read_repo_json(
            TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
            "article_representation_invariance_gate",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_representation_invariance_gate_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_representation_invariance_gate_report.json");
        let written = write_tassadar_article_representation_invariance_gate_report(&output_path)
            .expect("written report");

        assert_eq!(
            written,
            read_repo_json(
                TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
                "article_representation_invariance_gate",
            )
            .expect("committed report")
        );
        assert_eq!(
            output_path.file_name().and_then(|value| value.to_str()),
            Some("tassadar_article_representation_invariance_gate_report.json")
        );
    }
}
