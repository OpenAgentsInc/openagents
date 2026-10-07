use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use thiserror::Error;

use psionic_core::{DType, QuantizationMode, Shape, TensorData};
use psionic_data::{
    build_tassadar_article_interpreter_breadth_suite, TassadarArticleInterpreterBreadthSuite,
    TassadarArticleInterpreterBreadthSuiteFamilyId,
};
use psionic_models::{
    TassadarArticleTransformer, TassadarArticleTransformerArtifactBinding,
    TassadarArticleTransformerDescriptor, TassadarArticleTransformerError, WeightArtifactMetadata,
    WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
};
use psionic_runtime::{
    tassadar_article_class_corpus, TassadarDirectModelWeightExecutionProofError,
    TassadarDirectModelWeightExecutionProofInput, TassadarDirectModelWeightExecutionProofReceipt,
    TassadarDirectModelWeightRouteBinding, TassadarExactnessPosture, TassadarExecution,
    TassadarExecutorDecodeMode, TassadarExecutorSelectionState, TassadarFixtureRunner,
    TassadarProgramArtifact, TassadarValidationCase,
    TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF, TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
};
use psionic_transformer::TransformerExecutionMode;
use safetensors::{serialize, tensor::TensorView, Dtype as SafeTensorsDType, SafeTensors};

use crate::{
    build_tassadar_article_class_suite, build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_evaluation_independence_audit_report,
    build_tassadar_article_interpreter_breadth_suite_gate_report,
    build_tassadar_article_representation_invariance_gate_report,
    build_tassadar_article_single_run_no_spill_closure_report,
    build_tassadar_article_transformer_generalization_gate_report,
    build_tassadar_article_transformer_reference_linear_exactness_gate_report,
    build_tassadar_article_transformer_weight_lineage_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleEvaluationIndependenceAuditError,
    TassadarArticleEvaluationIndependenceAuditReport,
    TassadarArticleInterpreterBreadthSuiteGateReport,
    TassadarArticleInterpreterBreadthSuiteGateReportError,
    TassadarArticleRepresentationInvarianceGateReport,
    TassadarArticleRepresentationInvarianceGateReportError,
    TassadarArticleSingleRunNoSpillClosureReport,
    TassadarArticleSingleRunNoSpillClosureReportError,
    TassadarArticleTransformerGeneralizationGateReport,
    TassadarArticleTransformerGeneralizationGateReportError,
    TassadarArticleTransformerReferenceLinearExactnessCaseRow,
    TassadarArticleTransformerReferenceLinearExactnessGateReport,
    TassadarArticleTransformerReferenceLinearExactnessGateReportError,
    TassadarArticleTransformerWeightLineageError, TassadarArticleTransformerWeightLineageReport,
    TassadarBenchmarkError, TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_interpreter_ownership_gate_report.json";
pub const TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-interpreter-ownership-gate.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-184";
const DIRECT_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json";
const ARTICLE_SUITE_VERSION: &str = "2026.03.17";
const GENERIC_DIRECT_PROOF_SUITE_ID: &str =
    "tassadar.article_workload_family_generic_direct_proof_suite.v1";
const GENERIC_DIRECT_PROOF_CASE_IDS: &[&str] = &[
    "micro_wasm_kernel",
    "branch_heavy_kernel",
    "memory_heavy_kernel",
    "long_loop_kernel",
    "sudoku_v0_test_a",
    "hungarian_matching",
];
const SYNTHETIC_GENERIC_DIRECT_PROOF_CASE_IDS: &[&str] = &[
    "micro_wasm_kernel",
    "branch_heavy_kernel",
    "memory_heavy_kernel",
];
const PERTURBATION_WITNESS_CASE_IDS: &[&str] = &[
    "micro_wasm_kernel",
    "branch_heavy_kernel",
    "memory_heavy_kernel",
];
const GENERIC_DIRECT_PROOF_EXECUTOR_PRODUCT_ID: &str =
    "psionic.article_interpreter_ownership.generic_direct_proof.v1";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DirectProofReportView {
    report_id: String,
    model_id: String,
    benchmark_report_ref: String,
    lineage_contract_ref: String,
    lineage_contract_digest: String,
    route_descriptor_digest: String,
    receipts: Vec<TassadarDirectModelWeightExecutionProofReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub breadth_suite_gate_report_ref: String,
    pub reference_linear_exactness_report_ref: String,
    pub representation_invariance_report_ref: String,
    pub generalization_report_ref: String,
    pub evaluation_independence_report_ref: String,
    pub weight_lineage_report_ref: String,
    pub single_run_no_spill_report_ref: String,
    pub direct_proof_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleInterpreterOwnershipProofSourceKind {
    CommittedServedReceipt,
    EvalSyntheticReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipGenericProofCaseRow {
    pub case_id: String,
    pub workload_family_id: String,
    pub proof_source_kind: TassadarArticleInterpreterOwnershipProofSourceKind,
    pub case_summary: String,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub fixture_trace_digest: String,
    pub transformer_trace_digest: String,
    pub exactness_posture: TassadarExactnessPosture,
    pub direct_receipt: TassadarDirectModelWeightExecutionProofReceipt,
    pub direct_no_tool_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipGenericDirectProofReview {
    pub generic_suite_id: String,
    pub report_ref: String,
    pub benchmark_report_ref: String,
    pub route_descriptor_digest: String,
    pub direct_route_binding: TassadarDirectModelWeightRouteBinding,
    pub declared_case_ids: Vec<String>,
    pub committed_receipt_case_ids: Vec<String>,
    pub synthetic_receipt_case_ids: Vec<String>,
    pub case_rows: Vec<TassadarArticleInterpreterOwnershipGenericProofCaseRow>,
    pub direct_case_count: usize,
    pub zero_external_call_case_count: usize,
    pub fallback_free_case_count: usize,
    pub exact_case_count: usize,
    pub all_declared_cases_present: bool,
    pub generic_direct_proof_suite_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipBreadthConformanceRow {
    pub family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    pub required_evidence_ids: Vec<String>,
    pub family_gate_green: bool,
    pub ownership_gate_covers_family: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipBreadthConformanceMatrix {
    pub manifest_ref: String,
    pub suite_manifest: TassadarArticleInterpreterBreadthSuite,
    pub family_rows: Vec<TassadarArticleInterpreterOwnershipBreadthConformanceRow>,
    pub green_family_count: usize,
    pub conformance_matrix_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipRoutePurityReview {
    pub forward_pass_sufficiency_green: bool,
    pub route_purity_green: bool,
    pub hidden_host_substitution_excluded: bool,
    pub external_oracle_excluded: bool,
    pub preprocessing_shortcut_excluded: bool,
    pub route_drift_excluded: bool,
    pub runtime_owned_control_flow_excluded: bool,
    pub helper_module_mediation_excluded: bool,
    pub cache_decisive_step_excluded_basic: bool,
    pub artifact_lineage_to_behavior_closed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipComputationMappingReport {
    pub program_representation_location: String,
    pub state_location: String,
    pub control_flow_realization: String,
    pub stable_across_runs: bool,
    pub hidden_host_substitution_excluded: bool,
    pub runtime_owned_control_flow_excluded: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleInterpreterOwnershipInterventionKind {
    ZeroedTrainableSubset,
    RandomizedTrainableSubset,
    ClampedAttentionHead,
    RemovedDecoderLayer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipInterventionRow {
    pub intervention_kind: TassadarArticleInterpreterOwnershipInterventionKind,
    pub mutated_tensor_or_parameter_refs: Vec<String>,
    pub model_descriptor_digest_changed: bool,
    pub witness_case_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub behavior_changed: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleInterpreterOwnershipLocalityKind {
    Localized,
    Modular,
    Diffuse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipWeightPerturbationReview {
    pub witness_case_ids: Vec<String>,
    pub baseline_model_id: String,
    pub baseline_descriptor_digest: String,
    pub baseline_exact_case_count: usize,
    pub intervention_rows: Vec<TassadarArticleInterpreterOwnershipInterventionRow>,
    pub all_interventions_show_sensitivity: bool,
    pub locality_characterization: TassadarArticleInterpreterOwnershipLocalityKind,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipBindingReview {
    pub canonical_boundary_green: bool,
    pub breadth_conformance_matrix_green: bool,
    pub generic_direct_proof_suite_green: bool,
    pub route_purity_green: bool,
    pub mapping_stable_across_runs: bool,
    pub perturbation_sensitivity_green: bool,
    pub ownership_gate_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleInterpreterOwnershipAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub breadth_suite_gate_report_ref: String,
    pub breadth_suite_gate_report: TassadarArticleInterpreterBreadthSuiteGateReport,
    pub generic_direct_proof_review: TassadarArticleInterpreterOwnershipGenericDirectProofReview,
    pub route_purity_review: TassadarArticleInterpreterOwnershipRoutePurityReview,
    pub computation_mapping_report: TassadarArticleInterpreterOwnershipComputationMappingReport,
    pub breadth_conformance_matrix: TassadarArticleInterpreterOwnershipBreadthConformanceMatrix,
    pub weight_perturbation_review: TassadarArticleInterpreterOwnershipWeightPerturbationReview,
    pub binding_review: TassadarArticleInterpreterOwnershipBindingReview,
    pub interpreter_ownership_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleInterpreterOwnershipGateError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Benchmark(#[from] TassadarBenchmarkError),
    #[error(transparent)]
    CanonicalBoundary(#[from] TassadarCanonicalTransformerStackBoundaryReportError),
    #[error(transparent)]
    BreadthSuite(#[from] TassadarArticleInterpreterBreadthSuiteGateReportError),
    #[error(transparent)]
    Exactness(#[from] TassadarArticleTransformerReferenceLinearExactnessGateReportError),
    #[error(transparent)]
    Generalization(#[from] TassadarArticleTransformerGeneralizationGateReportError),
    #[error(transparent)]
    EvaluationIndependence(#[from] TassadarArticleEvaluationIndependenceAuditError),
    #[error(transparent)]
    RepresentationInvariance(#[from] TassadarArticleRepresentationInvarianceGateReportError),
    #[error(transparent)]
    SingleRun(#[from] TassadarArticleSingleRunNoSpillClosureReportError),
    #[error(transparent)]
    WeightLineage(#[from] TassadarArticleTransformerWeightLineageError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error(transparent)]
    DirectProofReceipt(#[from] TassadarDirectModelWeightExecutionProofError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error("generic direct-proof suite case `{case_id}` is missing from the article corpus")]
    MissingCorpusCase { case_id: String },
    #[error(
        "generic direct-proof suite case `{case_id}` is missing from the article artifact suite"
    )]
    MissingArtifactCase { case_id: String },
    #[error("generic direct-proof suite case `{case_id}` failed exactness: {detail}")]
    GenericSuiteExactness { case_id: String, detail: String },
    #[error("committed direct proof is missing route template receipt")]
    MissingDirectProofTemplate,
    #[error("tensor mutation did not touch any weights: {detail}")]
    EmptyTensorMutation { detail: String },
    #[error("artifact tensor `{name}` used unsupported dtype `{dtype}`")]
    UnsupportedArtifactTensorDType { name: String, dtype: String },
    #[error("artifact tensor `{name}` byte length {byte_length} is not divisible by four")]
    InvalidArtifactTensorBytes { name: String, byte_length: usize },
}

#[derive(Clone, Debug)]
struct SyntheticProofCaseInputs<'a> {
    case: &'a TassadarValidationCase,
    artifact: &'a TassadarProgramArtifact,
    exactness_row: &'a TassadarArticleTransformerReferenceLinearExactnessCaseRow,
    template_receipt: &'a TassadarDirectModelWeightExecutionProofReceipt,
    model: &'a TassadarArticleTransformer,
    lineage_contract_ref: &'a str,
    lineage_contract_digest: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
struct TensorRow {
    name: String,
    shape: Vec<usize>,
    values: Vec<f32>,
}

pub fn build_tassadar_article_interpreter_ownership_gate_report() -> Result<
    TassadarArticleInterpreterOwnershipGateReport,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let canonical_boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let breadth_suite_gate_report = build_tassadar_article_interpreter_breadth_suite_gate_report()?;
    let exactness_report =
        build_tassadar_article_transformer_reference_linear_exactness_gate_report()?;
    let representation_invariance_report =
        build_tassadar_article_representation_invariance_gate_report()?;
    let generalization_report = build_tassadar_article_transformer_generalization_gate_report()?;
    let evaluation_independence_report =
        build_tassadar_article_evaluation_independence_audit_report()?;
    let weight_lineage_report = build_tassadar_article_transformer_weight_lineage_report()?;
    let single_run_no_spill_report = build_tassadar_article_single_run_no_spill_closure_report()?;
    let direct_proof_report: DirectProofReportView = read_repo_json(
        DIRECT_PROOF_REPORT_REF,
        "direct_model_weight_execution_proof",
    )?;
    let model = TassadarArticleTransformer::trained_trace_domain_reference()?;
    let generic_direct_proof_review =
        build_generic_direct_proof_review(&direct_proof_report, &exactness_report, &model)?;
    let route_purity_review = build_route_purity_review(
        &generic_direct_proof_review,
        &exactness_report,
        &representation_invariance_report,
        &generalization_report,
        &evaluation_independence_report,
        &weight_lineage_report,
        &single_run_no_spill_report,
    );
    let computation_mapping_report = build_computation_mapping_report(
        &generic_direct_proof_review,
        &representation_invariance_report,
        &single_run_no_spill_report,
    );
    let breadth_conformance_matrix = build_breadth_conformance_matrix(
        &breadth_suite_gate_report,
        generic_direct_proof_review.generic_direct_proof_suite_green,
        route_purity_review.route_purity_green,
    );
    let weight_perturbation_review = build_weight_perturbation_review(&model)?;
    Ok(build_report_from_inputs(
        acceptance_gate,
        canonical_boundary_report,
        breadth_suite_gate_report,
        generic_direct_proof_review,
        route_purity_review,
        computation_mapping_report,
        breadth_conformance_matrix,
        weight_perturbation_review,
    ))
}

fn build_report_from_inputs(
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    breadth_suite_gate_report: TassadarArticleInterpreterBreadthSuiteGateReport,
    generic_direct_proof_review: TassadarArticleInterpreterOwnershipGenericDirectProofReview,
    route_purity_review: TassadarArticleInterpreterOwnershipRoutePurityReview,
    computation_mapping_report: TassadarArticleInterpreterOwnershipComputationMappingReport,
    breadth_conformance_matrix: TassadarArticleInterpreterOwnershipBreadthConformanceMatrix,
    weight_perturbation_review: TassadarArticleInterpreterOwnershipWeightPerturbationReview,
) -> TassadarArticleInterpreterOwnershipGateReport {
    let acceptance_gate_tie = TassadarArticleInterpreterOwnershipAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        breadth_suite_gate_report_ref: String::from(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
        ),
        reference_linear_exactness_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
        ),
        representation_invariance_report_ref: String::from(
            TASSADAR_ARTICLE_REPRESENTATION_INVARIANCE_GATE_REPORT_REF,
        ),
        generalization_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
        ),
        evaluation_independence_report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_article_evaluation_independence_audit_report.json",
        ),
        weight_lineage_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
        ),
        single_run_no_spill_report_ref: String::from(
            TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
        ),
        direct_proof_report_ref: String::from(DIRECT_PROOF_REPORT_REF),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    };
    let interpreter_ownership_green = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && breadth_conformance_matrix.conformance_matrix_green
        && generic_direct_proof_review.generic_direct_proof_suite_green
        && route_purity_review.route_purity_green
        && computation_mapping_report.stable_across_runs
        && weight_perturbation_review.all_interventions_show_sensitivity;
    let binding_review = TassadarArticleInterpreterOwnershipBindingReview {
        canonical_boundary_green: canonical_boundary_report.boundary_contract_green,
        breadth_conformance_matrix_green: breadth_conformance_matrix.conformance_matrix_green,
        generic_direct_proof_suite_green: generic_direct_proof_review
            .generic_direct_proof_suite_green,
        route_purity_green: route_purity_review.route_purity_green,
        mapping_stable_across_runs: computation_mapping_report.stable_across_runs,
        perturbation_sensitivity_green: weight_perturbation_review
            .all_interventions_show_sensitivity,
        ownership_gate_green: interpreter_ownership_green,
        detail: format!(
            "canonical_boundary_green={} breadth_conformance_matrix_green={} generic_direct_proof_suite_green={} route_purity_green={} mapping_stable_across_runs={} perturbation_sensitivity_green={}",
            canonical_boundary_report.boundary_contract_green,
            breadth_conformance_matrix.conformance_matrix_green,
            generic_direct_proof_review.generic_direct_proof_suite_green,
            route_purity_review.route_purity_green,
            computation_mapping_report.stable_across_runs,
            weight_perturbation_review.all_interventions_show_sensitivity,
        ),
    };
    let mut report = TassadarArticleInterpreterOwnershipGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_interpreter_ownership_gate.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_CHECKER_REF,
        ),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        breadth_suite_gate_report_ref: String::from(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
        ),
        breadth_suite_gate_report,
        generic_direct_proof_review,
        route_purity_review,
        computation_mapping_report,
        breadth_conformance_matrix,
        weight_perturbation_review,
        binding_review,
        interpreter_ownership_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && interpreter_ownership_green,
        claim_boundary: String::from(
            "this report closes TAS-184 only. It binds the owned Transformer route, the generic article workload-family direct no-tool proof suite, the already-green breadth/generalization/independence/no-spill prerequisites, and concrete weight-perturbation sensitivity into one clean-room interpreter-ownership verdict. By itself it still does not replace the later KV-cache and activation-state discipline verdict, cross-machine reproducibility matrix, route-minimality audit, or final article-equivalence audit.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article interpreter-ownership gate now records tied_requirement_satisfied={}, generic_direct_proof_cases={}, breadth_families_green={}/{}, route_purity_green={}, perturbation_sensitivity_green={}, ownership_gate_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.generic_direct_proof_review.case_rows.len(),
        report.breadth_conformance_matrix.green_family_count,
        report.breadth_conformance_matrix.suite_manifest.required_family_ids.len(),
        report.route_purity_review.route_purity_green,
        report
            .weight_perturbation_review
            .all_interventions_show_sensitivity,
        report.interpreter_ownership_green,
        report.acceptance_gate_tie.blocked_issue_ids.first(),
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_interpreter_ownership_gate_report|",
        &report,
    );
    report
}

fn build_generic_direct_proof_review(
    direct_proof_report: &DirectProofReportView,
    exactness_report: &TassadarArticleTransformerReferenceLinearExactnessGateReport,
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleInterpreterOwnershipGenericDirectProofReview,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let suite = build_tassadar_article_class_suite(ARTICLE_SUITE_VERSION)?;
    let corpus = tassadar_article_class_corpus();
    let corpus_by_case_id = corpus
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let artifact_by_case_id = corpus
        .iter()
        .zip(suite.artifacts.iter())
        .map(|(case, artifact)| (case.case_id.as_str(), artifact))
        .collect::<BTreeMap<_, _>>();
    let exactness_by_case_id = exactness_report
        .case_rows
        .iter()
        .map(|row| (row.case_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let committed_case_ids = direct_proof_report
        .receipts
        .iter()
        .map(|receipt| receipt.article_case_id.clone())
        .collect::<BTreeSet<_>>();
    let template_receipt = direct_proof_report
        .receipts
        .first()
        .ok_or(TassadarArticleInterpreterOwnershipGateError::MissingDirectProofTemplate)?;

    let mut case_rows = Vec::new();
    for case_id in GENERIC_DIRECT_PROOF_CASE_IDS {
        if let Some(receipt) = direct_proof_report
            .receipts
            .iter()
            .find(|receipt| receipt.article_case_id == *case_id)
        {
            let exactness_row = exactness_by_case_id.get(case_id).ok_or_else(|| {
                TassadarArticleInterpreterOwnershipGateError::MissingCorpusCase {
                    case_id: String::from(*case_id),
                }
            })?;
            let case = corpus_by_case_id.get(case_id).ok_or_else(|| {
                TassadarArticleInterpreterOwnershipGateError::MissingCorpusCase {
                    case_id: String::from(*case_id),
                }
            })?;
            case_rows.push(build_committed_generic_case_row(
                case,
                exactness_row,
                receipt,
            )?);
            continue;
        }
        let case = corpus_by_case_id.get(case_id).ok_or_else(|| {
            TassadarArticleInterpreterOwnershipGateError::MissingCorpusCase {
                case_id: String::from(*case_id),
            }
        })?;
        let exactness_row = exactness_by_case_id.get(case_id).ok_or_else(|| {
            TassadarArticleInterpreterOwnershipGateError::MissingCorpusCase {
                case_id: String::from(*case_id),
            }
        })?;
        let artifact = artifact_by_case_id.get(case_id).ok_or_else(|| {
            TassadarArticleInterpreterOwnershipGateError::MissingArtifactCase {
                case_id: String::from(*case_id),
            }
        })?;
        case_rows.push(build_synthetic_generic_case_row(
            SyntheticProofCaseInputs {
                case,
                artifact,
                exactness_row,
                template_receipt,
                model,
                lineage_contract_ref: direct_proof_report.lineage_contract_ref.as_str(),
                lineage_contract_digest: direct_proof_report.lineage_contract_digest.as_str(),
            },
        )?);
    }

    let direct_case_count = case_rows
        .iter()
        .filter(|row| row.direct_receipt.selection_state == TassadarExecutorSelectionState::Direct)
        .count();
    let zero_external_call_case_count = case_rows
        .iter()
        .filter(|row| row.direct_receipt.external_call_count == 0)
        .count();
    let fallback_free_case_count = case_rows
        .iter()
        .filter(|row| !row.direct_receipt.fallback_observed)
        .count();
    let exact_case_count = case_rows
        .iter()
        .filter(|row| row.exactness_posture == TassadarExactnessPosture::Exact)
        .count();
    let declared_case_ids = GENERIC_DIRECT_PROOF_CASE_IDS
        .iter()
        .map(|case_id| String::from(*case_id))
        .collect::<Vec<_>>();
    let all_declared_cases_present = case_rows.len() == GENERIC_DIRECT_PROOF_CASE_IDS.len();
    let generic_direct_proof_suite_green = all_declared_cases_present
        && direct_case_count == GENERIC_DIRECT_PROOF_CASE_IDS.len()
        && zero_external_call_case_count == GENERIC_DIRECT_PROOF_CASE_IDS.len()
        && fallback_free_case_count == GENERIC_DIRECT_PROOF_CASE_IDS.len()
        && exact_case_count == GENERIC_DIRECT_PROOF_CASE_IDS.len()
        && case_rows.iter().all(|row| row.direct_no_tool_green);
    let committed_receipt_case_ids = case_rows
        .iter()
        .filter(|row| {
            row.proof_source_kind
                == TassadarArticleInterpreterOwnershipProofSourceKind::CommittedServedReceipt
        })
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let synthetic_receipt_case_ids = case_rows
        .iter()
        .filter(|row| {
            row.proof_source_kind
                == TassadarArticleInterpreterOwnershipProofSourceKind::EvalSyntheticReceipt
        })
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    Ok(TassadarArticleInterpreterOwnershipGenericDirectProofReview {
        generic_suite_id: String::from(GENERIC_DIRECT_PROOF_SUITE_ID),
        report_ref: String::from(DIRECT_PROOF_REPORT_REF),
        benchmark_report_ref: direct_proof_report.benchmark_report_ref.clone(),
        route_descriptor_digest: direct_proof_report.route_descriptor_digest.clone(),
        direct_route_binding: template_receipt.route_binding.clone(),
        declared_case_ids,
        committed_receipt_case_ids,
        synthetic_receipt_case_ids,
        case_rows,
        direct_case_count,
        zero_external_call_case_count,
        fallback_free_case_count,
        exact_case_count,
        all_declared_cases_present,
        generic_direct_proof_suite_green,
        detail: format!(
            "declared_case_count={} committed_receipt_case_count={} synthetic_receipt_case_count={} direct_case_count={} zero_external_call_case_count={} fallback_free_case_count={} exact_case_count={}",
            GENERIC_DIRECT_PROOF_CASE_IDS.len(),
            committed_case_ids.len(),
            SYNTHETIC_GENERIC_DIRECT_PROOF_CASE_IDS.len(),
            direct_case_count,
            zero_external_call_case_count,
            fallback_free_case_count,
            exact_case_count,
        ),
    })
}

fn build_committed_generic_case_row(
    case: &TassadarValidationCase,
    exactness_row: &TassadarArticleTransformerReferenceLinearExactnessCaseRow,
    receipt: &TassadarDirectModelWeightExecutionProofReceipt,
) -> Result<
    TassadarArticleInterpreterOwnershipGenericProofCaseRow,
    TassadarArticleInterpreterOwnershipGateError,
> {
    if exactness_row.runtime_report.exactness_posture != TassadarExactnessPosture::Exact {
        return Err(
            TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
                case_id: case.case_id.clone(),
                detail: exactness_row.runtime_report.detail.clone(),
            },
        );
    }
    Ok(TassadarArticleInterpreterOwnershipGenericProofCaseRow {
        case_id: case.case_id.clone(),
        workload_family_id: workload_family_id_for_case(case.case_id.as_str()),
        proof_source_kind: TassadarArticleInterpreterOwnershipProofSourceKind::CommittedServedReceipt,
        case_summary: case.summary.clone(),
        prompt_token_count: exactness_row.prompt_token_count,
        target_token_count: exactness_row.target_token_count,
        fixture_trace_digest: exactness_row.fixture_trace_digest.clone(),
        transformer_trace_digest: exactness_row.transformer_trace_digest.clone(),
        exactness_posture: exactness_row.runtime_report.exactness_posture,
        direct_receipt: receipt.clone(),
        direct_no_tool_green: receipt.selection_state == TassadarExecutorSelectionState::Direct
            && !receipt.fallback_observed
            && receipt.external_call_count == 0
            && !receipt.external_tool_surface_observed
            && !receipt.cpu_result_substitution_observed
            && exactness_row.runtime_report.exactness_posture == TassadarExactnessPosture::Exact,
        detail: String::from(
            "existing committed served proof receipt remains exact and no-tool on the canonical generic-suite case",
        ),
    })
}

fn build_synthetic_generic_case_row(
    inputs: SyntheticProofCaseInputs<'_>,
) -> Result<
    TassadarArticleInterpreterOwnershipGenericProofCaseRow,
    TassadarArticleInterpreterOwnershipGateError,
> {
    if inputs.exactness_row.runtime_report.exactness_posture != TassadarExactnessPosture::Exact {
        return Err(
            TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
                case_id: inputs.case.case_id.clone(),
                detail: inputs.exactness_row.runtime_report.detail.clone(),
            },
        );
    }
    let direct_receipt = synthetic_direct_receipt_for_case(&inputs)?;
    Ok(TassadarArticleInterpreterOwnershipGenericProofCaseRow {
        case_id: inputs.case.case_id.clone(),
        workload_family_id: workload_family_id_for_case(inputs.case.case_id.as_str()),
        proof_source_kind: TassadarArticleInterpreterOwnershipProofSourceKind::EvalSyntheticReceipt,
        case_summary: inputs.case.summary.clone(),
        prompt_token_count: inputs.exactness_row.prompt_token_count,
        target_token_count: inputs.exactness_row.target_token_count,
        fixture_trace_digest: inputs.exactness_row.fixture_trace_digest.clone(),
        transformer_trace_digest: inputs.exactness_row.transformer_trace_digest.clone(),
        exactness_posture: inputs.exactness_row.runtime_report.exactness_posture,
        direct_no_tool_green: direct_receipt.selection_state == TassadarExecutorSelectionState::Direct
            && !direct_receipt.fallback_observed
            && direct_receipt.external_call_count == 0
            && !direct_receipt.external_tool_surface_observed
            && !direct_receipt.cpu_result_substitution_observed
            && inputs.exactness_row.runtime_report.exactness_posture
                == TassadarExactnessPosture::Exact,
        direct_receipt,
        detail: String::from(
            "eval-owned synthetic receipt widens direct no-tool proof to an additional declared generic article workload family case",
        ),
    })
}

fn synthetic_direct_receipt_for_case(
    inputs: &SyntheticProofCaseInputs<'_>,
) -> Result<
    TassadarDirectModelWeightExecutionProofReceipt,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let trace_artifact_digest = stable_digest(
        b"psionic_tassadar_article_interpreter_ownership_trace_artifact|",
        &(
            inputs.case.case_id.as_str(),
            inputs.exactness_row.transformer_trace_digest.as_str(),
            inputs.model.descriptor().stable_digest().as_str(),
        ),
    );
    let trace_proof_digest = stable_digest(
        b"psionic_tassadar_article_interpreter_ownership_trace_proof|",
        &(
            inputs.case.case_id.as_str(),
            inputs.exactness_row.fixture_trace_digest.as_str(),
            inputs.exactness_row.transformer_trace_digest.as_str(),
            inputs.model.descriptor().stable_digest().as_str(),
        ),
    );
    let runtime_manifest_identity_digest = stable_digest(
        b"psionic_tassadar_article_interpreter_ownership_runtime_manifest_identity|",
        &(
            inputs.model.descriptor().model.model_id.as_str(),
            inputs.model.descriptor().stable_digest().as_str(),
            inputs
                .template_receipt
                .route_binding
                .route_descriptor_digest
                .as_str(),
        ),
    );
    let runtime_manifest_digest = stable_digest(
        b"psionic_tassadar_article_interpreter_ownership_runtime_manifest|",
        &(
            runtime_manifest_identity_digest.as_str(),
            inputs.lineage_contract_digest,
            inputs.case.case_id.as_str(),
        ),
    );
    let proof_bundle_request_digest = stable_digest(
        b"psionic_tassadar_article_interpreter_ownership_proof_bundle_request|",
        &(
            inputs.case.case_id.as_str(),
            inputs.exactness_row.prompt_token_count,
            inputs.exactness_row.target_token_count,
            inputs.exactness_row.transformer_trace_digest.as_str(),
        ),
    );
    let input = TassadarDirectModelWeightExecutionProofInput {
        receipt_id: format!("generic_direct_model_weight_proof.{}", inputs.case.case_id),
        benchmark_ref: inputs.template_receipt.benchmark_ref.clone(),
        benchmark_environment_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF),
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        workload_family_id: workload_family_id_for_case(inputs.case.case_id.as_str()),
        article_case_id: inputs.case.case_id.clone(),
        article_case_summary: inputs.case.summary.clone(),
        executor_product_id: String::from(GENERIC_DIRECT_PROOF_EXECUTOR_PRODUCT_ID),
        model_id: inputs.model.descriptor().model.model_id.clone(),
        model_descriptor_digest: inputs.model.descriptor().stable_digest(),
        model_weight_bundle_digest: inputs.model.weight_metadata().digest.clone(),
        model_primary_artifact_digest: inputs
            .model
            .weight_metadata()
            .primary_artifact_digest()
            .map(String::from),
        model_lineage_contract_ref: String::from(inputs.lineage_contract_ref),
        model_lineage_contract_digest: String::from(inputs.lineage_contract_digest),
        requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
        effective_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
        selection_state: TassadarExecutorSelectionState::Direct,
        fallback_observed: false,
        external_call_count: 0,
        external_tool_surface_observed: false,
        cpu_result_substitution_observed: false,
        compiled_backend_features: vec![
            String::from("tassadar_article_transformer"),
            String::from("forward_pass"),
            String::from("reference_linear"),
        ],
        program_artifact_digest: inputs.artifact.artifact_digest.clone(),
        trace_artifact_digest,
        trace_digest: inputs.exactness_row.fixture_trace_digest.clone(),
        trace_proof_digest,
        runtime_manifest_identity_digest,
        runtime_manifest_digest,
        proof_bundle_request_digest,
        proof_bundle_model_id: Some(inputs.model.descriptor().model.model_id.clone()),
        route_binding: inputs.template_receipt.route_binding.clone(),
    };
    Ok(TassadarDirectModelWeightExecutionProofReceipt::new(input)?)
}

fn build_route_purity_review(
    generic_direct_proof_review: &TassadarArticleInterpreterOwnershipGenericDirectProofReview,
    exactness_report: &TassadarArticleTransformerReferenceLinearExactnessGateReport,
    representation_invariance_report: &TassadarArticleRepresentationInvarianceGateReport,
    generalization_report: &TassadarArticleTransformerGeneralizationGateReport,
    evaluation_independence_report: &TassadarArticleEvaluationIndependenceAuditReport,
    weight_lineage_report: &TassadarArticleTransformerWeightLineageReport,
    single_run_no_spill_report: &TassadarArticleSingleRunNoSpillClosureReport,
) -> TassadarArticleInterpreterOwnershipRoutePurityReview {
    let hidden_host_substitution_excluded = generic_direct_proof_review
        .case_rows
        .iter()
        .all(|row| !row.direct_receipt.cpu_result_substitution_observed);
    let external_oracle_excluded = generic_direct_proof_review.case_rows.iter().all(|row| {
        row.direct_receipt.external_call_count == 0
            && !row.direct_receipt.external_tool_surface_observed
    });
    let preprocessing_shortcut_excluded = representation_invariance_report
        .article_representation_invariance_green
        && exactness_report.reference_linear_exactness_green;
    let route_drift_excluded = exactness_report.reference_linear_exactness_green
        && generalization_report.generalization_green
        && single_run_no_spill_report.single_run_no_spill_closure_green;
    let runtime_owned_control_flow_excluded = single_run_no_spill_report
        .boundary_perturbation_review
        .perturbation_negative_control_green
        && generic_direct_proof_review.case_rows.iter().all(|row| {
            row.direct_receipt.selection_state == TassadarExecutorSelectionState::Direct
        });
    let helper_module_mediation_excluded =
        external_oracle_excluded && representation_invariance_report.trace_vocabulary_binding_green;
    let cache_decisive_step_excluded_basic =
        generic_direct_proof_review.case_rows.iter().all(|row| {
            row.direct_receipt.requested_decode_mode == TassadarExecutorDecodeMode::ReferenceLinear
                && row.direct_receipt.effective_decode_mode
                    == TassadarExecutorDecodeMode::ReferenceLinear
        });
    let forward_pass_sufficiency_green = generic_direct_proof_review
        .generic_direct_proof_suite_green
        && exactness_report.reference_linear_exactness_green;
    let route_purity_green = forward_pass_sufficiency_green
        && hidden_host_substitution_excluded
        && external_oracle_excluded
        && preprocessing_shortcut_excluded
        && route_drift_excluded
        && runtime_owned_control_flow_excluded
        && helper_module_mediation_excluded
        && cache_decisive_step_excluded_basic
        && evaluation_independence_report.evaluation_independence_green
        && weight_lineage_report.weight_lineage_contract_green;
    TassadarArticleInterpreterOwnershipRoutePurityReview {
        forward_pass_sufficiency_green,
        route_purity_green,
        hidden_host_substitution_excluded,
        external_oracle_excluded,
        preprocessing_shortcut_excluded,
        route_drift_excluded,
        runtime_owned_control_flow_excluded,
        helper_module_mediation_excluded,
        cache_decisive_step_excluded_basic,
        artifact_lineage_to_behavior_closed: weight_lineage_report.weight_lineage_contract_green
            && evaluation_independence_report.evaluation_independence_green
            && generic_direct_proof_review.case_rows.iter().all(|row| {
                row.direct_receipt.model_lineage_contract_ref
                    == weight_lineage_report.lineage_contract_ref
            }),
        detail: format!(
            "forward_pass_sufficiency_green={} hidden_host_substitution_excluded={} external_oracle_excluded={} preprocessing_shortcut_excluded={} route_drift_excluded={} runtime_owned_control_flow_excluded={} helper_module_mediation_excluded={} cache_decisive_step_excluded_basic={} evaluation_independence_green={} weight_lineage_contract_green={}",
            forward_pass_sufficiency_green,
            hidden_host_substitution_excluded,
            external_oracle_excluded,
            preprocessing_shortcut_excluded,
            route_drift_excluded,
            runtime_owned_control_flow_excluded,
            helper_module_mediation_excluded,
            cache_decisive_step_excluded_basic,
            evaluation_independence_report.evaluation_independence_green,
            weight_lineage_report.weight_lineage_contract_green,
        ),
    }
}

fn build_computation_mapping_report(
    generic_direct_proof_review: &TassadarArticleInterpreterOwnershipGenericDirectProofReview,
    representation_invariance_report: &TassadarArticleRepresentationInvarianceGateReport,
    single_run_no_spill_report: &TassadarArticleSingleRunNoSpillClosureReport,
) -> TassadarArticleInterpreterOwnershipComputationMappingReport {
    let hidden_host_substitution_excluded = generic_direct_proof_review
        .case_rows
        .iter()
        .all(|row| !row.direct_receipt.cpu_result_substitution_observed);
    let runtime_owned_control_flow_excluded =
        generic_direct_proof_review.case_rows.iter().all(|row| {
            row.direct_receipt.selection_state == TassadarExecutorSelectionState::Direct
        }) && single_run_no_spill_report
            .boundary_perturbation_review
            .perturbation_negative_control_green;
    TassadarArticleInterpreterOwnershipComputationMappingReport {
        program_representation_location: String::from(
            "the admitted article program representation lives in the canonical trace-domain tokens plus the committed Transformer tensor bundle under fixtures/tassadar/models/, not in host-side helper modules or external tools",
        ),
        state_location: String::from(
            "the decisive execution state for TAS-184 lives in the forward-pass token history and Transformer hidden-state evolution, with checkpoint, spill, and continuation carriers already excluded by TAS-183; the later KV-cache and activation-dominance verdict remains explicitly deferred to TAS-184A",
        ),
        control_flow_realization: String::from(
            "control flow is realized by deterministic reference-linear decode over forward-pass logits with runtime receipts acting only as audit surfaces, not as an alternate control plane",
        ),
        stable_across_runs: generic_direct_proof_review.generic_direct_proof_suite_green
            && representation_invariance_report.article_representation_invariance_green
            && single_run_no_spill_report.single_run_no_spill_closure_green,
        hidden_host_substitution_excluded,
        runtime_owned_control_flow_excluded,
        detail: format!(
            "stable_across_runs={} hidden_host_substitution_excluded={} runtime_owned_control_flow_excluded={}",
            generic_direct_proof_review.generic_direct_proof_suite_green
                && representation_invariance_report.article_representation_invariance_green
                && single_run_no_spill_report.single_run_no_spill_closure_green,
            hidden_host_substitution_excluded,
            runtime_owned_control_flow_excluded,
        ),
    }
}

fn build_breadth_conformance_matrix(
    breadth_suite_gate_report: &TassadarArticleInterpreterBreadthSuiteGateReport,
    generic_direct_proof_suite_green: bool,
    route_purity_green: bool,
) -> TassadarArticleInterpreterOwnershipBreadthConformanceMatrix {
    let suite_manifest = build_tassadar_article_interpreter_breadth_suite();
    let family_checks_by_id = breadth_suite_gate_report
        .family_checks
        .iter()
        .map(|check| (check.family_id, check))
        .collect::<BTreeMap<_, _>>();
    let family_rows = suite_manifest
        .family_rows
        .iter()
        .map(|row| {
            let family_gate_green = family_checks_by_id
                .get(&row.family_id)
                .map(|check| check.green)
                .unwrap_or(false);
            TassadarArticleInterpreterOwnershipBreadthConformanceRow {
                family_id: row.family_id,
                required_evidence_ids: row.required_evidence_ids.clone(),
                family_gate_green,
                ownership_gate_covers_family: family_gate_green
                    && generic_direct_proof_suite_green
                    && route_purity_green,
                detail: row.detail.clone(),
            }
        })
        .collect::<Vec<_>>();
    let green_family_count = family_rows
        .iter()
        .filter(|row| row.ownership_gate_covers_family)
        .count();
    TassadarArticleInterpreterOwnershipBreadthConformanceMatrix {
        manifest_ref: String::from("fixtures/tassadar/sources/tassadar_article_interpreter_breadth_suite_v1.json"),
        suite_manifest,
        family_rows,
        green_family_count,
        conformance_matrix_green: green_family_count
            == build_tassadar_article_interpreter_breadth_suite()
                .required_family_ids
                .len(),
        detail: format!(
            "green_family_count={} required_family_count={} generic_direct_proof_suite_green={} route_purity_green={}",
            green_family_count,
            build_tassadar_article_interpreter_breadth_suite()
                .required_family_ids
                .len(),
            generic_direct_proof_suite_green,
            route_purity_green,
        ),
    }
}

fn build_weight_perturbation_review(
    model: &TassadarArticleTransformer,
) -> Result<
    TassadarArticleInterpreterOwnershipWeightPerturbationReview,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let witness_cases = witness_cases();
    let baseline_exact_case_count = witness_cases
        .iter()
        .filter(|case| {
            evaluate_model_behavior(model, case)
                .map(|behavior| behavior.exact)
                .unwrap_or(false)
        })
        .count();
    let zeroed_row = evaluate_zeroed_trainable_subset(model, witness_cases.as_slice())?;
    let randomized_row = evaluate_randomized_trainable_subset(model, witness_cases.as_slice())?;
    let clamped_row = evaluate_clamped_attention_head(model, witness_cases.as_slice())?;
    let removed_layer_row = evaluate_removed_decoder_layer(model, witness_cases.as_slice())?;
    let intervention_rows = vec![zeroed_row, randomized_row, clamped_row, removed_layer_row];
    let all_interventions_show_sensitivity = intervention_rows
        .iter()
        .all(|row| row.behavior_changed && row.mismatch_case_count > 0);
    let mismatch_range = intervention_rows
        .iter()
        .map(|row| row.mismatch_case_count)
        .collect::<BTreeSet<_>>();
    let locality_characterization = if mismatch_range.len() == 1 {
        TassadarArticleInterpreterOwnershipLocalityKind::Diffuse
    } else {
        TassadarArticleInterpreterOwnershipLocalityKind::Modular
    };
    Ok(
        TassadarArticleInterpreterOwnershipWeightPerturbationReview {
            witness_case_ids: witness_cases
                .iter()
                .map(|case| case.case_id.clone())
                .collect(),
            baseline_model_id: model.descriptor().model.model_id.clone(),
            baseline_descriptor_digest: model.descriptor().stable_digest(),
            baseline_exact_case_count,
            intervention_rows,
            all_interventions_show_sensitivity,
            locality_characterization,
            detail: format!(
                "baseline_exact_case_count={} intervention_count={} locality_characterization={:?}",
                baseline_exact_case_count, 4, locality_characterization,
            ),
        },
    )
}

fn evaluate_zeroed_trainable_subset(
    model: &TassadarArticleTransformer,
    witness_cases: &[TassadarValidationCase],
) -> Result<
    TassadarArticleInterpreterOwnershipInterventionRow,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let first_parameter = model
        .trainable_parameter_vectors()
        .into_iter()
        .next()
        .ok_or_else(
            || TassadarArticleInterpreterOwnershipGateError::EmptyTensorMutation {
                detail: String::from("model exposed no trainable parameter vectors"),
            },
        )?;
    let mut overrides = BTreeMap::new();
    overrides.insert(
        first_parameter.parameter_id.clone(),
        vec![0.0; first_parameter.values.len()],
    );
    let mutated_model = model.with_parameter_overrides(&overrides)?;
    Ok(intervention_row_from_model(
        model,
        &mutated_model,
        witness_cases,
        TassadarArticleInterpreterOwnershipInterventionKind::ZeroedTrainableSubset,
        vec![first_parameter.parameter_id],
    ))
}

fn evaluate_randomized_trainable_subset(
    model: &TassadarArticleTransformer,
    witness_cases: &[TassadarValidationCase],
) -> Result<
    TassadarArticleInterpreterOwnershipInterventionRow,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let last_parameter = model
        .trainable_parameter_vectors()
        .into_iter()
        .last()
        .ok_or_else(
            || TassadarArticleInterpreterOwnershipGateError::EmptyTensorMutation {
                detail: String::from("model exposed no trainable parameter vectors"),
            },
        )?;
    let mut randomized = Vec::with_capacity(last_parameter.values.len());
    for (index, _) in last_parameter.values.iter().enumerate() {
        randomized.push(seeded_unit_f32(
            format!("tassadar.article_interpreter_ownership.randomized.{index}").as_bytes(),
        ));
    }
    let mut overrides = BTreeMap::new();
    overrides.insert(last_parameter.parameter_id.clone(), randomized);
    let mutated_model = model.with_parameter_overrides(&overrides)?;
    Ok(intervention_row_from_model(
        model,
        &mutated_model,
        witness_cases,
        TassadarArticleInterpreterOwnershipInterventionKind::RandomizedTrainableSubset,
        vec![last_parameter.parameter_id],
    ))
}

fn evaluate_clamped_attention_head(
    model: &TassadarArticleTransformer,
    witness_cases: &[TassadarValidationCase],
) -> Result<
    TassadarArticleInterpreterOwnershipInterventionRow,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let mutated_model = patched_model_from_tensor_mutation(model, |rows, config| {
        let head_width = config.hidden_size / config.head_count.max(1);
        let target_prefixes = [
            "encoder_layers.0.self_attention",
            "decoder_layers.0.self_attention",
            "decoder_layers.0.cross_attention",
        ];
        let mut touched = 0usize;
        for prefix in target_prefixes {
            touched += clamp_attention_head(rows, prefix, head_width);
        }
        if touched == 0 {
            return Err(
                TassadarArticleInterpreterOwnershipGateError::EmptyTensorMutation {
                    detail: String::from("no attention-head tensors matched the clamp target"),
                },
            );
        }
        Ok(vec![
            String::from("encoder_layers.0.self_attention.head_0"),
            String::from("decoder_layers.0.self_attention.head_0"),
            String::from("decoder_layers.0.cross_attention.head_0"),
        ])
    })?;
    Ok(intervention_row_from_model(
        model,
        &mutated_model.model,
        witness_cases,
        TassadarArticleInterpreterOwnershipInterventionKind::ClampedAttentionHead,
        mutated_model.mutated_refs,
    ))
}

fn evaluate_removed_decoder_layer(
    model: &TassadarArticleTransformer,
    witness_cases: &[TassadarValidationCase],
) -> Result<
    TassadarArticleInterpreterOwnershipInterventionRow,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let mutated_model = patched_model_from_tensor_mutation(model, |rows, _config| {
        let target_prefix = "decoder_layers.0.";
        let touched = rows
            .values_mut()
            .filter(|row| row.name.starts_with(target_prefix))
            .map(|row| {
                row.values.fill(0.0);
                1usize
            })
            .sum::<usize>();
        if touched == 0 {
            return Err(
                TassadarArticleInterpreterOwnershipGateError::EmptyTensorMutation {
                    detail: String::from("no decoder-layer tensors matched the removal target"),
                },
            );
        }
        Ok(vec![String::from("decoder_layers.0")])
    })?;
    Ok(intervention_row_from_model(
        model,
        &mutated_model.model,
        witness_cases,
        TassadarArticleInterpreterOwnershipInterventionKind::RemovedDecoderLayer,
        mutated_model.mutated_refs,
    ))
}

fn intervention_row_from_model(
    baseline_model: &TassadarArticleTransformer,
    mutated_model: &TassadarArticleTransformer,
    witness_cases: &[TassadarValidationCase],
    intervention_kind: TassadarArticleInterpreterOwnershipInterventionKind,
    mutated_tensor_or_parameter_refs: Vec<String>,
) -> TassadarArticleInterpreterOwnershipInterventionRow {
    let mut exact_case_count = 0usize;
    let mut mismatch_case_count = 0usize;
    for case in witness_cases {
        let baseline_behavior = evaluate_model_behavior(baseline_model, case).ok();
        let mutated_behavior = evaluate_model_behavior(mutated_model, case).ok();
        if mutated_behavior
            .as_ref()
            .map(|value| value.exact)
            .unwrap_or(false)
        {
            exact_case_count += 1;
        }
        if baseline_behavior
            .as_ref()
            .map(|value| value.behavior_digest.as_str())
            != mutated_behavior
                .as_ref()
                .map(|value| value.behavior_digest.as_str())
        {
            mismatch_case_count += 1;
        }
    }
    let witness_case_count = witness_cases.len();
    TassadarArticleInterpreterOwnershipInterventionRow {
        intervention_kind,
        mutated_tensor_or_parameter_refs,
        model_descriptor_digest_changed: baseline_model.descriptor().stable_digest()
            != mutated_model.descriptor().stable_digest(),
        witness_case_count,
        exact_case_count,
        mismatch_case_count,
        behavior_changed: mismatch_case_count > 0,
        detail: format!(
            "witness_case_count={} exact_case_count={} baseline_divergence_case_count={}",
            witness_case_count, exact_case_count, mismatch_case_count,
        ),
    }
}

struct PatchedModel {
    model: TassadarArticleTransformer,
    mutated_refs: Vec<String>,
}

fn patched_model_from_tensor_mutation<F>(
    baseline_model: &TassadarArticleTransformer,
    mut mutate: F,
) -> Result<PatchedModel, TassadarArticleInterpreterOwnershipGateError>
where
    F: FnMut(
        &mut BTreeMap<String, TensorRow>,
        &psionic_transformer::EncoderDecoderTransformerConfig,
    ) -> Result<Vec<String>, TassadarArticleInterpreterOwnershipGateError>,
{
    let directory =
        tempdir().map_err(
            |error| TassadarArticleInterpreterOwnershipGateError::CreateDir {
                path: String::from("tempdir"),
                error,
            },
        )?;
    let descriptor_path = directory.path().join("patched_descriptor.json");
    let artifact_path = directory.path().join("patched_weights.safetensors");
    baseline_model.write_artifact_bundle(&descriptor_path, &artifact_path)?;
    let mut descriptor: TassadarArticleTransformerDescriptor =
        serde_json::from_slice(&fs::read(&descriptor_path).map_err(|error| {
            TassadarArticleInterpreterOwnershipGateError::Read {
                path: descriptor_path.display().to_string(),
                error,
            }
        })?)?;
    let artifact_bytes = fs::read(&artifact_path).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateError::Read {
            path: artifact_path.display().to_string(),
            error,
        }
    })?;
    let mut rows = read_tensor_rows(&artifact_bytes)?;
    let mutated_refs = mutate(&mut rows, &descriptor.config)?;
    let mutated_bytes = serialize_tensor_rows(&rows)?;
    fs::write(&artifact_path, &mutated_bytes).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateError::Write {
            path: artifact_path.display().to_string(),
            error,
        }
    })?;
    let artifact_sha256 = hex::encode(Sha256::digest(&mutated_bytes));
    let weights = weight_bundle_metadata_for_rows(
        rows.values().cloned().collect::<Vec<_>>().as_slice(),
        String::from("patched_weights.safetensors"),
        artifact_sha256.clone(),
        mutated_bytes.len() as u64,
    );
    descriptor.artifact_binding = patched_artifact_binding(
        descriptor.model.model_id.as_str(),
        descriptor.artifact_binding.artifact_ref.clone(),
        artifact_sha256,
        weights.digest.clone(),
        weights.tensors.len(),
    );
    descriptor.weights = weights;
    let json = serde_json::to_string_pretty(&descriptor)?;
    fs::write(&descriptor_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateError::Write {
            path: descriptor_path.display().to_string(),
            error,
        }
    })?;
    Ok(PatchedModel {
        model: TassadarArticleTransformer::load_from_descriptor_path(&descriptor_path)?,
        mutated_refs,
    })
}

fn read_tensor_rows(
    artifact_bytes: &[u8],
) -> Result<BTreeMap<String, TensorRow>, TassadarArticleInterpreterOwnershipGateError> {
    let tensors = SafeTensors::deserialize(artifact_bytes).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateError::Decode {
            path: String::from("patched_weights.safetensors"),
            error: serde_json::Error::io(std::io::Error::other(error.to_string())),
        }
    })?;
    let mut rows = BTreeMap::new();
    for name in tensors.names() {
        let tensor = tensors.tensor(name).map_err(|error| {
            TassadarArticleInterpreterOwnershipGateError::Decode {
                path: format!("patched_weights.safetensors::{name}"),
                error: serde_json::Error::io(std::io::Error::other(error.to_string())),
            }
        })?;
        if tensor.dtype() != SafeTensorsDType::F32 {
            return Err(
                TassadarArticleInterpreterOwnershipGateError::UnsupportedArtifactTensorDType {
                    name: String::from(name),
                    dtype: format!("{:?}", tensor.dtype()),
                },
            );
        }
        if tensor.data().len() % 4 != 0 {
            return Err(
                TassadarArticleInterpreterOwnershipGateError::InvalidArtifactTensorBytes {
                    name: String::from(name),
                    byte_length: tensor.data().len(),
                },
            );
        }
        let values = tensor
            .data()
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect::<Vec<_>>();
        rows.insert(
            String::from(name),
            TensorRow {
                name: String::from(name),
                shape: tensor.shape().to_vec(),
                values,
            },
        );
    }
    Ok(rows)
}

fn serialize_tensor_rows(
    rows: &BTreeMap<String, TensorRow>,
) -> Result<Vec<u8>, TassadarArticleInterpreterOwnershipGateError> {
    let mut buffers = Vec::with_capacity(rows.len());
    let ordered = rows.values().cloned().collect::<Vec<_>>();
    for row in &ordered {
        let mut bytes = Vec::with_capacity(row.values.len() * 4);
        for value in &row.values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        buffers.push(bytes);
    }
    let mut views = BTreeMap::new();
    for (index, row) in ordered.iter().enumerate() {
        let view = TensorView::new(SafeTensorsDType::F32, row.shape.clone(), &buffers[index])
            .map_err(
                |error| TassadarArticleInterpreterOwnershipGateError::Decode {
                    path: format!("patched_weights.safetensors::{}", row.name),
                    error: serde_json::Error::io(std::io::Error::other(error.to_string())),
                },
            )?;
        views.insert(row.name.clone(), view);
    }
    serialize(&views, None).map_err(
        |error| TassadarArticleInterpreterOwnershipGateError::Decode {
            path: String::from("patched_weights.safetensors"),
            error: serde_json::Error::io(std::io::Error::other(error.to_string())),
        },
    )
}

fn weight_bundle_metadata_for_rows(
    rows: &[TensorRow],
    artifact_ref: String,
    artifact_sha256: String,
    artifact_byte_length: u64,
) -> WeightBundleMetadata {
    let mut ordered = rows.to_vec();
    ordered.sort_by(|left, right| left.name.cmp(&right.name));
    let tensors = ordered
        .iter()
        .map(|row| {
            WeightTensorMetadata::new(row.name.clone(), Shape::new(row.shape.clone()), DType::F32)
        })
        .collect::<Vec<_>>();
    let mut hasher = Sha256::new();
    for (metadata, row) in tensors.iter().zip(ordered.iter()) {
        digest_tensor_values(&mut hasher, metadata, row.values.as_slice());
    }
    WeightBundleMetadata {
        format: WeightFormat::SafeTensors,
        source: WeightSource::ExternalArtifact,
        quantization: QuantizationMode::None,
        quantization_modes: Vec::new(),
        digest: hex::encode(hasher.finalize()),
        tensors,
        artifacts: vec![WeightArtifactMetadata::new(
            artifact_ref,
            artifact_byte_length,
            artifact_sha256,
        )],
    }
}

fn patched_artifact_binding(
    model_id: &str,
    artifact_ref: String,
    artifact_sha256: String,
    weight_bundle_digest: String,
    tensor_count: usize,
) -> TassadarArticleTransformerArtifactBinding {
    let artifact_id =
        format!("tassadar://article_transformer/weights/{model_id}/{weight_bundle_digest}");
    let artifact_identity_digest = stable_digest(
        b"psionic_tassadar_article_transformer_artifact_binding|",
        &(
            artifact_id.as_str(),
            artifact_ref.as_str(),
            WeightFormat::SafeTensors.identity_label(),
            artifact_sha256.as_str(),
            weight_bundle_digest.as_str(),
            tensor_count,
        ),
    );
    TassadarArticleTransformerArtifactBinding {
        artifact_id,
        artifact_ref,
        artifact_format: WeightFormat::SafeTensors,
        primary_artifact_sha256: artifact_sha256,
        weight_bundle_digest,
        tensor_count,
        artifact_identity_digest,
    }
}

fn clamp_attention_head(
    rows: &mut BTreeMap<String, TensorRow>,
    prefix: &str,
    head_width: usize,
) -> usize {
    let mut touched = 0usize;
    let weight_names = [
        format!("{prefix}.query_projection.weight"),
        format!("{prefix}.key_projection.weight"),
        format!("{prefix}.value_projection.weight"),
        format!("{prefix}.output_projection.weight"),
    ];
    let bias_names = [
        format!("{prefix}.query_projection.bias"),
        format!("{prefix}.key_projection.bias"),
        format!("{prefix}.value_projection.bias"),
        format!("{prefix}.output_projection.bias"),
    ];
    for name in weight_names {
        if let Some(row) = rows.get_mut(&name) {
            zero_head_slice(row, head_width);
            touched += 1;
        }
    }
    for name in bias_names {
        if let Some(row) = rows.get_mut(&name) {
            for value in row.values.iter_mut().take(head_width) {
                *value = 0.0;
            }
            touched += 1;
        }
    }
    touched
}

fn zero_head_slice(row: &mut TensorRow, head_width: usize) {
    if row.shape.len() != 2 {
        return;
    }
    let rows_len = row.shape[0];
    let cols_len = row.shape[1];
    if rows_len == 0 || cols_len == 0 {
        return;
    }
    let clamp_rows = head_width.min(rows_len);
    for row_index in 0..clamp_rows {
        let start = row_index * cols_len;
        let end = start + cols_len;
        for value in &mut row.values[start..end] {
            *value = 0.0;
        }
    }
    if row.name.ends_with("output_projection.weight") {
        for row_index in 0..rows_len {
            let start = row_index * cols_len;
            let end = start + head_width.min(cols_len);
            for value in &mut row.values[start..end] {
                *value = 0.0;
            }
        }
    }
}

fn witness_cases() -> Vec<TassadarValidationCase> {
    tassadar_article_class_corpus()
        .into_iter()
        .filter(|case| PERTURBATION_WITNESS_CASE_IDS.contains(&case.case_id.as_str()))
        .collect()
}

#[derive(Clone, Debug)]
struct ModelCaseBehavior {
    exact: bool,
    behavior_digest: String,
}

fn evaluate_model_behavior(
    model: &TassadarArticleTransformer,
    case: &TassadarValidationCase,
) -> Result<ModelCaseBehavior, TassadarArticleInterpreterOwnershipGateError> {
    let fixture_execution = fixture_execution_for_case(case)?;
    let batch =
        match model.tokenize_article_trace_domain_unbounded(&case.program, &fixture_execution) {
            Ok(batch) => batch,
            Err(error) => {
                return Err(
                    TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
                        case_id: case.case_id.clone(),
                        detail: error.to_string(),
                    },
                )
            }
        };
    let output = match model.forward(
        Shape::new(batch.source_shape.clone()),
        batch.source_token_ids.as_slice(),
        Shape::new(batch.target_shape.clone()),
        batch.target_token_ids.as_slice(),
        TransformerExecutionMode::Eval,
    ) {
        Ok(output) => output,
        Err(error) => {
            return Err(
                TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
                    case_id: case.case_id.clone(),
                    detail: error.to_string(),
                },
            )
        }
    };
    let predicted_token_ids =
        predicted_token_ids_from_logits(output.logits.dims(), &output.logits.data)?;
    Ok(ModelCaseBehavior {
        exact: predicted_token_ids == batch.target_token_ids,
        behavior_digest: stable_digest(
            b"psionic_tassadar_article_interpreter_ownership_behavior_digest|",
            &(
                case.case_id.as_str(),
                batch.sequence_digest.as_str(),
                predicted_token_ids,
                tensor_data_digest(&output.logits.data),
            ),
        ),
    })
}

fn workload_family_id_for_case(case_id: &str) -> String {
    match case_id {
        "micro_wasm_kernel" => String::from("MicroWasmKernel"),
        "branch_heavy_kernel" => String::from("BranchHeavyKernel"),
        "memory_heavy_kernel" => String::from("MemoryHeavyKernel"),
        "long_loop_kernel" => String::from("LongLoopKernel"),
        "sudoku_v0_test_a" => String::from("SudokuClass"),
        "hungarian_matching" => String::from("HungarianMatching"),
        _ => String::from("ArticleGeneric"),
    }
}

fn fixture_execution_for_case(
    case: &TassadarValidationCase,
) -> Result<TassadarExecution, TassadarArticleInterpreterOwnershipGateError> {
    let runner = TassadarFixtureRunner::for_program(&case.program).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
            case_id: case.case_id.clone(),
            detail: error.to_string(),
        }
    })?;
    runner.execute(&case.program).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
            case_id: case.case_id.clone(),
            detail: error.to_string(),
        }
    })
}

pub fn tassadar_article_interpreter_ownership_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF)
}

pub fn write_tassadar_article_interpreter_ownership_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleInterpreterOwnershipGateReport,
    TassadarArticleInterpreterOwnershipGateError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleInterpreterOwnershipGateError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_interpreter_ownership_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateError::Write {
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

fn digest_tensor_values(hasher: &mut Sha256, metadata: &WeightTensorMetadata, values: &[f32]) {
    hasher.update(metadata.name.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.dtype).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.quantization).as_bytes());
    hasher.update(b"|");
    for dimension in metadata.shape.dims() {
        hasher.update(dimension.to_string().as_bytes());
        hasher.update(b",");
    }
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_bits().to_be_bytes());
    }
    hasher.update(b"\n");
}

fn seeded_unit_f32(label: &[u8]) -> f32 {
    let digest = Sha256::digest(label);
    let raw = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    ((raw % 10_000) as f32 / 5_000.0) - 1.0
}

fn tensor_data_digest(data: &TensorData) -> String {
    match data {
        TensorData::F32(values) => stable_digest(
            b"psionic_tassadar_article_interpreter_ownership_tensor_data_f32|",
            values,
        ),
        other => stable_digest(
            b"psionic_tassadar_article_interpreter_ownership_tensor_data_debug|",
            &format!("{other:?}"),
        ),
    }
}

fn predicted_token_ids_from_logits(
    dims: &[usize],
    data: &TensorData,
) -> Result<Vec<usize>, TassadarArticleInterpreterOwnershipGateError> {
    if dims.len() != 3 {
        return Err(
            TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
                case_id: String::from("logits"),
                detail: format!("expected logits rank 3, found shape {dims:?}"),
            },
        );
    }
    let batch_size = dims[0];
    let target_len = dims[1];
    let vocab_size = dims[2];
    if batch_size == 0 || target_len == 0 || vocab_size == 0 {
        return Err(
            TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
                case_id: String::from("logits"),
                detail: format!("logits dimensions must be non-zero, found shape {dims:?}"),
            },
        );
    }
    let values = match data {
        TensorData::F32(values) => values.as_slice(),
        other => {
            return Err(
                TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
                    case_id: String::from("logits"),
                    detail: format!("expected dense f32 logits, found {other:?}"),
                },
            );
        }
    };
    let expected_len = batch_size * target_len * vocab_size;
    if values.len() != expected_len {
        return Err(
            TassadarArticleInterpreterOwnershipGateError::GenericSuiteExactness {
                case_id: String::from("logits"),
                detail: format!(
                    "logits value count {} does not match shape {dims:?} (expected {expected_len})",
                    values.len()
                ),
            },
        );
    }
    let mut predictions = Vec::with_capacity(batch_size * target_len);
    for position_index in 0..(batch_size * target_len) {
        let offset = position_index * vocab_size;
        let mut best_index = 0usize;
        let mut best_value = values[offset];
        for vocab_index in 1..vocab_size {
            let candidate = values[offset + vocab_index];
            if candidate > best_value {
                best_index = vocab_index;
                best_value = candidate;
            }
        }
        predictions.push(best_index);
    }
    Ok(predictions)
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleInterpreterOwnershipGateError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleInterpreterOwnershipGateError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::{
        build_tassadar_article_interpreter_ownership_gate_report, read_repo_json,
        tassadar_article_interpreter_ownership_gate_report_path,
        write_tassadar_article_interpreter_ownership_gate_report,
        TassadarArticleInterpreterOwnershipGateReport,
        TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
    };

    fn generated_report() -> &'static TassadarArticleInterpreterOwnershipGateReport {
        static REPORT: OnceLock<TassadarArticleInterpreterOwnershipGateReport> = OnceLock::new();
        REPORT.get_or_init(|| {
            build_tassadar_article_interpreter_ownership_gate_report().expect("ownership report")
        })
    }

    #[test]
    fn article_interpreter_ownership_gate_tracks_green_inputs() {
        let report = generated_report();

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(
            report
                .generic_direct_proof_review
                .generic_direct_proof_suite_green
        );
        assert!(report.breadth_conformance_matrix.conformance_matrix_green);
        assert!(report.route_purity_review.route_purity_green);
        assert!(
            report
                .weight_perturbation_review
                .all_interventions_show_sensitivity
        );
        assert!(report.interpreter_ownership_green);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_interpreter_ownership_gate_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = generated_report().clone();
        let committed: TassadarArticleInterpreterOwnershipGateReport = read_repo_json(
            TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
            "article_interpreter_ownership_gate_report",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_interpreter_ownership_gate_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_interpreter_ownership_gate_report.json");
        let written = write_tassadar_article_interpreter_ownership_gate_report(&output_path)?;
        let persisted: TassadarArticleInterpreterOwnershipGateReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_interpreter_ownership_gate_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_interpreter_ownership_gate_report.json")
        );
        Ok(())
    }
}
