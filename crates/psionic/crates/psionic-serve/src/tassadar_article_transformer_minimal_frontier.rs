use std::{
    collections::BTreeSet,
    env,
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_article_fixture_transformer_behavior_case_rows_for_model,
    build_tassadar_article_transformer_weight_lineage_contract_for_bundle,
    write_tassadar_article_transformer_weight_production_evidence_bundle,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleFastRouteArchitectureSelectionReport,
    TassadarArticleFastRouteArchitectureSelectionError,
    TassadarArticleFastRouteCandidateKind, TassadarArticleFastRouteThroughputFloorReportError,
    TassadarArticleFastRouteThroughputFloorReport,
    TassadarArticleFixtureTransformerParityCaseRow, TassadarArticleFixtureTransformerParityError,
    TassadarArticleTransformerGeneralizationGateReport,
    TassadarArticleTransformerWeightLineageContract,
    TassadarArticleTransformerWeightLineageError,
    TassadarArticleTransformerWeightProductionError as WeightProductionEvidenceError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
};
use psionic_models::{
    tassadar_article_transformer_executor_descriptor, TassadarArticleTransformer,
    TassadarArticleTransformerEmbeddingStrategy, TassadarTraceTokenizer,
};
use psionic_runtime::{
    TassadarDirectModelWeightExecutionProofReceipt, TassadarExactnessPosture,
    TassadarExecutorDecodeMode, TassadarExecutorSelectionState,
    TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF,
};
use psionic_train::{
    build_tassadar_article_transformer_weight_production_evidence_bundle,
    run_tassadar_article_transformer_weight_production,
    TassadarArticleTransformerWeightProductionConfig,
    TassadarArticleTransformerWeightProductionError,
    TassadarArticleTransformerWeightProductionSuite,
};
use psionic_transformer::EncoderDecoderTransformerConfig;

use crate::{
    build_tassadar_direct_model_weight_execution_proof_receipt_for_article_session_with_lineage,
    LocalTassadarArticleExecutorSessionService, LocalTassadarArticleHybridWorkflowService,
    LocalTassadarExecutorService, LocalTassadarPlannerRouter,
    TassadarArticleExecutorSessionOutcome, TassadarArticleExecutorSessionRequest,
    TassadarArticleHybridWorkflowOutcome, TassadarArticleHybridWorkflowRequest,
    TassadarDirectModelWeightExecutionProofReportError, TassadarPlannerRouteDescriptorError,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_minimal_frontier_report.json";
pub const TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_CHECKER_REF: &str =
    "scripts/check-tassadar-article-transformer-minimal-frontier.sh";
pub const TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_article_transformer_minimal_frontier_v1";

const REPORT_SCHEMA_VERSION: u16 = 1;
const ISSUE_ID: &str = "TAS-R1";
const ISSUE_TITLE: &str = "Minimal Transformer size for article-equivalent behavior";
const DIRECT_PROOF_CASE_IDS: [&str; 3] =
    ["hungarian_matching", "memory_heavy_kernel", "sudoku_v0_test_a"];
const TRAINING_CASE_COUNT: usize = 1;
const HELD_OUT_CASE_COUNT: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierCandidateSpec {
    pub candidate_id: String,
    pub issue_order: usize,
    pub hidden_size: usize,
    pub feed_forward_size: usize,
    pub encoder_layer_count: usize,
    pub decoder_layer_count: usize,
    pub head_count: usize,
    pub base_model_id: String,
    pub produced_model_id: String,
    pub parameter_scalar_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierArtifactRefs {
    pub run_root_ref: String,
    pub base_descriptor_ref: String,
    pub base_artifact_ref: String,
    pub produced_descriptor_ref: String,
    pub produced_artifact_ref: String,
    pub evidence_bundle_ref: String,
    pub lineage_contract_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierStageAReview {
    pub suite_id: String,
    pub training_example_count: usize,
    pub held_out_example_count: usize,
    pub all_required_refs_exist: bool,
    pub weight_production_smoke_passed: bool,
    pub training_loss_improved: bool,
    pub training_exactness_improved: bool,
    pub produced_artifact_differs_from_base: bool,
    pub checkpoint_restore_matches_trained_state: bool,
    pub artifact_reload_matches_trained_state: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierExactnessCaseRow {
    pub case_id: String,
    pub program_id: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub trace_step_count: usize,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub within_transformer_context_window: bool,
    pub exactness_posture: TassadarExactnessPosture,
    pub fixture_trace_digest: String,
    pub transformer_trace_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierStageBReview {
    pub parity_case_rows: Vec<TassadarArticleFixtureTransformerParityCaseRow>,
    pub parity_all_declared_cases_present: bool,
    pub parity_all_cases_pass: bool,
    pub parity_passed: bool,
    pub reference_linear_case_rows: Vec<TassadarArticleTransformerMinimalFrontierExactnessCaseRow>,
    pub reference_linear_exact_case_count: usize,
    pub reference_linear_mismatch_case_count: usize,
    pub reference_linear_refused_case_count: usize,
    pub reference_linear_passed: bool,
    pub generalization_report_ref: String,
    pub canonical_generalization_green: bool,
    pub canonical_generalization_case_count: usize,
    pub generalization_passed: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierDirectProofCaseReview {
    pub case_id: String,
    pub receipt: Option<TassadarDirectModelWeightExecutionProofReceipt>,
    pub direct_selection: bool,
    pub fallback_free: bool,
    pub zero_external_call: bool,
    pub route_digest_matches: bool,
    pub lineage_matches: bool,
    pub trace_digest_matches_fixture: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierDirectProofReview {
    pub route_descriptor_digest: String,
    pub case_reviews: Vec<TassadarArticleTransformerMinimalFrontierDirectProofCaseReview>,
    pub direct_case_count: usize,
    pub fallback_free_case_count: usize,
    pub zero_external_call_case_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierFastRouteSelectionReview {
    pub report_ref: String,
    pub canonical_selected_candidate_kind: String,
    pub canonical_fast_route_selection_green: bool,
    pub candidate_route_descriptor_digest: String,
    pub candidate_hull_cache_direct_guaranteed: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierFastRouteSessionCaseReview {
    pub case_id: String,
    pub fixture_trace_digest: String,
    pub observed_trace_digest: Option<String>,
    pub selection_state: Option<TassadarExecutorSelectionState>,
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub exact_direct_hull_cache: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview {
    pub case_id: String,
    pub fixture_trace_digest: String,
    pub observed_trace_digest: Option<String>,
    pub planner_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub executor_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: Option<TassadarExecutorSelectionState>,
    pub exact_direct_hull_cache: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierThroughputFloorReview {
    pub report_ref: String,
    pub canonical_selected_candidate_kind: String,
    pub throughput_floor_green: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierStageCReview {
    pub direct_proof_review: TassadarArticleTransformerMinimalFrontierDirectProofReview,
    pub fast_route_selection_review:
        TassadarArticleTransformerMinimalFrontierFastRouteSelectionReview,
    pub article_session_reviews:
        Vec<TassadarArticleTransformerMinimalFrontierFastRouteSessionCaseReview>,
    pub hybrid_workflow_reviews:
        Vec<TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview>,
    pub throughput_floor_review: TassadarArticleTransformerMinimalFrontierThroughputFloorReview,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierCandidateReport {
    pub spec: TassadarArticleTransformerMinimalFrontierCandidateSpec,
    pub artifact_refs: TassadarArticleTransformerMinimalFrontierArtifactRefs,
    pub evidence_bundle_digest: String,
    pub lineage_contract_digest: String,
    pub stage_a_review: TassadarArticleTransformerMinimalFrontierStageAReview,
    pub stage_b_review: TassadarArticleTransformerMinimalFrontierStageBReview,
    pub stage_c_review: TassadarArticleTransformerMinimalFrontierStageCReview,
    pub all_stages_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerMinimalFrontierReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub issue_id: String,
    pub issue_title: String,
    pub acceptance_gate_report_ref: String,
    pub acceptance_gate_article_equivalence_green: bool,
    pub canonical_optional_issue_still_open: bool,
    pub run_root_ref: String,
    pub candidate_reports: Vec<TassadarArticleTransformerMinimalFrontierCandidateReport>,
    pub successful_candidate_ids: Vec<String>,
    pub minimal_successful_candidate_id: Option<String>,
    pub minimal_successful_model_id: Option<String>,
    pub frontier_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerMinimalFrontierError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    FastRouteSelection(#[from] TassadarArticleFastRouteArchitectureSelectionError),
    #[error(transparent)]
    ThroughputFloor(#[from] TassadarArticleFastRouteThroughputFloorReportError),
    #[error(transparent)]
    Parity(#[from] TassadarArticleFixtureTransformerParityError),
    #[error(transparent)]
    WeightProduction(#[from] TassadarArticleTransformerWeightProductionError),
    #[error(transparent)]
    WeightProductionEvidence(#[from] WeightProductionEvidenceError),
    #[error(transparent)]
    WeightLineage(#[from] TassadarArticleTransformerWeightLineageError),
    #[error(transparent)]
    Model(#[from] psionic_models::TassadarArticleTransformerError),
    #[error(transparent)]
    ArticleSession(#[from] crate::TassadarArticleExecutorSessionServiceError),
    #[error(transparent)]
    HybridWorkflow(#[from] crate::TassadarArticleHybridWorkflowServiceError),
    #[error(transparent)]
    RouteDescriptor(#[from] TassadarPlannerRouteDescriptorError),
    #[error(transparent)]
    DirectProof(#[from] TassadarDirectModelWeightExecutionProofReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to execute canonical article case `{case_id}` on the fixture baseline: {detail}")]
    FixtureExecution { case_id: String, detail: String },
    #[error("candidate `{candidate_id}` is missing parity evidence for case `{case_id}`")]
    MissingParityCase {
        candidate_id: String,
        case_id: String,
    },
    #[error("candidate `{candidate_id}` invariant failed: {detail}")]
    CandidateInvariant { candidate_id: String, detail: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy)]
struct MinimalFrontierCandidateConfig {
    candidate_id: &'static str,
    hidden_size: usize,
    feed_forward_size: usize,
    encoder_layer_count: usize,
    decoder_layer_count: usize,
    head_count: usize,
}

pub fn build_tassadar_article_transformer_minimal_frontier_report(
) -> Result<TassadarArticleTransformerMinimalFrontierReport, TassadarArticleTransformerMinimalFrontierError>
{
    let acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport =
        read_repo_json(TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF)?;
    let fast_route_selection: TassadarArticleFastRouteArchitectureSelectionReport =
        read_repo_json(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)?;
    let throughput_floor: TassadarArticleFastRouteThroughputFloorReport =
        read_repo_json(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF)?;
    let expected_environment_refs = vec![String::from(
        TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF,
    )];
    let mut candidate_reports = Vec::new();
    for (issue_order, candidate) in selected_candidate_frontier()?.into_iter().enumerate() {
        candidate_reports.push(build_candidate_report(
            candidate,
            issue_order,
            expected_environment_refs.as_slice(),
            &fast_route_selection,
            &throughput_floor,
        )?);
    }
    let successful_candidate_ids = candidate_reports
        .iter()
        .filter(|report| report.all_stages_green)
        .map(|report| report.spec.candidate_id.clone())
        .collect::<Vec<_>>();
    let minimal_successful = candidate_reports
        .iter()
        .filter(|report| report.all_stages_green)
        .min_by_key(|report| (report.spec.parameter_scalar_count, report.spec.issue_order));
    let frontier_green = minimal_successful.is_some();
    let minimal_successful_candidate_id =
        minimal_successful.map(|report| report.spec.candidate_id.clone());
    let minimal_successful_model_id =
        minimal_successful.map(|report| report.spec.produced_model_id.clone());
    let mut report = TassadarArticleTransformerMinimalFrontierReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.article_transformer_minimal_frontier.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_CHECKER_REF,
        ),
        issue_id: String::from(ISSUE_ID),
        issue_title: String::from(ISSUE_TITLE),
        acceptance_gate_report_ref: String::from(
            psionic_eval::TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        acceptance_gate_article_equivalence_green: acceptance_gate.article_equivalence_green,
        canonical_optional_issue_still_open: acceptance_gate
            .optional_open_issue_ids
            .iter()
            .any(|issue_id| issue_id == ISSUE_ID),
        run_root_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_RUN_ROOT_REF),
        successful_candidate_ids,
        minimal_successful_candidate_id,
        minimal_successful_model_id,
        frontier_green,
        candidate_reports,
        claim_boundary: String::from(
            "this report freezes one research-only minimal-size frontier over six article-transformer candidate configurations. It keeps candidate-specific base and trained artifacts, candidate lineage manifests, reference-linear behavior checks, direct-proof rebinding, and fast-route route-surface checks explicit while leaving the canonical TAS-158 acceptance gate, canonical TAS-169A lineage contract, and the public article-equivalence claim unchanged.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Minimal article-transformer frontier now records candidate_count={}, successful_candidate_count={}, minimal_successful_candidate_id={}, canonical_optional_issue_still_open={}, and frontier_green={}.",
        report.candidate_reports.len(),
        report.successful_candidate_ids.len(),
        report
            .minimal_successful_candidate_id
            .as_deref()
            .unwrap_or("none"),
        report.canonical_optional_issue_still_open,
        report.frontier_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_minimal_frontier_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_article_transformer_minimal_frontier_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_REPORT_REF)
}

pub fn write_tassadar_article_transformer_minimal_frontier_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerMinimalFrontierReport,
    TassadarArticleTransformerMinimalFrontierError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerMinimalFrontierError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let run_root = repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_RUN_ROOT_REF);
    fs::create_dir_all(&run_root).map_err(|error| {
        TassadarArticleTransformerMinimalFrontierError::CreateDir {
            path: run_root.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_article_transformer_minimal_frontier_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerMinimalFrontierError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn candidate_frontier() -> [MinimalFrontierCandidateConfig; 6] {
    [
        MinimalFrontierCandidateConfig {
            candidate_id: "h8_ff16_e2_d2_heads2",
            hidden_size: 8,
            feed_forward_size: 16,
            encoder_layer_count: 2,
            decoder_layer_count: 2,
            head_count: 2,
        },
        MinimalFrontierCandidateConfig {
            candidate_id: "h6_ff12_e2_d2_heads2",
            hidden_size: 6,
            feed_forward_size: 12,
            encoder_layer_count: 2,
            decoder_layer_count: 2,
            head_count: 2,
        },
        MinimalFrontierCandidateConfig {
            candidate_id: "h4_ff8_e2_d2_heads2",
            hidden_size: 4,
            feed_forward_size: 8,
            encoder_layer_count: 2,
            decoder_layer_count: 2,
            head_count: 2,
        },
        MinimalFrontierCandidateConfig {
            candidate_id: "h8_ff16_e1_d1_heads2",
            hidden_size: 8,
            feed_forward_size: 16,
            encoder_layer_count: 1,
            decoder_layer_count: 1,
            head_count: 2,
        },
        MinimalFrontierCandidateConfig {
            candidate_id: "h6_ff12_e1_d1_heads2",
            hidden_size: 6,
            feed_forward_size: 12,
            encoder_layer_count: 1,
            decoder_layer_count: 1,
            head_count: 2,
        },
        MinimalFrontierCandidateConfig {
            candidate_id: "h4_ff8_e1_d1_heads2",
            hidden_size: 4,
            feed_forward_size: 8,
            encoder_layer_count: 1,
            decoder_layer_count: 1,
            head_count: 2,
        },
    ]
}

fn selected_candidate_frontier(
) -> Result<Vec<MinimalFrontierCandidateConfig>, TassadarArticleTransformerMinimalFrontierError> {
    let candidates = candidate_frontier().into_iter().collect::<Vec<_>>();
    let Some(filter) = env::var_os("PSIONIC_TASSADAR_MINIMAL_FRONTIER_CANDIDATE_FILTER") else {
        return Ok(candidates);
    };
    let filter = filter.to_string_lossy().trim().to_string();
    let requested = filter
        .split(',')
        .map(str::trim)
        .filter(|candidate_id| !candidate_id.is_empty())
        .collect::<BTreeSet<_>>();
    let filtered = candidates
        .into_iter()
        .filter(|candidate| requested.contains(candidate.candidate_id))
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        return Err(TassadarArticleTransformerMinimalFrontierError::CandidateInvariant {
            candidate_id: String::from("candidate_filter"),
            detail: format!("no frontier candidates matched filter `{filter}`"),
        });
    }
    Ok(filtered)
}

fn build_candidate_report(
    candidate: MinimalFrontierCandidateConfig,
    issue_order: usize,
    expected_environment_refs: &[String],
    fast_route_selection: &psionic_eval::TassadarArticleFastRouteArchitectureSelectionReport,
    throughput_floor: &psionic_eval::TassadarArticleFastRouteThroughputFloorReport,
) -> Result<
    TassadarArticleTransformerMinimalFrontierCandidateReport,
    TassadarArticleTransformerMinimalFrontierError,
> {
    trace_progress(&format!("candidate {}: start", candidate.candidate_id));
    let config = candidate_model_config(candidate);
    let artifact_refs = candidate_artifact_refs(candidate);
    let base_model_id = format!(
        "tassadar-article-transformer-minimal-frontier-{}-base-v0",
        candidate.candidate_id
    );
    let produced_model_id = format!(
        "tassadar-article-transformer-minimal-frontier-{}-trained-v0",
        candidate.candidate_id
    );
    let suite_model = TassadarArticleTransformer::paper_faithful_reference(
        config.clone(),
        TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput,
    )?
    .with_model_identity(base_model_id.clone(), "v0")?;
    let parameter_scalar_count = suite_model
        .trainable_parameter_vectors()
        .iter()
        .map(|parameter| parameter.values.len())
        .sum::<usize>();
    let suite = TassadarArticleTransformerWeightProductionSuite::for_model(&suite_model)?;
    let run_config = candidate_weight_production_config(
        candidate,
        config,
        artifact_refs.clone(),
        base_model_id.clone(),
        produced_model_id.clone(),
    )?;
    let outcome = run_tassadar_article_transformer_weight_production(&suite, &run_config)?;
    let evidence_bundle = build_tassadar_article_transformer_weight_production_evidence_bundle(
        &suite,
        &run_config,
        &outcome,
    );
    let evidence_bundle = write_tassadar_article_transformer_weight_production_evidence_bundle(
        repo_ref_path(artifact_refs.evidence_bundle_ref.as_str()),
        &evidence_bundle,
    )?;
    let lineage_contract = build_tassadar_article_transformer_weight_lineage_contract_for_bundle(
        artifact_refs.evidence_bundle_ref.clone(),
        &evidence_bundle,
        &outcome.base_model,
        &outcome.produced_model,
        format!(
            "tassadar.article_transformer_minimal_frontier.{}.lineage_contract.v1",
            candidate.candidate_id
        ),
        ISSUE_ID,
        "this research-only lineage contract freezes one TAS-R1 candidate artifact pair without rebinding the canonical TAS-169A lineage contract. It binds the candidate-specific workload set, training-config snapshot, source inventory, descriptor digests, checkpoint lineage, and committed candidate artifact digests into one challengeable manifest while keeping the canonical public article-equivalence claim unchanged.",
    )?;
    write_repo_json(artifact_refs.lineage_contract_ref.as_str(), &lineage_contract)?;

    let stage_a_review = build_stage_a_review(
        &suite,
        &artifact_refs,
        &evidence_bundle,
        candidate.candidate_id,
    );
    trace_progress(&format!(
        "candidate {}: stage_a passed={}",
        candidate.candidate_id, stage_a_review.passed
    ));
    let stage_b_review = if stage_a_review.passed {
        build_stage_b_review(candidate.candidate_id, &outcome.produced_model, expected_environment_refs)?
    } else {
        skipped_stage_b_review("skipped because Stage A did not pass")
    };
    trace_progress(&format!(
        "candidate {}: stage_b passed={}",
        candidate.candidate_id, stage_b_review.passed
    ));
    let executor_service = LocalTassadarExecutorService::new()
        .with_model_descriptor(tassadar_article_transformer_executor_descriptor(
            &outcome.produced_model,
        ));
    let stage_c_review = if stage_b_review.passed {
        build_stage_c_review(
            candidate.candidate_id,
            &produced_model_id,
            &lineage_contract,
            &stage_b_review.parity_case_rows,
            &executor_service,
            fast_route_selection,
            throughput_floor,
        )?
    } else {
        skipped_stage_c_review("skipped because Stage B did not pass")
    };
    trace_progress(&format!(
        "candidate {}: stage_c passed={}",
        candidate.candidate_id, stage_c_review.passed
    ));
    let spec = TassadarArticleTransformerMinimalFrontierCandidateSpec {
        candidate_id: String::from(candidate.candidate_id),
        issue_order,
        hidden_size: candidate.hidden_size,
        feed_forward_size: candidate.feed_forward_size,
        encoder_layer_count: candidate.encoder_layer_count,
        decoder_layer_count: candidate.decoder_layer_count,
        head_count: candidate.head_count,
        base_model_id,
        produced_model_id,
        parameter_scalar_count,
    };
    let all_stages_green = stage_a_review.passed && stage_b_review.passed && stage_c_review.passed;
    let detail = format!(
        "stage_a_green={} stage_b_green={} stage_c_green={}",
        stage_a_review.passed, stage_b_review.passed, stage_c_review.passed
    );
    Ok(TassadarArticleTransformerMinimalFrontierCandidateReport {
        spec,
        artifact_refs,
        evidence_bundle_digest: evidence_bundle.bundle_digest,
        lineage_contract_digest: lineage_contract.contract_digest,
        stage_a_review,
        stage_b_review,
        stage_c_review,
        all_stages_green,
        detail,
    })
}

fn trace_progress(message: &str) {
    if env::var_os("PSIONIC_TASSADAR_MINIMAL_FRONTIER_TRACE_PROGRESS").is_some() {
        eprintln!("{message}");
    }
    if let Some(path) = env::var_os("PSIONIC_TASSADAR_MINIMAL_FRONTIER_TRACE_FILE") {
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| {
                use std::io::Write;
                writeln!(file, "{message}")
            });
    }
}

fn candidate_model_config(candidate: MinimalFrontierCandidateConfig) -> EncoderDecoderTransformerConfig {
    let tokenizer = TassadarTraceTokenizer::new();
    let mut config = TassadarArticleTransformer::trace_domain_reference_config(&tokenizer);
    config.hidden_size = candidate.hidden_size;
    config.feed_forward_size = candidate.feed_forward_size;
    config.encoder_layer_count = candidate.encoder_layer_count;
    config.decoder_layer_count = candidate.decoder_layer_count;
    config.head_count = candidate.head_count;
    config
}

fn candidate_artifact_refs(
    candidate: MinimalFrontierCandidateConfig,
) -> TassadarArticleTransformerMinimalFrontierArtifactRefs {
    let run_root_ref = format!(
        "{}/{}",
        TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_RUN_ROOT_REF,
        candidate.candidate_id
    );
    TassadarArticleTransformerMinimalFrontierArtifactRefs {
        base_descriptor_ref: format!("{run_root_ref}/base_descriptor.json"),
        base_artifact_ref: format!("{run_root_ref}/base_weights.safetensors"),
        produced_descriptor_ref: format!("{run_root_ref}/trained_descriptor.json"),
        produced_artifact_ref: format!("{run_root_ref}/trained_weights.safetensors"),
        evidence_bundle_ref: format!("{run_root_ref}/weight_production_bundle.json"),
        lineage_contract_ref: format!("{run_root_ref}/lineage_contract.json"),
        run_root_ref,
    }
}

fn candidate_weight_production_config(
    candidate: MinimalFrontierCandidateConfig,
    model_config: EncoderDecoderTransformerConfig,
    artifact_refs: TassadarArticleTransformerMinimalFrontierArtifactRefs,
    base_model_id: String,
    produced_model_id: String,
) -> Result<TassadarArticleTransformerWeightProductionConfig, TassadarArticleTransformerMinimalFrontierError>
{
    let mut config = TassadarArticleTransformerWeightProductionConfig::reference()?;
    config.run_id = format!(
        "tassadar-article-transformer-minimal-frontier-{}-weight-production-v1",
        candidate.candidate_id
    );
    config.checkpoint_family = format!(
        "train.tassadar.article_transformer.minimal_frontier.{}",
        candidate.candidate_id
    );
    config.model_config = model_config;
    config.embedding_strategy =
        TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput;
    config.base_model_id = base_model_id;
    config.base_model_revision = String::from("v0");
    config.base_descriptor_ref = artifact_refs.base_descriptor_ref;
    config.base_artifact_ref = artifact_refs.base_artifact_ref;
    config.produced_descriptor_ref = artifact_refs.produced_descriptor_ref;
    config.produced_artifact_ref = artifact_refs.produced_artifact_ref;
    config.produced_model_id = produced_model_id;
    config.produced_model_revision = String::from("v0");
    Ok(config)
}

fn build_stage_a_review(
    suite: &TassadarArticleTransformerWeightProductionSuite,
    artifact_refs: &TassadarArticleTransformerMinimalFrontierArtifactRefs,
    evidence_bundle: &psionic_eval::TassadarArticleTransformerWeightProductionEvidenceBundle,
    candidate_id: &str,
) -> TassadarArticleTransformerMinimalFrontierStageAReview {
    let required_refs = [
        artifact_refs.base_descriptor_ref.as_str(),
        artifact_refs.base_artifact_ref.as_str(),
        artifact_refs.produced_descriptor_ref.as_str(),
        artifact_refs.produced_artifact_ref.as_str(),
        artifact_refs.evidence_bundle_ref.as_str(),
        artifact_refs.lineage_contract_ref.as_str(),
    ];
    let all_required_refs_exist = required_refs
        .into_iter()
        .all(|relative_ref| repo_ref_path(relative_ref).exists());
    let training_loss_improved =
        evidence_bundle.final_training_mean_loss <= evidence_bundle.initial_training_mean_loss;
    let training_exactness_improved = evidence_bundle.final_training_token_exactness_bps
        >= evidence_bundle.initial_training_token_exactness_bps;
    let weight_production_smoke_passed =
        training_loss_improved && training_exactness_improved && evidence_bundle.step_evidence.len() == 1;
    let passed = suite.training_examples.len() == TRAINING_CASE_COUNT
        && suite.held_out_examples.len() == HELD_OUT_CASE_COUNT
        && all_required_refs_exist
        && weight_production_smoke_passed
        && evidence_bundle.produced_artifact_differs_from_base
        && evidence_bundle.checkpoint.restore_matches_trained_state
        && evidence_bundle.artifact_reload_matches_trained_state;
    TassadarArticleTransformerMinimalFrontierStageAReview {
        suite_id: suite.suite_id.clone(),
        training_example_count: suite.training_examples.len(),
        held_out_example_count: suite.held_out_examples.len(),
        all_required_refs_exist,
        weight_production_smoke_passed,
        training_loss_improved,
        training_exactness_improved,
        produced_artifact_differs_from_base: evidence_bundle.produced_artifact_differs_from_base,
        checkpoint_restore_matches_trained_state: evidence_bundle
            .checkpoint
            .restore_matches_trained_state,
        artifact_reload_matches_trained_state: evidence_bundle.artifact_reload_matches_trained_state,
        passed,
        detail: format!(
            "candidate_id={} suite_counts={}/{} refs_exist={} smoke_passed={} produced_artifact_differs_from_base={} checkpoint_restore_matches={} artifact_reload_matches={}",
            candidate_id,
            suite.training_examples.len(),
            suite.held_out_examples.len(),
            all_required_refs_exist,
            weight_production_smoke_passed,
            evidence_bundle.produced_artifact_differs_from_base,
            evidence_bundle.checkpoint.restore_matches_trained_state,
            evidence_bundle.artifact_reload_matches_trained_state
        ),
    }
}

fn build_stage_b_review(
    candidate_id: &str,
    model: &TassadarArticleTransformer,
    _expected_environment_refs: &[String],
) -> Result<
    TassadarArticleTransformerMinimalFrontierStageBReview,
    TassadarArticleTransformerMinimalFrontierError,
> {
    let generalization_report: TassadarArticleTransformerGeneralizationGateReport =
        read_repo_json(TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF)?;
    let (
        parity_case_rows,
        declared_case_ids,
        _corpus_digest,
        _fixture,
        _transformer_model_artifact,
    ) = build_tassadar_article_fixture_transformer_behavior_case_rows_for_model(model)?;
    let observed_case_ids = parity_case_rows
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<BTreeSet<_>>();
    let parity_all_declared_cases_present = !parity_case_rows.is_empty()
        && observed_case_ids == declared_case_ids.iter().cloned().collect::<BTreeSet<_>>();
    let parity_all_cases_pass = parity_case_rows.iter().all(|row| row.case_passed);
    let parity_passed = parity_all_declared_cases_present && parity_all_cases_pass;

    let reference_linear_case_rows =
        build_reference_linear_exactness_case_rows_from_parity(&parity_case_rows);
    let reference_linear_exact_case_count = reference_linear_case_rows
        .iter()
        .filter(|row| row.exactness_posture == TassadarExactnessPosture::Exact)
        .count();
    let reference_linear_mismatch_case_count = reference_linear_case_rows
        .iter()
        .filter(|row| row.exactness_posture == TassadarExactnessPosture::Mismatch)
        .count();
    let reference_linear_refused_case_count = reference_linear_case_rows
        .iter()
        .filter(|row| row.exactness_posture == TassadarExactnessPosture::Refused)
        .count();
    let reference_linear_passed = reference_linear_exact_case_count
        == reference_linear_case_rows.len()
        && reference_linear_mismatch_case_count == 0
        && reference_linear_refused_case_count == 0;

    if !parity_passed || !reference_linear_passed {
        return Ok(TassadarArticleTransformerMinimalFrontierStageBReview {
            parity_case_rows,
            parity_all_declared_cases_present,
            parity_all_cases_pass,
            parity_passed,
            reference_linear_case_rows,
            reference_linear_exact_case_count,
            reference_linear_mismatch_case_count,
            reference_linear_refused_case_count,
            reference_linear_passed,
            generalization_report_ref: String::from(
                TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
            ),
            canonical_generalization_green: generalization_report.generalization_green,
            canonical_generalization_case_count: generalization_report.case_count,
            generalization_passed: false,
            passed: false,
            detail: format!(
                "candidate_id={} parity_passed={} reference_linear_passed={} canonical_generalization_green={} generalization_skipped=true",
                candidate_id,
                parity_passed,
                reference_linear_passed,
                generalization_report.generalization_green,
            ),
        });
    }

    let generalization_passed =
        generalization_report.generalization_green && generalization_report.case_count > 0;

    let passed = parity_passed && reference_linear_passed && generalization_passed;
    Ok(TassadarArticleTransformerMinimalFrontierStageBReview {
        parity_case_rows,
        parity_all_declared_cases_present,
        parity_all_cases_pass,
        parity_passed,
        reference_linear_case_rows,
        reference_linear_exact_case_count,
        reference_linear_mismatch_case_count,
        reference_linear_refused_case_count,
        reference_linear_passed,
        generalization_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
        ),
        canonical_generalization_green: generalization_report.generalization_green,
        canonical_generalization_case_count: generalization_report.case_count,
        generalization_passed,
        passed,
        detail: format!(
            "candidate_id={} parity_passed={} reference_linear_passed={} canonical_generalization_green={} canonical_generalization_case_count={} generalization_passed={}",
            candidate_id,
            parity_passed,
            reference_linear_passed,
            generalization_report.generalization_green,
            generalization_report.case_count,
            generalization_passed
        ),
    })
}

fn build_stage_c_review(
    candidate_id: &str,
    candidate_model_id: &str,
    lineage_contract: &TassadarArticleTransformerWeightLineageContract,
    parity_case_rows: &[TassadarArticleFixtureTransformerParityCaseRow],
    executor_service: &LocalTassadarExecutorService,
    fast_route_selection: &psionic_eval::TassadarArticleFastRouteArchitectureSelectionReport,
    throughput_floor: &psionic_eval::TassadarArticleFastRouteThroughputFloorReport,
) -> Result<
    TassadarArticleTransformerMinimalFrontierStageCReview,
    TassadarArticleTransformerMinimalFrontierError,
> {
    let planner_router = LocalTassadarPlannerRouter::new().with_executor_service(executor_service.clone());
    let route_descriptor = planner_router.route_capability_descriptor(Some(candidate_model_id))?;
    let direct_proof_review = build_direct_proof_review(
        candidate_id,
        candidate_model_id,
        lineage_contract,
        parity_case_rows,
        executor_service,
        &route_descriptor,
    )?;
    let fast_route_selection_review = build_fast_route_selection_review(
        &route_descriptor,
        fast_route_selection,
    );
    let throughput_floor_review = build_throughput_floor_review(throughput_floor);
    let (article_session_reviews, hybrid_workflow_reviews) = if direct_proof_review.passed
        && fast_route_selection_review.passed
    {
        (
            build_fast_route_session_reviews(
                candidate_model_id,
                parity_case_rows,
                executor_service,
            )?,
            build_fast_route_hybrid_reviews(
                candidate_model_id,
                parity_case_rows,
                executor_service,
            )?,
        )
    } else {
        (
            skipped_fast_route_session_reviews(
                candidate_id,
                parity_case_rows,
                "skipped because direct proof or fast-route selection did not pass",
            )?,
            skipped_fast_route_hybrid_reviews(
                candidate_id,
                parity_case_rows,
                "skipped because direct proof or fast-route selection did not pass",
            )?,
        )
    };
    let passed = direct_proof_review.passed
        && fast_route_selection_review.passed
        && article_session_reviews
            .iter()
            .all(|review| review.exact_direct_hull_cache)
        && hybrid_workflow_reviews
            .iter()
            .all(|review| review.exact_direct_hull_cache)
        && throughput_floor_review.passed;
    Ok(TassadarArticleTransformerMinimalFrontierStageCReview {
        direct_proof_review,
        fast_route_selection_review,
        article_session_reviews,
        hybrid_workflow_reviews,
        throughput_floor_review,
        passed,
        detail: format!("candidate_id={} stage_c_green={}", candidate_id, passed),
    })
}

fn build_reference_linear_exactness_case_rows_from_parity(
    parity_case_rows: &[TassadarArticleFixtureTransformerParityCaseRow],
) -> Vec<TassadarArticleTransformerMinimalFrontierExactnessCaseRow> {
    parity_case_rows
        .iter()
        .map(|row| {
            let exactness_posture = if row.case_passed {
                TassadarExactnessPosture::Exact
            } else if !row.transformer_routeable {
                TassadarExactnessPosture::Refused
            } else {
                TassadarExactnessPosture::Mismatch
            };
            TassadarArticleTransformerMinimalFrontierExactnessCaseRow {
                case_id: row.case_id.clone(),
                program_id: row.program_id.clone(),
                requested_decode_mode: row.requested_decode_mode,
                trace_step_count: row.fixture_trace_step_count,
                prompt_token_count: row.prompt_token_count,
                target_token_count: row.target_token_count,
                within_transformer_context_window: row.within_transformer_context_window,
                exactness_posture,
                fixture_trace_digest: row.fixture_trace_digest.clone(),
                transformer_trace_digest: row.roundtrip_trace_digest.clone(),
                detail: row.detail.clone(),
            }
        })
        .collect()
}

fn build_direct_proof_review(
    candidate_id: &str,
    candidate_model_id: &str,
    lineage_contract: &TassadarArticleTransformerWeightLineageContract,
    parity_case_rows: &[TassadarArticleFixtureTransformerParityCaseRow],
    executor_service: &LocalTassadarExecutorService,
    route_descriptor: &psionic_router::TassadarPlannerExecutorRouteDescriptor,
) -> Result<
    TassadarArticleTransformerMinimalFrontierDirectProofReview,
    TassadarArticleTransformerMinimalFrontierError,
> {
    let article_session_service = LocalTassadarArticleExecutorSessionService::new()
        .with_executor_service(executor_service.clone());
    let mut case_reviews = Vec::new();
    let ordered_case_ids = ordered_stage_c_case_ids(candidate_id, parity_case_rows)?;
    let mut keep_evaluating = true;
    for case_id in ordered_case_ids {
        if !keep_evaluating {
            case_reviews.push(
                TassadarArticleTransformerMinimalFrontierDirectProofCaseReview {
                    case_id: String::from(case_id),
                    receipt: None,
                    direct_selection: false,
                    fallback_free: false,
                    zero_external_call: false,
                    route_digest_matches: false,
                    lineage_matches: false,
                    trace_digest_matches_fixture: false,
                    passed: false,
                    detail: String::from(
                        "skipped because an earlier cheaper direct-proof case already failed",
                    ),
                },
            );
            continue;
        }
        let parity_row = parity_row_for_case(candidate_id, parity_case_rows, case_id)?;
        trace_progress(&format!(
            "candidate {}: direct_proof case {} start trace_steps={} target_tokens={}",
            candidate_id,
            case_id,
            parity_row.fixture_trace_step_count,
            parity_row.target_token_count
        ));
        if let Err(detail) = ensure_case_supports_direct_proof(parity_row) {
            case_reviews.push(
                TassadarArticleTransformerMinimalFrontierDirectProofCaseReview {
                    case_id: String::from(case_id),
                    receipt: None,
                    direct_selection: false,
                    fallback_free: false,
                    zero_external_call: false,
                    route_digest_matches: false,
                    lineage_matches: false,
                    trace_digest_matches_fixture: false,
                    passed: false,
                    detail,
                },
            );
            keep_evaluating = false;
            continue;
        }
        let request = TassadarArticleExecutorSessionRequest::new(
            format!("minimal-frontier-direct-proof-{candidate_id}-{case_id}"),
            case_id,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )
        .with_requested_model_id(candidate_model_id);
        let outcome = article_session_service.execute_without_derived_views(&request)?;
        match outcome {
            TassadarArticleExecutorSessionOutcome::Completed { response } => {
                let receipt =
                    build_tassadar_direct_model_weight_execution_proof_receipt_for_article_session_with_lineage(
                        executor_service,
                        &request,
                        &response,
                        lineage_contract.evidence_bundle_ref.clone().replace(
                            "weight_production_bundle.json",
                            "lineage_contract.json",
                        ),
                        lineage_contract.contract_digest.clone(),
                    )?;
                let direct_selection =
                    receipt.selection_state == TassadarExecutorSelectionState::Direct
                        && receipt.effective_decode_mode
                            == TassadarExecutorDecodeMode::ReferenceLinear;
                let fallback_free = !receipt.fallback_observed;
                let zero_external_call =
                    receipt.external_call_count == 0 && !receipt.cpu_result_substitution_observed;
                let route_digest_matches =
                    receipt.route_binding.route_descriptor_digest == route_descriptor.descriptor_digest;
                let lineage_matches = receipt.model_lineage_contract_digest
                    == lineage_contract.contract_digest
                    && receipt.model_lineage_contract_ref
                        == lineage_contract.evidence_bundle_ref.replace(
                            "weight_production_bundle.json",
                            "lineage_contract.json",
                        );
                let trace_digest_matches_fixture =
                    receipt.trace_digest == parity_row.fixture_trace_digest;
                let passed = receipt.model_id == candidate_model_id
                    && direct_selection
                    && fallback_free
                    && zero_external_call
                    && route_digest_matches
                    && lineage_matches
                    && trace_digest_matches_fixture;
                case_reviews.push(
                    TassadarArticleTransformerMinimalFrontierDirectProofCaseReview {
                        case_id: String::from(case_id),
                        receipt: Some(receipt),
                        direct_selection,
                        fallback_free,
                        zero_external_call,
                        route_digest_matches,
                        lineage_matches,
                        trace_digest_matches_fixture,
                        passed,
                        detail: format!(
                            "model_matches={} direct_selection={} fallback_free={} zero_external_call={} route_digest_matches={} lineage_matches={} trace_digest_matches_fixture={}",
                            true,
                            direct_selection,
                            fallback_free,
                            zero_external_call,
                            route_digest_matches,
                            lineage_matches,
                            trace_digest_matches_fixture,
                        ),
                    },
                );
                trace_progress(&format!(
                    "candidate {}: direct_proof case {} passed={}",
                    candidate_id, case_id, passed
                ));
                if !passed {
                    keep_evaluating = false;
                }
            }
            TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
                case_reviews.push(
                    TassadarArticleTransformerMinimalFrontierDirectProofCaseReview {
                        case_id: String::from(case_id),
                        receipt: None,
                        direct_selection: false,
                        fallback_free: false,
                        zero_external_call: false,
                        route_digest_matches: false,
                        lineage_matches: false,
                        trace_digest_matches_fixture: false,
                        passed: false,
                        detail: refusal.detail,
                    },
                );
                trace_progress(&format!(
                    "candidate {}: direct_proof case {} passed=false refused",
                    candidate_id, case_id
                ));
                keep_evaluating = false;
            }
        }
    }
    let direct_case_count = case_reviews.iter().filter(|review| review.direct_selection).count();
    let fallback_free_case_count =
        case_reviews.iter().filter(|review| review.fallback_free).count();
    let zero_external_call_case_count = case_reviews
        .iter()
        .filter(|review| review.zero_external_call)
        .count();
    let passed = case_reviews.iter().all(|review| review.passed);
    Ok(TassadarArticleTransformerMinimalFrontierDirectProofReview {
        route_descriptor_digest: route_descriptor.descriptor_digest.clone(),
        case_reviews,
        direct_case_count,
        fallback_free_case_count,
        zero_external_call_case_count,
        passed,
        detail: format!(
            "direct_case_count={} fallback_free_case_count={} zero_external_call_case_count={} passed={}",
            direct_case_count, fallback_free_case_count, zero_external_call_case_count, passed
        ),
    })
}

fn build_fast_route_selection_review(
    route_descriptor: &psionic_router::TassadarPlannerExecutorRouteDescriptor,
    fast_route_selection: &psionic_eval::TassadarArticleFastRouteArchitectureSelectionReport,
) -> TassadarArticleTransformerMinimalFrontierFastRouteSelectionReview {
    let candidate_hull_cache_direct_guaranteed = route_descriptor
        .decode_capabilities
        .iter()
        .find(|capability| {
            capability.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
        })
        .map(|capability| {
            capability.route_posture
                == psionic_router::TassadarPlannerExecutorRoutePosture::DirectGuaranteed
        })
        .unwrap_or(false);
    let canonical_selected_candidate_kind =
        fast_route_selection.selected_candidate_kind.label().to_string();
    let passed = fast_route_selection.fast_route_selection_green
        && fast_route_selection.selected_candidate_kind
            == TassadarArticleFastRouteCandidateKind::HullCacheRuntime
        && candidate_hull_cache_direct_guaranteed;
    TassadarArticleTransformerMinimalFrontierFastRouteSelectionReview {
        report_ref: String::from(
            psionic_eval::TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        ),
        canonical_selected_candidate_kind,
        canonical_fast_route_selection_green: fast_route_selection.fast_route_selection_green,
        candidate_route_descriptor_digest: route_descriptor.descriptor_digest.clone(),
        candidate_hull_cache_direct_guaranteed,
        passed,
        detail: format!(
            "canonical_fast_route_selection_green={} canonical_selected_candidate_kind={} candidate_hull_cache_direct_guaranteed={}",
            fast_route_selection.fast_route_selection_green,
            fast_route_selection.selected_candidate_kind.label(),
            candidate_hull_cache_direct_guaranteed
        ),
    }
}

fn build_fast_route_session_reviews(
    candidate_model_id: &str,
    parity_case_rows: &[TassadarArticleFixtureTransformerParityCaseRow],
    executor_service: &LocalTassadarExecutorService,
) -> Result<
    Vec<TassadarArticleTransformerMinimalFrontierFastRouteSessionCaseReview>,
    TassadarArticleTransformerMinimalFrontierError,
> {
    let article_session_service = LocalTassadarArticleExecutorSessionService::new()
        .with_executor_service(executor_service.clone());
    let mut reviews = Vec::new();
    let mut keep_evaluating = true;
    for case_id in ordered_stage_c_case_ids(candidate_model_id, parity_case_rows)? {
        let parity_row = parity_row_for_case(candidate_model_id, parity_case_rows, case_id)?;
        if !keep_evaluating {
            reviews.push(
                TassadarArticleTransformerMinimalFrontierFastRouteSessionCaseReview {
                    case_id: String::from(case_id),
                    fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                    observed_trace_digest: None,
                    selection_state: None,
                    effective_decode_mode: None,
                    exact_direct_hull_cache: false,
                    detail: String::from(
                        "skipped because an earlier cheaper fast-route session case already failed",
                    ),
                },
            );
            continue;
        }
        let request = TassadarArticleExecutorSessionRequest::new(
            format!("minimal-frontier-fast-route-session-{candidate_model_id}-{case_id}"),
            case_id,
            TassadarExecutorDecodeMode::HullCache,
        )
        .with_requested_model_id(candidate_model_id);
        let outcome = article_session_service.execute(&request)?;
        let review = match outcome {
            TassadarArticleExecutorSessionOutcome::Completed { response } => {
                let selection = response.executor_response.execution_report.selection;
                let exact_direct_hull_cache = selection.selection_state
                    == TassadarExecutorSelectionState::Direct
                    && selection.effective_decode_mode
                        == Some(TassadarExecutorDecodeMode::HullCache)
                    && response.proof_identity.trace_digest == parity_row.fixture_trace_digest;
                TassadarArticleTransformerMinimalFrontierFastRouteSessionCaseReview {
                    case_id: String::from(case_id),
                    fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                    observed_trace_digest: Some(response.proof_identity.trace_digest.clone()),
                    selection_state: Some(selection.selection_state),
                    effective_decode_mode: selection.effective_decode_mode,
                    exact_direct_hull_cache,
                    detail: format!(
                        "selection_state={:?} effective_decode_mode={:?} trace_digest_matches_fixture={}",
                        selection.selection_state,
                        selection.effective_decode_mode,
                        response.proof_identity.trace_digest == parity_row.fixture_trace_digest
                    ),
                }
            }
            TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
                TassadarArticleTransformerMinimalFrontierFastRouteSessionCaseReview {
                    case_id: String::from(case_id),
                    fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                    observed_trace_digest: None,
                    selection_state: refusal
                        .selection
                        .as_ref()
                        .map(|selection| selection.selection_state),
                    effective_decode_mode: refusal
                        .selection
                        .as_ref()
                        .and_then(|selection| selection.effective_decode_mode),
                    exact_direct_hull_cache: false,
                    detail: refusal.detail,
                }
            }
        };
        if !review.exact_direct_hull_cache {
            keep_evaluating = false;
        }
        reviews.push(review);
    }
    Ok(reviews)
}

fn build_fast_route_hybrid_reviews(
    candidate_model_id: &str,
    parity_case_rows: &[TassadarArticleFixtureTransformerParityCaseRow],
    executor_service: &LocalTassadarExecutorService,
) -> Result<
    Vec<TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview>,
    TassadarArticleTransformerMinimalFrontierError,
> {
    let planner_router = LocalTassadarPlannerRouter::new().with_executor_service(executor_service.clone());
    let hybrid_workflow_service = LocalTassadarArticleHybridWorkflowService::new()
        .with_planner_router(planner_router);
    let mut reviews = Vec::new();
    let mut keep_evaluating = true;
    for case_id in ordered_stage_c_case_ids(candidate_model_id, parity_case_rows)? {
        let parity_row = parity_row_for_case(candidate_model_id, parity_case_rows, case_id)?;
        if !keep_evaluating {
            reviews.push(
                TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview {
                    case_id: String::from(case_id),
                    fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                    observed_trace_digest: None,
                    planner_effective_decode_mode: None,
                    executor_effective_decode_mode: None,
                    selection_state: None,
                    exact_direct_hull_cache: false,
                    detail: String::from(
                        "skipped because an earlier cheaper hybrid fast-route case already failed",
                    ),
                },
            );
            continue;
        }
        let request = TassadarArticleHybridWorkflowRequest::new(
            format!("minimal-frontier-fast-route-hybrid-{candidate_model_id}-{case_id}"),
            format!("planner-session-{candidate_model_id}-{case_id}"),
            "planner-article-fixture-v0",
            format!("workflow-step-{case_id}"),
            format!("delegate exact `{case_id}` article workload into Tassadar"),
            case_id,
            TassadarExecutorDecodeMode::HullCache,
        )
        .with_requested_model_id(candidate_model_id);
        let outcome = hybrid_workflow_service.execute(&request)?;
        let review = match outcome {
            TassadarArticleHybridWorkflowOutcome::Completed { response } => {
                let routing = response.planner_response.routing_decision;
                let selection =
                    response.planner_response.executor_response.execution_report.selection;
                let exact_direct_hull_cache = routing.effective_decode_mode
                    == Some(TassadarExecutorDecodeMode::HullCache)
                    && selection.selection_state == TassadarExecutorSelectionState::Direct
                    && selection.effective_decode_mode
                        == Some(TassadarExecutorDecodeMode::HullCache)
                    && response.proof_identity.trace_digest == parity_row.fixture_trace_digest;
                TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview {
                    case_id: String::from(case_id),
                    fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                    observed_trace_digest: Some(response.proof_identity.trace_digest.clone()),
                    planner_effective_decode_mode: routing.effective_decode_mode,
                    executor_effective_decode_mode: selection.effective_decode_mode,
                    selection_state: Some(selection.selection_state),
                    exact_direct_hull_cache,
                    detail: format!(
                        "planner_effective_decode_mode={:?} executor_selection_state={:?} executor_effective_decode_mode={:?} trace_digest_matches_fixture={}",
                        routing.effective_decode_mode,
                        selection.selection_state,
                        selection.effective_decode_mode,
                        response.proof_identity.trace_digest == parity_row.fixture_trace_digest
                    ),
                }
            }
            TassadarArticleHybridWorkflowOutcome::Fallback { fallback } => {
                TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview {
                    case_id: String::from(case_id),
                    fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                    observed_trace_digest: None,
                    planner_effective_decode_mode: Some(
                        fallback.planner_fallback.routing_decision.effective_decode_mode,
                    )
                    .flatten(),
                    executor_effective_decode_mode: None,
                    selection_state: None,
                    exact_direct_hull_cache: false,
                    detail: fallback.planner_fallback.routing_decision.detail,
                }
            }
            TassadarArticleHybridWorkflowOutcome::Refused { refusal } => {
                TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview {
                    case_id: String::from(case_id),
                    fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                    observed_trace_digest: None,
                    planner_effective_decode_mode: refusal
                        .planner_refusal
                        .as_ref()
                        .and_then(|planner_refusal| {
                            planner_refusal.routing_decision.effective_decode_mode
                        }),
                    executor_effective_decode_mode: None,
                    selection_state: None,
                    exact_direct_hull_cache: false,
                    detail: refusal.detail,
                }
            }
        };
        if !review.exact_direct_hull_cache {
            keep_evaluating = false;
        }
        reviews.push(review);
    }
    Ok(reviews)
}

fn build_throughput_floor_review(
    throughput_floor: &psionic_eval::TassadarArticleFastRouteThroughputFloorReport,
) -> TassadarArticleTransformerMinimalFrontierThroughputFloorReview {
    let canonical_selected_candidate_kind = throughput_floor
        .selection_prerequisite
        .selected_candidate_kind
        .clone();
    let passed = throughput_floor.throughput_floor_green
        && canonical_selected_candidate_kind == "hull_cache_runtime";
    TassadarArticleTransformerMinimalFrontierThroughputFloorReview {
        report_ref: String::from(
            psionic_eval::TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
        ),
        canonical_selected_candidate_kind,
        throughput_floor_green: throughput_floor.throughput_floor_green,
        passed,
        detail: format!(
            "throughput_floor_green={} selected_candidate_kind={}",
            throughput_floor.throughput_floor_green,
            throughput_floor.selection_prerequisite.selected_candidate_kind
        ),
    }
}

fn parity_row_for_case<'a>(
    candidate_id: &str,
    parity_case_rows: &'a [TassadarArticleFixtureTransformerParityCaseRow],
    case_id: &str,
) -> Result<
    &'a TassadarArticleFixtureTransformerParityCaseRow,
    TassadarArticleTransformerMinimalFrontierError,
> {
    parity_case_rows
        .iter()
        .find(|row| row.case_id == case_id)
        .ok_or_else(|| TassadarArticleTransformerMinimalFrontierError::MissingParityCase {
            candidate_id: String::from(candidate_id),
            case_id: String::from(case_id),
        })
}

fn ordered_stage_c_case_ids<'a>(
    candidate_id: &str,
    parity_case_rows: &'a [TassadarArticleFixtureTransformerParityCaseRow],
) -> Result<Vec<&'a str>, TassadarArticleTransformerMinimalFrontierError> {
    let mut rows = DIRECT_PROOF_CASE_IDS
        .into_iter()
        .map(|case_id| {
            parity_row_for_case(candidate_id, parity_case_rows, case_id)
                .map(|row| (case_id, row.fixture_trace_step_count))
        })
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_by_key(|(_, trace_step_count)| *trace_step_count);
    Ok(rows.into_iter().map(|(case_id, _)| case_id).collect())
}

fn ensure_case_supports_direct_proof(
    parity_row: &TassadarArticleFixtureTransformerParityCaseRow,
) -> Result<(), String> {
    if parity_row.requested_decode_mode != TassadarExecutorDecodeMode::ReferenceLinear {
        return Err(format!(
            "expected reference_linear decode, found `{}`",
            parity_row.requested_decode_mode.as_str()
        ));
    }
    if parity_row.fixture_selection_state != "direct" {
        return Err(format!(
            "fixture baseline selection drifted to `{}`",
            parity_row.fixture_selection_state
        ));
    }
    if parity_row.fixture_effective_decode_mode != Some(TassadarExecutorDecodeMode::ReferenceLinear)
    {
        return Err(format!(
            "fixture baseline effective decode drifted to `{}`",
            parity_row
                .fixture_effective_decode_mode
                .map_or("none", TassadarExecutorDecodeMode::as_str)
        ));
    }
    if !parity_row.fixture_routeable
        || !parity_row.transformer_routeable
        || !parity_row.trace_shape_parity
        || !parity_row.output_parity
        || !parity_row.behavior_parity
        || !parity_row.case_passed
    {
        return Err(parity_row.detail.clone());
    }
    Ok(())
}

fn skipped_stage_b_review(detail: &str) -> TassadarArticleTransformerMinimalFrontierStageBReview {
    TassadarArticleTransformerMinimalFrontierStageBReview {
        parity_case_rows: Vec::new(),
        parity_all_declared_cases_present: false,
        parity_all_cases_pass: false,
        parity_passed: false,
        reference_linear_case_rows: Vec::new(),
        reference_linear_exact_case_count: 0,
        reference_linear_mismatch_case_count: 0,
        reference_linear_refused_case_count: 0,
        reference_linear_passed: false,
        generalization_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
        ),
        canonical_generalization_green: false,
        canonical_generalization_case_count: 0,
        generalization_passed: false,
        passed: false,
        detail: String::from(detail),
    }
}

fn skipped_fast_route_session_reviews(
    candidate_id: &str,
    parity_case_rows: &[TassadarArticleFixtureTransformerParityCaseRow],
    detail: &str,
) -> Result<
    Vec<TassadarArticleTransformerMinimalFrontierFastRouteSessionCaseReview>,
    TassadarArticleTransformerMinimalFrontierError,
> {
    ordered_stage_c_case_ids(candidate_id, parity_case_rows)?
        .into_iter()
        .map(|case_id| {
            let parity_row = parity_row_for_case(candidate_id, parity_case_rows, case_id)?;
            Ok(TassadarArticleTransformerMinimalFrontierFastRouteSessionCaseReview {
                case_id: String::from(case_id),
                fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                observed_trace_digest: None,
                selection_state: None,
                effective_decode_mode: None,
                exact_direct_hull_cache: false,
                detail: String::from(detail),
            })
        })
        .collect()
}

fn skipped_fast_route_hybrid_reviews(
    candidate_id: &str,
    parity_case_rows: &[TassadarArticleFixtureTransformerParityCaseRow],
    detail: &str,
) -> Result<
    Vec<TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview>,
    TassadarArticleTransformerMinimalFrontierError,
> {
    ordered_stage_c_case_ids(candidate_id, parity_case_rows)?
        .into_iter()
        .map(|case_id| {
            let parity_row = parity_row_for_case(candidate_id, parity_case_rows, case_id)?;
            Ok(TassadarArticleTransformerMinimalFrontierFastRouteHybridCaseReview {
                case_id: String::from(case_id),
                fixture_trace_digest: parity_row.fixture_trace_digest.clone(),
                observed_trace_digest: None,
                planner_effective_decode_mode: None,
                executor_effective_decode_mode: None,
                selection_state: None,
                exact_direct_hull_cache: false,
                detail: String::from(detail),
            })
        })
        .collect()
}

fn skipped_stage_c_review(detail: &str) -> TassadarArticleTransformerMinimalFrontierStageCReview {
    TassadarArticleTransformerMinimalFrontierStageCReview {
        direct_proof_review: TassadarArticleTransformerMinimalFrontierDirectProofReview {
            route_descriptor_digest: String::new(),
            case_reviews: Vec::new(),
            direct_case_count: 0,
            fallback_free_case_count: 0,
            zero_external_call_case_count: 0,
            passed: false,
            detail: String::from(detail),
        },
        fast_route_selection_review:
            TassadarArticleTransformerMinimalFrontierFastRouteSelectionReview {
                report_ref: String::from(
                    psionic_eval::TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                ),
                canonical_selected_candidate_kind: String::new(),
                canonical_fast_route_selection_green: false,
                candidate_route_descriptor_digest: String::new(),
                candidate_hull_cache_direct_guaranteed: false,
                passed: false,
                detail: String::from(detail),
            },
        article_session_reviews: Vec::new(),
        hybrid_workflow_reviews: Vec::new(),
        throughput_floor_review: TassadarArticleTransformerMinimalFrontierThroughputFloorReview {
            report_ref: String::from(
                psionic_eval::TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
            ),
            canonical_selected_candidate_kind: String::new(),
            throughput_floor_green: false,
            passed: false,
            detail: String::from(detail),
        },
        passed: false,
        detail: String::from(detail),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-serve should live under <repo>/crates/psionic-serve")
        .to_path_buf()
}

fn repo_ref_path(relative_ref: &str) -> PathBuf {
    let path = Path::new(relative_ref);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root().join(path)
    }
}

fn write_repo_json<T: Serialize>(
    relative_ref: &str,
    value: &T,
) -> Result<(), TassadarArticleTransformerMinimalFrontierError> {
    let path = repo_ref_path(relative_ref);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerMinimalFrontierError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(&path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerMinimalFrontierError::Write {
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

fn read_repo_json<T: DeserializeOwned>(
    relative_ref: &str,
) -> Result<T, TassadarArticleTransformerMinimalFrontierError> {
    let path = repo_ref_path(relative_ref);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleTransformerMinimalFrontierError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerMinimalFrontierError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_minimal_frontier_report, read_repo_json,
        tassadar_article_transformer_minimal_frontier_report_path,
        write_tassadar_article_transformer_minimal_frontier_report,
        TassadarArticleTransformerMinimalFrontierReport,
        TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_REPORT_REF,
    };

    #[test]
    fn minimal_frontier_report_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_minimal_frontier_report().expect("frontier report");
        let committed: TassadarArticleTransformerMinimalFrontierReport =
            read_repo_json(TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_REPORT_REF)
                .expect("committed frontier report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_minimal_frontier_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory.path().join("tassadar_article_transformer_minimal_frontier_report.json");
        let written = write_tassadar_article_transformer_minimal_frontier_report(&output_path)
            .expect("write frontier report");
        let persisted: TassadarArticleTransformerMinimalFrontierReport =
            serde_json::from_str(&std::fs::read_to_string(&output_path).expect("read written"))
                .expect("decode written");

        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_minimal_frontier_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_minimal_frontier_report.json")
        );
    }
}
