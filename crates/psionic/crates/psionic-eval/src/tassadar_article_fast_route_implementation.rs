use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    build_tassadar_article_transformer_trained_executor_descriptor,
    TassadarExecutorModelDescriptor, TASSADAR_ARTICLE_TRANSFORMER_TRAINED_EXECUTOR_DESCRIPTOR_REF,
};
use psionic_runtime::{TassadarExecutorDecodeMode, TassadarExecutorSelectionState};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_implementation_report.json";
pub const TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_CHECKER_REF: &str =
    "scripts/check-tassadar-article-fast-route-implementation.sh";

const DIRECT_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json";
const ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_executor_session_artifact.json";
const ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hybrid_workflow_artifact.json";
const ARTICLE_TRANSFORMER_REPLACEMENT_PUBLICATION_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_replacement_publication.json";
const TIED_REQUIREMENT_ID: &str = "TAS-173";
const ARTICLE_DIRECT_HULL_CASE_NAME: &str = "direct_memory_heavy_hull";
const HYBRID_DIRECT_HULL_CASE_NAME: &str = "delegated_memory_heavy_hull";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteImplementationAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteImplementationDescriptorReview {
    pub descriptor_ref: String,
    pub model_id: String,
    pub model_descriptor_digest: String,
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
    pub hull_cache_supported: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteImplementationReplacementReview {
    pub publication_ref: String,
    pub transformer_model_id: String,
    pub replacement_certified: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteImplementationSessionReview {
    pub artifact_ref: String,
    pub case_name: String,
    pub model_id: String,
    pub model_descriptor_digest: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub fast_path_integrated: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteImplementationHybridReview {
    pub artifact_ref: String,
    pub case_name: String,
    pub model_id: String,
    pub model_descriptor_digest: String,
    pub planner_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub executor_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub fast_path_integrated: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteImplementationDirectProofReview {
    pub report_ref: String,
    pub model_id: String,
    pub model_descriptor_digest: String,
    pub route_descriptor_digest: String,
    pub direct_case_count: u32,
    pub fallback_free_case_count: u32,
    pub zero_external_call_case_count: u32,
    pub descriptor_binding_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteImplementationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleFastRouteImplementationAcceptanceGateTie,
    pub selected_candidate_kind: String,
    pub descriptor_review: TassadarArticleFastRouteImplementationDescriptorReview,
    pub replacement_review: TassadarArticleFastRouteImplementationReplacementReview,
    pub article_session_review: TassadarArticleFastRouteImplementationSessionReview,
    pub hybrid_route_review: TassadarArticleFastRouteImplementationHybridReview,
    pub direct_proof_review: TassadarArticleFastRouteImplementationDirectProofReview,
    pub fast_route_implementation_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct FastRouteArchitectureSelectionReportView {
    selected_candidate_kind: String,
    fast_route_selection_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DirectModelWeightExecutionProofReportView {
    model_id: String,
    model_descriptor_digest: String,
    route_descriptor_digest: String,
    direct_case_count: u32,
    fallback_free_case_count: u32,
    zero_external_call_case_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleTransformerReplacementPublicationView {
    transformer_model_id: String,
    replacement_certified: bool,
    article_equivalence_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionArtifactView {
    cases: Vec<ArticleExecutorSessionArtifactCaseView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionArtifactCaseView {
    name: String,
    outcome: ArticleExecutorSessionOutcomeView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionOutcomeView {
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<ArticleExecutorSessionCompletedResponseView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionCompletedResponseView {
    executor_response: ArticleExecutorResponseView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorResponseView {
    model_descriptor: TassadarExecutorModelDescriptor,
    execution_report: ArticleExecutorExecutionReportView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorExecutionReportView {
    selection: ArticleExecutorSelectionView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSelectionView {
    requested_decode_mode: TassadarExecutorDecodeMode,
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    selection_state: TassadarExecutorSelectionState,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowArtifactView {
    cases: Vec<ArticleHybridWorkflowArtifactCaseView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowArtifactCaseView {
    name: String,
    outcome: ArticleHybridWorkflowOutcomeView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowOutcomeView {
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<ArticleHybridWorkflowCompletedResponseView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowCompletedResponseView {
    planner_response: ArticleHybridPlannerResponseView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridPlannerResponseView {
    routing_decision: ArticleHybridRoutingDecisionView,
    executor_response: ArticleExecutorResponseView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridRoutingDecisionView {
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
}

#[derive(Debug, Error)]
pub enum TassadarArticleFastRouteImplementationReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Model(#[from] psionic_models::TassadarArticleTransformerError),
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
    #[error("internal TAS-173 invariant failed: {detail}")]
    Invariant { detail: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_fast_route_implementation_report() -> Result<
    TassadarArticleFastRouteImplementationReport,
    TassadarArticleFastRouteImplementationReportError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let selection_report: FastRouteArchitectureSelectionReportView = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        "article_fast_route_architecture_selection_report",
    )?;
    let replacement_publication: ArticleTransformerReplacementPublicationView = read_repo_json(
        ARTICLE_TRANSFORMER_REPLACEMENT_PUBLICATION_REF,
        "article_transformer_replacement_publication",
    )?;
    let article_session_artifact: ArticleExecutorSessionArtifactView = read_repo_json(
        ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF,
        "article_executor_session_artifact",
    )?;
    let article_hybrid_artifact: ArticleHybridWorkflowArtifactView = read_repo_json(
        ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF,
        "article_hybrid_workflow_artifact",
    )?;
    let direct_proof_report: DirectModelWeightExecutionProofReportView = read_repo_json(
        DIRECT_PROOF_REPORT_REF,
        "direct_model_weight_execution_proof_report",
    )?;

    if selection_report.selected_candidate_kind != "hull_cache_runtime"
        || !selection_report.fast_route_selection_green
    {
        return Err(TassadarArticleFastRouteImplementationReportError::Invariant {
            detail: String::from(
                "TAS-172 selection report must still choose hull_cache_runtime before TAS-173 can close",
            ),
        });
    }

    let descriptor = build_tassadar_article_transformer_trained_executor_descriptor()?;
    let descriptor_digest = descriptor.stable_digest();
    let acceptance_gate_tie = build_acceptance_gate_tie(&acceptance_gate)?;
    let descriptor_review = build_descriptor_review(&descriptor, &descriptor_digest);
    let replacement_review =
        build_replacement_review(&replacement_publication, descriptor.model.model_id.as_str());
    let article_session_review = build_article_session_review(
        &article_session_artifact,
        descriptor.model.model_id.as_str(),
        descriptor_digest.as_str(),
    )?;
    let hybrid_route_review = build_hybrid_route_review(
        &article_hybrid_artifact,
        descriptor.model.model_id.as_str(),
        descriptor_digest.as_str(),
    )?;
    let direct_proof_review = build_direct_proof_review(
        &direct_proof_report,
        descriptor.model.model_id.as_str(),
        descriptor_digest.as_str(),
    );
    let fast_route_implementation_green = acceptance_gate_tie.tied_requirement_satisfied
        && descriptor_review.hull_cache_supported
        && replacement_review.replacement_certified
        && article_session_review.fast_path_integrated
        && hybrid_route_review.fast_path_integrated
        && direct_proof_review.descriptor_binding_green;

    let mut report = TassadarArticleFastRouteImplementationReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_fast_route_implementation.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_CHECKER_REF),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        selected_candidate_kind: selection_report.selected_candidate_kind,
        descriptor_review,
        replacement_review,
        article_session_review,
        hybrid_route_review,
        direct_proof_review,
        fast_route_implementation_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && fast_route_implementation_green,
        claim_boundary: String::from(
            "this report closes only TAS-173. It proves the canonical trained Transformer-backed article model now owns the selected HullCache fast path at the descriptor, article-session, hybrid-route, and direct-proof surfaces. It does not yet claim fast-route no-fallback closure, throughput-floor closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article fast-route implementation report now records selected_candidate_kind=`{}`, tied_requirement_satisfied={}, fast_route_implementation_green={}, article_session_model_id=`{}`, hybrid_model_id=`{}`, and direct_proof_descriptor_binding_green={}.",
        report.selected_candidate_kind,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.fast_route_implementation_green,
        report.article_session_review.model_id,
        report.hybrid_route_review.model_id,
        report.direct_proof_review.descriptor_binding_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_fast_route_implementation_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_article_fast_route_implementation_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF)
}

pub fn write_tassadar_article_fast_route_implementation_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFastRouteImplementationReport,
    TassadarArticleFastRouteImplementationReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFastRouteImplementationReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_fast_route_implementation_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteImplementationReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_acceptance_gate_tie(
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
) -> Result<
    TassadarArticleFastRouteImplementationAcceptanceGateTie,
    TassadarArticleFastRouteImplementationReportError,
> {
    let tied_requirement = acceptance_gate
        .requirement_rows
        .iter()
        .find(|row| row.requirement_id == TIED_REQUIREMENT_ID)
        .ok_or_else(
            || TassadarArticleFastRouteImplementationReportError::Invariant {
                detail: format!("acceptance gate missing requirement `{TIED_REQUIREMENT_ID}`"),
            },
        )?;
    Ok(TassadarArticleFastRouteImplementationAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: tied_requirement.satisfied,
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    })
}

fn build_descriptor_review(
    descriptor: &TassadarExecutorModelDescriptor,
    descriptor_digest: &str,
) -> TassadarArticleFastRouteImplementationDescriptorReview {
    let hull_cache_supported = descriptor
        .compatibility
        .supported_decode_modes
        .contains(&TassadarExecutorDecodeMode::HullCache);
    TassadarArticleFastRouteImplementationDescriptorReview {
        descriptor_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_TRAINED_EXECUTOR_DESCRIPTOR_REF,
        ),
        model_id: descriptor.model.model_id.clone(),
        model_descriptor_digest: String::from(descriptor_digest),
        supported_decode_modes: descriptor.compatibility.supported_decode_modes.clone(),
        hull_cache_supported,
        detail: String::from(
            "the canonical served executor descriptor for the trained article Transformer now explicitly advertises ReferenceLinear plus HullCache on the owned article profile boundary",
        ),
    }
}

fn build_replacement_review(
    replacement_publication: &ArticleTransformerReplacementPublicationView,
    expected_model_id: &str,
) -> TassadarArticleFastRouteImplementationReplacementReview {
    TassadarArticleFastRouteImplementationReplacementReview {
        publication_ref: String::from(ARTICLE_TRANSFORMER_REPLACEMENT_PUBLICATION_REF),
        transformer_model_id: replacement_publication.transformer_model_id.clone(),
        replacement_certified: replacement_publication.replacement_certified,
        article_equivalence_green: replacement_publication.article_equivalence_green,
        detail: if replacement_publication.transformer_model_id == expected_model_id
            && replacement_publication.replacement_certified
        {
            String::from(
                "the replacement publication still certifies the trained Transformer-backed article route as the bounded canonical served model identity",
            )
        } else {
            format!(
                "replacement publication drifted to model `{}` or lost certification",
                replacement_publication.transformer_model_id
            )
        },
    }
}

fn build_article_session_review(
    artifact: &ArticleExecutorSessionArtifactView,
    expected_model_id: &str,
    expected_descriptor_digest: &str,
) -> Result<
    TassadarArticleFastRouteImplementationSessionReview,
    TassadarArticleFastRouteImplementationReportError,
> {
    let case = artifact
        .cases
        .iter()
        .find(|case| case.name == ARTICLE_DIRECT_HULL_CASE_NAME)
        .ok_or_else(
            || TassadarArticleFastRouteImplementationReportError::Invariant {
                detail: format!(
                    "article executor artifact missing case `{ARTICLE_DIRECT_HULL_CASE_NAME}`"
                ),
            },
        )?;
    let response = case
        .outcome
        .response
        .as_ref()
        .filter(|_| case.outcome.status == "completed")
        .ok_or_else(
            || TassadarArticleFastRouteImplementationReportError::Invariant {
                detail: format!(
                    "article executor case `{ARTICLE_DIRECT_HULL_CASE_NAME}` did not complete"
                ),
            },
        )?;
    let model_descriptor = &response.executor_response.model_descriptor;
    let selection = &response.executor_response.execution_report.selection;
    let model_descriptor_digest = model_descriptor.stable_digest();
    let fast_path_integrated = model_descriptor.model.model_id == expected_model_id
        && model_descriptor_digest == expected_descriptor_digest
        && selection.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
        && selection.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
        && selection.selection_state == TassadarExecutorSelectionState::Direct;
    Ok(TassadarArticleFastRouteImplementationSessionReview {
        artifact_ref: String::from(ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF),
        case_name: String::from(ARTICLE_DIRECT_HULL_CASE_NAME),
        model_id: model_descriptor.model.model_id.clone(),
        model_descriptor_digest,
        requested_decode_mode: selection.requested_decode_mode,
        effective_decode_mode: selection.effective_decode_mode,
        selection_state: selection.selection_state,
        fast_path_integrated,
        detail: if fast_path_integrated {
            String::from(
                "the canonical article-session surface now serves the trained Transformer model directly on HullCache for the direct memory-heavy article workload",
            )
        } else {
            String::from(
                "article-session artifact does not yet prove direct HullCache execution on the trained Transformer descriptor",
            )
        },
    })
}

fn build_hybrid_route_review(
    artifact: &ArticleHybridWorkflowArtifactView,
    expected_model_id: &str,
    expected_descriptor_digest: &str,
) -> Result<
    TassadarArticleFastRouteImplementationHybridReview,
    TassadarArticleFastRouteImplementationReportError,
> {
    let case = artifact
        .cases
        .iter()
        .find(|case| case.name == HYBRID_DIRECT_HULL_CASE_NAME)
        .ok_or_else(
            || TassadarArticleFastRouteImplementationReportError::Invariant {
                detail: format!(
                    "article hybrid artifact missing case `{HYBRID_DIRECT_HULL_CASE_NAME}`"
                ),
            },
        )?;
    let response = case
        .outcome
        .response
        .as_ref()
        .filter(|_| case.outcome.status == "completed")
        .ok_or_else(
            || TassadarArticleFastRouteImplementationReportError::Invariant {
                detail: format!(
                    "article hybrid case `{HYBRID_DIRECT_HULL_CASE_NAME}` did not complete"
                ),
            },
        )?;
    let model_descriptor = &response.planner_response.executor_response.model_descriptor;
    let selection = &response
        .planner_response
        .executor_response
        .execution_report
        .selection;
    let model_descriptor_digest = model_descriptor.stable_digest();
    let fast_path_integrated = model_descriptor.model.model_id == expected_model_id
        && model_descriptor_digest == expected_descriptor_digest
        && response
            .planner_response
            .routing_decision
            .effective_decode_mode
            == Some(TassadarExecutorDecodeMode::HullCache)
        && selection.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
        && selection.selection_state == TassadarExecutorSelectionState::Direct;
    Ok(TassadarArticleFastRouteImplementationHybridReview {
        artifact_ref: String::from(ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF),
        case_name: String::from(HYBRID_DIRECT_HULL_CASE_NAME),
        model_id: model_descriptor.model.model_id.clone(),
        model_descriptor_digest,
        planner_effective_decode_mode: response
            .planner_response
            .routing_decision
            .effective_decode_mode,
        executor_effective_decode_mode: selection.effective_decode_mode,
        selection_state: selection.selection_state,
        fast_path_integrated,
        detail: if fast_path_integrated {
            String::from(
                "the canonical hybrid planner-to-executor workflow now routes the trained Transformer model through HullCache on the delegated direct article case",
            )
        } else {
            String::from(
                "hybrid workflow artifact does not yet prove HullCache delegation on the trained Transformer descriptor",
            )
        },
    })
}

fn build_direct_proof_review(
    direct_proof_report: &DirectModelWeightExecutionProofReportView,
    expected_model_id: &str,
    expected_descriptor_digest: &str,
) -> TassadarArticleFastRouteImplementationDirectProofReview {
    let descriptor_binding_green = direct_proof_report.model_id == expected_model_id
        && direct_proof_report.model_descriptor_digest == expected_descriptor_digest
        && direct_proof_report.direct_case_count == 3
        && direct_proof_report.fallback_free_case_count == 3
        && direct_proof_report.zero_external_call_case_count == 3;
    TassadarArticleFastRouteImplementationDirectProofReview {
        report_ref: String::from(DIRECT_PROOF_REPORT_REF),
        model_id: direct_proof_report.model_id.clone(),
        model_descriptor_digest: direct_proof_report.model_descriptor_digest.clone(),
        route_descriptor_digest: direct_proof_report.route_descriptor_digest.clone(),
        direct_case_count: direct_proof_report.direct_case_count,
        fallback_free_case_count: direct_proof_report.fallback_free_case_count,
        zero_external_call_case_count: direct_proof_report.zero_external_call_case_count,
        descriptor_binding_green,
        detail: if descriptor_binding_green {
            String::from(
                "the direct model-weight proof report now binds the trained Transformer model id, one canonical model-descriptor digest, and one published route digest into the proof family",
            )
        } else {
            String::from(
                "direct proof report does not yet bind the trained Transformer descriptor digest cleanly across the proof family",
            )
        },
    }
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

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleFastRouteImplementationReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFastRouteImplementationReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteImplementationReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(not(test))]
fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleFastRouteImplementationReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFastRouteImplementationReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteImplementationReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fast_route_implementation_report, read_repo_json,
        tassadar_article_fast_route_implementation_report_path,
        write_tassadar_article_fast_route_implementation_report,
        TassadarArticleFastRouteImplementationReport,
        TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
    };
    use psionic_runtime::TassadarExecutorDecodeMode;

    #[test]
    fn fast_route_implementation_report_is_machine_legible() {
        let report = build_tassadar_article_fast_route_implementation_report().expect("report");

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-173");
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(report.selected_candidate_kind, "hull_cache_runtime");
        assert_eq!(
            report.article_session_review.requested_decode_mode,
            TassadarExecutorDecodeMode::HullCache
        );
        assert_eq!(
            report.article_session_review.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::HullCache)
        );
        assert_eq!(
            report.hybrid_route_review.planner_effective_decode_mode,
            Some(TassadarExecutorDecodeMode::HullCache)
        );
        assert!(report.direct_proof_review.descriptor_binding_green);
        assert!(report.fast_route_implementation_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn fast_route_implementation_report_matches_committed_truth() {
        let generated = build_tassadar_article_fast_route_implementation_report().expect("report");
        let committed: TassadarArticleFastRouteImplementationReport = read_repo_json(
            TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
            "report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_fast_route_implementation_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fast_route_implementation_report.json");
        let written = write_tassadar_article_fast_route_implementation_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleFastRouteImplementationReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fast_route_implementation_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_fast_route_implementation_report.json")
        );
    }
}
