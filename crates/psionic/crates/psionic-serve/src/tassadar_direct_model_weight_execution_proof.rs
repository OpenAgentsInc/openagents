use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_article_fixture_transformer_parity_report,
    TassadarArticleFixtureTransformerParityCaseRow, TassadarArticleFixtureTransformerParityError,
    TassadarArticleFixtureTransformerParityReport,
    TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
};
use psionic_models::TassadarArticleTransformer;
use psionic_router::{
    bind_tassadar_direct_model_weight_route, TassadarDirectModelWeightRouteBindingError,
};
use psionic_runtime::{
    TassadarDirectModelWeightExecutionProofError, TassadarDirectModelWeightExecutionProofInput,
    TassadarDirectModelWeightExecutionProofReceipt, TassadarExecutorDecodeMode,
};

use crate::{
    LocalTassadarExecutorService, LocalTassadarPlannerRouter,
    TassadarArticleExecutorSessionRequest, TassadarArticleExecutorSessionResponse,
    TassadarPlannerRouteDescriptorError,
};

pub const TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json";

const REPORT_SCHEMA_VERSION: u16 = 3;
const CANONICAL_CASE_IDS: [&str; 3] =
    ["long_loop_kernel", "sudoku_v0_test_a", "hungarian_matching"];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TrainedTraceBoundLineageContractView {
    contract_id: String,
    produced_descriptor_ref: String,
    produced_artifact_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDirectModelWeightExecutionProofReport {
    pub schema_version: u16,
    pub report_id: String,
    pub product_id: String,
    pub model_id: String,
    pub model_descriptor_digest: String,
    pub historical_fixture_model_id: String,
    pub benchmark_report_ref: String,
    pub parity_report_ref: String,
    pub parity_report_digest: String,
    pub lineage_contract_ref: String,
    pub lineage_contract_digest: String,
    pub route_descriptor_digest: String,
    pub case_ids: Vec<String>,
    pub receipts: Vec<TassadarDirectModelWeightExecutionProofReceipt>,
    pub direct_case_count: u32,
    pub fallback_free_case_count: u32,
    pub zero_external_call_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarDirectModelWeightExecutionProofReport {
    fn new(
        model_id: String,
        model_descriptor_digest: String,
        historical_fixture_model_id: String,
        parity_report_ref: String,
        parity_report_digest: String,
        lineage_contract_ref: String,
        lineage_contract_digest: String,
        route_descriptor_digest: String,
        receipts: Vec<TassadarDirectModelWeightExecutionProofReceipt>,
    ) -> Self {
        let benchmark_report_ref = receipts
            .first()
            .map(|receipt| receipt.benchmark_report_ref.clone())
            .unwrap_or_default();
        let case_ids = receipts
            .iter()
            .map(|receipt| receipt.article_case_id.clone())
            .collect::<Vec<_>>();
        let direct_case_count = receipts
            .iter()
            .filter(|receipt| {
                receipt.selection_state == psionic_runtime::TassadarExecutorSelectionState::Direct
            })
            .count() as u32;
        let fallback_free_case_count = receipts
            .iter()
            .filter(|receipt| !receipt.fallback_observed)
            .count() as u32;
        let zero_external_call_case_count = receipts
            .iter()
            .filter(|receipt| receipt.external_call_count == 0)
            .count() as u32;
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.direct_model_weight_execution_proof.v3"),
            product_id: String::from(crate::ARTICLE_EXECUTOR_SESSION_PRODUCT_ID),
            model_id,
            model_descriptor_digest,
            historical_fixture_model_id,
            benchmark_report_ref,
            parity_report_ref,
            parity_report_digest,
            lineage_contract_ref,
            lineage_contract_digest,
            route_descriptor_digest,
            case_ids,
            receipts,
            direct_case_count,
            fallback_free_case_count,
            zero_external_call_case_count,
            claim_boundary: String::from(
                "this report moves the bounded direct no-tool proof family off the historical fixture model and onto the trained trace-bound Transformer-backed reference-linear route for the three canonical proof workloads named here. It does so only because the committed fixture-to-Transformer parity certificate proves exact trace and terminal-state parity on those cases, and it binds the trained weight-lineage contract into every receipt. It does not yet claim full declared-workload exactness, fast-route closure, benchmark parity, or final article-equivalence green status.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Transformer-backed direct model-weight execution proof now freezes {} canonical article workloads on route `{}` with direct_cases={}, fallback_free_cases={}, zero_external_call_cases={}, parity_report_ref=`{}`, and lineage_contract_ref=`{}`.",
            report.receipts.len(),
            report.route_descriptor_digest,
            report.direct_case_count,
            report.fallback_free_case_count,
            report.zero_external_call_case_count,
            report.parity_report_ref,
            report.lineage_contract_ref,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_direct_model_weight_execution_proof_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarDirectModelWeightExecutionProofReportError {
    #[error(transparent)]
    ParityReport(#[from] TassadarArticleFixtureTransformerParityError),
    #[error(transparent)]
    Model(#[from] psionic_models::TassadarArticleTransformerError),
    #[error(transparent)]
    ArticleSessionService(#[from] crate::TassadarArticleExecutorSessionServiceError),
    #[error(transparent)]
    RouteDescriptor(#[from] TassadarPlannerRouteDescriptorError),
    #[error(transparent)]
    RouteBinding(#[from] TassadarDirectModelWeightRouteBindingError),
    #[error(transparent)]
    ProofReceipt(#[from] TassadarDirectModelWeightExecutionProofError),
    #[error("canonical article case `{case_id}` is missing from the current article corpus")]
    MissingCase { case_id: String },
    #[error("fixture baseline execution failed for canonical article case `{case_id}`: {detail}")]
    FixtureExecution { case_id: String, detail: String },
    #[error("program artifact assembly failed for canonical article case `{case_id}`: {detail}")]
    ProgramArtifact { case_id: String, detail: String },
    #[error("fixture-to-Transformer parity report is missing canonical proof case `{case_id}`")]
    MissingParityCase { case_id: String },
    #[error(
        "fixture-to-Transformer parity report did not certify canonical proof case `{case_id}`: {detail}"
    )]
    ParityCaseNotCertified { case_id: String, detail: String },
    #[error("trained trace-bound lineage contract mismatched the committed descriptor or artifact refs: {detail}")]
    LineageContractMismatch { detail: String },
    #[error("article session `{case_id}` did not complete: {detail}")]
    CaseDidNotComplete { case_id: String, detail: String },
    #[error("article session `{case_id}` completed without a direct model-weight proof receipt")]
    MissingReceipt { case_id: String },
    #[error("failed to publish the trained Transformer capability surface: {detail}")]
    CapabilityPublication { detail: String },
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

pub fn build_tassadar_direct_model_weight_execution_proof_receipt_for_article_session(
    executor_service: &LocalTassadarExecutorService,
    request: &TassadarArticleExecutorSessionRequest,
    response: &TassadarArticleExecutorSessionResponse,
) -> Result<
    TassadarDirectModelWeightExecutionProofReceipt,
    TassadarDirectModelWeightExecutionProofReportError,
> {
    let (model_lineage_contract_ref, model_lineage_contract_digest) =
        model_lineage_contract_binding(&response.executor_response.model_descriptor)?;
    build_tassadar_direct_model_weight_execution_proof_receipt_for_article_session_with_lineage(
        executor_service,
        request,
        response,
        model_lineage_contract_ref,
        model_lineage_contract_digest,
    )
}

pub fn build_tassadar_direct_model_weight_execution_proof_receipt_for_article_session_with_lineage(
    executor_service: &LocalTassadarExecutorService,
    request: &TassadarArticleExecutorSessionRequest,
    response: &TassadarArticleExecutorSessionResponse,
    model_lineage_contract_ref: impl Into<String>,
    model_lineage_contract_digest: impl Into<String>,
) -> Result<
    TassadarDirectModelWeightExecutionProofReceipt,
    TassadarDirectModelWeightExecutionProofReportError,
> {
    let model_lineage_contract_ref = model_lineage_contract_ref.into();
    let model_lineage_contract_digest = model_lineage_contract_digest.into();
    let route_descriptor = LocalTassadarPlannerRouter::new()
        .with_executor_service(executor_service.clone())
        .route_capability_descriptor(Some(
            response
                .executor_response
                .model_descriptor
                .model
                .model_id
                .as_str(),
        ))?;
    let route_binding =
        bind_tassadar_direct_model_weight_route(&route_descriptor, request.requested_decode_mode)?;
    let selection = &response.executor_response.execution_report.selection;
    let compiled_backend_features = response
        .executor_response
        .evidence_bundle
        .proof_bundle
        .runtime_identity
        .backend_toolchain
        .compiled_backend_features
        .clone();
    let external_call_count = compiled_backend_features
        .iter()
        .filter(|feature| is_external_tool_marker(feature))
        .count() as u32;
    let external_tool_surface_observed = external_call_count > 0;
    let cpu_result_substitution_observed = compiled_backend_features
        .iter()
        .any(|feature| is_cpu_substitution_marker(feature));
    let fallback_observed = selection.selection_state
        != psionic_runtime::TassadarExecutorSelectionState::Direct
        || selection.effective_decode_mode != Some(request.requested_decode_mode);
    Ok(TassadarDirectModelWeightExecutionProofReceipt::new(
        TassadarDirectModelWeightExecutionProofInput {
            receipt_id: format!(
                "direct_model_weight_proof.{}",
                response.benchmark_identity.case_id
            ),
            benchmark_ref: response.benchmark_identity.benchmark_ref.clone(),
            benchmark_environment_ref: response
                .benchmark_identity
                .benchmark_environment_ref
                .clone(),
            benchmark_report_ref: response.benchmark_identity.benchmark_report_ref.clone(),
            workload_family_id: response.benchmark_identity.workload_family.clone(),
            article_case_id: response.benchmark_identity.case_id.clone(),
            article_case_summary: response.benchmark_identity.case_summary.clone(),
            executor_product_id: response.executor_response.product_id.clone(),
            model_id: response
                .executor_response
                .model_descriptor
                .model
                .model_id
                .clone(),
            model_descriptor_digest: response.executor_response.model_descriptor.stable_digest(),
            model_weight_bundle_digest: response
                .executor_response
                .model_descriptor
                .weights
                .digest
                .clone(),
            model_primary_artifact_digest: response
                .executor_response
                .model_descriptor
                .weights
                .primary_artifact_digest()
                .map(String::from),
            model_lineage_contract_ref,
            model_lineage_contract_digest,
            requested_decode_mode: request.requested_decode_mode,
            effective_decode_mode: selection.effective_decode_mode,
            selection_state: selection.selection_state,
            fallback_observed,
            external_call_count,
            external_tool_surface_observed,
            cpu_result_substitution_observed,
            compiled_backend_features,
            program_artifact_digest: response.proof_identity.program_artifact_digest.clone(),
            trace_artifact_digest: response.proof_identity.trace_artifact_digest.clone(),
            trace_digest: response.proof_identity.trace_digest.clone(),
            trace_proof_digest: response.proof_identity.trace_proof_digest.clone(),
            runtime_manifest_identity_digest: response
                .proof_identity
                .runtime_manifest_identity_digest
                .clone(),
            runtime_manifest_digest: response.proof_identity.runtime_manifest_digest.clone(),
            proof_bundle_request_digest: response
                .proof_identity
                .proof_bundle_request_digest
                .clone(),
            proof_bundle_model_id: response.proof_identity.proof_bundle_model_id.clone(),
            route_binding,
        },
    )?)
}

fn model_lineage_contract_binding(
    model_descriptor: &psionic_models::TassadarExecutorModelDescriptor,
) -> Result<(String, String), TassadarDirectModelWeightExecutionProofReportError> {
    if model_descriptor.model.model_id == TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID {
        read_trained_trace_bound_lineage_contract_digest()
    } else {
        Ok((
            fixture_lineage_contract_ref(model_descriptor.model.model_id.as_str()),
            fixture_lineage_contract_digest(
                model_descriptor.model.model_id.as_str(),
                model_descriptor.weights.digest.as_str(),
            ),
        ))
    }
}

pub fn build_tassadar_direct_model_weight_execution_proof_report() -> Result<
    TassadarDirectModelWeightExecutionProofReport,
    TassadarDirectModelWeightExecutionProofReportError,
> {
    let parity_report = build_tassadar_article_fixture_transformer_parity_report()?;
    if parity_report.transformer_model_artifact.model_id
        != TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
    {
        return Err(
            TassadarDirectModelWeightExecutionProofReportError::LineageContractMismatch {
                detail: format!(
                    "parity report model `{}` did not match trained trace-bound model `{}`",
                    parity_report.transformer_model_artifact.model_id,
                    TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
                ),
            },
        );
    }
    let (lineage_contract_ref, lineage_contract_digest) =
        read_trained_trace_bound_lineage_contract_digest()?;
    let executor_service = LocalTassadarExecutorService::new();
    let capability_publication = executor_service
        .capability_publication(Some(
            TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID,
        ))
        .map_err(|error| {
            TassadarDirectModelWeightExecutionProofReportError::CapabilityPublication {
                detail: error.to_string(),
            }
        })?;
    let model_descriptor_digest = capability_publication.model_descriptor.stable_digest();
    let model_weight_bundle_digest = capability_publication.model_descriptor.weights.digest.clone();
    let model_primary_artifact_digest = capability_publication
        .model_descriptor
        .weights
        .primary_artifact_digest()
        .map(String::from);
    let route_descriptor = LocalTassadarPlannerRouter::new()
        .with_executor_service(executor_service.clone())
        .route_capability_descriptor(Some(
            TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID,
        ))?;
    let article_session_service = crate::LocalTassadarArticleExecutorSessionService::new();
    let mut receipts = Vec::new();
    for case_id in CANONICAL_CASE_IDS {
        let parity_row = parity_row_for_case(&parity_report, case_id)?;
        ensure_case_is_certified_for_transformer_direct_proof(parity_row)?;
        let request = TassadarArticleExecutorSessionRequest::new(
            format!("direct-proof-{case_id}"),
            case_id,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )
        .with_requested_model_id(TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID)
        .require_direct_model_weight_proof();
        let outcome = article_session_service.execute_without_derived_views(&request)?;
        let receipt = match outcome {
            crate::TassadarArticleExecutorSessionOutcome::Completed { response } => response
                .direct_model_weight_execution_proof_receipt
                .ok_or_else(|| {
                    TassadarDirectModelWeightExecutionProofReportError::MissingReceipt {
                        case_id: String::from(case_id),
                    }
                })?,
            crate::TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
                return Err(
                    TassadarDirectModelWeightExecutionProofReportError::CaseDidNotComplete {
                        case_id: String::from(case_id),
                        detail: refusal.detail,
                    },
                );
            }
        };
        if receipt.route_binding.route_descriptor_digest != route_descriptor.descriptor_digest {
            return Err(
                TassadarDirectModelWeightExecutionProofReportError::CaseDidNotComplete {
                    case_id: String::from(case_id),
                    detail: format!(
                        "proof receipt route digest `{}` did not match published direct-proof route `{}`",
                        receipt.route_binding.route_descriptor_digest,
                        route_descriptor.descriptor_digest
                    ),
                },
            );
        }
        if receipt.model_id != capability_publication.model_descriptor.model.model_id {
            return Err(
                TassadarDirectModelWeightExecutionProofReportError::CaseDidNotComplete {
                    case_id: String::from(case_id),
                    detail: format!(
                        "proof receipt model `{}` did not match published Transformer model `{}`",
                        receipt.model_id, capability_publication.model_descriptor.model.model_id
                    ),
                },
            );
        }
        if receipt.model_weight_bundle_digest != model_weight_bundle_digest {
            return Err(
                TassadarDirectModelWeightExecutionProofReportError::CaseDidNotComplete {
                    case_id: String::from(case_id),
                    detail: format!(
                        "proof receipt weight bundle digest `{}` did not match published Transformer weight bundle `{}`",
                        receipt.model_weight_bundle_digest, model_weight_bundle_digest
                    ),
                },
            );
        }
        if receipt.model_primary_artifact_digest != model_primary_artifact_digest {
            return Err(
                TassadarDirectModelWeightExecutionProofReportError::CaseDidNotComplete {
                    case_id: String::from(case_id),
                    detail: format!(
                        "proof receipt primary artifact digest `{:?}` did not match published Transformer primary artifact `{:?}`",
                        receipt.model_primary_artifact_digest, model_primary_artifact_digest
                    ),
                },
            );
        }
        if receipt.model_lineage_contract_ref != lineage_contract_ref
            || receipt.model_lineage_contract_digest != lineage_contract_digest
        {
            return Err(
                TassadarDirectModelWeightExecutionProofReportError::CaseDidNotComplete {
                    case_id: String::from(case_id),
                    detail: format!(
                        "proof receipt lineage drifted to ref=`{}` digest=`{}`",
                        receipt.model_lineage_contract_ref, receipt.model_lineage_contract_digest
                    ),
                },
            );
        }
        receipts.push(receipt);
    }
    Ok(TassadarDirectModelWeightExecutionProofReport::new(
        capability_publication.model_descriptor.model.model_id.clone(),
        model_descriptor_digest,
        parity_report.fixture_model_id,
        String::from(TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF),
        parity_report.report_digest,
        lineage_contract_ref,
        lineage_contract_digest,
        route_descriptor.descriptor_digest,
        receipts,
    ))
}

#[must_use]
pub fn tassadar_direct_model_weight_execution_proof_report_path() -> PathBuf {
    repo_root().join(TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF)
}

pub fn write_tassadar_direct_model_weight_execution_proof_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarDirectModelWeightExecutionProofReport,
    TassadarDirectModelWeightExecutionProofReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarDirectModelWeightExecutionProofReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_direct_model_weight_execution_proof_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarDirectModelWeightExecutionProofReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn is_external_tool_marker(feature: &str) -> bool {
    feature.contains("external_tool")
        || feature.contains("tool_call")
        || feature.contains("external_call")
}

fn is_cpu_substitution_marker(feature: &str) -> bool {
    feature.contains("cpu_result_substitution") || feature.contains("result_substitution")
}

fn parity_row_for_case<'a>(
    parity_report: &'a TassadarArticleFixtureTransformerParityReport,
    case_id: &str,
) -> Result<
    &'a TassadarArticleFixtureTransformerParityCaseRow,
    TassadarDirectModelWeightExecutionProofReportError,
> {
    parity_report
        .case_rows
        .iter()
        .find(|row| row.case_id == case_id)
        .ok_or_else(
            || TassadarDirectModelWeightExecutionProofReportError::MissingParityCase {
                case_id: String::from(case_id),
            },
        )
}

fn ensure_case_is_certified_for_transformer_direct_proof(
    parity_row: &TassadarArticleFixtureTransformerParityCaseRow,
) -> Result<(), TassadarDirectModelWeightExecutionProofReportError> {
    if parity_row.requested_decode_mode != TassadarExecutorDecodeMode::ReferenceLinear {
        return Err(
            TassadarDirectModelWeightExecutionProofReportError::ParityCaseNotCertified {
                case_id: parity_row.case_id.clone(),
                detail: format!(
                    "expected reference_linear decode, found `{}`",
                    parity_row.requested_decode_mode.as_str()
                ),
            },
        );
    }
    if parity_row.fixture_selection_state != "direct" {
        return Err(
            TassadarDirectModelWeightExecutionProofReportError::ParityCaseNotCertified {
                case_id: parity_row.case_id.clone(),
                detail: format!(
                    "fixture baseline selection drifted to `{}`",
                    parity_row.fixture_selection_state
                ),
            },
        );
    }
    if parity_row.fixture_effective_decode_mode != Some(TassadarExecutorDecodeMode::ReferenceLinear)
    {
        return Err(
            TassadarDirectModelWeightExecutionProofReportError::ParityCaseNotCertified {
                case_id: parity_row.case_id.clone(),
                detail: format!(
                    "fixture baseline effective decode drifted to `{}`",
                    parity_row
                        .fixture_effective_decode_mode
                        .map_or("none", TassadarExecutorDecodeMode::as_str)
                ),
            },
        );
    }
    if !parity_row.fixture_routeable
        || !parity_row.transformer_routeable
        || !parity_row.trace_shape_parity
        || !parity_row.output_parity
        || !parity_row.behavior_parity
        || !parity_row.case_passed
    {
        return Err(
            TassadarDirectModelWeightExecutionProofReportError::ParityCaseNotCertified {
                case_id: parity_row.case_id.clone(),
                detail: parity_row.detail.clone(),
            },
        );
    }
    Ok(())
}

fn read_trained_trace_bound_lineage_contract_digest(
) -> Result<(String, String), TassadarDirectModelWeightExecutionProofReportError> {
    let path = TassadarArticleTransformer::trained_trace_bound_lineage_contract_path();
    let bytes = fs::read(&path).map_err(|error| {
        TassadarDirectModelWeightExecutionProofReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    let contract: TrainedTraceBoundLineageContractView =
        serde_json::from_slice(&bytes).map_err(|error| {
            TassadarDirectModelWeightExecutionProofReportError::Decode {
                path: path.display().to_string(),
                error,
            }
        })?;
    if contract.produced_descriptor_ref
        != TassadarArticleTransformer::TRAINED_TRACE_BOUND_DESCRIPTOR_REF
        || contract.produced_artifact_ref
            != TassadarArticleTransformer::TRAINED_TRACE_BOUND_ARTIFACT_REF
    {
        return Err(
            TassadarDirectModelWeightExecutionProofReportError::LineageContractMismatch {
                detail: format!(
                    "contract `{}` pointed at descriptor `{}` and artifact `{}`",
                    contract.contract_id,
                    contract.produced_descriptor_ref,
                    contract.produced_artifact_ref
                ),
            },
        );
    }
    Ok((
        String::from(TassadarArticleTransformer::TRAINED_TRACE_BOUND_LINEAGE_CONTRACT_REF),
        hex::encode(Sha256::digest(&bytes)),
    ))
}

fn fixture_lineage_contract_ref(model_id: &str) -> String {
    format!("lineage://tassadar_executor_fixture/{model_id}")
}

fn fixture_lineage_contract_digest(model_id: &str, weight_bundle_digest: &str) -> String {
    stable_digest(
        b"psionic_tassadar_executor_fixture_lineage|",
        &(model_id, weight_bundle_digest),
    )
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-serve should live under <repo>/crates/psionic-serve")
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
) -> Result<T, TassadarDirectModelWeightExecutionProofReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarDirectModelWeightExecutionProofReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarDirectModelWeightExecutionProofReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_direct_model_weight_execution_proof_report,
        ensure_case_is_certified_for_transformer_direct_proof, read_repo_json,
        tassadar_direct_model_weight_execution_proof_report_path,
        write_tassadar_direct_model_weight_execution_proof_report,
        TassadarDirectModelWeightExecutionProofReport,
        TassadarDirectModelWeightExecutionProofReportError,
        TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF,
    };
    use psionic_eval::build_tassadar_article_fixture_transformer_parity_report;
    use psionic_models::TassadarArticleTransformer;

    #[test]
    fn direct_model_weight_execution_proof_report_is_machine_legible() {
        let report = build_tassadar_direct_model_weight_execution_proof_report().expect("report");

        assert_eq!(report.receipts.len(), 3);
        assert_eq!(report.direct_case_count, 3);
        assert_eq!(report.fallback_free_case_count, 3);
        assert_eq!(report.zero_external_call_case_count, 3);
        assert_eq!(
            report.model_id,
            TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        );
        assert!(!report.model_descriptor_digest.is_empty());
        assert_eq!(
            report.historical_fixture_model_id,
            "tassadar-executor-article-i32-compute-v0"
        );
        assert_eq!(
            report.parity_report_ref,
            "fixtures/tassadar/reports/tassadar_article_fixture_transformer_parity_report.json"
        );
        assert_eq!(
            report.lineage_contract_ref,
            "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_lineage_contract.json"
        );
        assert!(report.case_ids.contains(&String::from("long_loop_kernel")));
        assert!(report.case_ids.contains(&String::from("sudoku_v0_test_a")));
        assert!(report
            .case_ids
            .contains(&String::from("hungarian_matching")));
        assert!(report
            .receipts
            .iter()
            .all(|receipt| receipt.route_binding.route_descriptor_digest
                == report.route_descriptor_digest));
        assert!(report
            .receipts
            .iter()
            .all(|receipt| receipt.model_id == report.model_id));
        assert!(report
            .receipts
            .iter()
            .all(|receipt| receipt.model_weight_bundle_digest
                == report.receipts[0].model_weight_bundle_digest));
        assert!(report.receipts.iter().all(|receipt| {
            receipt.model_lineage_contract_ref == report.lineage_contract_ref
                && receipt.model_id == report.model_id
        }));
    }

    #[test]
    fn transformer_direct_proof_refuses_uncertified_parity_rows() {
        let mut parity_report =
            build_tassadar_article_fixture_transformer_parity_report().expect("parity report");
        let row = parity_report
            .case_rows
            .iter_mut()
            .find(|row| row.case_id == "hungarian_matching")
            .expect("matching parity row");
        row.output_parity = false;
        row.case_passed = false;

        match ensure_case_is_certified_for_transformer_direct_proof(row).unwrap_err() {
            TassadarDirectModelWeightExecutionProofReportError::ParityCaseNotCertified {
                case_id,
                detail,
            } => {
                assert_eq!(case_id, "hungarian_matching");
                assert_eq!(detail, row.detail);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn direct_model_weight_execution_proof_report_matches_committed_truth() {
        let generated =
            build_tassadar_direct_model_weight_execution_proof_report().expect("report");
        let committed: TassadarDirectModelWeightExecutionProofReport =
            read_repo_json(TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_direct_model_weight_execution_proof_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_direct_model_weight_execution_proof_report.json");
        let written = write_tassadar_direct_model_weight_execution_proof_report(&output_path)
            .expect("write report");
        let persisted: TassadarDirectModelWeightExecutionProofReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_direct_model_weight_execution_proof_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_direct_model_weight_execution_proof_report.json")
        );
    }
}
