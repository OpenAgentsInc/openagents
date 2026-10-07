use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{TassadarArticleTransformer, TassadarArticleTransformerError};
use psionic_runtime::{
    tassadar_article_class_corpus, tassadar_hungarian_10x10_corpus,
    tassadar_hungarian_10x10_matching_program, tassadar_hungarian_v0_matching_program,
    tassadar_sudoku_9x9_corpus, tassadar_sudoku_9x9_search_program, tassadar_sudoku_v0_corpus,
    tassadar_sudoku_v0_search_program, TassadarExactnessPosture, TassadarExactnessRefusalReport,
    TassadarExecution, TassadarExecutorDecodeMode, TassadarExecutorSelectionDiagnostic,
    TassadarExecutorSelectionState, TassadarFixtureRunner, TassadarInstruction, TassadarProgram,
    TassadarValidationCase,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TassadarArticleTransformerReferenceLinearExactnessGateReport,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_generalization_gate_report.json";
pub const TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-transformer-generalization.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-171B";
const GENERALIZATION_RUNTIME_BACKEND: &str = "tassadar_article_transformer.generalization.v1";
const SUDOKU_V0_CELL_COUNT: usize = 16;
const SUDOKU_9X9_CELL_COUNT: usize = 81;
const HUNGARIAN_V0_DIM: usize = 4;
const HUNGARIAN_V0_MATRIX_CELL_COUNT: usize = 16;
const HUNGARIAN_10X10_DIM: usize = 10;
const HUNGARIAN_10X10_MATRIX_CELL_COUNT: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerGeneralizationFamily {
    SudokuV0,
    HungarianV0,
    Sudoku9x9,
    Hungarian10x10,
}

impl TassadarArticleTransformerGeneralizationFamily {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SudokuV0 => "sudoku_v0",
            Self::HungarianV0 => "hungarian_v0",
            Self::Sudoku9x9 => "sudoku_9x9",
            Self::Hungarian10x10 => "hungarian_10x10",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerGeneralizationSizeTier {
    Bounded,
    ArticleScale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerGeneralizationEvidenceTag {
    RandomizedProgram,
    AdversarialVariant,
    SizeStructureScaling,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub exactness_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationExactnessPrerequisiteReview {
    pub report_ref: String,
    pub reference_linear_exactness_green: bool,
    pub declared_case_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationCaseRow {
    pub case_id: String,
    pub family: TassadarArticleTransformerGeneralizationFamily,
    pub size_tier: TassadarArticleTransformerGeneralizationSizeTier,
    pub evidence_tags: Vec<TassadarArticleTransformerGeneralizationEvidenceTag>,
    pub program_id: String,
    pub input_digest: String,
    pub program_payload_digest: String,
    pub matches_declared_program_payload: bool,
    pub matches_family_corpus_input: bool,
    pub out_of_distribution_relative_to_declared_corpus: bool,
    pub instruction_count: usize,
    pub trace_step_count: usize,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub within_transformer_context_window: bool,
    pub runtime_report: TassadarExactnessRefusalReport,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationCurriculumRunRow {
    pub run_id: String,
    pub ordered_case_ids: Vec<String>,
    pub mixed_family_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub total_trace_step_count: usize,
    pub total_prompt_token_count: usize,
    pub total_target_token_count: usize,
    pub all_cases_out_of_distribution: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationRandomizedProgramReview {
    pub case_count: usize,
    pub family_count: usize,
    pub out_of_distribution_case_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationAdversarialVariantReview {
    pub case_count: usize,
    pub family_count: usize,
    pub out_of_distribution_case_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationSizeStructureScalingReview {
    pub case_count: usize,
    pub bounded_case_count: usize,
    pub article_scale_case_count: usize,
    pub family_count: usize,
    pub min_instruction_count: usize,
    pub max_instruction_count: usize,
    pub min_trace_step_count: usize,
    pub max_trace_step_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationCurriculumOrderReview {
    pub run_count: usize,
    pub total_case_evaluations: usize,
    pub mixed_family_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub all_runs_exact: bool,
    pub all_runs_out_of_distribution: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationComplexityScalingReview {
    pub case_count: usize,
    pub min_prompt_token_count: usize,
    pub max_prompt_token_count: usize,
    pub min_target_token_count: usize,
    pub max_target_token_count: usize,
    pub min_trace_step_count: usize,
    pub max_trace_step_count: usize,
    pub min_instruction_count: usize,
    pub max_instruction_count: usize,
    pub exact_error_rate_bps: u32,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleTransformerGeneralizationAcceptanceGateTie,
    pub exactness_prerequisite_review:
        TassadarArticleTransformerGeneralizationExactnessPrerequisiteReview,
    pub transformer_model_id: String,
    pub case_rows: Vec<TassadarArticleTransformerGeneralizationCaseRow>,
    pub curriculum_runs: Vec<TassadarArticleTransformerGeneralizationCurriculumRunRow>,
    pub randomized_program_review:
        TassadarArticleTransformerGeneralizationRandomizedProgramReview,
    pub adversarial_variant_review:
        TassadarArticleTransformerGeneralizationAdversarialVariantReview,
    pub size_structure_scaling_review:
        TassadarArticleTransformerGeneralizationSizeStructureScalingReview,
    pub curriculum_order_review:
        TassadarArticleTransformerGeneralizationCurriculumOrderReview,
    pub complexity_scaling_review:
        TassadarArticleTransformerGeneralizationComplexityScalingReview,
    pub case_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub out_of_distribution_case_count: usize,
    pub mismatch_case_ids: Vec<String>,
    pub refused_case_ids: Vec<String>,
    pub generalization_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerGeneralizationGateReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error("failed to execute fixture baseline for `{case_id}`: {detail}")]
    FixtureExecution { case_id: String, detail: String },
    #[error("internal Tassadar generalization invariant failed: {detail}")]
    Invariant { detail: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
struct GeneratedCase {
    validation_case: TassadarValidationCase,
    family: TassadarArticleTransformerGeneralizationFamily,
    size_tier: TassadarArticleTransformerGeneralizationSizeTier,
    evidence_tags: Vec<TassadarArticleTransformerGeneralizationEvidenceTag>,
    input_digest: String,
    program_payload_digest: String,
    matches_declared_program_payload: bool,
    matches_family_corpus_input: bool,
    curriculum_candidate: bool,
}

struct EvaluatedCase {
    runtime_report: TassadarExactnessRefusalReport,
    trace_step_count: usize,
    prompt_token_count: usize,
    target_token_count: usize,
    within_transformer_context_window: bool,
}

#[derive(Serialize)]
struct ProgramPayloadView<'a> {
    profile_id: &'a str,
    local_count: usize,
    memory_slots: usize,
    initial_memory: &'a [i32],
    instructions: &'a [TassadarInstruction],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationEvaluation {
    pub case_rows: Vec<TassadarArticleTransformerGeneralizationCaseRow>,
    pub curriculum_runs: Vec<TassadarArticleTransformerGeneralizationCurriculumRunRow>,
}

pub fn build_tassadar_article_transformer_generalization_gate_report() -> Result<
    TassadarArticleTransformerGeneralizationGateReport,
    TassadarArticleTransformerGeneralizationGateReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let exactness_report: TassadarArticleTransformerReferenceLinearExactnessGateReport =
        read_repo_json(TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF)?;
    let model = TassadarArticleTransformer::trained_trace_domain_reference()?;
    let evaluation = build_tassadar_article_transformer_generalization_evaluation_for_model(&model)?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        exactness_report,
        model.descriptor().model.model_id.clone(),
        evaluation.case_rows,
        evaluation.curriculum_runs,
    ))
}

pub fn build_tassadar_article_transformer_generalization_evaluation_for_model(
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleTransformerGeneralizationEvaluation,
    TassadarArticleTransformerGeneralizationGateReportError,
> {
    let generated_cases = build_generated_cases()?;
    let case_rows = generated_cases
        .iter()
        .map(|case| build_case_row(case, model))
        .collect::<Result<Vec<_>, _>>()?;
    let curriculum_runs = build_curriculum_runs(generated_cases.as_slice(), model)?;
    Ok(TassadarArticleTransformerGeneralizationEvaluation {
        case_rows,
        curriculum_runs,
    })
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    exactness_report: TassadarArticleTransformerReferenceLinearExactnessGateReport,
    transformer_model_id: String,
    case_rows: Vec<TassadarArticleTransformerGeneralizationCaseRow>,
    curriculum_runs: Vec<TassadarArticleTransformerGeneralizationCurriculumRunRow>,
) -> TassadarArticleTransformerGeneralizationGateReport {
    let acceptance_gate_tie = TassadarArticleTransformerGeneralizationAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        exactness_gate_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let exactness_prerequisite_review = exactness_prerequisite_review(&exactness_report);
    let randomized_program_review = randomized_program_review(case_rows.as_slice());
    let adversarial_variant_review = adversarial_variant_review(case_rows.as_slice());
    let size_structure_scaling_review = size_structure_scaling_review(case_rows.as_slice());
    let curriculum_order_review = curriculum_order_review(curriculum_runs.as_slice());
    let complexity_scaling_review = complexity_scaling_review(case_rows.as_slice());
    let case_count = case_rows.len();
    let exact_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Exact)
        .count();
    let mismatch_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Mismatch)
        .count();
    let refused_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Refused)
        .count();
    let out_of_distribution_case_count = case_rows
        .iter()
        .filter(|row| row.out_of_distribution_relative_to_declared_corpus)
        .count();
    let mismatch_case_ids = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Mismatch)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let refused_case_ids = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Refused)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let generalization_green = acceptance_gate_tie.tied_requirement_satisfied
        && exactness_prerequisite_review.passed
        && exact_case_count == case_count
        && mismatch_case_count == 0
        && refused_case_count == 0
        && out_of_distribution_case_count == case_count
        && randomized_program_review.passed
        && adversarial_variant_review.passed
        && size_structure_scaling_review.passed
        && curriculum_order_review.passed
        && complexity_scaling_review.passed;
    let article_equivalence_green =
        generalization_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTransformerGeneralizationGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_transformer.generalization_gate.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_CHECKER_REF,
        ),
        acceptance_gate_tie,
        exactness_prerequisite_review,
        transformer_model_id,
        case_rows,
        curriculum_runs,
        randomized_program_review,
        adversarial_variant_review,
        size_structure_scaling_review,
        curriculum_order_review,
        complexity_scaling_review,
        case_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
        out_of_distribution_case_count,
        mismatch_case_ids,
        refused_case_ids,
        generalization_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this gate certifies only that the owned Transformer-backed reference-linear route stays exact on a deterministic held-out and adversarial article-envelope generalization suite, including mixed-order runs and size/structure scaling across bounded and article-scale Sudoku and Hungarian programs. It does not yet certify dataset contamination independence, fast-route promotion, article-demo benchmark parity, single-run no-spill closure, clean-room weight causality, route minimality, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Transformer-backed generalization gate now records case_count={}, exact_case_count={}, mismatch_case_count={}, refused_case_count={}, out_of_distribution_case_count={}, randomized_case_count={}, adversarial_case_count={}, curriculum_run_count={}, generalization_green={}, and article_equivalence_green={}.",
        report.case_count,
        report.exact_case_count,
        report.mismatch_case_count,
        report.refused_case_count,
        report.out_of_distribution_case_count,
        report.randomized_program_review.case_count,
        report.adversarial_variant_review.case_count,
        report.curriculum_order_review.run_count,
        report.generalization_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_generalization_gate_report|",
        &report,
    );
    report
}

fn exactness_prerequisite_review(
    exactness_report: &TassadarArticleTransformerReferenceLinearExactnessGateReport,
) -> TassadarArticleTransformerGeneralizationExactnessPrerequisiteReview {
    let passed = exactness_report.reference_linear_exactness_green
        && exactness_report.exact_case_count == exactness_report.declared_case_count
        && exactness_report.mismatch_case_count == 0
        && exactness_report.refused_case_count == 0;
    let detail = format!(
        "reference_linear_exactness_green={} declared_case_count={} exact_case_count={} mismatch_case_count={} refused_case_count={}",
        exactness_report.reference_linear_exactness_green,
        exactness_report.declared_case_count,
        exactness_report.exact_case_count,
        exactness_report.mismatch_case_count,
        exactness_report.refused_case_count,
    );
    TassadarArticleTransformerGeneralizationExactnessPrerequisiteReview {
        report_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF),
        reference_linear_exactness_green: exactness_report.reference_linear_exactness_green,
        declared_case_count: exactness_report.declared_case_count,
        exact_case_count: exactness_report.exact_case_count,
        mismatch_case_count: exactness_report.mismatch_case_count,
        refused_case_count: exactness_report.refused_case_count,
        passed,
        detail,
    }
}

fn randomized_program_review(
    case_rows: &[TassadarArticleTransformerGeneralizationCaseRow],
) -> TassadarArticleTransformerGeneralizationRandomizedProgramReview {
    let rows = case_rows
        .iter()
        .filter(|row| has_evidence_tag(row, TassadarArticleTransformerGeneralizationEvidenceTag::RandomizedProgram))
        .collect::<Vec<_>>();
    let family_count = rows
        .iter()
        .map(|row| row.family)
        .collect::<BTreeSet<_>>()
        .len();
    let out_of_distribution_case_count = rows
        .iter()
        .filter(|row| row.out_of_distribution_relative_to_declared_corpus)
        .count();
    let exact_case_count = rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Exact)
        .count();
    let mismatch_case_count = rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Mismatch)
        .count();
    let refused_case_count = rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Refused)
        .count();
    let passed = rows.len() >= 2
        && family_count >= 2
        && out_of_distribution_case_count == rows.len()
        && exact_case_count == rows.len()
        && mismatch_case_count == 0
        && refused_case_count == 0;
    let detail = format!(
        "case_count={} family_count={} out_of_distribution_case_count={} exact_case_count={} mismatch_case_count={} refused_case_count={}",
        rows.len(),
        family_count,
        out_of_distribution_case_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
    );
    TassadarArticleTransformerGeneralizationRandomizedProgramReview {
        case_count: rows.len(),
        family_count,
        out_of_distribution_case_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
        passed,
        detail,
    }
}

fn adversarial_variant_review(
    case_rows: &[TassadarArticleTransformerGeneralizationCaseRow],
) -> TassadarArticleTransformerGeneralizationAdversarialVariantReview {
    let rows = case_rows
        .iter()
        .filter(|row| has_evidence_tag(row, TassadarArticleTransformerGeneralizationEvidenceTag::AdversarialVariant))
        .collect::<Vec<_>>();
    let family_count = rows
        .iter()
        .map(|row| row.family)
        .collect::<BTreeSet<_>>()
        .len();
    let out_of_distribution_case_count = rows
        .iter()
        .filter(|row| row.out_of_distribution_relative_to_declared_corpus)
        .count();
    let exact_case_count = rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Exact)
        .count();
    let mismatch_case_count = rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Mismatch)
        .count();
    let refused_case_count = rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Refused)
        .count();
    let passed = rows.len() >= 2
        && family_count >= 2
        && out_of_distribution_case_count == rows.len()
        && exact_case_count == rows.len()
        && mismatch_case_count == 0
        && refused_case_count == 0;
    let detail = format!(
        "case_count={} family_count={} out_of_distribution_case_count={} exact_case_count={} mismatch_case_count={} refused_case_count={}",
        rows.len(),
        family_count,
        out_of_distribution_case_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
    );
    TassadarArticleTransformerGeneralizationAdversarialVariantReview {
        case_count: rows.len(),
        family_count,
        out_of_distribution_case_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
        passed,
        detail,
    }
}

fn size_structure_scaling_review(
    case_rows: &[TassadarArticleTransformerGeneralizationCaseRow],
) -> TassadarArticleTransformerGeneralizationSizeStructureScalingReview {
    let case_count = case_rows.len();
    let bounded_case_count = case_rows
        .iter()
        .filter(|row| row.size_tier == TassadarArticleTransformerGeneralizationSizeTier::Bounded)
        .count();
    let article_scale_case_count = case_rows
        .iter()
        .filter(|row| {
            row.size_tier == TassadarArticleTransformerGeneralizationSizeTier::ArticleScale
        })
        .count();
    let family_count = case_rows
        .iter()
        .map(|row| row.family)
        .collect::<BTreeSet<_>>()
        .len();
    let min_instruction_count = case_rows
        .iter()
        .map(|row| row.instruction_count)
        .min()
        .unwrap_or(0);
    let max_instruction_count = case_rows
        .iter()
        .map(|row| row.instruction_count)
        .max()
        .unwrap_or(0);
    let min_trace_step_count = case_rows
        .iter()
        .map(|row| row.trace_step_count)
        .min()
        .unwrap_or(0);
    let max_trace_step_count = case_rows
        .iter()
        .map(|row| row.trace_step_count)
        .max()
        .unwrap_or(0);
    let exact_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Exact)
        .count();
    let mismatch_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Mismatch)
        .count();
    let refused_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Refused)
        .count();
    let passed = case_count >= 6
        && bounded_case_count >= 2
        && article_scale_case_count >= 4
        && family_count >= 4
        && exact_case_count == case_count
        && mismatch_case_count == 0
        && refused_case_count == 0
        && max_instruction_count > min_instruction_count
        && max_trace_step_count > min_trace_step_count;
    let detail = format!(
        "case_count={} bounded_case_count={} article_scale_case_count={} family_count={} min_instruction_count={} max_instruction_count={} min_trace_step_count={} max_trace_step_count={} exact_case_count={} mismatch_case_count={} refused_case_count={}",
        case_count,
        bounded_case_count,
        article_scale_case_count,
        family_count,
        min_instruction_count,
        max_instruction_count,
        min_trace_step_count,
        max_trace_step_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
    );
    TassadarArticleTransformerGeneralizationSizeStructureScalingReview {
        case_count,
        bounded_case_count,
        article_scale_case_count,
        family_count,
        min_instruction_count,
        max_instruction_count,
        min_trace_step_count,
        max_trace_step_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
        passed,
        detail,
    }
}

fn curriculum_order_review(
    curriculum_runs: &[TassadarArticleTransformerGeneralizationCurriculumRunRow],
) -> TassadarArticleTransformerGeneralizationCurriculumOrderReview {
    let run_count = curriculum_runs.len();
    let total_case_evaluations = curriculum_runs
        .iter()
        .map(|run| run.exact_case_count + run.mismatch_case_count + run.refused_case_count)
        .sum::<usize>();
    let mixed_family_count = curriculum_runs
        .iter()
        .map(|run| run.mixed_family_count)
        .max()
        .unwrap_or(0);
    let exact_case_count = curriculum_runs
        .iter()
        .map(|run| run.exact_case_count)
        .sum::<usize>();
    let mismatch_case_count = curriculum_runs
        .iter()
        .map(|run| run.mismatch_case_count)
        .sum::<usize>();
    let refused_case_count = curriculum_runs
        .iter()
        .map(|run| run.refused_case_count)
        .sum::<usize>();
    let all_runs_exact = curriculum_runs.iter().all(|run| run.passed);
    let all_runs_out_of_distribution = curriculum_runs
        .iter()
        .all(|run| run.all_cases_out_of_distribution);
    let passed = run_count >= 2
        && total_case_evaluations >= 8
        && mixed_family_count >= 4
        && all_runs_exact
        && all_runs_out_of_distribution
        && mismatch_case_count == 0
        && refused_case_count == 0;
    let detail = format!(
        "run_count={} total_case_evaluations={} mixed_family_count={} exact_case_count={} mismatch_case_count={} refused_case_count={} all_runs_exact={} all_runs_out_of_distribution={}",
        run_count,
        total_case_evaluations,
        mixed_family_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
        all_runs_exact,
        all_runs_out_of_distribution,
    );
    TassadarArticleTransformerGeneralizationCurriculumOrderReview {
        run_count,
        total_case_evaluations,
        mixed_family_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
        all_runs_exact,
        all_runs_out_of_distribution,
        passed,
        detail,
    }
}

fn complexity_scaling_review(
    case_rows: &[TassadarArticleTransformerGeneralizationCaseRow],
) -> TassadarArticleTransformerGeneralizationComplexityScalingReview {
    let case_count = case_rows.len();
    let min_prompt_token_count = case_rows
        .iter()
        .map(|row| row.prompt_token_count)
        .min()
        .unwrap_or(0);
    let max_prompt_token_count = case_rows
        .iter()
        .map(|row| row.prompt_token_count)
        .max()
        .unwrap_or(0);
    let min_target_token_count = case_rows
        .iter()
        .map(|row| row.target_token_count)
        .min()
        .unwrap_or(0);
    let max_target_token_count = case_rows
        .iter()
        .map(|row| row.target_token_count)
        .max()
        .unwrap_or(0);
    let min_trace_step_count = case_rows
        .iter()
        .map(|row| row.trace_step_count)
        .min()
        .unwrap_or(0);
    let max_trace_step_count = case_rows
        .iter()
        .map(|row| row.trace_step_count)
        .max()
        .unwrap_or(0);
    let min_instruction_count = case_rows
        .iter()
        .map(|row| row.instruction_count)
        .min()
        .unwrap_or(0);
    let max_instruction_count = case_rows
        .iter()
        .map(|row| row.instruction_count)
        .max()
        .unwrap_or(0);
    let error_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture != TassadarExactnessPosture::Exact)
        .count();
    let exact_error_rate_bps = if case_count == 0 {
        10_000
    } else {
        ((error_case_count * 10_000) / case_count) as u32
    };
    let passed = case_count >= 6
        && exact_error_rate_bps == 0
        && max_prompt_token_count > min_prompt_token_count
        && max_target_token_count > min_target_token_count
        && max_trace_step_count > min_trace_step_count
        && max_instruction_count > min_instruction_count;
    let detail = format!(
        "case_count={} min_prompt_token_count={} max_prompt_token_count={} min_target_token_count={} max_target_token_count={} min_trace_step_count={} max_trace_step_count={} min_instruction_count={} max_instruction_count={} exact_error_rate_bps={}",
        case_count,
        min_prompt_token_count,
        max_prompt_token_count,
        min_target_token_count,
        max_target_token_count,
        min_trace_step_count,
        max_trace_step_count,
        min_instruction_count,
        max_instruction_count,
        exact_error_rate_bps,
    );
    TassadarArticleTransformerGeneralizationComplexityScalingReview {
        case_count,
        min_prompt_token_count,
        max_prompt_token_count,
        min_target_token_count,
        max_target_token_count,
        min_trace_step_count,
        max_trace_step_count,
        min_instruction_count,
        max_instruction_count,
        exact_error_rate_bps,
        passed,
        detail,
    }
}

fn build_case_row(
    case: &GeneratedCase,
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleTransformerGeneralizationCaseRow,
    TassadarArticleTransformerGeneralizationGateReportError,
> {
    let evaluated = evaluate_generated_case(case, model)?;
    let out_of_distribution_relative_to_declared_corpus =
        !case.matches_declared_program_payload && !case.matches_family_corpus_input;
    let detail = if evaluated.runtime_report.exactness_posture == TassadarExactnessPosture::Exact {
        format!(
            "owned Transformer-backed reference-linear route stayed exact on `{}` ({}) with evidence_tags=[{}], out_of_distribution_relative_to_declared_corpus={}",
            case.validation_case.case_id,
            case.family.label(),
            format_evidence_tags(case.evidence_tags.as_slice()),
            out_of_distribution_relative_to_declared_corpus,
        )
    } else {
        evaluated.runtime_report.detail.clone()
    };
    Ok(TassadarArticleTransformerGeneralizationCaseRow {
        case_id: case.validation_case.case_id.clone(),
        family: case.family,
        size_tier: case.size_tier,
        evidence_tags: case.evidence_tags.clone(),
        program_id: case.validation_case.program.program_id.clone(),
        input_digest: case.input_digest.clone(),
        program_payload_digest: case.program_payload_digest.clone(),
        matches_declared_program_payload: case.matches_declared_program_payload,
        matches_family_corpus_input: case.matches_family_corpus_input,
        out_of_distribution_relative_to_declared_corpus,
        instruction_count: case.validation_case.program.instructions.len(),
        trace_step_count: evaluated.trace_step_count,
        prompt_token_count: evaluated.prompt_token_count,
        target_token_count: evaluated.target_token_count,
        within_transformer_context_window: evaluated.within_transformer_context_window,
        runtime_report: evaluated.runtime_report,
        detail,
    })
}

fn build_curriculum_runs(
    cases: &[GeneratedCase],
    model: &TassadarArticleTransformer,
) -> Result<
    Vec<TassadarArticleTransformerGeneralizationCurriculumRunRow>,
    TassadarArticleTransformerGeneralizationGateReportError,
> {
    let case_map = cases
        .iter()
        .filter(|case| case.curriculum_candidate)
        .map(|case| (case.validation_case.case_id.clone(), case))
        .collect::<BTreeMap<_, _>>();
    let run_orders = [
        (
            "mixed_distribution_run_a",
            vec![
                "randomized_sudoku_v0_holdout_a",
                "adversarial_hungarian_10x10_permuted_a",
                "randomized_hungarian_v0_holdout_a",
                "adversarial_sudoku_9x9_clustered_a",
            ],
        ),
        (
            "mixed_distribution_run_b",
            vec![
                "adversarial_sudoku_9x9_clustered_a",
                "randomized_hungarian_v0_holdout_a",
                "adversarial_hungarian_10x10_permuted_a",
                "randomized_sudoku_v0_holdout_a",
            ],
        ),
    ];
    let mut runs = Vec::with_capacity(run_orders.len());
    for (run_id, ordered_case_ids) in run_orders {
        let ordered_cases = ordered_case_ids
            .iter()
            .map(|case_id| {
                case_map.get(*case_id).copied().ok_or_else(|| {
                    TassadarArticleTransformerGeneralizationGateReportError::Invariant {
                        detail: format!("missing curriculum candidate `{case_id}`"),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mixed_family_count = ordered_cases
            .iter()
            .map(|case| case.family)
            .collect::<BTreeSet<_>>()
            .len();
        let mut exact_case_count = 0usize;
        let mut mismatch_case_count = 0usize;
        let mut refused_case_count = 0usize;
        let mut total_trace_step_count = 0usize;
        let mut total_prompt_token_count = 0usize;
        let mut total_target_token_count = 0usize;
        let all_cases_out_of_distribution = ordered_cases.iter().all(|case| {
            !case.matches_declared_program_payload && !case.matches_family_corpus_input
        });

        for case in &ordered_cases {
            let evaluated = evaluate_generated_case(case, model)?;
            total_trace_step_count += evaluated.trace_step_count;
            total_prompt_token_count += evaluated.prompt_token_count;
            total_target_token_count += evaluated.target_token_count;
            match evaluated.runtime_report.exactness_posture {
                TassadarExactnessPosture::Exact => exact_case_count += 1,
                TassadarExactnessPosture::Mismatch => mismatch_case_count += 1,
                TassadarExactnessPosture::Refused => refused_case_count += 1,
            }
        }

        let passed = mismatch_case_count == 0 && refused_case_count == 0;
        let detail = format!(
            "run_id={} mixed_family_count={} exact_case_count={} mismatch_case_count={} refused_case_count={} total_trace_step_count={} total_prompt_token_count={} total_target_token_count={} all_cases_out_of_distribution={}",
            run_id,
            mixed_family_count,
            exact_case_count,
            mismatch_case_count,
            refused_case_count,
            total_trace_step_count,
            total_prompt_token_count,
            total_target_token_count,
            all_cases_out_of_distribution,
        );
        runs.push(TassadarArticleTransformerGeneralizationCurriculumRunRow {
            run_id: String::from(run_id),
            ordered_case_ids: ordered_case_ids.iter().map(|id| String::from(*id)).collect(),
            mixed_family_count,
            exact_case_count,
            mismatch_case_count,
            refused_case_count,
            total_trace_step_count,
            total_prompt_token_count,
            total_target_token_count,
            all_cases_out_of_distribution,
            passed,
            detail,
        });
    }
    Ok(runs)
}

fn evaluate_generated_case(
    case: &GeneratedCase,
    model: &TassadarArticleTransformer,
) -> Result<EvaluatedCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let fixture_execution = fixture_execution_for_case(&case.validation_case)?;
    match model.roundtrip_article_trace_domain_unbounded(
        &case.validation_case.program,
        &fixture_execution,
    ) {
        Ok(roundtrip) => {
            let transformer_execution = roundtrip.decoded_trace.materialize_execution(
                fixture_execution.program_id.clone(),
                fixture_execution.profile_id.clone(),
                String::from(GENERALIZATION_RUNTIME_BACKEND),
                fixture_execution.trace_abi.clone(),
            );
            let runtime_report = TassadarExactnessRefusalReport::from_selection_and_execution(
                generalization_subject_id(case.validation_case.case_id.as_str()),
                &direct_generalization_selection(
                    &case.validation_case,
                    fixture_execution.trace_abi.schema_version,
                ),
                &fixture_execution,
                &transformer_execution,
            );
            Ok(EvaluatedCase {
                runtime_report,
                trace_step_count: fixture_execution.steps.len(),
                prompt_token_count: roundtrip.batch.prompt_token_count,
                target_token_count: roundtrip.batch.target_token_count,
                within_transformer_context_window: within_transformer_context_window(
                    model,
                    roundtrip.batch.source_token_ids.len(),
                    roundtrip.batch.target_token_ids.len(),
                ),
            })
        }
        Err(error) => Ok(EvaluatedCase {
            runtime_report: TassadarExactnessRefusalReport::from_refusal(
                generalization_subject_id(case.validation_case.case_id.as_str()),
                &refused_generalization_selection(
                    &case.validation_case,
                    fixture_execution.trace_abi.schema_version,
                    error.to_string(),
                ),
                None,
            ),
            trace_step_count: fixture_execution.steps.len(),
            prompt_token_count: 0,
            target_token_count: 0,
            within_transformer_context_window: false,
        }),
    }
}

fn build_generated_cases(
) -> Result<Vec<GeneratedCase>, TassadarArticleTransformerGeneralizationGateReportError> {
    let declared_program_payload_digests = declared_program_payload_digests();
    let family_input_digest_sets = family_input_digest_sets();
    Ok(vec![
        build_randomized_sudoku_v0_case(
            &declared_program_payload_digests,
            family_input_digest_sets
                .get(&TassadarArticleTransformerGeneralizationFamily::SudokuV0)
                .expect("sudoku_v0 family digests"),
        )?,
        build_randomized_hungarian_v0_case(
            &declared_program_payload_digests,
            family_input_digest_sets
                .get(&TassadarArticleTransformerGeneralizationFamily::HungarianV0)
                .expect("hungarian_v0 family digests"),
        )?,
        build_adversarial_sudoku_9x9_case(
            &declared_program_payload_digests,
            family_input_digest_sets
                .get(&TassadarArticleTransformerGeneralizationFamily::Sudoku9x9)
                .expect("sudoku_9x9 family digests"),
        )?,
        build_adversarial_hungarian_10x10_case(
            &declared_program_payload_digests,
            family_input_digest_sets
                .get(&TassadarArticleTransformerGeneralizationFamily::Hungarian10x10)
                .expect("hungarian_10x10 family digests"),
        )?,
        build_scaling_sudoku_9x9_case(
            &declared_program_payload_digests,
            family_input_digest_sets
                .get(&TassadarArticleTransformerGeneralizationFamily::Sudoku9x9)
                .expect("sudoku_9x9 family digests"),
        )?,
        build_scaling_hungarian_10x10_case(
            &declared_program_payload_digests,
            family_input_digest_sets
                .get(&TassadarArticleTransformerGeneralizationFamily::Hungarian10x10)
                .expect("hungarian_10x10 family digests"),
        )?,
    ])
}

fn build_randomized_sudoku_v0_case(
    declared_program_payload_digests: &BTreeSet<String>,
    family_input_digests: &BTreeSet<String>,
) -> Result<GeneratedCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let solved = solved_sudoku_v0_grid()?;
    let masked_indices = deterministic_unique_indices(17, 8, SUDOKU_V0_CELL_COUNT);
    let puzzle = masked_grid(solved, masked_indices.as_slice(), "sudoku_v0 randomized puzzle")?;
    let program = tassadar_sudoku_v0_search_program(
        "tassadar.randomized_sudoku_v0_holdout_a.v1",
        puzzle,
    );
    finish_generated_case(
        "randomized_sudoku_v0_holdout_a",
        String::from(
            "deterministically randomized held-out 4x4 Sudoku-v0 puzzle inside the bounded article envelope",
        ),
        TassadarArticleTransformerGeneralizationFamily::SudokuV0,
        TassadarArticleTransformerGeneralizationSizeTier::Bounded,
        evidence_tags(&[
            TassadarArticleTransformerGeneralizationEvidenceTag::RandomizedProgram,
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling,
        ]),
        true,
        &puzzle,
        program,
        declared_program_payload_digests,
        family_input_digests,
    )
}

fn build_randomized_hungarian_v0_case(
    declared_program_payload_digests: &BTreeSet<String>,
    family_input_digests: &BTreeSet<String>,
) -> Result<GeneratedCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let target_assignment = deterministic_permutation(23, HUNGARIAN_V0_DIM)?
        .try_into()
        .map_err(|_| TassadarArticleTransformerGeneralizationGateReportError::Invariant {
            detail: String::from("expected 4-wide Hungarian-v0 permutation"),
        })?;
    let cost_matrix = randomized_hungarian_v0_cost_matrix(target_assignment, 31);
    let program = tassadar_hungarian_v0_matching_program(
        "tassadar.randomized_hungarian_v0_holdout_a.v1",
        cost_matrix,
    );
    finish_generated_case(
        "randomized_hungarian_v0_holdout_a",
        String::from(
            "deterministically randomized held-out 4x4 Hungarian matching matrix inside the bounded article envelope",
        ),
        TassadarArticleTransformerGeneralizationFamily::HungarianV0,
        TassadarArticleTransformerGeneralizationSizeTier::Bounded,
        evidence_tags(&[
            TassadarArticleTransformerGeneralizationEvidenceTag::RandomizedProgram,
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling,
        ]),
        true,
        &cost_matrix,
        program,
        declared_program_payload_digests,
        family_input_digests,
    )
}

fn build_adversarial_sudoku_9x9_case(
    declared_program_payload_digests: &BTreeSet<String>,
    family_input_digests: &BTreeSet<String>,
) -> Result<GeneratedCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let solved = solved_sudoku_9x9_grid()?;
    let masked_indices = [
        0, 1, 4, 7, 8, 10, 13, 16, 18, 21, 24, 27, 30, 31, 34, 37, 40, 43, 46, 49, 52, 54,
        57, 60, 63, 66, 69, 72, 75, 78,
    ];
    let puzzle = masked_grid(
        solved,
        masked_indices.as_slice(),
        "sudoku_9x9 clustered adversarial puzzle",
    )?;
    let program = tassadar_sudoku_9x9_search_program(
        "tassadar.adversarial_sudoku_9x9_clustered_a.v1",
        puzzle,
    );
    finish_generated_case(
        "adversarial_sudoku_9x9_clustered_a",
        String::from(
            "clustered 9x9 Sudoku hold-out with row and sub-grid concentration intended to stress search structure",
        ),
        TassadarArticleTransformerGeneralizationFamily::Sudoku9x9,
        TassadarArticleTransformerGeneralizationSizeTier::ArticleScale,
        evidence_tags(&[
            TassadarArticleTransformerGeneralizationEvidenceTag::AdversarialVariant,
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling,
        ]),
        true,
        &puzzle,
        program,
        declared_program_payload_digests,
        family_input_digests,
    )
}

fn build_adversarial_hungarian_10x10_case(
    declared_program_payload_digests: &BTreeSet<String>,
    family_input_digests: &BTreeSet<String>,
) -> Result<GeneratedCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let base_matrix = base_hungarian_10x10_matrix()?;
    let cost_matrix = transformed_hungarian_10x10_cost_matrix(
        &base_matrix,
        [9, 0, 8, 1, 7, 2, 6, 3, 5, 4],
        [4, 9, 3, 8, 2, 7, 1, 6, 0, 5],
        [0, 2, 1, 3, 0, 4, 1, 5, 2, 6],
        [1, 0, 2, 1, 3, 0, 4, 1, 5, 2],
    );
    let program = tassadar_hungarian_10x10_matching_program(
        "tassadar.adversarial_hungarian_10x10_permuted_a.v1",
        cost_matrix,
    );
    finish_generated_case(
        "adversarial_hungarian_10x10_permuted_a",
        String::from(
            "row- and column-permuted 10x10 Hungarian matrix with structured bias shifts intended to disrupt memorized row-order heuristics",
        ),
        TassadarArticleTransformerGeneralizationFamily::Hungarian10x10,
        TassadarArticleTransformerGeneralizationSizeTier::ArticleScale,
        evidence_tags(&[
            TassadarArticleTransformerGeneralizationEvidenceTag::AdversarialVariant,
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling,
        ]),
        true,
        &cost_matrix,
        program,
        declared_program_payload_digests,
        family_input_digests,
    )
}

fn build_scaling_sudoku_9x9_case(
    declared_program_payload_digests: &BTreeSet<String>,
    family_input_digests: &BTreeSet<String>,
) -> Result<GeneratedCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let solved = solved_sudoku_9x9_grid()?;
    let masked_indices = deterministic_unique_indices(41, 33, SUDOKU_9X9_CELL_COUNT);
    let puzzle = masked_grid(solved, masked_indices.as_slice(), "sudoku_9x9 scaling puzzle")?;
    let program = tassadar_sudoku_9x9_search_program(
        "tassadar.scaling_sudoku_9x9_interleaved_b.v1",
        puzzle,
    );
    finish_generated_case(
        "scaling_sudoku_9x9_interleaved_b",
        String::from(
            "larger deterministic 9x9 Sudoku hold-out used to extend the token- and trace-scaling envelope",
        ),
        TassadarArticleTransformerGeneralizationFamily::Sudoku9x9,
        TassadarArticleTransformerGeneralizationSizeTier::ArticleScale,
        evidence_tags(&[TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling]),
        false,
        &puzzle,
        program,
        declared_program_payload_digests,
        family_input_digests,
    )
}

fn build_scaling_hungarian_10x10_case(
    declared_program_payload_digests: &BTreeSet<String>,
    family_input_digests: &BTreeSet<String>,
) -> Result<GeneratedCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let base_matrix = base_hungarian_10x10_matrix()?;
    let cost_matrix = transformed_hungarian_10x10_cost_matrix(
        &base_matrix,
        [5, 2, 7, 0, 9, 1, 8, 3, 6, 4],
        [6, 1, 8, 3, 0, 9, 2, 7, 4, 5],
        [2, 1, 0, 2, 1, 0, 3, 1, 4, 2],
        [0, 1, 0, 2, 1, 3, 1, 4, 2, 5],
    );
    let program = tassadar_hungarian_10x10_matching_program(
        "tassadar.scaling_hungarian_10x10_permuted_b.v1",
        cost_matrix,
    );
    finish_generated_case(
        "scaling_hungarian_10x10_permuted_b",
        String::from(
            "larger deterministic 10x10 Hungarian hold-out used to extend the instruction- and trace-scaling envelope",
        ),
        TassadarArticleTransformerGeneralizationFamily::Hungarian10x10,
        TassadarArticleTransformerGeneralizationSizeTier::ArticleScale,
        evidence_tags(&[TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling]),
        false,
        &cost_matrix,
        program,
        declared_program_payload_digests,
        family_input_digests,
    )
}

fn finish_generated_case<const N: usize>(
    case_id: &str,
    summary: String,
    family: TassadarArticleTransformerGeneralizationFamily,
    size_tier: TassadarArticleTransformerGeneralizationSizeTier,
    evidence_tags: Vec<TassadarArticleTransformerGeneralizationEvidenceTag>,
    curriculum_candidate: bool,
    input_values: &[i32; N],
    program: TassadarProgram,
    declared_program_payload_digests: &BTreeSet<String>,
    family_input_digests: &BTreeSet<String>,
) -> Result<GeneratedCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let validation_case = computed_validation_case(case_id, summary, program)?;
    let input_digest =
        stable_digest(b"psionic_tassadar_generalization_input|", &input_values.as_slice());
    let program_payload_digest = program_payload_digest(&validation_case.program);
    Ok(GeneratedCase {
        validation_case,
        family,
        size_tier,
        evidence_tags,
        input_digest: input_digest.clone(),
        program_payload_digest: program_payload_digest.clone(),
        matches_declared_program_payload: declared_program_payload_digests
            .contains(&program_payload_digest),
        matches_family_corpus_input: family_input_digests.contains(&input_digest),
        curriculum_candidate,
    })
}

fn computed_validation_case(
    case_id: &str,
    summary: String,
    program: TassadarProgram,
) -> Result<TassadarValidationCase, TassadarArticleTransformerGeneralizationGateReportError> {
    let execution = fixture_execution_for_program(case_id, &program)?;
    Ok(TassadarValidationCase {
        case_id: String::from(case_id),
        summary,
        program,
        expected_trace: execution.steps,
        expected_outputs: execution.outputs,
    })
}

fn solved_sudoku_v0_grid()
-> Result<[i32; SUDOKU_V0_CELL_COUNT], TassadarArticleTransformerGeneralizationGateReportError> {
    let outputs = tassadar_sudoku_v0_corpus()
        .into_iter()
        .next()
        .ok_or_else(|| TassadarArticleTransformerGeneralizationGateReportError::Invariant {
            detail: String::from("sudoku_v0 corpus should not be empty"),
        })?
        .validation_case
        .expected_outputs;
    outputs
        .try_into()
        .map_err(|_| TassadarArticleTransformerGeneralizationGateReportError::Invariant {
            detail: String::from("sudoku_v0 solved grid should have 16 cells"),
        })
}

fn solved_sudoku_9x9_grid()
-> Result<[i32; SUDOKU_9X9_CELL_COUNT], TassadarArticleTransformerGeneralizationGateReportError>
{
    let outputs = tassadar_sudoku_9x9_corpus()
        .into_iter()
        .next()
        .ok_or_else(|| TassadarArticleTransformerGeneralizationGateReportError::Invariant {
            detail: String::from("sudoku_9x9 corpus should not be empty"),
        })?
        .validation_case
        .expected_outputs;
    outputs
        .try_into()
        .map_err(|_| TassadarArticleTransformerGeneralizationGateReportError::Invariant {
            detail: String::from("sudoku_9x9 solved grid should have 81 cells"),
        })
}

fn base_hungarian_10x10_matrix() -> Result<
    [i32; HUNGARIAN_10X10_MATRIX_CELL_COUNT],
    TassadarArticleTransformerGeneralizationGateReportError,
> {
    let values = tassadar_hungarian_10x10_corpus()
        .into_iter()
        .next()
        .ok_or_else(|| TassadarArticleTransformerGeneralizationGateReportError::Invariant {
            detail: String::from("hungarian_10x10 corpus should not be empty"),
        })?
        .cost_matrix;
    values.try_into().map_err(|_| {
        TassadarArticleTransformerGeneralizationGateReportError::Invariant {
            detail: String::from("hungarian_10x10 base matrix should have 100 cells"),
        }
    })
}

fn masked_grid<const N: usize>(
    mut solved_grid: [i32; N],
    masked_indices: &[usize],
    label: &str,
) -> Result<[i32; N], TassadarArticleTransformerGeneralizationGateReportError> {
    for index in masked_indices {
        if *index >= N {
            return Err(TassadarArticleTransformerGeneralizationGateReportError::Invariant {
                detail: format!("{label} used out-of-range masked index {index}"),
            });
        }
        solved_grid[*index] = 0;
    }
    Ok(solved_grid)
}

fn randomized_hungarian_v0_cost_matrix(
    target_assignment: [usize; HUNGARIAN_V0_DIM],
    seed: u64,
) -> [i32; HUNGARIAN_V0_MATRIX_CELL_COUNT] {
    let mut state = seed;
    let mut matrix = [0; HUNGARIAN_V0_MATRIX_CELL_COUNT];
    for row in 0..HUNGARIAN_V0_DIM {
        for column in 0..HUNGARIAN_V0_DIM {
            let noise = (next_lcg(&mut state) % 7) as i32;
            let index = row * HUNGARIAN_V0_DIM + column;
            matrix[index] = if column == target_assignment[row] {
                2 + (row as i32 * 3) + noise
            } else {
                40 + (row as i32 * 9) + (column as i32 * 5) + noise
            };
        }
    }
    matrix
}

fn transformed_hungarian_10x10_cost_matrix(
    base_matrix: &[i32; HUNGARIAN_10X10_MATRIX_CELL_COUNT],
    row_permutation: [usize; HUNGARIAN_10X10_DIM],
    column_permutation: [usize; HUNGARIAN_10X10_DIM],
    row_biases: [i32; HUNGARIAN_10X10_DIM],
    column_biases: [i32; HUNGARIAN_10X10_DIM],
) -> [i32; HUNGARIAN_10X10_MATRIX_CELL_COUNT] {
    let mut matrix = [0; HUNGARIAN_10X10_MATRIX_CELL_COUNT];
    for row in 0..HUNGARIAN_10X10_DIM {
        for column in 0..HUNGARIAN_10X10_DIM {
            let source_row = row_permutation[row];
            let source_column = column_permutation[column];
            matrix[row * HUNGARIAN_10X10_DIM + column] =
                base_matrix[source_row * HUNGARIAN_10X10_DIM + source_column]
                    + row_biases[row]
                    + column_biases[column];
        }
    }
    matrix
}

fn declared_program_payload_digests() -> BTreeSet<String> {
    tassadar_article_class_corpus()
        .into_iter()
        .map(|case| program_payload_digest(&case.program))
        .collect()
}

fn family_input_digest_sets(
) -> BTreeMap<TassadarArticleTransformerGeneralizationFamily, BTreeSet<String>> {
    BTreeMap::from([
        (
            TassadarArticleTransformerGeneralizationFamily::SudokuV0,
            tassadar_sudoku_v0_corpus()
                .into_iter()
                .map(|case| stable_digest(b"psionic_tassadar_generalization_input|", &case.puzzle_cells))
                .collect(),
        ),
        (
            TassadarArticleTransformerGeneralizationFamily::HungarianV0,
            psionic_runtime::tassadar_hungarian_v0_corpus()
                .into_iter()
                .map(|case| stable_digest(b"psionic_tassadar_generalization_input|", &case.cost_matrix))
                .collect(),
        ),
        (
            TassadarArticleTransformerGeneralizationFamily::Sudoku9x9,
            tassadar_sudoku_9x9_corpus()
                .into_iter()
                .map(|case| stable_digest(b"psionic_tassadar_generalization_input|", &case.puzzle_cells))
                .collect(),
        ),
        (
            TassadarArticleTransformerGeneralizationFamily::Hungarian10x10,
            tassadar_hungarian_10x10_corpus()
                .into_iter()
                .map(|case| stable_digest(b"psionic_tassadar_generalization_input|", &case.cost_matrix))
                .collect(),
        ),
    ])
}

fn program_payload_digest(program: &TassadarProgram) -> String {
    stable_digest(
        b"psionic_tassadar_generalization_program_payload|",
        &ProgramPayloadView {
            profile_id: program.profile_id.as_str(),
            local_count: program.local_count,
            memory_slots: program.memory_slots,
            initial_memory: program.initial_memory.as_slice(),
            instructions: program.instructions.as_slice(),
        },
    )
}

fn generalization_subject_id(case_id: &str) -> String {
    format!("tassadar.article_transformer.generalization.{case_id}")
}

fn direct_generalization_selection(
    case: &TassadarValidationCase,
    trace_abi_version: u16,
) -> TassadarExecutorSelectionDiagnostic {
    TassadarExecutorSelectionDiagnostic {
        program_id: case.program.program_id.clone(),
        runtime_backend: String::from(GENERALIZATION_RUNTIME_BACKEND),
        requested_profile_id: case.program.profile_id.clone(),
        requested_trace_abi_version: trace_abi_version,
        requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
        effective_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
        selection_state: TassadarExecutorSelectionState::Direct,
        selection_reason: None,
        detail: String::from(
            "reference-linear Transformer-backed generalization lane stayed direct on the held-out article-envelope case",
        ),
        model_supported_decode_modes: vec![TassadarExecutorDecodeMode::ReferenceLinear],
    }
}

fn refused_generalization_selection(
    case: &TassadarValidationCase,
    trace_abi_version: u16,
    detail: String,
) -> TassadarExecutorSelectionDiagnostic {
    TassadarExecutorSelectionDiagnostic {
        program_id: case.program.program_id.clone(),
        runtime_backend: String::from(GENERALIZATION_RUNTIME_BACKEND),
        requested_profile_id: case.program.profile_id.clone(),
        requested_trace_abi_version: trace_abi_version,
        requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
        effective_decode_mode: None,
        selection_state: TassadarExecutorSelectionState::Refused,
        selection_reason: None,
        detail,
        model_supported_decode_modes: vec![TassadarExecutorDecodeMode::ReferenceLinear],
    }
}

fn within_transformer_context_window(
    model: &TassadarArticleTransformer,
    source_token_count: usize,
    target_token_count: usize,
) -> bool {
    source_token_count <= model.descriptor().config.max_source_positions
        && target_token_count <= model.descriptor().config.max_target_positions
}

fn fixture_execution_for_program(
    case_id: &str,
    program: &TassadarProgram,
) -> Result<TassadarExecution, TassadarArticleTransformerGeneralizationGateReportError> {
    let runner = TassadarFixtureRunner::for_program(program).map_err(|error| {
        TassadarArticleTransformerGeneralizationGateReportError::FixtureExecution {
            case_id: String::from(case_id),
            detail: error.to_string(),
        }
    })?;
    runner.execute(program).map_err(|error| {
        TassadarArticleTransformerGeneralizationGateReportError::FixtureExecution {
            case_id: String::from(case_id),
            detail: error.to_string(),
        }
    })
}

fn fixture_execution_for_case(
    case: &TassadarValidationCase,
) -> Result<TassadarExecution, TassadarArticleTransformerGeneralizationGateReportError> {
    fixture_execution_for_program(case.case_id.as_str(), &case.program)
}

fn evidence_tags(
    tags: &[TassadarArticleTransformerGeneralizationEvidenceTag],
) -> Vec<TassadarArticleTransformerGeneralizationEvidenceTag> {
    tags.iter().copied().collect::<BTreeSet<_>>().into_iter().collect()
}

fn has_evidence_tag(
    row: &TassadarArticleTransformerGeneralizationCaseRow,
    tag: TassadarArticleTransformerGeneralizationEvidenceTag,
) -> bool {
    row.evidence_tags.iter().any(|candidate| *candidate == tag)
}

fn format_evidence_tags(
    tags: &[TassadarArticleTransformerGeneralizationEvidenceTag],
) -> String {
    tags.iter()
        .map(|tag| match tag {
            TassadarArticleTransformerGeneralizationEvidenceTag::RandomizedProgram => {
                "randomized_program"
            }
            TassadarArticleTransformerGeneralizationEvidenceTag::AdversarialVariant => {
                "adversarial_variant"
            }
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling => {
                "size_structure_scaling"
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn deterministic_unique_indices(seed: u64, count: usize, limit: usize) -> Vec<usize> {
    let mut state = seed;
    let mut indices = BTreeSet::new();
    while indices.len() < count {
        indices.insert((next_lcg(&mut state) as usize) % limit);
    }
    indices.into_iter().collect()
}

fn deterministic_permutation(
    seed: u64,
    len: usize,
) -> Result<Vec<usize>, TassadarArticleTransformerGeneralizationGateReportError> {
    if len == 0 {
        return Err(TassadarArticleTransformerGeneralizationGateReportError::Invariant {
            detail: String::from("deterministic permutation length must be non-zero"),
        });
    }
    let mut values = (0..len).collect::<Vec<_>>();
    let mut state = seed;
    for index in (1..len).rev() {
        let swap_index = (next_lcg(&mut state) as usize) % (index + 1);
        values.swap(index, swap_index);
    }
    Ok(values)
}

fn next_lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}

pub fn tassadar_article_transformer_generalization_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF)
}

pub fn write_tassadar_article_transformer_generalization_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerGeneralizationGateReport,
    TassadarArticleTransformerGeneralizationGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerGeneralizationGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_generalization_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerGeneralizationGateReportError::Write {
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

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarArticleTransformerGeneralizationGateReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTransformerGeneralizationGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerGeneralizationGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarArticleTransformerGeneralizationGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleTransformerGeneralizationGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerGeneralizationGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_curriculum_runs, build_generated_cases, build_report_from_inputs,
        build_tassadar_article_transformer_generalization_gate_report, read_json,
        tassadar_article_transformer_generalization_gate_report_path,
        write_tassadar_article_transformer_generalization_gate_report,
        TassadarArticleTransformerGeneralizationEvidenceTag,
        TassadarArticleTransformerGeneralizationGateReport,
    };
    use crate::build_tassadar_article_equivalence_acceptance_gate_report;
    use psionic_models::TassadarArticleTransformer;
    use psionic_runtime::TassadarExactnessPosture;

    #[test]
    fn generalization_gate_is_green_without_final_article_equivalence() {
        let report =
            build_tassadar_article_transformer_generalization_gate_report().expect("report");

        assert_eq!(report.case_count, 6);
        assert_eq!(report.exact_case_count, 6);
        assert_eq!(report.mismatch_case_count, 0);
        assert_eq!(report.refused_case_count, 0);
        assert_eq!(report.out_of_distribution_case_count, 6);
        assert_eq!(report.curriculum_order_review.run_count, 2);
        assert!(report.generalization_green);
        assert!(report.article_equivalence_green);
        assert!(report.randomized_program_review.passed);
        assert!(report.adversarial_variant_review.passed);
        assert!(report.size_structure_scaling_review.passed);
        assert!(report.curriculum_order_review.passed);
        assert!(report.complexity_scaling_review.passed);
        assert!(report.case_rows.iter().all(|row| {
            row.runtime_report.exactness_posture == TassadarExactnessPosture::Exact
                && row.out_of_distribution_relative_to_declared_corpus
        }));
    }

    #[test]
    fn generalization_gate_turns_red_when_ood_coverage_breaks() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let exactness_report = super::read_repo_json(
            crate::TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
        )
        .expect("exactness report");
        let model = TassadarArticleTransformer::trained_trace_domain_reference().expect("model");
        let generated_cases = build_generated_cases().expect("generated cases");
        let mut case_rows = generated_cases
            .iter()
            .map(|case| super::build_case_row(case, &model))
            .collect::<Result<Vec<_>, _>>()
            .expect("case rows");
        let curriculum_runs =
            build_curriculum_runs(generated_cases.as_slice(), &model).expect("curriculum runs");
        let row = case_rows.first_mut().expect("first case row");
        row.matches_family_corpus_input = true;
        row.out_of_distribution_relative_to_declared_corpus = false;
        row.detail = String::from("synthetic non-ood overlap for gate red-path coverage");

        let report = build_report_from_inputs(
            acceptance_gate_report,
            exactness_report,
            model.descriptor().model.model_id.clone(),
            case_rows,
            curriculum_runs,
        );

        assert_eq!(report.out_of_distribution_case_count, 5);
        assert!(!report.generalization_green);
        assert!(!report.randomized_program_review.passed || !report.size_structure_scaling_review.passed);
    }

    #[test]
    fn generalization_gate_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_generalization_gate_report().expect("report");
        let committed: TassadarArticleTransformerGeneralizationGateReport =
            read_json(tassadar_article_transformer_generalization_gate_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_generalization_gate_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_generalization_gate_report.json");
        let written = write_tassadar_article_transformer_generalization_gate_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleTransformerGeneralizationGateReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_generalization_gate_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_generalization_gate_report.json")
        );
        assert!(written.case_rows.iter().any(|row| {
            row.evidence_tags.contains(
                &TassadarArticleTransformerGeneralizationEvidenceTag::RandomizedProgram,
            )
        }));
    }
}
