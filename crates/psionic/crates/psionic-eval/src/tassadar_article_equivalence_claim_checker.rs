use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_equivalence_blocker_matrix_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleEquivalenceBlockerMatrixReport,
    TassadarArticleEquivalenceBlockerMatrixReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
};

pub const TASSADAR_ARTICLE_EQUIVALENCE_CLAIM_CHECKER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_claim_checker_report.json";
pub const TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-article-equivalence-final-audit.sh";

const CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_canonical_transformer_stack_boundary_report.json";
const ARTICLE_TRANSFORMER_ARTIFACT_DESCRIPTOR_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_artifact_descriptor_report.json";
const ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_weight_lineage_report.json";
const ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fixture_transformer_parity_report.json";
const ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_reference_linear_exactness_gate_report.json";
const ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_generalization_gate_report.json";
const ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_evaluation_independence_audit_report.json";
const ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_representation_invariance_gate_report.json";
const ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_frontend_compiler_envelope_report.json";
const ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json";
const ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_demo_frontend_parity_report.json";
const ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_interpreter_breadth_suite_gate_report.json";
const ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_architecture_selection_report.json";
const ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_exactness_report.json";
const ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_throughput_floor_report.json";
const ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_demo_benchmark_equivalence_gate_report.json";
const ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_single_run_no_spill_closure_report.json";
const ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_interpreter_ownership_gate_report.json";
const ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_kv_activation_discipline_audit_report.json";
const ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_route_minimality_audit_report.json";
const ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_cross_machine_reproducibility_matrix_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleEquivalenceClaimCategory {
    Mechanistic,
    Behavioral,
    Operational,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceClaimCheckerRow {
    pub prerequisite_id: String,
    pub category: TassadarArticleEquivalenceClaimCategory,
    pub report_ref: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceCanonicalIdentityReview {
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_decode_mode: String,
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceExclusionReview {
    pub excluded_capability_ids: Vec<String>,
    pub optional_open_issue_ids: Vec<String>,
    pub general_compute_widening_allowed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceClaimCheckerReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub blocker_matrix_report_ref: String,
    pub blocker_matrix_report: TassadarArticleEquivalenceBlockerMatrixReport,
    pub acceptance_gate_report_ref: String,
    pub acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    pub prerequisite_rows: Vec<TassadarArticleEquivalenceClaimCheckerRow>,
    pub green_prerequisite_ids: Vec<String>,
    pub failed_prerequisite_ids: Vec<String>,
    pub mechanistic_verdict_green: bool,
    pub behavioral_verdict_green: bool,
    pub operational_verdict_green: bool,
    pub canonical_identity_review: TassadarArticleEquivalenceCanonicalIdentityReview,
    pub exclusion_review: TassadarArticleEquivalenceExclusionReview,
    pub public_article_equivalence_claim_allowed: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleEquivalenceClaimCheckerError {
    #[error(transparent)]
    BlockerMatrix(#[from] TassadarArticleEquivalenceBlockerMatrixReportError),
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
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
    #[error("missing `{path}` in `{report_ref}`")]
    MissingPath { report_ref: String, path: String },
    #[error("expected `{path}` in `{report_ref}` to be a boolean")]
    InvalidBool { report_ref: String, path: String },
    #[error("expected `{path}` in `{report_ref}` to be a string")]
    InvalidString { report_ref: String, path: String },
    #[error("expected `{path}` in `{report_ref}` to be an array of strings")]
    InvalidStringArray { report_ref: String, path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_equivalence_claim_checker_report(
) -> Result<TassadarArticleEquivalenceClaimCheckerReport, TassadarArticleEquivalenceClaimCheckerError>
{
    let blocker_matrix_report = build_tassadar_article_equivalence_blocker_matrix_report()?;
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;

    let boundary_report = read_repo_json(CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF)?;
    let artifact_descriptor_report =
        read_repo_json(ARTICLE_TRANSFORMER_ARTIFACT_DESCRIPTOR_REPORT_REF)?;
    let weight_lineage_report = read_repo_json(ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF)?;
    let fixture_parity_report = read_repo_json(ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF)?;
    let reference_linear_report =
        read_repo_json(ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF)?;
    let generalization_report = read_repo_json(ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF)?;
    let evaluation_independence_report =
        read_repo_json(ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_REPORT_REF)?;
    let invariance_report = read_repo_json(ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF)?;
    let frontend_envelope_report = read_repo_json(ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF)?;
    let frontend_corpus_report = read_repo_json(ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF)?;
    let demo_frontend_report = read_repo_json(ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF)?;
    let breadth_suite_report = read_repo_json(ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF)?;
    let fast_route_selection_report =
        read_repo_json(ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)?;
    let fast_route_exactness_report = read_repo_json(ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF)?;
    let fast_route_throughput_report =
        read_repo_json(ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF)?;
    let demo_benchmark_report = read_repo_json(ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF)?;
    let single_run_report = read_repo_json(ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF)?;
    let interpreter_ownership_report =
        read_repo_json(ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF)?;
    let kv_activation_report = read_repo_json(ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF)?;
    let route_minimality_report = read_repo_json(ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF)?;
    let cross_machine_report =
        read_repo_json(ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF)?;

    let prerequisite_rows = vec![
        prerequisite_row(
            "owned_transformer_stack_boundary",
            TassadarArticleEquivalenceClaimCategory::Mechanistic,
            CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
            bool_at(&boundary_report, CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF, &["boundary_contract_green"])?,
            "the canonical owned article route remains bound to the declared `psionic-transformer` stack boundary",
        ),
        prerequisite_row(
            "non_fixture_article_model_artifact",
            TassadarArticleEquivalenceClaimCategory::Mechanistic,
            ARTICLE_TRANSFORMER_ARTIFACT_DESCRIPTOR_REPORT_REF,
            bool_at(
                &artifact_descriptor_report,
                ARTICLE_TRANSFORMER_ARTIFACT_DESCRIPTOR_REPORT_REF,
                &["artifact_descriptor_contract_green"],
            )? && bool_at(
                &weight_lineage_report,
                ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
                &["weight_lineage_contract_green"],
            )?,
            "the canonical article model and weight artifact are non-fixture, artifact-backed, and lineage-bound",
        ),
        prerequisite_row(
            "reference_linear_direct_proof",
            TassadarArticleEquivalenceClaimCategory::Mechanistic,
            ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
            bool_at(
                &fixture_parity_report,
                ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
                &["replacement_certified"],
            )? && bool_at(
                &reference_linear_report,
                ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
                &["reference_linear_exactness_green"],
            )?,
            "the Transformer-backed reference-linear route owns the bounded direct-proof and exactness surface",
        ),
        prerequisite_row(
            "anti_memorization_and_independence",
            TassadarArticleEquivalenceClaimCategory::Mechanistic,
            ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
            bool_at(
                &generalization_report,
                ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
                &["generalization_green"],
            )? && bool_at(
                &evaluation_independence_report,
                ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_REPORT_REF,
                &["evaluation_independence_green"],
            )?,
            "held-out generalization, anti-memorization, and evaluation-independence gates remain green",
        ),
        prerequisite_row(
            "representation_invariance",
            TassadarArticleEquivalenceClaimCategory::Mechanistic,
            ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
            bool_at(
                &invariance_report,
                ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
                &["article_representation_invariance_green"],
            )? && bool_at(
                &invariance_report,
                ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
                &["trace_vocabulary_binding_green"],
            )?,
            "prompt/tokenization invariance and trace-vocabulary binding remain green on the canonical route",
        ),
        prerequisite_row(
            "frontend_compiler_gate",
            TassadarArticleEquivalenceClaimCategory::Behavioral,
            ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF,
            bool_at(
                &frontend_envelope_report,
                ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF,
                &["envelope_manifest_green"],
            )? && bool_at(
                &frontend_envelope_report,
                ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF,
                &["toolchain_identity_green"],
            )? && bool_at(
                &frontend_envelope_report,
                ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF,
                &["refusal_taxonomy_green"],
            )? && bool_at(
                &frontend_corpus_report,
                ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF,
                &["compile_matrix_green"],
            )? && bool_at(
                &frontend_corpus_report,
                ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF,
                &["category_coverage_green"],
            )? && bool_at(
                &frontend_corpus_report,
                ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF,
                &["envelope_alignment_green"],
            )? && bool_at(
                &demo_frontend_report,
                ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF,
                &["demo_frontend_parity_green"],
            )?,
            "the declared frontend/compiler envelope, corpus compile matrix, and demo-source parity stay green",
        ),
        prerequisite_row(
            "interpreter_breadth_gate",
            TassadarArticleEquivalenceClaimCategory::Behavioral,
            ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
            bool_at(
                &breadth_suite_report,
                ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
                &["breadth_gate_green"],
            )?,
            "the declared interpreter breadth suite remains green for the canonical article envelope",
        ),
        prerequisite_row(
            "fast_route_gate",
            TassadarArticleEquivalenceClaimCategory::Behavioral,
            ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
            bool_at(
                &fast_route_selection_report,
                ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                &["fast_route_selection_green"],
            )? && bool_at(
                &fast_route_exactness_report,
                ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF,
                &["exactness_green"],
            )? && bool_at(
                &fast_route_throughput_report,
                ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
                &["throughput_floor_green"],
            )?,
            "the selected HullCache fast route remains selected, exact, and under the declared throughput-floor contract",
        ),
        prerequisite_row(
            "demo_benchmark_gate",
            TassadarArticleEquivalenceClaimCategory::Behavioral,
            ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF,
            bool_at(
                &demo_benchmark_report,
                ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF,
                &["article_demo_benchmark_equivalence_gate_green"],
            )?,
            "the Hungarian demo, named Arto parity row, and hard-Sudoku benchmark suite remain jointly green",
        ),
        prerequisite_row(
            "single_run_operator_gate",
            TassadarArticleEquivalenceClaimCategory::Operational,
            ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
            bool_at(
                &single_run_report,
                ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
                &["single_run_no_spill_closure_green"],
            )?,
            "the canonical route still stays inside one no-resume same-run operator envelope",
        ),
        prerequisite_row(
            "interpreter_ownership_gate",
            TassadarArticleEquivalenceClaimCategory::Mechanistic,
            ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
            bool_at(
                &interpreter_ownership_report,
                ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
                &["interpreter_ownership_green"],
            )?,
            "the stronger interpreter-in-weights ownership reading remains green on the canonical route",
        ),
        prerequisite_row(
            "kv_activation_discipline_verdict",
            TassadarArticleEquivalenceClaimCategory::Mechanistic,
            ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
            bool_at(
                &kv_activation_report,
                ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
                &["kv_activation_discipline_green"],
            )?,
            "acceptable versus non-acceptable state carriers remain explicit and green",
        ),
        prerequisite_row(
            "route_minimality_and_publication_verdict",
            TassadarArticleEquivalenceClaimCategory::Operational,
            ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
            bool_at(
                &route_minimality_report,
                ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
                &["route_minimality_audit_green"],
            )? && bool_at(
                &route_minimality_report,
                ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
                &["public_verdict_review", "public_verdict_green"],
            )?,
            "the canonical claim route stays minimal and its bounded public verdict is no longer suppressed",
        ),
        prerequisite_row(
            "cross_machine_reproducibility_matrix",
            TassadarArticleEquivalenceClaimCategory::Operational,
            ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
            bool_at(
                &cross_machine_report,
                ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
                &["reproducibility_matrix_green"],
            )? && bool_at(
                &cross_machine_report,
                ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
                &["deterministic_mode_green"],
            )? && bool_at(
                &cross_machine_report,
                ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
                &["throughput_floor_stability_green"],
            )?,
            "the declared machine matrix remains deterministic and stable on the canonical route",
        ),
        prerequisite_row(
            "final_acceptance_gate",
            TassadarArticleEquivalenceClaimCategory::Operational,
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            acceptance_gate_report.acceptance_status == TassadarArticleEquivalenceAcceptanceStatus::Green
                && acceptance_gate_report.article_equivalence_green
                && acceptance_gate_report.blocked_issue_ids.is_empty()
                && acceptance_gate_report.blocked_blocker_ids.is_empty(),
            "the frozen acceptance gate has turned fully green with no remaining required blocker or issue rows",
        ),
    ];

    let green_prerequisite_ids = prerequisite_rows
        .iter()
        .filter(|row| row.green)
        .map(|row| row.prerequisite_id.clone())
        .collect::<Vec<_>>();
    let failed_prerequisite_ids = prerequisite_rows
        .iter()
        .filter(|row| !row.green)
        .map(|row| row.prerequisite_id.clone())
        .collect::<Vec<_>>();

    let mechanistic_verdict_green = prerequisite_rows
        .iter()
        .filter(|row| row.category == TassadarArticleEquivalenceClaimCategory::Mechanistic)
        .all(|row| row.green);
    let behavioral_verdict_green = prerequisite_rows
        .iter()
        .filter(|row| row.category == TassadarArticleEquivalenceClaimCategory::Behavioral)
        .all(|row| row.green);
    let operational_verdict_green = prerequisite_rows
        .iter()
        .filter(|row| row.category == TassadarArticleEquivalenceClaimCategory::Operational)
        .all(|row| row.green);

    let canonical_identity_review = TassadarArticleEquivalenceCanonicalIdentityReview {
        canonical_model_id: string_at(
            &route_minimality_report,
            ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
            &["canonical_claim_route_review", "transformer_model_id"],
        )?,
        canonical_weight_artifact_id: string_at(
            &weight_lineage_report,
            ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
            &[
                "lineage_contract",
                "produced_model_artifact_binding",
                "artifact_id",
            ],
        )?,
        canonical_weight_bundle_digest: string_at(
            &weight_lineage_report,
            ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
            &[
                "lineage_contract",
                "produced_model_artifact_binding",
                "weight_bundle_digest",
            ],
        )?,
        canonical_weight_primary_artifact_sha256: string_at(
            &weight_lineage_report,
            ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
            &[
                "lineage_contract",
                "produced_model_artifact_binding",
                "primary_artifact_sha256",
            ],
        )?,
        canonical_route_id: string_at(
            &route_minimality_report,
            ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
            &["canonical_claim_route_review", "canonical_claim_route_id"],
        )?,
        canonical_route_descriptor_digest: string_at(
            &route_minimality_report,
            ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
            &[
                "canonical_claim_route_review",
                "projected_route_descriptor_digest",
            ],
        )?,
        canonical_decode_mode: string_at(
            &route_minimality_report,
            ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
            &["canonical_claim_route_review", "selected_decode_mode"],
        )?,
        current_host_machine_class_id: string_at(
            &cross_machine_report,
            ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
            &["machine_matrix_review", "current_host_machine_class_id"],
        )?,
        supported_machine_class_ids: string_array_at(
            &cross_machine_report,
            ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
            &["machine_matrix_review", "supported_machine_class_ids"],
        )?,
        detail: format!(
            "canonical_model_id=`{}` canonical_weight_artifact_id=`{}` canonical_route_id=`{}` canonical_decode_mode=`{}` supported_machine_classes={}",
            string_at(
                &route_minimality_report,
                ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
                &["canonical_claim_route_review", "transformer_model_id"],
            )?,
            string_at(
                &weight_lineage_report,
                ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
                &[
                    "lineage_contract",
                    "produced_model_artifact_binding",
                    "artifact_id",
                ],
            )?,
            string_at(
                &route_minimality_report,
                ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
                &["canonical_claim_route_review", "canonical_claim_route_id"],
            )?,
            string_at(
                &route_minimality_report,
                ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
                &["canonical_claim_route_review", "selected_decode_mode"],
            )?,
            string_array_at(
                &cross_machine_report,
                ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
                &["machine_matrix_review", "supported_machine_class_ids"],
            )?
            .len(),
        ),
    };

    let exclusion_review = TassadarArticleEquivalenceExclusionReview {
        excluded_capability_ids: vec![
            String::from("arbitrary_c_ingress_outside_declared_article_envelope"),
            String::from("arbitrary_wasm_ingress_outside_declared_article_envelope"),
            String::from("planner_or_hybrid_route_as_canonical_public_article_route"),
            String::from("stochastic_execution_on_canonical_article_route"),
            String::from("generic_interpreter_in_weights_claim_outside_declared_article_envelope"),
            String::from("minimal_transformer_size_research_question"),
            String::from("post_article_universality_and_turing_completeness_rebase"),
        ],
        optional_open_issue_ids: acceptance_gate_report.optional_open_issue_ids.clone(),
        general_compute_widening_allowed: false,
        detail: format!(
            "excluded_capability_count={} optional_open_issue_count={} general_compute_widening_allowed=false",
            7,
            acceptance_gate_report.optional_open_issue_ids.len(),
        ),
    };

    let public_article_equivalence_claim_allowed = acceptance_gate_report.public_claim_allowed
        && mechanistic_verdict_green
        && behavioral_verdict_green
        && operational_verdict_green
        && failed_prerequisite_ids.is_empty();
    let article_equivalence_green = blocker_matrix_report.article_equivalence_green
        && acceptance_gate_report.article_equivalence_green
        && public_article_equivalence_claim_allowed;

    let mut report = TassadarArticleEquivalenceClaimCheckerReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_equivalence.claim_checker.report.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_CHECKER_REF),
        blocker_matrix_report_ref: String::from(TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF),
        blocker_matrix_report,
        acceptance_gate_report_ref: String::from(TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF),
        acceptance_gate_report,
        prerequisite_rows,
        green_prerequisite_ids,
        failed_prerequisite_ids,
        mechanistic_verdict_green,
        behavioral_verdict_green,
        operational_verdict_green,
        canonical_identity_review,
        exclusion_review,
        public_article_equivalence_claim_allowed,
        article_equivalence_green,
        claim_boundary: String::from(
            "this claim checker closes bounded article equivalence only for the declared `psionic-transformer` article route, the canonical trained trace-bound article model and weight artifact, the direct HullCache claim route, and the declared host CPU machine matrix. It does not widen public claims to arbitrary C ingress, arbitrary Wasm ingress, stochastic execution, planner-mediated canonical routes, generic interpreter-in-weights claims outside the declared article envelope, or the post-article universality bridge.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article-equivalence claim checker now records green_prerequisites={}/{}, mechanistic_verdict_green={}, behavioral_verdict_green={}, operational_verdict_green={}, optional_open_issues={}, public_article_equivalence_claim_allowed={}, and article_equivalence_green={}.",
        report.green_prerequisite_ids.len(),
        report.prerequisite_rows.len(),
        report.mechanistic_verdict_green,
        report.behavioral_verdict_green,
        report.operational_verdict_green,
        report.exclusion_review.optional_open_issue_ids.len(),
        report.public_article_equivalence_claim_allowed,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_equivalence_claim_checker_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_article_equivalence_claim_checker_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_EQUIVALENCE_CLAIM_CHECKER_REPORT_REF)
}

pub fn write_tassadar_article_equivalence_claim_checker_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleEquivalenceClaimCheckerReport, TassadarArticleEquivalenceClaimCheckerError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleEquivalenceClaimCheckerError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_equivalence_claim_checker_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleEquivalenceClaimCheckerError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn prerequisite_row(
    prerequisite_id: &str,
    category: TassadarArticleEquivalenceClaimCategory,
    report_ref: &str,
    green: bool,
    detail: &str,
) -> TassadarArticleEquivalenceClaimCheckerRow {
    TassadarArticleEquivalenceClaimCheckerRow {
        prerequisite_id: String::from(prerequisite_id),
        category,
        report_ref: String::from(report_ref),
        green,
        detail: String::from(detail),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn read_repo_json(
    relative_path: &str,
) -> Result<Value, TassadarArticleEquivalenceClaimCheckerError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleEquivalenceClaimCheckerError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleEquivalenceClaimCheckerError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn bool_at(
    value: &Value,
    report_ref: &str,
    path: &[&str],
) -> Result<bool, TassadarArticleEquivalenceClaimCheckerError> {
    let joined = path.join(".");
    value_at_path(value, path)
        .ok_or_else(
            || TassadarArticleEquivalenceClaimCheckerError::MissingPath {
                report_ref: String::from(report_ref),
                path: joined.clone(),
            },
        )?
        .as_bool()
        .ok_or_else(
            || TassadarArticleEquivalenceClaimCheckerError::InvalidBool {
                report_ref: String::from(report_ref),
                path: joined,
            },
        )
}

fn string_at(
    value: &Value,
    report_ref: &str,
    path: &[&str],
) -> Result<String, TassadarArticleEquivalenceClaimCheckerError> {
    let joined = path.join(".");
    value_at_path(value, path)
        .ok_or_else(
            || TassadarArticleEquivalenceClaimCheckerError::MissingPath {
                report_ref: String::from(report_ref),
                path: joined.clone(),
            },
        )?
        .as_str()
        .map(String::from)
        .ok_or_else(
            || TassadarArticleEquivalenceClaimCheckerError::InvalidString {
                report_ref: String::from(report_ref),
                path: joined,
            },
        )
}

fn string_array_at(
    value: &Value,
    report_ref: &str,
    path: &[&str],
) -> Result<Vec<String>, TassadarArticleEquivalenceClaimCheckerError> {
    let joined = path.join(".");
    let array = value_at_path(value, path)
        .ok_or_else(
            || TassadarArticleEquivalenceClaimCheckerError::MissingPath {
                report_ref: String::from(report_ref),
                path: joined.clone(),
            },
        )?
        .as_array()
        .ok_or_else(
            || TassadarArticleEquivalenceClaimCheckerError::InvalidStringArray {
                report_ref: String::from(report_ref),
                path: joined.clone(),
            },
        )?;
    array
        .iter()
        .map(|value| {
            value.as_str().map(String::from).ok_or_else(|| {
                TassadarArticleEquivalenceClaimCheckerError::InvalidStringArray {
                    report_ref: String::from(report_ref),
                    path: joined.clone(),
                }
            })
        })
        .collect()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;

    use super::{
        build_tassadar_article_equivalence_claim_checker_report,
        tassadar_article_equivalence_claim_checker_report_path,
        write_tassadar_article_equivalence_claim_checker_report,
        TassadarArticleEquivalenceClaimCheckerReport,
        TASSADAR_ARTICLE_EQUIVALENCE_CLAIM_CHECKER_REPORT_REF,
    };

    #[test]
    fn article_equivalence_claim_checker_is_green_and_bounded() {
        let report = build_tassadar_article_equivalence_claim_checker_report().expect("report");

        assert!(report.blocker_matrix_report.article_equivalence_green);
        assert!(report.acceptance_gate_report.article_equivalence_green);
        assert!(report.failed_prerequisite_ids.is_empty());
        assert!(report.mechanistic_verdict_green);
        assert!(report.behavioral_verdict_green);
        assert!(report.operational_verdict_green);
        assert_eq!(
            report.canonical_identity_review.canonical_route_id.as_str(),
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(
            report
                .canonical_identity_review
                .canonical_decode_mode
                .as_str(),
            "hull_cache"
        );
        assert_eq!(
            report
                .canonical_identity_review
                .supported_machine_class_ids
                .len(),
            2
        );
        assert_eq!(
            report.exclusion_review.optional_open_issue_ids,
            vec![String::from("TAS-R1")]
        );
        assert!(report.public_article_equivalence_claim_allowed);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_equivalence_claim_checker_matches_committed_truth() {
        let generated = build_tassadar_article_equivalence_claim_checker_report().expect("report");
        let committed: TassadarArticleEquivalenceClaimCheckerReport =
            read_json(tassadar_article_equivalence_claim_checker_report_path()).expect("committed");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_ARTICLE_EQUIVALENCE_CLAIM_CHECKER_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_article_equivalence_claim_checker_report.json"
        );
    }

    #[test]
    fn write_article_equivalence_claim_checker_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_equivalence_claim_checker_report.json");
        let written =
            write_tassadar_article_equivalence_claim_checker_report(&output_path).expect("write");
        let persisted: TassadarArticleEquivalenceClaimCheckerReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_equivalence_claim_checker_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_equivalence_claim_checker_report.json")
        );
    }

    fn read_json<T: DeserializeOwned>(path: PathBuf) -> Result<T, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }
}
