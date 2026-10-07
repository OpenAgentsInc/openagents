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
    tassadar_sudoku_v0_search_program, TassadarExecution, TassadarFixtureRunner,
    TassadarInstruction, TassadarProgram, TassadarValidationCase,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    read_tassadar_article_transformer_weight_lineage_contract,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TassadarArticleTransformerGeneralizationEvidenceTag,
    TassadarArticleTransformerGeneralizationFamily,
    TassadarArticleTransformerGeneralizationGateReport,
    TassadarArticleTransformerGeneralizationSizeTier,
    TassadarArticleTransformerWeightLineageContract, TassadarArticleTransformerWeightLineageError,
    TassadarArticleTransformerWeightProductionSplit,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
};

pub const TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_evaluation_independence_audit_report.json";
pub const TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-article-evaluation-independence.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-171C";
const SUDOKU_V0_CELL_COUNT: usize = 16;
const SUDOKU_9X9_CELL_COUNT: usize = 81;
const HUNGARIAN_V0_DIM: usize = 4;
const HUNGARIAN_V0_MATRIX_CELL_COUNT: usize = 16;
const HUNGARIAN_10X10_DIM: usize = 10;
const HUNGARIAN_10X10_MATRIX_CELL_COUNT: usize = 100;
const MAX_TARGET_WINDOW_TOKENS: usize = 32;
const SOURCE_TOKEN_DIGEST_PREFIX: &[u8] =
    b"psionic_tassadar_article_transformer_weight_production_source_tokens|";
const TARGET_TOKEN_DIGEST_PREFIX: &[u8] =
    b"psionic_tassadar_article_transformer_weight_production_target_tokens|";
const SEQUENCE_DIGEST_PREFIX: &[u8] =
    b"psionic_tassadar_article_transformer_weight_production_window_sequence|";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEvaluationIndependenceAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub generalization_gate_report_ref: String,
    pub lineage_contract_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEvaluationIndependenceGeneralizationPrerequisiteReview {
    pub report_ref: String,
    pub generalization_green: bool,
    pub case_count: usize,
    pub out_of_distribution_case_count: usize,
    pub audit_case_ids_match_generalization_report: bool,
    pub audit_input_digests_match_generalization_report: bool,
    pub audit_program_payload_digests_match_generalization_report: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEvaluationIndependenceTrainingLineageReview {
    pub lineage_contract_ref: String,
    pub source_corpus_id: String,
    pub suite_id: String,
    pub training_case_count: usize,
    pub held_out_case_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEvaluationIndependenceTrainingCaseRow {
    pub case_id: String,
    pub split: TassadarArticleTransformerWeightProductionSplit,
    pub profile_id: String,
    pub trace_step_count: usize,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub full_target_token_count: usize,
    pub source_token_digest: String,
    pub target_token_digest: String,
    pub sequence_digest: String,
    pub source_prefix_digest: String,
    pub target_prefix_digest: String,
    pub sequence_prefix_digest: String,
    pub generator_id: String,
    pub generator_rule_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEvaluationIndependenceEvaluationCaseRow {
    pub case_id: String,
    pub family: TassadarArticleTransformerGeneralizationFamily,
    pub size_tier: TassadarArticleTransformerGeneralizationSizeTier,
    pub evidence_tags: Vec<TassadarArticleTransformerGeneralizationEvidenceTag>,
    pub generator_id: String,
    pub generator_seed: Option<u64>,
    pub generator_rule_digest: String,
    pub profile_id: String,
    pub trace_step_count: usize,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub full_target_token_count: usize,
    pub input_digest: String,
    pub program_payload_digest: String,
    pub source_token_digest: String,
    pub target_token_digest: String,
    pub sequence_digest: String,
    pub source_prefix_digest: String,
    pub target_prefix_digest: String,
    pub sequence_prefix_digest: String,
    pub raw_target_token_overlap: bool,
    pub generic_only_target_token_overlap: bool,
    pub exact_case_id_overlap: bool,
    pub exact_source_token_overlap: bool,
    pub exact_target_token_overlap: bool,
    pub exact_sequence_overlap: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEvaluationExclusionManifest {
    pub source_corpus_id: String,
    pub training_suite_id: String,
    pub training_case_ids: Vec<String>,
    pub evaluation_case_ids: Vec<String>,
    pub exact_case_id_overlap_ids: Vec<String>,
    pub exact_source_token_overlap_case_ids: Vec<String>,
    pub exact_target_token_overlap_case_ids: Vec<String>,
    pub generic_target_token_overlap_case_ids: Vec<String>,
    pub exact_sequence_overlap_case_ids: Vec<String>,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleNearDuplicateDetectionReview {
    pub compared_pair_count: usize,
    pub shared_source_prefix_pair_ids: Vec<String>,
    pub shared_target_prefix_pair_ids: Vec<String>,
    pub shared_sequence_prefix_pair_ids: Vec<String>,
    pub near_duplicate_pair_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleGeneratorOverlapAudit {
    pub training_generator_ids: Vec<String>,
    pub evaluation_generator_ids: Vec<String>,
    pub shared_generator_ids: Vec<String>,
    pub shared_generator_rule_digests: Vec<String>,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFeatureDistributionSimilarityReview {
    pub training_profile_ids: Vec<String>,
    pub evaluation_profile_ids: Vec<String>,
    pub shared_profile_ids: Vec<String>,
    pub training_prompt_token_range: [usize; 2],
    pub evaluation_prompt_token_range: [usize; 2],
    pub training_target_token_range: [usize; 2],
    pub evaluation_target_token_range: [usize; 2],
    pub training_trace_step_range: [usize; 2],
    pub evaluation_trace_step_range: [usize; 2],
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEvaluationIndependenceAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleEvaluationIndependenceAcceptanceGateTie,
    pub generalization_prerequisite_review:
        TassadarArticleEvaluationIndependenceGeneralizationPrerequisiteReview,
    pub training_lineage_review: TassadarArticleEvaluationIndependenceTrainingLineageReview,
    pub training_case_rows: Vec<TassadarArticleEvaluationIndependenceTrainingCaseRow>,
    pub evaluation_case_rows: Vec<TassadarArticleEvaluationIndependenceEvaluationCaseRow>,
    pub exclusion_manifest: TassadarArticleEvaluationExclusionManifest,
    pub near_duplicate_review: TassadarArticleNearDuplicateDetectionReview,
    pub generator_overlap_audit: TassadarArticleGeneratorOverlapAudit,
    pub feature_distribution_review: TassadarArticleFeatureDistributionSimilarityReview,
    pub evaluation_independence_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleEvaluationIndependenceAuditError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    WeightLineage(#[from] TassadarArticleTransformerWeightLineageError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error("failed to execute fixture baseline for `{case_id}`: {detail}")]
    FixtureExecution { case_id: String, detail: String },
    #[error("internal Tassadar evaluation-independence invariant failed: {detail}")]
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
struct EvaluationCaseSpec {
    validation_case: TassadarValidationCase,
    family: TassadarArticleTransformerGeneralizationFamily,
    size_tier: TassadarArticleTransformerGeneralizationSizeTier,
    evidence_tags: Vec<TassadarArticleTransformerGeneralizationEvidenceTag>,
    input_digest: String,
    program_payload_digest: String,
    generator_id: String,
    generator_seed: Option<u64>,
    generator_rule_digest: String,
}

struct TokenizationRecord {
    profile_id: String,
    trace_step_count: usize,
    prompt_token_count: usize,
    target_token_count: usize,
    full_target_token_count: usize,
    source_token_digest: String,
    target_token_digest: String,
    sequence_digest: String,
    source_prefix_digest: String,
    target_prefix_digest: String,
    sequence_prefix_digest: String,
}

#[derive(Serialize)]
struct ProgramPayloadView<'a> {
    profile_id: &'a str,
    local_count: usize,
    memory_slots: usize,
    initial_memory: &'a [i32],
    instructions: &'a [TassadarInstruction],
}

pub fn build_tassadar_article_evaluation_independence_audit_report() -> Result<
    TassadarArticleEvaluationIndependenceAuditReport,
    TassadarArticleEvaluationIndependenceAuditError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let generalization_report: TassadarArticleTransformerGeneralizationGateReport =
        read_repo_json(TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF)?;
    let lineage_contract = read_tassadar_article_transformer_weight_lineage_contract(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
    )?;
    let model = TassadarArticleTransformer::trained_trace_domain_reference()?;
    let training_case_rows = build_training_case_rows(&lineage_contract, &model)?;
    let evaluation_case_rows = build_evaluation_case_rows(&lineage_contract, &model)?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        generalization_report,
        lineage_contract,
        training_case_rows,
        evaluation_case_rows,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    generalization_report: TassadarArticleTransformerGeneralizationGateReport,
    lineage_contract: TassadarArticleTransformerWeightLineageContract,
    training_case_rows: Vec<TassadarArticleEvaluationIndependenceTrainingCaseRow>,
    evaluation_case_rows: Vec<TassadarArticleEvaluationIndependenceEvaluationCaseRow>,
) -> TassadarArticleEvaluationIndependenceAuditReport {
    let acceptance_gate_tie = TassadarArticleEvaluationIndependenceAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        generalization_gate_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
        ),
        lineage_contract_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let generalization_prerequisite_review =
        generalization_prerequisite_review(&generalization_report, evaluation_case_rows.as_slice());
    let training_lineage_review = training_lineage_review(&lineage_contract);
    let exclusion_manifest = exclusion_manifest(
        &lineage_contract,
        training_case_rows.as_slice(),
        evaluation_case_rows.as_slice(),
    );
    let near_duplicate_review = near_duplicate_review(
        training_case_rows.as_slice(),
        evaluation_case_rows.as_slice(),
    );
    let generator_overlap_audit = generator_overlap_audit(
        &lineage_contract,
        training_case_rows.as_slice(),
        evaluation_case_rows.as_slice(),
    );
    let feature_distribution_review = feature_distribution_review(
        training_case_rows.as_slice(),
        evaluation_case_rows.as_slice(),
    );
    let evaluation_independence_green = acceptance_gate_tie.tied_requirement_satisfied
        && generalization_prerequisite_review.passed
        && training_lineage_review.passed
        && exclusion_manifest.passed
        && near_duplicate_review.passed
        && generator_overlap_audit.passed
        && feature_distribution_review.passed;
    let article_equivalence_green =
        evaluation_independence_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleEvaluationIndependenceAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_evaluation_independence_audit.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_CHECKER_REF,
        ),
        acceptance_gate_tie,
        generalization_prerequisite_review,
        training_lineage_review,
        training_case_rows,
        evaluation_case_rows,
        exclusion_manifest,
        near_duplicate_review,
        generator_overlap_audit,
        feature_distribution_review,
        evaluation_independence_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this audit certifies only that the current held-out and adversarial article-envelope evaluation suite stays separated from the current trained weight-production slice by explicit case-id exclusion, token-digest exclusion, prefix-window duplicate checks, generator provenance, and profile-level feature separation. It does not yet certify fast-route promotion, benchmark parity, single-run no-spill closure, clean-room weight causality, route minimality, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article evaluation-independence audit now records training_case_count={}, evaluation_case_count={}, exact_case_id_overlap_count={}, exact_source_token_overlap_count={}, exact_target_token_overlap_count={}, exact_sequence_overlap_count={}, near_duplicate_pair_count={}, shared_generator_id_count={}, shared_generator_rule_digest_count={}, shared_profile_id_count={}, evaluation_independence_green={}, and article_equivalence_green={}.",
        report.training_case_rows.len(),
        report.evaluation_case_rows.len(),
        report.exclusion_manifest.exact_case_id_overlap_ids.len(),
        report.exclusion_manifest.exact_source_token_overlap_case_ids.len(),
        report.exclusion_manifest.exact_target_token_overlap_case_ids.len(),
        report.exclusion_manifest.exact_sequence_overlap_case_ids.len(),
        report.near_duplicate_review.near_duplicate_pair_count,
        report.generator_overlap_audit.shared_generator_ids.len(),
        report.generator_overlap_audit.shared_generator_rule_digests.len(),
        report.feature_distribution_review.shared_profile_ids.len(),
        report.evaluation_independence_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_evaluation_independence_audit_report|",
        &report,
    );
    report
}

fn generalization_prerequisite_review(
    generalization_report: &TassadarArticleTransformerGeneralizationGateReport,
    evaluation_case_rows: &[TassadarArticleEvaluationIndependenceEvaluationCaseRow],
) -> TassadarArticleEvaluationIndependenceGeneralizationPrerequisiteReview {
    let audit_case_ids = evaluation_case_rows
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<BTreeSet<_>>();
    let report_case_ids = generalization_report
        .case_rows
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<BTreeSet<_>>();
    let audit_input_digests = evaluation_case_rows
        .iter()
        .map(|row| row.input_digest.clone())
        .collect::<BTreeSet<_>>();
    let report_input_digests = generalization_report
        .case_rows
        .iter()
        .map(|row| row.input_digest.clone())
        .collect::<BTreeSet<_>>();
    let audit_program_payload_digests = evaluation_case_rows
        .iter()
        .map(|row| row.program_payload_digest.clone())
        .collect::<BTreeSet<_>>();
    let report_program_payload_digests = generalization_report
        .case_rows
        .iter()
        .map(|row| row.program_payload_digest.clone())
        .collect::<BTreeSet<_>>();
    let audit_case_ids_match_generalization_report = audit_case_ids == report_case_ids;
    let audit_input_digests_match_generalization_report =
        audit_input_digests == report_input_digests;
    let audit_program_payload_digests_match_generalization_report =
        audit_program_payload_digests == report_program_payload_digests;
    let passed = generalization_report.generalization_green
        && generalization_report.out_of_distribution_case_count == generalization_report.case_count
        && audit_case_ids_match_generalization_report
        && audit_input_digests_match_generalization_report
        && audit_program_payload_digests_match_generalization_report;
    let detail = format!(
        "generalization_green={} case_count={} out_of_distribution_case_count={} audit_case_ids_match_generalization_report={} audit_input_digests_match_generalization_report={} audit_program_payload_digests_match_generalization_report={}",
        generalization_report.generalization_green,
        generalization_report.case_count,
        generalization_report.out_of_distribution_case_count,
        audit_case_ids_match_generalization_report,
        audit_input_digests_match_generalization_report,
        audit_program_payload_digests_match_generalization_report,
    );
    TassadarArticleEvaluationIndependenceGeneralizationPrerequisiteReview {
        report_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF),
        generalization_green: generalization_report.generalization_green,
        case_count: generalization_report.case_count,
        out_of_distribution_case_count: generalization_report.out_of_distribution_case_count,
        audit_case_ids_match_generalization_report,
        audit_input_digests_match_generalization_report,
        audit_program_payload_digests_match_generalization_report,
        passed,
        detail,
    }
}

fn training_lineage_review(
    lineage_contract: &TassadarArticleTransformerWeightLineageContract,
) -> TassadarArticleEvaluationIndependenceTrainingLineageReview {
    let training_case_count = lineage_contract.training_cases.len();
    let held_out_case_count = lineage_contract.held_out_cases.len();
    let passed = !lineage_contract
        .training_config
        .source_corpus_id
        .trim()
        .is_empty()
        && !lineage_contract.training_config.suite_id.trim().is_empty()
        && training_case_count > 0
        && held_out_case_count > 0;
    let detail = format!(
        "source_corpus_id={} suite_id={} training_case_count={} held_out_case_count={}",
        lineage_contract.training_config.source_corpus_id,
        lineage_contract.training_config.suite_id,
        training_case_count,
        held_out_case_count,
    );
    TassadarArticleEvaluationIndependenceTrainingLineageReview {
        lineage_contract_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
        ),
        source_corpus_id: lineage_contract.training_config.source_corpus_id.clone(),
        suite_id: lineage_contract.training_config.suite_id.clone(),
        training_case_count,
        held_out_case_count,
        passed,
        detail,
    }
}

fn exclusion_manifest(
    lineage_contract: &TassadarArticleTransformerWeightLineageContract,
    training_case_rows: &[TassadarArticleEvaluationIndependenceTrainingCaseRow],
    evaluation_case_rows: &[TassadarArticleEvaluationIndependenceEvaluationCaseRow],
) -> TassadarArticleEvaluationExclusionManifest {
    let training_case_ids = training_case_rows
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let evaluation_case_ids = evaluation_case_rows
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let exact_case_id_overlap_ids = evaluation_case_rows
        .iter()
        .filter(|row| row.exact_case_id_overlap)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let exact_source_token_overlap_case_ids = evaluation_case_rows
        .iter()
        .filter(|row| row.exact_source_token_overlap)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let exact_target_token_overlap_case_ids = evaluation_case_rows
        .iter()
        .filter(|row| row.exact_target_token_overlap)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let generic_target_token_overlap_case_ids = evaluation_case_rows
        .iter()
        .filter(|row| row.generic_only_target_token_overlap)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let exact_sequence_overlap_case_ids = evaluation_case_rows
        .iter()
        .filter(|row| row.exact_sequence_overlap)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let passed = exact_case_id_overlap_ids.is_empty()
        && exact_source_token_overlap_case_ids.is_empty()
        && exact_sequence_overlap_case_ids.is_empty();
    let detail = format!(
        "source_corpus_id={} suite_id={} training_case_count={} evaluation_case_count={} exact_case_id_overlap_count={} exact_source_token_overlap_count={} exact_target_token_overlap_count={} generic_target_token_overlap_count={} exact_sequence_overlap_count={}",
        lineage_contract.training_config.source_corpus_id,
        lineage_contract.training_config.suite_id,
        training_case_ids.len(),
        evaluation_case_ids.len(),
        exact_case_id_overlap_ids.len(),
        exact_source_token_overlap_case_ids.len(),
        exact_target_token_overlap_case_ids.len(),
        generic_target_token_overlap_case_ids.len(),
        exact_sequence_overlap_case_ids.len(),
    );
    TassadarArticleEvaluationExclusionManifest {
        source_corpus_id: lineage_contract.training_config.source_corpus_id.clone(),
        training_suite_id: lineage_contract.training_config.suite_id.clone(),
        training_case_ids,
        evaluation_case_ids,
        exact_case_id_overlap_ids,
        exact_source_token_overlap_case_ids,
        exact_target_token_overlap_case_ids,
        generic_target_token_overlap_case_ids,
        exact_sequence_overlap_case_ids,
        passed,
        detail,
    }
}

fn near_duplicate_review(
    training_case_rows: &[TassadarArticleEvaluationIndependenceTrainingCaseRow],
    evaluation_case_rows: &[TassadarArticleEvaluationIndependenceEvaluationCaseRow],
) -> TassadarArticleNearDuplicateDetectionReview {
    let mut shared_source_prefix_pair_ids = Vec::new();
    let mut shared_target_prefix_pair_ids = Vec::new();
    let mut shared_sequence_prefix_pair_ids = Vec::new();

    for training_case in training_case_rows {
        for evaluation_case in evaluation_case_rows {
            let pair_id = format!("{}->{}", training_case.case_id, evaluation_case.case_id);
            if training_case.source_prefix_digest == evaluation_case.source_prefix_digest {
                shared_source_prefix_pair_ids.push(pair_id.clone());
            }
            if training_case.target_prefix_digest == evaluation_case.target_prefix_digest {
                shared_target_prefix_pair_ids.push(pair_id.clone());
            }
            if training_case.sequence_prefix_digest == evaluation_case.sequence_prefix_digest {
                shared_sequence_prefix_pair_ids.push(pair_id);
            }
        }
    }

    let compared_pair_count = training_case_rows.len() * evaluation_case_rows.len();
    let near_duplicate_pair_count = shared_source_prefix_pair_ids
        .iter()
        .chain(shared_sequence_prefix_pair_ids.iter())
        .collect::<BTreeSet<_>>()
        .len();
    let passed =
        shared_source_prefix_pair_ids.is_empty() && shared_sequence_prefix_pair_ids.is_empty();
    let detail = format!(
        "compared_pair_count={} shared_source_prefix_pair_count={} shared_target_prefix_pair_count={} shared_sequence_prefix_pair_count={} near_duplicate_pair_count={}",
        compared_pair_count,
        shared_source_prefix_pair_ids.len(),
        shared_target_prefix_pair_ids.len(),
        shared_sequence_prefix_pair_ids.len(),
        near_duplicate_pair_count,
    );
    TassadarArticleNearDuplicateDetectionReview {
        compared_pair_count,
        shared_source_prefix_pair_ids,
        shared_target_prefix_pair_ids,
        shared_sequence_prefix_pair_ids,
        near_duplicate_pair_count,
        passed,
        detail,
    }
}

fn generator_overlap_audit(
    lineage_contract: &TassadarArticleTransformerWeightLineageContract,
    training_case_rows: &[TassadarArticleEvaluationIndependenceTrainingCaseRow],
    evaluation_case_rows: &[TassadarArticleEvaluationIndependenceEvaluationCaseRow],
) -> TassadarArticleGeneratorOverlapAudit {
    let training_generator_ids = training_case_rows
        .iter()
        .map(|row| row.generator_id.clone())
        .collect::<BTreeSet<_>>();
    let evaluation_generator_ids = evaluation_case_rows
        .iter()
        .map(|row| row.generator_id.clone())
        .collect::<BTreeSet<_>>();
    let shared_generator_ids = training_generator_ids
        .intersection(&evaluation_generator_ids)
        .cloned()
        .collect::<Vec<_>>();
    let training_rule_digests = training_case_rows
        .iter()
        .map(|row| row.generator_rule_digest.clone())
        .collect::<BTreeSet<_>>();
    let evaluation_rule_digests = evaluation_case_rows
        .iter()
        .map(|row| row.generator_rule_digest.clone())
        .collect::<BTreeSet<_>>();
    let shared_generator_rule_digests = training_rule_digests
        .intersection(&evaluation_rule_digests)
        .cloned()
        .collect::<Vec<_>>();
    let passed = shared_generator_ids.is_empty() && shared_generator_rule_digests.is_empty();
    let detail = format!(
        "training_suite_id={} training_generator_count={} evaluation_generator_count={} shared_generator_id_count={} shared_generator_rule_digest_count={}",
        lineage_contract.training_config.suite_id,
        training_generator_ids.len(),
        evaluation_generator_ids.len(),
        shared_generator_ids.len(),
        shared_generator_rule_digests.len(),
    );
    TassadarArticleGeneratorOverlapAudit {
        training_generator_ids: training_generator_ids.into_iter().collect(),
        evaluation_generator_ids: evaluation_generator_ids.into_iter().collect(),
        shared_generator_ids,
        shared_generator_rule_digests,
        passed,
        detail,
    }
}

fn feature_distribution_review(
    training_case_rows: &[TassadarArticleEvaluationIndependenceTrainingCaseRow],
    evaluation_case_rows: &[TassadarArticleEvaluationIndependenceEvaluationCaseRow],
) -> TassadarArticleFeatureDistributionSimilarityReview {
    let training_profile_ids = training_case_rows
        .iter()
        .map(|row| row.profile_id.clone())
        .collect::<BTreeSet<_>>();
    let evaluation_profile_ids = evaluation_case_rows
        .iter()
        .map(|row| row.profile_id.clone())
        .collect::<BTreeSet<_>>();
    let shared_profile_ids = training_profile_ids
        .intersection(&evaluation_profile_ids)
        .cloned()
        .collect::<Vec<_>>();
    let training_prompt_token_range = range_from_values(
        training_case_rows
            .iter()
            .map(|row| row.prompt_token_count)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let evaluation_prompt_token_range = range_from_values(
        evaluation_case_rows
            .iter()
            .map(|row| row.prompt_token_count)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let training_target_token_range = range_from_values(
        training_case_rows
            .iter()
            .map(|row| row.full_target_token_count)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let evaluation_target_token_range = range_from_values(
        evaluation_case_rows
            .iter()
            .map(|row| row.full_target_token_count)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let training_trace_step_range = range_from_values(
        training_case_rows
            .iter()
            .map(|row| row.trace_step_count)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let evaluation_trace_step_range = range_from_values(
        evaluation_case_rows
            .iter()
            .map(|row| row.trace_step_count)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let passed = shared_profile_ids.is_empty();
    let detail = format!(
        "training_profile_count={} evaluation_profile_count={} shared_profile_id_count={} training_prompt_token_range={:?} evaluation_prompt_token_range={:?} training_target_token_range={:?} evaluation_target_token_range={:?} training_trace_step_range={:?} evaluation_trace_step_range={:?}",
        training_profile_ids.len(),
        evaluation_profile_ids.len(),
        shared_profile_ids.len(),
        training_prompt_token_range,
        evaluation_prompt_token_range,
        training_target_token_range,
        evaluation_target_token_range,
        training_trace_step_range,
        evaluation_trace_step_range,
    );
    TassadarArticleFeatureDistributionSimilarityReview {
        training_profile_ids: training_profile_ids.into_iter().collect(),
        evaluation_profile_ids: evaluation_profile_ids.into_iter().collect(),
        shared_profile_ids,
        training_prompt_token_range,
        evaluation_prompt_token_range,
        training_target_token_range,
        evaluation_target_token_range,
        training_trace_step_range,
        evaluation_trace_step_range,
        passed,
        detail,
    }
}

fn build_training_case_rows(
    lineage_contract: &TassadarArticleTransformerWeightLineageContract,
    model: &TassadarArticleTransformer,
) -> Result<
    Vec<TassadarArticleEvaluationIndependenceTrainingCaseRow>,
    TassadarArticleEvaluationIndependenceAuditError,
> {
    let article_case_map = tassadar_article_class_corpus()
        .into_iter()
        .map(|case| (case.case_id.clone(), case))
        .collect::<BTreeMap<_, _>>();
    let training_generator_id = lineage_contract.training_config.suite_id.clone();
    let training_generator_rule_digest = lineage_contract.workload_set_digest.clone();
    lineage_contract
        .training_cases
        .iter()
        .chain(lineage_contract.held_out_cases.iter())
        .map(|case| {
            let validation_case = article_case_map.get(case.case_id.as_str()).ok_or_else(|| {
                TassadarArticleEvaluationIndependenceAuditError::Invariant {
                    detail: format!(
                        "missing article-class case `{}` for training audit",
                        case.case_id
                    ),
                }
            })?;
            let tokenization = tokenize_case(validation_case, model)?;
            Ok(TassadarArticleEvaluationIndependenceTrainingCaseRow {
                case_id: case.case_id.clone(),
                split: case.split,
                profile_id: case.profile_id.clone(),
                trace_step_count: case.trace_step_count,
                prompt_token_count: case.prompt_token_count,
                target_token_count: case.target_token_count,
                full_target_token_count: case.full_target_token_count,
                source_token_digest: case.source_token_digest.clone(),
                target_token_digest: case.target_token_digest.clone(),
                sequence_digest: case.sequence_digest.clone(),
                source_prefix_digest: tokenization.source_prefix_digest,
                target_prefix_digest: tokenization.target_prefix_digest,
                sequence_prefix_digest: tokenization.sequence_prefix_digest,
                generator_id: training_generator_id.clone(),
                generator_rule_digest: training_generator_rule_digest.clone(),
            })
        })
        .collect()
}

fn build_evaluation_case_rows(
    lineage_contract: &TassadarArticleTransformerWeightLineageContract,
    model: &TassadarArticleTransformer,
) -> Result<
    Vec<TassadarArticleEvaluationIndependenceEvaluationCaseRow>,
    TassadarArticleEvaluationIndependenceAuditError,
> {
    let training_case_ids = lineage_contract
        .training_cases
        .iter()
        .chain(lineage_contract.held_out_cases.iter())
        .map(|case| case.case_id.clone())
        .collect::<BTreeSet<_>>();
    let training_source_token_digests = lineage_contract
        .training_cases
        .iter()
        .chain(lineage_contract.held_out_cases.iter())
        .map(|case| case.source_token_digest.clone())
        .collect::<BTreeSet<_>>();
    let training_target_token_digests = lineage_contract
        .training_cases
        .iter()
        .chain(lineage_contract.held_out_cases.iter())
        .map(|case| case.target_token_digest.clone())
        .collect::<BTreeSet<_>>();
    let training_sequence_digests = lineage_contract
        .training_cases
        .iter()
        .chain(lineage_contract.held_out_cases.iter())
        .map(|case| case.sequence_digest.clone())
        .collect::<BTreeSet<_>>();
    build_evaluation_case_specs()?
        .into_iter()
        .map(|spec| {
            let tokenization = tokenize_case(&spec.validation_case, model)?;
            let exact_case_id_overlap = training_case_ids.contains(&spec.validation_case.case_id);
            let exact_source_token_overlap =
                training_source_token_digests.contains(&tokenization.source_token_digest);
            let raw_target_token_overlap =
                training_target_token_digests.contains(&tokenization.target_token_digest);
            let exact_sequence_overlap =
                training_sequence_digests.contains(&tokenization.sequence_digest);
            let exact_target_token_overlap = raw_target_token_overlap
                && (exact_case_id_overlap || exact_source_token_overlap || exact_sequence_overlap);
            let generic_only_target_token_overlap =
                raw_target_token_overlap && !exact_target_token_overlap;
            let detail = format!(
                "generator_id={} generator_seed={} exact_case_id_overlap={} exact_source_token_overlap={} raw_target_token_overlap={} exact_target_token_overlap={} generic_only_target_token_overlap={} exact_sequence_overlap={}",
                spec.generator_id,
                spec.generator_seed
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| String::from("none")),
                exact_case_id_overlap,
                exact_source_token_overlap,
                raw_target_token_overlap,
                exact_target_token_overlap,
                generic_only_target_token_overlap,
                exact_sequence_overlap,
            );
            Ok(TassadarArticleEvaluationIndependenceEvaluationCaseRow {
                case_id: spec.validation_case.case_id,
                family: spec.family,
                size_tier: spec.size_tier,
                evidence_tags: spec.evidence_tags,
                generator_id: spec.generator_id,
                generator_seed: spec.generator_seed,
                generator_rule_digest: spec.generator_rule_digest,
                profile_id: tokenization.profile_id,
                trace_step_count: tokenization.trace_step_count,
                prompt_token_count: tokenization.prompt_token_count,
                target_token_count: tokenization.target_token_count,
                full_target_token_count: tokenization.full_target_token_count,
                input_digest: spec.input_digest,
                program_payload_digest: spec.program_payload_digest,
                source_token_digest: tokenization.source_token_digest,
                target_token_digest: tokenization.target_token_digest,
                sequence_digest: tokenization.sequence_digest,
                source_prefix_digest: tokenization.source_prefix_digest,
                target_prefix_digest: tokenization.target_prefix_digest,
                sequence_prefix_digest: tokenization.sequence_prefix_digest,
                raw_target_token_overlap,
                generic_only_target_token_overlap,
                exact_case_id_overlap,
                exact_source_token_overlap,
                exact_target_token_overlap,
                exact_sequence_overlap,
                detail,
            })
        })
        .collect()
}

fn tokenize_case(
    case: &TassadarValidationCase,
    model: &TassadarArticleTransformer,
) -> Result<TokenizationRecord, TassadarArticleEvaluationIndependenceAuditError> {
    let execution = fixture_execution_for_case(case)?;
    let batch = model.tokenize_article_trace_domain_unbounded(&case.program, &execution)?;
    let target_tokens = batch
        .target_token_ids
        .iter()
        .copied()
        .take(MAX_TARGET_WINDOW_TOKENS)
        .collect::<Vec<_>>();
    let source_prefix = batch
        .source_token_ids
        .iter()
        .copied()
        .take(64)
        .collect::<Vec<_>>();
    let target_prefix = target_tokens.iter().copied().take(16).collect::<Vec<_>>();
    Ok(TokenizationRecord {
        profile_id: case.program.profile_id.clone(),
        trace_step_count: execution.steps.len(),
        prompt_token_count: batch.prompt_token_count,
        target_token_count: target_tokens.len(),
        full_target_token_count: batch.target_token_count,
        source_token_digest: stable_digest(SOURCE_TOKEN_DIGEST_PREFIX, &batch.source_token_ids),
        target_token_digest: stable_digest(TARGET_TOKEN_DIGEST_PREFIX, &target_tokens),
        sequence_digest: stable_digest(
            SEQUENCE_DIGEST_PREFIX,
            &(batch.source_token_ids.as_slice(), target_tokens.as_slice()),
        ),
        source_prefix_digest: stable_digest(
            b"psionic_tassadar_article_evaluation_independence_source_prefix|",
            &source_prefix,
        ),
        target_prefix_digest: stable_digest(
            b"psionic_tassadar_article_evaluation_independence_target_prefix|",
            &target_prefix,
        ),
        sequence_prefix_digest: stable_digest(
            b"psionic_tassadar_article_evaluation_independence_sequence_prefix|",
            &(source_prefix.as_slice(), target_prefix.as_slice()),
        ),
    })
}

fn build_evaluation_case_specs(
) -> Result<Vec<EvaluationCaseSpec>, TassadarArticleEvaluationIndependenceAuditError> {
    Ok(vec![
        randomized_sudoku_v0_eval_case()?,
        randomized_hungarian_v0_eval_case()?,
        adversarial_sudoku_9x9_eval_case()?,
        adversarial_hungarian_10x10_eval_case()?,
        scaling_sudoku_9x9_eval_case()?,
        scaling_hungarian_10x10_eval_case()?,
    ])
}

fn randomized_sudoku_v0_eval_case(
) -> Result<EvaluationCaseSpec, TassadarArticleEvaluationIndependenceAuditError> {
    let solved = solved_sudoku_v0_grid()?;
    let masked_indices = deterministic_unique_indices(17, 8, SUDOKU_V0_CELL_COUNT);
    let puzzle = masked_grid(
        solved,
        masked_indices.as_slice(),
        "sudoku_v0 randomized puzzle",
    )?;
    let program =
        tassadar_sudoku_v0_search_program("tassadar.randomized_sudoku_v0_holdout_a.v1", puzzle);
    evaluation_case_spec(
        "randomized_sudoku_v0_holdout_a",
        String::from("randomized_sudoku_v0_mask_lcg"),
        Some(17),
        stable_digest(
            b"psionic_tassadar_article_evaluation_independence_generator_rule|",
            &masked_indices,
        ),
        TassadarArticleTransformerGeneralizationFamily::SudokuV0,
        TassadarArticleTransformerGeneralizationSizeTier::Bounded,
        vec![
            TassadarArticleTransformerGeneralizationEvidenceTag::RandomizedProgram,
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling,
        ],
        &puzzle,
        program,
    )
}

fn randomized_hungarian_v0_eval_case(
) -> Result<EvaluationCaseSpec, TassadarArticleEvaluationIndependenceAuditError> {
    let target_assignment = deterministic_permutation(23, HUNGARIAN_V0_DIM)?
        .try_into()
        .map_err(
            |_| TassadarArticleEvaluationIndependenceAuditError::Invariant {
                detail: String::from("expected 4-wide Hungarian-v0 permutation"),
            },
        )?;
    let cost_matrix = randomized_hungarian_v0_cost_matrix(target_assignment, 31);
    let program = tassadar_hungarian_v0_matching_program(
        "tassadar.randomized_hungarian_v0_holdout_a.v1",
        cost_matrix,
    );
    evaluation_case_spec(
        "randomized_hungarian_v0_holdout_a",
        String::from("randomized_hungarian_v0_matrix_lcg"),
        Some(23),
        stable_digest(
            b"psionic_tassadar_article_evaluation_independence_generator_rule|",
            &(target_assignment, 31u64),
        ),
        TassadarArticleTransformerGeneralizationFamily::HungarianV0,
        TassadarArticleTransformerGeneralizationSizeTier::Bounded,
        vec![
            TassadarArticleTransformerGeneralizationEvidenceTag::RandomizedProgram,
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling,
        ],
        &cost_matrix,
        program,
    )
}

fn adversarial_sudoku_9x9_eval_case(
) -> Result<EvaluationCaseSpec, TassadarArticleEvaluationIndependenceAuditError> {
    let solved = solved_sudoku_9x9_grid()?;
    let masked_indices = [
        0, 1, 4, 7, 8, 10, 13, 16, 18, 21, 24, 27, 30, 31, 34, 37, 40, 43, 46, 49, 52, 54, 57, 60,
        63, 66, 69, 72, 75, 78,
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
    evaluation_case_spec(
        "adversarial_sudoku_9x9_clustered_a",
        String::from("adversarial_sudoku_9x9_clustered_mask"),
        None,
        stable_digest(
            b"psionic_tassadar_article_evaluation_independence_generator_rule|",
            &masked_indices,
        ),
        TassadarArticleTransformerGeneralizationFamily::Sudoku9x9,
        TassadarArticleTransformerGeneralizationSizeTier::ArticleScale,
        vec![
            TassadarArticleTransformerGeneralizationEvidenceTag::AdversarialVariant,
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling,
        ],
        &puzzle,
        program,
    )
}

fn adversarial_hungarian_10x10_eval_case(
) -> Result<EvaluationCaseSpec, TassadarArticleEvaluationIndependenceAuditError> {
    let base_matrix = base_hungarian_10x10_matrix()?;
    let row_permutation = [9, 0, 8, 1, 7, 2, 6, 3, 5, 4];
    let column_permutation = [4, 9, 3, 8, 2, 7, 1, 6, 0, 5];
    let row_biases = [0, 2, 1, 3, 0, 4, 1, 5, 2, 6];
    let column_biases = [1, 0, 2, 1, 3, 0, 4, 1, 5, 2];
    let cost_matrix = transformed_hungarian_10x10_cost_matrix(
        &base_matrix,
        row_permutation,
        column_permutation,
        row_biases,
        column_biases,
    );
    let program = tassadar_hungarian_10x10_matching_program(
        "tassadar.adversarial_hungarian_10x10_permuted_a.v1",
        cost_matrix,
    );
    evaluation_case_spec(
        "adversarial_hungarian_10x10_permuted_a",
        String::from("adversarial_hungarian_10x10_permuted_bias"),
        None,
        stable_digest(
            b"psionic_tassadar_article_evaluation_independence_generator_rule|",
            &(
                row_permutation,
                column_permutation,
                row_biases,
                column_biases,
            ),
        ),
        TassadarArticleTransformerGeneralizationFamily::Hungarian10x10,
        TassadarArticleTransformerGeneralizationSizeTier::ArticleScale,
        vec![
            TassadarArticleTransformerGeneralizationEvidenceTag::AdversarialVariant,
            TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling,
        ],
        &cost_matrix,
        program,
    )
}

fn scaling_sudoku_9x9_eval_case(
) -> Result<EvaluationCaseSpec, TassadarArticleEvaluationIndependenceAuditError> {
    let solved = solved_sudoku_9x9_grid()?;
    let masked_indices = deterministic_unique_indices(41, 33, SUDOKU_9X9_CELL_COUNT);
    let puzzle = masked_grid(
        solved,
        masked_indices.as_slice(),
        "sudoku_9x9 scaling puzzle",
    )?;
    let program =
        tassadar_sudoku_9x9_search_program("tassadar.scaling_sudoku_9x9_interleaved_b.v1", puzzle);
    evaluation_case_spec(
        "scaling_sudoku_9x9_interleaved_b",
        String::from("scaling_sudoku_9x9_mask_lcg"),
        Some(41),
        stable_digest(
            b"psionic_tassadar_article_evaluation_independence_generator_rule|",
            &masked_indices,
        ),
        TassadarArticleTransformerGeneralizationFamily::Sudoku9x9,
        TassadarArticleTransformerGeneralizationSizeTier::ArticleScale,
        vec![TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling],
        &puzzle,
        program,
    )
}

fn scaling_hungarian_10x10_eval_case(
) -> Result<EvaluationCaseSpec, TassadarArticleEvaluationIndependenceAuditError> {
    let base_matrix = base_hungarian_10x10_matrix()?;
    let row_permutation = [5, 2, 7, 0, 9, 1, 8, 3, 6, 4];
    let column_permutation = [6, 1, 8, 3, 0, 9, 2, 7, 4, 5];
    let row_biases = [2, 1, 0, 2, 1, 0, 3, 1, 4, 2];
    let column_biases = [0, 1, 0, 2, 1, 3, 1, 4, 2, 5];
    let cost_matrix = transformed_hungarian_10x10_cost_matrix(
        &base_matrix,
        row_permutation,
        column_permutation,
        row_biases,
        column_biases,
    );
    let program = tassadar_hungarian_10x10_matching_program(
        "tassadar.scaling_hungarian_10x10_permuted_b.v1",
        cost_matrix,
    );
    evaluation_case_spec(
        "scaling_hungarian_10x10_permuted_b",
        String::from("scaling_hungarian_10x10_permuted_bias"),
        None,
        stable_digest(
            b"psionic_tassadar_article_evaluation_independence_generator_rule|",
            &(
                row_permutation,
                column_permutation,
                row_biases,
                column_biases,
            ),
        ),
        TassadarArticleTransformerGeneralizationFamily::Hungarian10x10,
        TassadarArticleTransformerGeneralizationSizeTier::ArticleScale,
        vec![TassadarArticleTransformerGeneralizationEvidenceTag::SizeStructureScaling],
        &cost_matrix,
        program,
    )
}

fn evaluation_case_spec<const N: usize>(
    case_id: &str,
    generator_id: String,
    generator_seed: Option<u64>,
    generator_rule_digest: String,
    family: TassadarArticleTransformerGeneralizationFamily,
    size_tier: TassadarArticleTransformerGeneralizationSizeTier,
    evidence_tags: Vec<TassadarArticleTransformerGeneralizationEvidenceTag>,
    input_values: &[i32; N],
    program: TassadarProgram,
) -> Result<EvaluationCaseSpec, TassadarArticleEvaluationIndependenceAuditError> {
    let validation_case = computed_validation_case(case_id, program)?;
    let program_payload_digest = program_payload_digest(&validation_case.program);
    Ok(EvaluationCaseSpec {
        validation_case,
        family,
        size_tier,
        evidence_tags,
        input_digest: stable_digest(
            b"psionic_tassadar_generalization_input|",
            &input_values.as_slice(),
        ),
        program_payload_digest,
        generator_id,
        generator_seed,
        generator_rule_digest,
    })
}

fn computed_validation_case(
    case_id: &str,
    program: TassadarProgram,
) -> Result<TassadarValidationCase, TassadarArticleEvaluationIndependenceAuditError> {
    let execution = fixture_execution_for_program(case_id, &program)?;
    Ok(TassadarValidationCase {
        case_id: String::from(case_id),
        summary: format!("evaluation-independence audit reconstruction for `{case_id}`"),
        program,
        expected_trace: execution.steps,
        expected_outputs: execution.outputs,
    })
}

fn solved_sudoku_v0_grid(
) -> Result<[i32; SUDOKU_V0_CELL_COUNT], TassadarArticleEvaluationIndependenceAuditError> {
    let outputs = tassadar_sudoku_v0_corpus()
        .into_iter()
        .next()
        .ok_or_else(
            || TassadarArticleEvaluationIndependenceAuditError::Invariant {
                detail: String::from("sudoku_v0 corpus should not be empty"),
            },
        )?
        .validation_case
        .expected_outputs;
    outputs.try_into().map_err(
        |_| TassadarArticleEvaluationIndependenceAuditError::Invariant {
            detail: String::from("sudoku_v0 solved grid should have 16 cells"),
        },
    )
}

fn solved_sudoku_9x9_grid(
) -> Result<[i32; SUDOKU_9X9_CELL_COUNT], TassadarArticleEvaluationIndependenceAuditError> {
    let outputs = tassadar_sudoku_9x9_corpus()
        .into_iter()
        .next()
        .ok_or_else(
            || TassadarArticleEvaluationIndependenceAuditError::Invariant {
                detail: String::from("sudoku_9x9 corpus should not be empty"),
            },
        )?
        .validation_case
        .expected_outputs;
    outputs.try_into().map_err(
        |_| TassadarArticleEvaluationIndependenceAuditError::Invariant {
            detail: String::from("sudoku_9x9 solved grid should have 81 cells"),
        },
    )
}

fn base_hungarian_10x10_matrix(
) -> Result<[i32; HUNGARIAN_10X10_MATRIX_CELL_COUNT], TassadarArticleEvaluationIndependenceAuditError>
{
    let values = tassadar_hungarian_10x10_corpus()
        .into_iter()
        .next()
        .ok_or_else(
            || TassadarArticleEvaluationIndependenceAuditError::Invariant {
                detail: String::from("hungarian_10x10 corpus should not be empty"),
            },
        )?
        .cost_matrix;
    values.try_into().map_err(
        |_| TassadarArticleEvaluationIndependenceAuditError::Invariant {
            detail: String::from("hungarian_10x10 base matrix should have 100 cells"),
        },
    )
}

fn masked_grid<const N: usize>(
    mut solved_grid: [i32; N],
    masked_indices: &[usize],
    label: &str,
) -> Result<[i32; N], TassadarArticleEvaluationIndependenceAuditError> {
    for index in masked_indices {
        if *index >= N {
            return Err(TassadarArticleEvaluationIndependenceAuditError::Invariant {
                detail: format!("{label} used out-of-range masked index {index}"),
            });
        }
        solved_grid[*index] = 0;
    }
    Ok(solved_grid)
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
) -> Result<Vec<usize>, TassadarArticleEvaluationIndependenceAuditError> {
    if len == 0 {
        return Err(TassadarArticleEvaluationIndependenceAuditError::Invariant {
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
            matrix[row * HUNGARIAN_10X10_DIM + column] = base_matrix
                [source_row * HUNGARIAN_10X10_DIM + source_column]
                + row_biases[row]
                + column_biases[column];
        }
    }
    matrix
}

fn next_lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}

fn fixture_execution_for_program(
    case_id: &str,
    program: &TassadarProgram,
) -> Result<TassadarExecution, TassadarArticleEvaluationIndependenceAuditError> {
    let runner = TassadarFixtureRunner::for_program(program).map_err(|error| {
        TassadarArticleEvaluationIndependenceAuditError::FixtureExecution {
            case_id: String::from(case_id),
            detail: error.to_string(),
        }
    })?;
    runner.execute(program).map_err(|error| {
        TassadarArticleEvaluationIndependenceAuditError::FixtureExecution {
            case_id: String::from(case_id),
            detail: error.to_string(),
        }
    })
}

fn fixture_execution_for_case(
    case: &TassadarValidationCase,
) -> Result<TassadarExecution, TassadarArticleEvaluationIndependenceAuditError> {
    fixture_execution_for_program(case.case_id.as_str(), &case.program)
}

fn range_from_values(values: &[usize]) -> [usize; 2] {
    [
        values.iter().copied().min().unwrap_or(0),
        values.iter().copied().max().unwrap_or(0),
    ]
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

pub fn tassadar_article_evaluation_independence_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_REPORT_REF)
}

pub fn write_tassadar_article_evaluation_independence_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleEvaluationIndependenceAuditReport,
    TassadarArticleEvaluationIndependenceAuditError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleEvaluationIndependenceAuditError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_evaluation_independence_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleEvaluationIndependenceAuditError::Write {
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
) -> Result<T, TassadarArticleEvaluationIndependenceAuditError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarArticleEvaluationIndependenceAuditError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleEvaluationIndependenceAuditError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarArticleEvaluationIndependenceAuditError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarArticleEvaluationIndependenceAuditError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleEvaluationIndependenceAuditError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_evaluation_case_rows, build_report_from_inputs,
        build_tassadar_article_evaluation_independence_audit_report, build_training_case_rows,
        read_json, tassadar_article_evaluation_independence_audit_report_path,
        write_tassadar_article_evaluation_independence_audit_report,
        TassadarArticleEvaluationIndependenceAuditReport,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        read_tassadar_article_transformer_weight_lineage_contract,
        TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
    };
    use psionic_models::TassadarArticleTransformer;

    #[test]
    fn evaluation_independence_audit_is_green_without_final_green() {
        let report = build_tassadar_article_evaluation_independence_audit_report().expect("report");

        assert_eq!(report.training_case_rows.len(), 4);
        assert_eq!(report.evaluation_case_rows.len(), 6);
        assert!(report.generalization_prerequisite_review.passed);
        assert!(report.training_lineage_review.passed);
        assert!(report.exclusion_manifest.passed);
        assert!(report.near_duplicate_review.passed);
        assert!(report.generator_overlap_audit.passed);
        assert!(report.feature_distribution_review.passed);
        assert!(report.evaluation_independence_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn evaluation_independence_audit_turns_red_on_exact_overlap() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let generalization_report =
            super::read_repo_json(TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF)
                .expect("generalization report");
        let lineage_contract = read_tassadar_article_transformer_weight_lineage_contract(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
        )
        .expect("lineage contract");
        let model = TassadarArticleTransformer::trained_trace_domain_reference().expect("model");
        let training_case_rows =
            build_training_case_rows(&lineage_contract, &model).expect("training case rows");
        let mut evaluation_case_rows =
            build_evaluation_case_rows(&lineage_contract, &model).expect("evaluation case rows");
        let row = evaluation_case_rows
            .first_mut()
            .expect("first evaluation case row");
        row.exact_source_token_overlap = true;
        row.detail = String::from("synthetic exact-overlap red path");

        let report = build_report_from_inputs(
            acceptance_gate_report,
            generalization_report,
            lineage_contract,
            training_case_rows,
            evaluation_case_rows,
        );

        assert!(!report.exclusion_manifest.passed);
        assert!(!report.evaluation_independence_green);
    }

    #[test]
    fn evaluation_independence_audit_matches_committed_truth() {
        let generated =
            build_tassadar_article_evaluation_independence_audit_report().expect("report");
        let committed: TassadarArticleEvaluationIndependenceAuditReport =
            read_json(tassadar_article_evaluation_independence_audit_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_evaluation_independence_audit_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_evaluation_independence_audit_report.json");
        let written = write_tassadar_article_evaluation_independence_audit_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleEvaluationIndependenceAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_evaluation_independence_audit_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_evaluation_independence_audit_report.json")
        );
    }
}
