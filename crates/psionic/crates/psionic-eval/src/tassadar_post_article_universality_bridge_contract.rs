use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    build_tassadar_post_article_canonical_computational_model_statement_report,
    build_tassadar_tcm_v1_runtime_contract_report,
    TassadarPostArticleCanonicalComputationalModelStatement,
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
    TassadarTcmV1RuntimeContractReport, TassadarTcmV1RuntimeContractReportError,
    TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
    TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_equivalence_final_audit_report,
    build_tassadar_article_route_minimality_audit_report,
    build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
    build_tassadar_turing_completeness_closeout_audit_report,
    build_tassadar_universality_verdict_split_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError, TassadarArticleEquivalenceFinalAuditError,
    TassadarArticleEquivalenceFinalAuditReport, TassadarArticleRouteMinimalityAuditError,
    TassadarArticleRouteMinimalityAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
    TassadarTuringCompletenessCloseoutAuditReport,
    TassadarTuringCompletenessCloseoutAuditReportError, TassadarTuringCompletenessCloseoutStatus,
    TassadarUniversalityVerdictSplitReport, TassadarUniversalityVerdictSplitReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF,
    TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
    TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
    TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_bridge_contract_report.json";
pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-universality-bridge-contract.sh";

const BRIDGE_MACHINE_IDENTITY_ID: &str =
    "tassadar.post_article_universality_bridge.machine_identity.v1";
const DIRECT_CARRIER_ID: &str =
    "tassadar.post_article_universality_bridge.direct_article_equivalent_carrier.v1";
const RESUMABLE_CARRIER_ID: &str =
    "tassadar.post_article_universality_bridge.resumable_universality_carrier.v1";
const RESERVED_CAPABILITY_PLANE_ID: &str =
    "tassadar.post_article_universality_bridge.reserved_capability_plane.v1";
const DATA_PLANE_ID: &str = "tassadar.post_article_universality_bridge.data_plane.v1";
const CONTROL_PLANE_ID: &str = "tassadar.post_article_universality_bridge.control_plane.v1";
const CAPABILITY_PLANE_ID: &str = "tassadar.post_article_universality_bridge.capability_plane.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleUniversalityBridgeStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCarrierTopology {
    ExplicitSplitAcrossDirectAndResumableLanes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCarrierClass {
    DirectArticleEquivalent,
    ResumableUniversality,
    ReservedCapabilityPlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePlaneKind {
    Data,
    Control,
    Capability,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBridgeMachineIdentityTuple {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub direct_decode_mode: String,
    pub continuation_contract_id: String,
    pub continuation_contract_ref: String,
    pub continuation_contract_digest: String,
    pub carrier_topology: TassadarPostArticleCarrierTopology,
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBridgeDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleHistoricalBindingRow {
    pub binding_id: String,
    pub historical_artifact_ref: String,
    pub historical_artifact_digest: String,
    pub preserved_without_rewrite: bool,
    pub canonical_machine_identity_bound: bool,
    pub bound_claim_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCarrierRow {
    pub carrier_id: String,
    pub carrier_class: TassadarPostArticleCarrierClass,
    pub current_posture: String,
    pub bound_machine_identity_id: String,
    pub route_posture: String,
    pub claim_ids: Vec<String>,
    pub state_classes: Vec<String>,
    pub source_refs: Vec<String>,
    pub widening_exclusions: Vec<String>,
    pub carrier_contract_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePlaneContractRow {
    pub plane_id: String,
    pub plane_kind: TassadarPostArticlePlaneKind,
    pub current_posture: String,
    pub owner_statement: String,
    pub source_refs: Vec<String>,
    pub reserved_issue_ids: Vec<String>,
    pub plane_contract_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleReservationHookRow {
    pub hook_id: String,
    pub purpose: String,
    pub reserved_issue_ids: Vec<String>,
    pub current_posture: String,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBridgeValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityBridgeContractReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub article_equivalence_acceptance_gate_report_ref: String,
    pub article_equivalence_final_audit_report_ref: String,
    pub article_route_minimality_audit_report_ref: String,
    pub canonical_computational_model_statement_report_ref: String,
    pub execution_semantics_proof_transport_audit_report_ref: String,
    pub tcm_v1_runtime_contract_report_ref: String,
    pub universality_verdict_split_report_ref: String,
    pub turing_completeness_closeout_audit_report_ref: String,
    pub article_equivalence_acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    pub article_equivalence_final_audit_report: TassadarArticleEquivalenceFinalAuditReport,
    pub article_route_minimality_audit_report: TassadarArticleRouteMinimalityAuditReport,
    pub canonical_computational_model_statement_report:
        TassadarPostArticleCanonicalComputationalModelStatementReport,
    pub execution_semantics_proof_transport_audit_report:
        TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    pub tcm_v1_runtime_contract_report: TassadarTcmV1RuntimeContractReport,
    pub universality_verdict_split_report: TassadarUniversalityVerdictSplitReport,
    pub turing_completeness_closeout_audit_report: TassadarTuringCompletenessCloseoutAuditReport,
    pub bridge_machine_identity: TassadarPostArticleBridgeMachineIdentityTuple,
    pub dependency_rows: Vec<TassadarPostArticleBridgeDependencyRow>,
    pub historical_binding_rows: Vec<TassadarPostArticleHistoricalBindingRow>,
    pub carrier_topology: TassadarPostArticleCarrierTopology,
    pub carrier_rows: Vec<TassadarPostArticleCarrierRow>,
    pub computational_model_statement: TassadarPostArticleCanonicalComputationalModelStatement,
    pub plane_contract_rows: Vec<TassadarPostArticlePlaneContractRow>,
    pub reserved_later_invariant_ids: Vec<String>,
    pub reservation_hook_rows: Vec<TassadarPostArticleReservationHookRow>,
    pub validation_rows: Vec<TassadarPostArticleBridgeValidationRow>,
    pub bridge_status: TassadarPostArticleUniversalityBridgeStatus,
    pub bridge_contract_green: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleUniversalityBridgeContractReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    FinalAudit(#[from] TassadarArticleEquivalenceFinalAuditError),
    #[error(transparent)]
    RouteMinimality(#[from] TassadarArticleRouteMinimalityAuditError),
    #[error(transparent)]
    CanonicalComputationalModel(
        #[from] TassadarPostArticleCanonicalComputationalModelStatementReportError,
    ),
    #[error(transparent)]
    ExecutionSemanticsProofTransport(
        #[from] TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
    ),
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error(transparent)]
    VerdictSplit(#[from] TassadarUniversalityVerdictSplitReportError),
    #[error(transparent)]
    TuringCloseout(#[from] TassadarTuringCompletenessCloseoutAuditReportError),
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

pub fn build_tassadar_post_article_universality_bridge_contract_report() -> Result<
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
> {
    let article_equivalence_acceptance_gate_report =
        build_tassadar_article_equivalence_acceptance_gate_report()?;
    let article_equivalence_final_audit_report =
        build_tassadar_article_equivalence_final_audit_report()?;
    let article_route_minimality_audit_report =
        build_tassadar_article_route_minimality_audit_report()?;
    let canonical_computational_model_statement_report =
        build_tassadar_post_article_canonical_computational_model_statement_report()?;
    let execution_semantics_proof_transport_audit_report =
        build_tassadar_post_article_execution_semantics_proof_transport_audit_report()?;
    let tcm_v1_runtime_contract_report = build_tassadar_tcm_v1_runtime_contract_report()?;
    let universality_verdict_split_report = build_tassadar_universality_verdict_split_report()?;
    let turing_completeness_closeout_audit_report =
        build_tassadar_turing_completeness_closeout_audit_report()?;

    let carrier_topology =
        TassadarPostArticleCarrierTopology::ExplicitSplitAcrossDirectAndResumableLanes;
    let bridge_machine_identity = TassadarPostArticleBridgeMachineIdentityTuple {
        machine_identity_id: String::from(BRIDGE_MACHINE_IDENTITY_ID),
        canonical_model_id: article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_model_id
            .clone(),
        canonical_weight_artifact_id: article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_weight_artifact_id
            .clone(),
        canonical_weight_bundle_digest: article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_route_id
            .clone(),
        canonical_route_descriptor_digest: article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_route_descriptor_digest
            .clone(),
        direct_decode_mode: article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_decode_mode
            .clone(),
        continuation_contract_id: tcm_v1_runtime_contract_report.report_id.clone(),
        continuation_contract_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
        continuation_contract_digest: tcm_v1_runtime_contract_report.report_digest.clone(),
        carrier_topology,
        current_host_machine_class_id: article_equivalence_final_audit_report
            .machine_matrix_review
            .current_host_machine_class_id
            .clone(),
        supported_machine_class_ids: article_equivalence_final_audit_report
            .machine_matrix_review
            .supported_machine_class_ids
            .clone(),
        detail: format!(
            "machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` direct_decode_mode=`{}` continuation_contract_id=`{}` carrier_topology={:?} supported_machine_classes={}",
            BRIDGE_MACHINE_IDENTITY_ID,
            article_equivalence_final_audit_report
                .canonical_closure_review
                .canonical_model_id,
            article_equivalence_final_audit_report
                .canonical_closure_review
                .canonical_route_id,
            article_equivalence_final_audit_report
                .canonical_closure_review
                .canonical_decode_mode,
            tcm_v1_runtime_contract_report.report_id,
            carrier_topology,
            article_equivalence_final_audit_report
                .machine_matrix_review
                .supported_machine_class_ids
                .len(),
        ),
    };

    let canonical_route_identity_consistent = article_equivalence_final_audit_report
        .canonical_closure_review
        .canonical_model_id
        == article_route_minimality_audit_report
            .canonical_claim_route_review
            .transformer_model_id
        && article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_route_id
            == article_route_minimality_audit_report
                .canonical_claim_route_review
                .canonical_claim_route_id
        && article_equivalence_final_audit_report
            .canonical_closure_review
            .canonical_route_descriptor_digest
            == article_route_minimality_audit_report
                .canonical_claim_route_review
                .projected_route_descriptor_digest;

    let dependency_rows = vec![
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("canonical_article_equivalence_acceptance_gate_green"),
            satisfied: article_equivalence_acceptance_gate_report.article_equivalence_green
                && article_equivalence_acceptance_gate_report.public_claim_allowed,
            source_refs: vec![String::from(
                TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            )],
            detail: String::from(
                "the bridge contract may only attach to the post-`TAS-186` route after the bounded article-equivalence acceptance gate is green on its own declared envelope",
            ),
        },
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("canonical_article_equivalence_final_audit_green"),
            satisfied: article_equivalence_final_audit_report.article_equivalence_green
                && article_equivalence_final_audit_report
                    .public_article_equivalence_claim_allowed,
            source_refs: vec![String::from(
                TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF,
            )],
            detail: String::from(
                "the bridge contract binds only to the closed canonical article route, not to pre-closeout or fixture-backed article history",
            ),
        },
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("canonical_route_identity_consistent"),
            satisfied: canonical_route_identity_consistent
                && article_route_minimality_audit_report.route_minimality_audit_green,
            source_refs: vec![
                String::from(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF),
                String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "the canonical model id, route id, and route digest must stay identical across the final article-equivalence closeout and the route-minimality audit before historical universality artifacts can be rebound to that machine identity",
            ),
        },
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("canonical_computational_model_statement_published"),
            satisfied: canonical_computational_model_statement_report.statement_green
                && canonical_computational_model_statement_report
                    .computational_model_statement
                    .machine_identity_id
                    == BRIDGE_MACHINE_IDENTITY_ID,
            source_refs: vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            )],
            detail: String::from(
                "the bridge now projects the separately published canonical computational-model statement instead of inventing a bridge-local model statement.",
            ),
        },
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("execution_semantics_proof_transport_audit_green"),
            satisfied: execution_semantics_proof_transport_audit_report.audit_green
                && execution_semantics_proof_transport_audit_report.proof_transport_complete
                && execution_semantics_proof_transport_audit_report.machine_identity_id
                    == BRIDGE_MACHINE_IDENTITY_ID
                && execution_semantics_proof_transport_audit_report.canonical_route_id
                    == bridge_machine_identity.canonical_route_id,
            source_refs: vec![String::from(
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            )],
            detail: String::from(
                "the bridge now depends on the separately published execution-semantics proof-transport audit instead of leaving semantic transport as a later implied property.",
            ),
        },
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("historical_tcm_v1_substrate_present"),
            satisfied: tcm_v1_runtime_contract_report.substrate_model.model_id == "tcm.v1",
            source_refs: vec![tcm_v1_runtime_contract_report.substrate_model_ref.clone()],
            detail: String::from(
                "the bridge contract still depends on the historical `TCM.v1` substrate model rather than inventing a new substrate story",
            ),
        },
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("historical_tcm_v1_runtime_contract_green"),
            satisfied: tcm_v1_runtime_contract_report.overall_green,
            source_refs: vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            detail: String::from(
                "the rebased bridge still inherits the historical continuation and effect boundary only through the committed `TCM.v1` runtime contract",
            ),
        },
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("historical_universality_verdict_split_explicit"),
            satisfied: universality_verdict_split_report.overall_green
                && universality_verdict_split_report.theory_green
                && universality_verdict_split_report.operator_green
                && !universality_verdict_split_report.served_green,
            source_refs: vec![String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF)],
            detail: String::from(
                "the historical theory/operator/served split must stay explicit so the rebased carrier cannot silently widen into served/public universality",
            ),
        },
        TassadarPostArticleBridgeDependencyRow {
            dependency_id: String::from("historical_turing_completeness_closeout_green"),
            satisfied: turing_completeness_closeout_audit_report.claim_status
                == TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed
                && turing_completeness_closeout_audit_report.theory_green
                && turing_completeness_closeout_audit_report.operator_green
                && !turing_completeness_closeout_audit_report.served_green,
            source_refs: vec![String::from(
                TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
            )],
            detail: String::from(
                "the bridge exists to rebind the already-green bounded closeout to the canonical owned route without rewriting the historical closeout artifact",
            ),
        },
    ];

    let historical_binding_rows = vec![
        TassadarPostArticleHistoricalBindingRow {
            binding_id: String::from("tcm_v1_substrate_rows_bound"),
            historical_artifact_ref: tcm_v1_runtime_contract_report.substrate_model_ref.clone(),
            historical_artifact_digest: tcm_v1_runtime_contract_report
                .substrate_model
                .model_digest
                .clone(),
            preserved_without_rewrite: true,
            canonical_machine_identity_bound: canonical_route_identity_consistent,
            bound_claim_ids: vec![
                String::from("declared_substrate_model"),
                String::from("control_rows"),
                String::from("memory_rows"),
                String::from("continuation_rows"),
                String::from("effect_boundary_rows"),
            ],
            detail: String::from(
                "the historical `TCM.v1` substrate rows remain untouched and are now cited as the computational substrate below the canonical post-`TAS-186` machine identity instead of being copied into a new artifact family",
            ),
        },
        TassadarPostArticleHistoricalBindingRow {
            binding_id: String::from("tcm_v1_runtime_contract_bound"),
            historical_artifact_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
            historical_artifact_digest: tcm_v1_runtime_contract_report.report_digest.clone(),
            preserved_without_rewrite: true,
            canonical_machine_identity_bound: canonical_route_identity_consistent
                && tcm_v1_runtime_contract_report.overall_green,
            bound_claim_ids: tcm_v1_runtime_contract_report
                .satisfied_runtime_semantic_ids
                .clone(),
            detail: String::from(
                "the historical runtime contract remains the sole continuation and effect carrier for the resumable lane while the canonical route identity now says which owned Transformer route that continuation semantics attaches to",
            ),
        },
        TassadarPostArticleHistoricalBindingRow {
            binding_id: String::from("historical_verdict_split_bound"),
            historical_artifact_ref: String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            historical_artifact_digest: universality_verdict_split_report.report_digest.clone(),
            preserved_without_rewrite: true,
            canonical_machine_identity_bound: canonical_route_identity_consistent
                && universality_verdict_split_report.overall_green,
            bound_claim_ids: vec![
                String::from("theory_green"),
                String::from("operator_green"),
                String::from("served_suppressed"),
            ],
            detail: String::from(
                "the historical verdict split remains authoritative about theory/operator/served posture, while this bridge adds the missing canonical owned-route identity tuple that the older verdict did not carry",
            ),
        },
        TassadarPostArticleHistoricalBindingRow {
            binding_id: String::from("historical_turing_closeout_bound"),
            historical_artifact_ref: String::from(
                TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
            ),
            historical_artifact_digest: turing_completeness_closeout_audit_report
                .report_digest
                .clone(),
            preserved_without_rewrite: true,
            canonical_machine_identity_bound: canonical_route_identity_consistent
                && turing_completeness_closeout_audit_report.claim_status
                    == TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed,
            bound_claim_ids: turing_completeness_closeout_audit_report.explicit_scope.clone(),
            detail: String::from(
                "the historical bounded closeout remains authoritative about theory/operator Turing-completeness posture; this bridge only adds the explicit post-`TAS-186` machine identity and the direct-vs-resumable split that the older closeout did not name",
            ),
        },
    ];

    let carrier_rows = vec![
        TassadarPostArticleCarrierRow {
            carrier_id: String::from(DIRECT_CARRIER_ID),
            carrier_class: TassadarPostArticleCarrierClass::DirectArticleEquivalent,
            current_posture: String::from("implemented"),
            bound_machine_identity_id: String::from(BRIDGE_MACHINE_IDENTITY_ID),
            route_posture: format!(
                "direct deterministic `{}` route on `{}`",
                article_equivalence_final_audit_report
                    .canonical_closure_review
                    .canonical_decode_mode,
                article_equivalence_final_audit_report
                    .canonical_closure_review
                    .canonical_route_id
            ),
            claim_ids: vec![
                String::from("bounded_article_equivalence"),
                String::from("direct_route_minimality"),
                String::from("single_run_no_spill_article_execution"),
                String::from("declared_cpu_machine_matrix_reproducibility"),
            ],
            state_classes: vec![
                String::from("weights_owned"),
                String::from("same_run_ephemeral_kv_cache"),
                String::from("same_run_ephemeral_activation_history"),
            ],
            source_refs: vec![
                String::from(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF),
                String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF),
            ],
            widening_exclusions: vec![
                String::from("checkpoint_resume"),
                String::from("spill_tape_extension"),
                String::from("process_object_durability"),
                String::from("planner_or_hybrid_canonical_route"),
                String::from("stochastic_execution"),
            ],
            carrier_contract_green: article_equivalence_final_audit_report.article_equivalence_green
                && article_route_minimality_audit_report.route_minimality_audit_green,
            detail: String::from(
                "the direct carrier is the already-closed bounded article route only; it proves direct article-equivalent behavior on the canonical model and route, but it does not carry resumable universality semantics",
            ),
        },
        TassadarPostArticleCarrierRow {
            carrier_id: String::from(RESUMABLE_CARRIER_ID),
            carrier_class: TassadarPostArticleCarrierClass::ResumableUniversality,
            current_posture: String::from("implemented"),
            bound_machine_identity_id: String::from(BRIDGE_MACHINE_IDENTITY_ID),
            route_posture: String::from(
                "bounded resumable continuation family above the canonical machine identity and under the historical `TCM.v1` runtime contract",
            ),
            claim_ids: vec![
                String::from("theory_green_universality"),
                String::from("operator_green_universality"),
                String::from("bounded_turing_completeness_closeout"),
            ],
            state_classes: vec![
                String::from("weights_owned"),
                String::from("resumed_checkpoint_state"),
                String::from("durable_process_object_state"),
                String::from("spill_tape_extension"),
            ],
            source_refs: vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                String::from(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            widening_exclusions: vec![
                String::from("direct_article_equivalence_claim"),
                String::from("served_public_universality"),
                String::from("weighted_plugin_capability_execution"),
            ],
            carrier_contract_green: turing_completeness_closeout_audit_report.claim_status
                == TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed,
            detail: String::from(
                "the resumable carrier preserves the historical Turing-completeness lane and now says explicitly that the lane is resumable rather than a direct article-equivalent same-run claim",
            ),
        },
        TassadarPostArticleCarrierRow {
            carrier_id: String::from(RESERVED_CAPABILITY_PLANE_ID),
            carrier_class: TassadarPostArticleCarrierClass::ReservedCapabilityPlane,
            current_posture: String::from("reserved_not_implemented"),
            bound_machine_identity_id: String::from(BRIDGE_MACHINE_IDENTITY_ID),
            route_posture: String::from(
                "reserved above the bridge as a later plugin and capability layer",
            ),
            claim_ids: vec![
                String::from("future_weighted_plugin_control"),
                String::from("future_bounded_software_capability_platform"),
            ],
            state_classes: vec![String::from("future_packet_bound_capability_receipts")],
            source_refs: vec![
                String::from(
                    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md",
                ),
                String::from(
                    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md",
                ),
            ],
            widening_exclusions: vec![
                String::from("implicit_capability_inheritance_from_tcm_v1"),
                String::from("implicit_plugin_control_from_article_equivalence"),
                String::from("served_public_plugin_platform"),
            ],
            carrier_contract_green: true,
            detail: String::from(
                "the capability layer is explicit and reserved; the bridge contract names it so later plugin work cannot be smuggled in as an implicit extension of either the direct or resumable carriers",
            ),
        },
    ];

    let computational_model_statement = canonical_computational_model_statement_report
        .computational_model_statement
        .clone();

    let plane_contract_rows = vec![
        TassadarPostArticlePlaneContractRow {
            plane_id: String::from(DATA_PLANE_ID),
            plane_kind: TassadarPostArticlePlaneKind::Data,
            current_posture: String::from("implemented"),
            owner_statement: String::from(
                "the data plane carries pure compute evolution on the canonical owned Transformer route and the declared `TCM.v1` substrate rows only",
            ),
            source_refs: vec![
                String::from(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF),
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
            ],
            reserved_issue_ids: Vec::new(),
            plane_contract_green: true,
            detail: String::from(
                "the bridge contract keeps the compute substrate explicit and prevents capability or publication logic from being treated as part of the machine's pure compute plane",
            ),
        },
        TassadarPostArticlePlaneContractRow {
            plane_id: String::from(CONTROL_PLANE_ID),
            plane_kind: TassadarPostArticlePlaneKind::Control,
            current_posture: String::from("contract_frozen_pending_provenance_proof"),
            owner_statement: String::from(
                "the control plane carries branch, retry, stop, and resumable workflow semantics, but host may execute continuation mechanics only without silently deciding workflow",
            ),
            source_refs: vec![
                String::from(
                    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md",
                ),
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
            ],
            reserved_issue_ids: vec![String::from("TAS-188"), String::from("TAS-188A")],
            plane_contract_green: true,
            detail: String::from(
                "the contract is frozen here, but the stronger semantic-preservation and decision-provenance proofs remain separate later bridge issues instead of being hand-waved into existence now",
            ),
        },
        TassadarPostArticlePlaneContractRow {
            plane_id: String::from(CAPABILITY_PLANE_ID),
            plane_kind: TassadarPostArticlePlaneKind::Capability,
            current_posture: String::from("reserved_above_bridge"),
            owner_statement: String::from(
                "the capability plane is a later plugin layer above the bridge; it may not silently inherit or rewrite lower-plane compute or control claims",
            ),
            source_refs: vec![
                String::from(
                    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md",
                ),
                String::from(
                    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md",
                ),
            ],
            reserved_issue_ids: vec![String::from("TAS-195"), String::from("TAS-197")],
            plane_contract_green: true,
            detail: String::from(
                "the bridge contract names the future plugin/capability plane explicitly so later capability work cannot masquerade as an implicit extension of the lower-plane machine model",
            ),
        },
    ];

    let reserved_later_invariant_ids = vec![
        String::from("choice_set_integrity"),
        String::from("resource_transparency"),
        String::from("scheduling_ownership"),
    ];
    let reservation_hook_rows = vec![
        TassadarPostArticleReservationHookRow {
            hook_id: String::from("packet_boundary_hook"),
            purpose: String::from(
                "reserve one forward-compatible packet boundary for later plugin invocation and transport work",
            ),
            reserved_issue_ids: vec![String::from("TAS-199")],
            current_posture: String::from("reserved_not_implemented"),
            source_refs: vec![String::from(
                "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md",
            )],
            detail: String::from(
                "the bridge contract names the packet boundary as a future capability hook instead of pretending the current compute receipts already carry plugin packets",
            ),
        },
        TassadarPostArticleReservationHookRow {
            hook_id: String::from("capability_invocation_slot"),
            purpose: String::from(
                "reserve one explicit capability-invocation slot above the rebased control plane",
            ),
            reserved_issue_ids: vec![String::from("TAS-200")],
            current_posture: String::from("reserved_not_implemented"),
            source_refs: vec![String::from(
                "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md",
            )],
            detail: String::from(
                "the bridge keeps the capability call boundary explicit and future-facing instead of leaving room for implicit host capability injection",
            ),
        },
        TassadarPostArticleReservationHookRow {
            hook_id: String::from("receipt_extensibility_field"),
            purpose: String::from(
                "reserve receipt extensibility for later plugin invocation, replay, and conformance receipts",
            ),
            reserved_issue_ids: vec![String::from("TAS-201")],
            current_posture: String::from("reserved_not_implemented"),
            source_refs: vec![String::from(
                "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md",
            )],
            detail: String::from(
                "the bridge does not backfill plugin receipt fields into the current article or universality receipts; it reserves that space explicitly for the later plugin tranche",
            ),
        },
        TassadarPostArticleReservationHookRow {
            hook_id: String::from("schema_version_negotiation_hook"),
            purpose: String::from(
                "keep schema-version negotiation and canonical-machine inheritance explicit above the bridge while the later stability frontier remains separate",
            ),
            reserved_issue_ids: vec![String::from("TAS-216")],
            current_posture: String::from("reserved_after_canonical_machine_closure_bundle"),
            source_refs: vec![String::from(
                "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md",
            )],
            detail: String::from(
                "the bridge now delegates schema stability, weighted-controller ownership, bounded plugin-platform closeout, canonical-machine locking, the published computational-model statement, the execution-semantics proof-transport audit, the closed continuation non-computationality boundary, the closed fast-route legitimacy, equivalent-choice neutrality, downward non-influence, the anti-drift closeout, and the published canonical machine closure bundle to TAS-203A through TAS-216 instead of widening the bridge itself into the terminal machine or publication surface",
            ),
        },
    ];

    let supporting_material_refs = vec![
        String::from("docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md"),
        String::from(
            "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md",
        ),
        String::from("docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md"),
        tcm_v1_runtime_contract_report.substrate_model_ref.clone(),
        String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
        String::from(TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF),
        String::from(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF),
        String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF),
        String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
        String::from(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF),
    ];

    let validation_rows = vec![
        TassadarPostArticleBridgeValidationRow {
            validation_id: String::from("canonical_computational_model_statement_published"),
            green: canonical_computational_model_statement_report.statement_green
                && computational_model_statement.machine_identity_id == BRIDGE_MACHINE_IDENTITY_ID,
            source_refs: vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            )],
            detail: String::from(
                "the bridge now projects the separately published canonical computational-model statement instead of carrying a bridge-local shadow copy.",
            ),
        },
        TassadarPostArticleBridgeValidationRow {
            validation_id: String::from("execution_semantics_proof_transport_audit_closed"),
            green: execution_semantics_proof_transport_audit_report.audit_green
                && execution_semantics_proof_transport_audit_report.proof_transport_complete
                && execution_semantics_proof_transport_audit_report.plugin_execution_transport_bound,
            source_refs: vec![String::from(
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            )],
            detail: String::from(
                "the bridge now cites the separate execution-semantics proof-transport audit instead of carrying semantic transport as an unresolved follow-on gap.",
            ),
        },
        TassadarPostArticleBridgeValidationRow {
            validation_id: String::from("helper_substitution_quarantined"),
            green: article_route_minimality_audit_report
                .execution_ownership_review
                .hidden_host_substitution_excluded
                && article_route_minimality_audit_report
                    .execution_ownership_review
                    .external_oracle_excluded
                && article_route_minimality_audit_report
                    .execution_ownership_review
                    .helper_module_mediation_excluded,
            source_refs: vec![String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF)],
            detail: String::from(
                "helper substitution remains excluded on the canonical article route, so the bridge does not inherit a hidden host helper lane by implication",
            ),
        },
        TassadarPostArticleBridgeValidationRow {
            validation_id: String::from("route_drift_rejected"),
            green: canonical_route_identity_consistent
                && article_route_minimality_audit_report
                    .execution_ownership_review
                    .route_drift_excluded,
            source_refs: vec![
                String::from(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF),
                String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "the canonical route id and route digest stay fixed across the direct article closeout and the bridge, so the historical universality claim is not rebound onto a drifting route",
            ),
        },
        TassadarPostArticleBridgeValidationRow {
            validation_id: String::from("continuation_abuse_quarantined"),
            green: article_route_minimality_audit_report
                .continuation_boundary_review
                .continuation_boundary_green
                && !article_route_minimality_audit_report
                    .continuation_boundary_review
                    .checkpoint_restore_allowed
                && !article_route_minimality_audit_report
                    .continuation_boundary_review
                    .spill_tape_extension_allowed
                && !article_route_minimality_audit_report
                    .continuation_boundary_review
                    .external_persisted_continuation_allowed,
            source_refs: vec![
                String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF),
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
            ],
            detail: String::from(
                "the direct carrier still refuses checkpoint, spill, and external persisted continuation, so the bridge keeps direct article-equivalent truth distinct from resumable universality truth",
            ),
        },
        TassadarPostArticleBridgeValidationRow {
            validation_id: String::from("semantic_drift_blocked"),
            green: historical_binding_rows
                .iter()
                .all(|row| row.preserved_without_rewrite && row.canonical_machine_identity_bound)
                && article_route_minimality_audit_report
                    .execution_ownership_review
                    .artifact_lineage_to_behavior_closed
                && article_equivalence_final_audit_report.all_article_lines_matched,
            source_refs: vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF),
                String::from(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF),
            ],
            detail: String::from(
                "historical artifacts stay intact, the canonical machine identity is explicit, and the bridge binds those facts without rewriting either the direct article proof or the resumable closeout into a new semantic claim class",
            ),
        },
        TassadarPostArticleBridgeValidationRow {
            validation_id: String::from("historical_artifacts_preserved_without_rewrite"),
            green: historical_binding_rows
                .iter()
                .all(|row| row.preserved_without_rewrite),
            source_refs: historical_binding_rows
                .iter()
                .map(|row| row.historical_artifact_ref.clone())
                .collect(),
            detail: String::from(
                "the bridge contract cites the historical `TAS-151` through `TAS-156` artifacts by digest and ref instead of replacing them with rewritten copies",
            ),
        },
        TassadarPostArticleBridgeValidationRow {
            validation_id: String::from("overclaim_posture_explicit"),
            green: !turing_completeness_closeout_audit_report.served_green,
            source_refs: vec![
                String::from(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF),
                String::from(
                    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md",
                ),
            ],
            detail: String::from(
                "the bridge remains a bounded contract only; it does not by itself allow the rebased Turing-completeness claim, weighted plugin control, served/public universality, or arbitrary software capability",
            ),
        },
    ];

    let bridge_contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && validation_rows.iter().all(|row| row.green)
        && carrier_rows.iter().all(|row| row.carrier_contract_green)
        && plane_contract_rows
            .iter()
            .all(|row| row.plane_contract_green);
    let bridge_status = if bridge_contract_green {
        TassadarPostArticleUniversalityBridgeStatus::Green
    } else {
        TassadarPostArticleUniversalityBridgeStatus::Blocked
    };

    let mut report = TassadarPostArticleUniversalityBridgeContractReport {
        schema_version: 1,
        report_id: String::from("tassadar.post_article_universality_bridge.contract.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_CHECKER_REF,
        ),
        supporting_material_refs,
        article_equivalence_acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        article_equivalence_final_audit_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF,
        ),
        article_route_minimality_audit_report_ref: String::from(
            TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
        ),
        canonical_computational_model_statement_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
        ),
        execution_semantics_proof_transport_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
        ),
        tcm_v1_runtime_contract_report_ref: String::from(
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
        ),
        universality_verdict_split_report_ref: String::from(
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        turing_completeness_closeout_audit_report_ref: String::from(
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
        ),
        article_equivalence_acceptance_gate_report,
        article_equivalence_final_audit_report,
        article_route_minimality_audit_report,
        canonical_computational_model_statement_report,
        execution_semantics_proof_transport_audit_report,
        tcm_v1_runtime_contract_report,
        universality_verdict_split_report,
        turing_completeness_closeout_audit_report,
        bridge_machine_identity,
        dependency_rows,
        historical_binding_rows,
        carrier_topology,
        carrier_rows,
        computational_model_statement,
        plane_contract_rows,
        reserved_later_invariant_ids,
        reservation_hook_rows,
        validation_rows,
        bridge_status,
        bridge_contract_green,
        rebase_claim_allowed: false,
        plugin_capability_claim_allowed: false,
        served_public_universality_allowed: false,
        claim_boundary: String::from(
            "this bridge contract freezes only the post-`TAS-186` rebasing boundary between the historical `TCM.v1` universality closeout and the canonical owned article-equivalence route. It binds the old closeout to one explicit post-article machine identity tuple, one coarse direct-vs-resumable carrier topology, one separately published canonical computational-model statement, one separately published execution-semantics proof-transport audit, and one explicit three-plane contract. It does not by itself prove fast-route legitimacy, control-plane provenance, final canonical carrier binding, served/public universality, weighted plugin control, or arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article universality bridge contract keeps dependencies={}/{}, historical_bindings={}, carrier_topology={:?}, validation_rows={}/{}, bridge_status={:?}, rebase_claim_allowed={}, and reserved_capability_hooks={}.",
        report.dependency_rows.iter().filter(|row| row.satisfied).count(),
        report.dependency_rows.len(),
        report.historical_binding_rows.len(),
        report.carrier_topology,
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.bridge_status,
        report.rebase_claim_allowed,
        report.reservation_hook_rows.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_universality_bridge_contract_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_universality_bridge_contract_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)
}

pub fn write_tassadar_post_article_universality_bridge_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleUniversalityBridgeContractReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_universality_bridge_contract_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleUniversalityBridgeContractReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleUniversalityBridgeContractReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleUniversalityBridgeContractReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalityBridgeContractReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_universality_bridge_contract_report, read_json,
        tassadar_post_article_universality_bridge_contract_report_path,
        write_tassadar_post_article_universality_bridge_contract_report,
        TassadarPostArticleCarrierTopology, TassadarPostArticlePlaneKind,
        TassadarPostArticleUniversalityBridgeContractReport,
        TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_bridge_contract_freezes_identity_and_split_without_widening() {
        let report =
            build_tassadar_post_article_universality_bridge_contract_report().expect("report");

        assert!(report.bridge_contract_green);
        assert_eq!(
            report.bridge_machine_identity.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            report.carrier_topology,
            TassadarPostArticleCarrierTopology::ExplicitSplitAcrossDirectAndResumableLanes
        );
        assert_eq!(
            report.bridge_machine_identity.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(
            report.bridge_machine_identity.continuation_contract_id,
            "tassadar.tcm_v1.runtime_contract.report.v1"
        );
        assert!(!report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert_eq!(
            report.reserved_later_invariant_ids,
            vec![
                String::from("choice_set_integrity"),
                String::from("resource_transparency"),
                String::from("scheduling_ownership"),
            ]
        );
        assert!(report.plane_contract_rows.iter().any(|row| row.plane_kind
            == TassadarPostArticlePlaneKind::Capability
            && row.reserved_issue_ids.contains(&String::from("TAS-195"))));
    }

    #[test]
    fn post_article_bridge_contract_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_universality_bridge_contract_report().expect("report");
        let committed: TassadarPostArticleUniversalityBridgeContractReport =
            read_json(tassadar_post_article_universality_bridge_contract_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_universality_bridge_contract_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_universality_bridge_contract_report.json")
        );
    }

    #[test]
    fn write_post_article_bridge_contract_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_universality_bridge_contract_report.json");
        let written = write_tassadar_post_article_universality_bridge_contract_report(&output_path)
            .expect("write report");
        let persisted: TassadarPostArticleUniversalityBridgeContractReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(written, persisted);
        assert_eq!(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_universality_bridge_contract_report.json"
        );
    }
}
