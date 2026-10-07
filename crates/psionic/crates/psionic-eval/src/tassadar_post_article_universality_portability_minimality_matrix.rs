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
    TassadarBroadInternalComputePortabilityReport,
    TassadarBroadInternalComputePortabilityRowStatus,
    TassadarBroadInternalComputeSuppressionReason,
    TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_broad_internal_compute_portability_report,
    build_tassadar_post_article_canonical_route_universal_substrate_gate_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_turing_completeness_closeout_audit_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleFastRouteArchitectureSelectionReport, TassadarArticleRouteMinimalityAuditReport,
    TassadarBroadInternalComputePortabilityReportError,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarTuringCompletenessCloseoutAuditReport,
    TassadarTuringCompletenessCloseoutAuditReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
    TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_portability_minimality_matrix_report.json";
pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-universality-portability-minimality-matrix.sh";

const ARTICLE_CLOSEOUT_PROFILE_ID: &str = "tassadar.internal_compute.article_closeout.v1";
const ROADMAP_TAS_SYNC_REF: &str = "docs/ROADMAP_TASSADAR_TAS_SYNC.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass {
    ProofCarrying,
    ObservationalContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleUniversalityRouteCarrierStatus {
    InsideUniversalityCarrier,
    OutsideUniversalityCarrierAccelerationOnly,
    OutsideUniversalityCarrierResearchOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityMachineMatrixRow {
    pub profile_id: String,
    pub backend_family: String,
    pub toolchain_family: String,
    pub machine_class_id: String,
    pub portability_row_status: TassadarBroadInternalComputePortabilityRowStatus,
    pub publication_allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppression_reason: Option<TassadarBroadInternalComputeSuppressionReason>,
    pub declared_by_bridge_machine_identity: bool,
    pub supports_rebased_universality: bool,
    pub resumed_execution_parity_explicit: bool,
    pub row_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityRouteClassificationRow {
    pub candidate_kind: String,
    pub route_carrier_status: TassadarPostArticleUniversalityRouteCarrierStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_descriptor_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decode_mode: Option<String>,
    pub semantics_equivalence_explicit: bool,
    pub inherits_universality: bool,
    pub classification_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityMinimalityRow {
    pub minimality_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityPortabilityMinimalityValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub universal_substrate_gate_report_ref: String,
    pub broad_internal_compute_portability_report_ref: String,
    pub turing_completeness_closeout_audit_report_ref: String,
    pub article_equivalence_acceptance_gate_report_ref: String,
    pub bridge_contract_report_ref: String,
    pub article_fast_route_architecture_selection_report_ref: String,
    pub article_route_minimality_audit_report_ref: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub supporting_material_rows:
        Vec<TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialRow>,
    pub machine_matrix_rows: Vec<TassadarPostArticleUniversalityMachineMatrixRow>,
    pub route_classification_rows: Vec<TassadarPostArticleUniversalityRouteClassificationRow>,
    pub minimality_rows: Vec<TassadarPostArticleUniversalityMinimalityRow>,
    pub bounded_universality_story_carried: bool,
    pub universal_substrate_gate_allowed: bool,
    pub machine_matrix_green: bool,
    pub route_classification_green: bool,
    pub minimality_green: bool,
    pub served_suppression_boundary_preserved: bool,
    pub served_conformance_envelope_defined: bool,
    pub matrix_status: TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus,
    pub matrix_green: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub validation_rows: Vec<TassadarPostArticleUniversalityPortabilityMinimalityValidationRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError {
    #[error(transparent)]
    UniversalSubstrateGate(
        #[from] TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
    ),
    #[error(transparent)]
    BroadPortability(#[from] TassadarBroadInternalComputePortabilityReportError),
    #[error(transparent)]
    TuringCloseout(#[from] TassadarTuringCompletenessCloseoutAuditReportError),
    #[error(transparent)]
    ArticleGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_post_article_universality_portability_minimality_matrix_report() -> Result<
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
> {
    let universal_substrate_gate =
        build_tassadar_post_article_canonical_route_universal_substrate_gate_report()?;
    let broad_portability = build_tassadar_broad_internal_compute_portability_report()?;
    let turing_closeout = build_tassadar_turing_completeness_closeout_audit_report()?;
    let article_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let architecture_selection: TassadarArticleFastRouteArchitectureSelectionReport =
        read_artifact(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            "article_fast_route_architecture_selection_report",
        )?;
    let route_minimality: TassadarArticleRouteMinimalityAuditReport = read_artifact(
        TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
        "article_route_minimality_audit_report",
    )?;
    Ok(build_report_from_inputs(
        universal_substrate_gate,
        broad_portability,
        turing_closeout,
        article_gate,
        bridge,
        architecture_selection,
        route_minimality,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    universal_substrate_gate: TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    broad_portability: TassadarBroadInternalComputePortabilityReport,
    turing_closeout: TassadarTuringCompletenessCloseoutAuditReport,
    article_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    bridge: TassadarPostArticleUniversalityBridgeContractReport,
    architecture_selection: TassadarArticleFastRouteArchitectureSelectionReport,
    route_minimality: TassadarArticleRouteMinimalityAuditReport,
) -> TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport {
    let supporting_material_rows = build_supporting_material_rows(
        &universal_substrate_gate,
        &broad_portability,
        &turing_closeout,
        &article_gate,
        &bridge,
        &architecture_selection,
        &route_minimality,
    );
    let machine_matrix_rows = build_machine_matrix_rows(
        &broad_portability,
        &bridge,
        &turing_closeout,
        &universal_substrate_gate,
    );
    let route_classification_rows = build_route_classification_rows(
        &universal_substrate_gate,
        &bridge,
        &architecture_selection,
        &route_minimality,
    );
    let minimality_rows = build_minimality_rows(
        &universal_substrate_gate,
        &route_classification_rows,
        &route_minimality,
    );
    let rebase_claim_allowed = false;
    let plugin_capability_claim_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;
    let validation_rows = build_validation_rows(
        &universal_substrate_gate,
        &turing_closeout,
        &machine_matrix_rows,
        &route_classification_rows,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );

    let proof_carrying_rows_green = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ProofCarrying
        })
        .all(|row| row.satisfied);
    let machine_matrix_green = machine_matrix_rows.iter().all(|row| row.row_green)
        && machine_matrix_rows
            .iter()
            .filter(|row| row.declared_by_bridge_machine_identity)
            .all(|row| row.supports_rebased_universality);
    let route_classification_green = route_classification_rows
        .iter()
        .all(|row| row.classification_green)
        && route_classification_rows.iter().any(|row| {
            row.route_carrier_status
                == TassadarPostArticleUniversalityRouteCarrierStatus::InsideUniversalityCarrier
                && row.inherits_universality
        });
    let minimality_green = minimality_rows.iter().all(|row| row.green);
    let validation_green = validation_rows.iter().all(|row| row.green);
    let served_suppression_boundary_preserved =
        turing_closeout.served_green == false && !served_public_universality_allowed;
    let served_conformance_envelope_defined = true;
    let matrix_green = proof_carrying_rows_green
        && machine_matrix_green
        && route_classification_green
        && minimality_green
        && validation_green;
    let matrix_status = if matrix_green {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus::Green
    } else {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus::Blocked
    };
    let deferred_issue_ids = vec![String::from("TAS-194")];

    let mut report = TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_universality_portability_minimality_matrix.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_CHECKER_REF,
        ),
        universal_substrate_gate_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
        ),
        broad_internal_compute_portability_report_ref: String::from(
            TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
        ),
        turing_completeness_closeout_audit_report_ref: String::from(
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
        ),
        article_equivalence_acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
        ),
        article_fast_route_architecture_selection_report_ref: String::from(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        ),
        article_route_minimality_audit_report_ref: String::from(
            TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
        ),
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
        canonical_weight_artifact_id: bridge
            .bridge_machine_identity
            .canonical_weight_artifact_id
            .clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        continuation_contract_id: bridge.bridge_machine_identity.continuation_contract_id.clone(),
        supporting_material_rows,
        machine_matrix_rows,
        route_classification_rows,
        minimality_rows,
        bounded_universality_story_carried: universal_substrate_gate
            .bounded_universality_story_carried,
        universal_substrate_gate_allowed: universal_substrate_gate.universal_substrate_gate_allowed,
        machine_matrix_green,
        route_classification_green,
        minimality_green,
        served_suppression_boundary_preserved,
        served_conformance_envelope_defined,
        matrix_status,
        matrix_green,
        deferred_issue_ids,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        validation_rows,
        claim_boundary: String::from(
            "this matrix proves that the post-`TAS-186` bounded universality lane is attached to one declared machine matrix and one explicit route-carrier classification. It keeps current-host and declared-class machine cells, explicit suppression outside the declared envelope, explicit carrier binding for the selected canonical HullCache route, explicit acceleration-only or research-only posture for non-selected fast routes, and explicit minimality over machine components, semantics, and preserved transforms. It does not by itself publish the rebased theory/operator/served verdict split, admit served/public universality, admit weighted plugin control, or admit arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article universality portability/minimality matrix keeps supporting_materials={}/8, machine_rows={}/{}, route_rows={}/{}, minimality_rows={}/{}, validation_rows={}/{}, served_suppression_boundary_preserved={}, and matrix_status={:?}.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.machine_matrix_rows.iter().filter(|row| row.row_green).count(),
        report.machine_matrix_rows.len(),
        report
            .route_classification_rows
            .iter()
            .filter(|row| row.classification_green)
            .count(),
        report.route_classification_rows.len(),
        report.minimality_rows.iter().filter(|row| row.green).count(),
        report.minimality_rows.len(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.served_suppression_boundary_preserved,
        report.matrix_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_universality_portability_minimality_matrix_report|",
        &report,
    );
    report
}

fn build_supporting_material_rows(
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    broad_portability: &TassadarBroadInternalComputePortabilityReport,
    turing_closeout: &TassadarTuringCompletenessCloseoutAuditReport,
    article_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    architecture_selection: &TassadarArticleFastRouteArchitectureSelectionReport,
    route_minimality: &TassadarArticleRouteMinimalityAuditReport,
) -> Vec<TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialRow> {
    vec![
        supporting_material_row(
            "canonical_route_universal_substrate_gate",
            TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ProofCarrying,
            universal_substrate_gate.gate_status
                == TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus::Green
                && universal_substrate_gate.universal_substrate_gate_allowed,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            Some(universal_substrate_gate.report_id.clone()),
            Some(universal_substrate_gate.report_digest.clone()),
            "the canonical-route universal-substrate gate must stay green before the portability/minimality matrix can inherit any bounded universality story on the canonical route.",
        ),
        supporting_material_row(
            "broad_internal_compute_portability",
            TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ProofCarrying,
            broad_portability
                .publication_allowed_profile_ids
                .iter()
                .any(|id| id == ARTICLE_CLOSEOUT_PROFILE_ID),
            TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
            Some(broad_portability.report_id.clone()),
            Some(broad_portability.report_digest.clone()),
            "the broad internal-compute portability report must keep the article-closeout profile explicit across declared CPU classes so the rebased universality matrix inherits a machine envelope instead of a single-host anecdote.",
        ),
        supporting_material_row(
            "historical_turing_closeout_audit",
            TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ProofCarrying,
            turing_closeout.theory_green && turing_closeout.operator_green && !turing_closeout.served_green,
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
            Some(turing_closeout.report_id.clone()),
            Some(turing_closeout.report_digest.clone()),
            "the historical closeout audit must stay theory-green and operator-green with served suppression explicit so the rebased matrix preserves the older public boundary instead of widening it silently.",
        ),
        supporting_material_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ProofCarrying,
            article_gate.article_equivalence_green
                && article_gate.public_claim_allowed
                && article_gate.blocked_issue_ids.is_empty()
                && article_gate.blocked_blocker_ids.is_empty(),
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            Some(article_gate.report_id.clone()),
            Some(article_gate.report_digest.clone()),
            "the article-equivalence acceptance gate must stay green so the portability/minimality matrix remains bound to the declared canonical owned route.",
        ),
        supporting_material_row(
            "post_article_bridge_contract",
            TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge contract must stay green so machine classes, carrier split, and canonical route identity remain frozen while the machine matrix is extended.",
        ),
        supporting_material_row(
            "article_fast_route_architecture_selection",
            TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ProofCarrying,
            architecture_selection.fast_route_selection_green
                && architecture_selection.article_equivalence_green,
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            Some(architecture_selection.report_id.clone()),
            Some(architecture_selection.report_digest.clone()),
            "the fast-route architecture-selection report must stay green so the matrix classifies real route candidates rather than an invented route list.",
        ),
        supporting_material_row(
            "article_route_minimality_audit",
            TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ProofCarrying,
            route_minimality.route_minimality_audit_green
                && route_minimality.article_equivalence_green,
            TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
            Some(route_minimality.report_id.clone()),
            Some(route_minimality.report_digest.clone()),
            "the article route-minimality audit must stay green so component boundaries, route purity, and state-carrier minimality remain machine-checkable on the same canonical route id.",
        ),
        supporting_material_row(
            "tassadar_sync_context",
            TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass::ObservationalContext,
            true,
            ROADMAP_TAS_SYNC_REF,
            None,
            None,
            "the TAS sync file remains observational context only here and records the implementation lineage without substituting for the proof-carrying matrix artifacts.",
        ),
    ]
}

fn build_machine_matrix_rows(
    broad_portability: &TassadarBroadInternalComputePortabilityReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    turing_closeout: &TassadarTuringCompletenessCloseoutAuditReport,
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
) -> Vec<TassadarPostArticleUniversalityMachineMatrixRow> {
    let resumed_execution_parity_explicit =
        turing_closeout.operator_green && universal_substrate_gate.universal_substrate_gate_allowed;
    let route_drift_suppressed =
        gate_validation_green(universal_substrate_gate, "route_drift_rejected");

    broad_portability
        .rows
        .iter()
        .filter(|row| {
            row.profile_id == ARTICLE_CLOSEOUT_PROFILE_ID && row.backend_family == "cpu_reference"
        })
        .map(|row| {
            let declared_by_bridge_machine_identity = bridge
                .bridge_machine_identity
                .supported_machine_class_ids
                .iter()
                .any(|id| id == &row.machine_class_id);
            let supports_rebased_universality = declared_by_bridge_machine_identity
                && row.publication_allowed
                && resumed_execution_parity_explicit
                && route_drift_suppressed;
            let row_green = if declared_by_bridge_machine_identity {
                supports_rebased_universality
            } else {
                !row.publication_allowed
                    && row.suppression_reason
                        == Some(
                            TassadarBroadInternalComputeSuppressionReason::OutsideDeclaredEnvelope,
                        )
            };
            TassadarPostArticleUniversalityMachineMatrixRow {
                profile_id: row.profile_id.clone(),
                backend_family: row.backend_family.clone(),
                toolchain_family: row.toolchain_family.clone(),
                machine_class_id: row.machine_class_id.clone(),
                portability_row_status: row.row_status,
                publication_allowed: row.publication_allowed,
                suppression_reason: row.suppression_reason,
                declared_by_bridge_machine_identity,
                supports_rebased_universality,
                resumed_execution_parity_explicit,
                row_green,
                detail: format!(
                    "{} resumed_execution_parity_explicit={} declared_by_bridge_machine_identity={} supports_rebased_universality={}",
                    row.note,
                    resumed_execution_parity_explicit,
                    declared_by_bridge_machine_identity,
                    supports_rebased_universality,
                ),
            }
        })
        .collect()
}

fn build_route_classification_rows(
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    architecture_selection: &TassadarArticleFastRouteArchitectureSelectionReport,
    route_minimality: &TassadarArticleRouteMinimalityAuditReport,
) -> Vec<TassadarPostArticleUniversalityRouteClassificationRow> {
    architecture_selection
        .routeability_checks
        .iter()
        .map(|check| {
            let is_selected = check.candidate_kind == architecture_selection.selected_candidate_kind;
            let semantics_equivalence_explicit = is_selected
                && universal_substrate_gate.universal_substrate_gate_allowed
                && route_minimality.route_minimality_audit_green
                && route_minimality.canonical_claim_route_review.canonical_claim_route_green
                && route_minimality.canonical_claim_route_review.canonical_claim_route_id
                    == bridge.bridge_machine_identity.canonical_route_id
                && check.projected_route_descriptor_digest.as_deref()
                    == Some(
                        bridge
                            .bridge_machine_identity
                            .canonical_route_descriptor_digest
                            .as_str(),
                    )
                && check.requested_decode_mode
                    == Some(route_minimality.canonical_claim_route_review.selected_decode_mode);
            let route_carrier_status = if is_selected {
                TassadarPostArticleUniversalityRouteCarrierStatus::InsideUniversalityCarrier
            } else if check.candidate_kind.label() == "linear_recurrent_runtime" {
                TassadarPostArticleUniversalityRouteCarrierStatus::OutsideUniversalityCarrierAccelerationOnly
            } else {
                TassadarPostArticleUniversalityRouteCarrierStatus::OutsideUniversalityCarrierResearchOnly
            };
            let inherits_universality = matches!(
                route_carrier_status,
                TassadarPostArticleUniversalityRouteCarrierStatus::InsideUniversalityCarrier
            );
            let classification_green = if inherits_universality {
                semantics_equivalence_explicit
            } else {
                !semantics_equivalence_explicit
            };
            let detail = if inherits_universality {
                format!(
                    "candidate `{}` is the selected canonical route and stays inside the universality carrier because the universal-substrate gate is green, the route-minimality audit is green, the canonical route id matches `{}`, and the route descriptor digest stays `{}`.",
                    check.candidate_kind.label(),
                    bridge.bridge_machine_identity.canonical_route_id,
                    bridge.bridge_machine_identity.canonical_route_descriptor_digest,
                )
            } else {
                format!(
                    "candidate `{}` remains outside the universality carrier as an explicit acceleration-only or research-only surface because no semantics-equivalence proof binds it to the canonical route.",
                    check.candidate_kind.label(),
                )
            };
            TassadarPostArticleUniversalityRouteClassificationRow {
                candidate_kind: String::from(check.candidate_kind.label()),
                route_carrier_status,
                route_id: if inherits_universality {
                    Some(bridge.bridge_machine_identity.canonical_route_id.clone())
                } else {
                    None
                },
                route_descriptor_digest: check.projected_route_descriptor_digest.clone(),
                decode_mode: check
                    .requested_decode_mode
                    .map(|mode| String::from(mode.as_str())),
                semantics_equivalence_explicit,
                inherits_universality,
                classification_green,
                detail,
            }
        })
        .collect()
}

fn build_minimality_rows(
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    route_classification_rows: &[TassadarPostArticleUniversalityRouteClassificationRow],
    route_minimality: &TassadarArticleRouteMinimalityAuditReport,
) -> Vec<TassadarPostArticleUniversalityMinimalityRow> {
    let acceleration_surfaces_classified = route_classification_rows
        .iter()
        .filter(|row| {
            row.route_carrier_status
                != TassadarPostArticleUniversalityRouteCarrierStatus::InsideUniversalityCarrier
        })
        .all(|row| row.classification_green && !row.inherits_universality);

    vec![
        minimality_row(
            "machine_component_boundary_minimality",
            route_minimality.execution_ownership_review.route_purity_green
                && route_minimality.state_carrier_review.state_carrier_minimality_green,
            vec![String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF)],
            "machine-level minimality remains explicit because route purity and state-carrier minimality both stay green on the canonical route instead of reducing minimality to route shape alone.",
        ),
        minimality_row(
            "preserved_transform_minimality",
            gate_validation_green(universal_substrate_gate, "canonical_identity_binding_consistent")
                && gate_validation_green(universal_substrate_gate, "semantic_drift_blocked")
                && route_minimality.continuation_boundary_review.continuation_boundary_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF),
            ],
            "machine-level minimality remains explicit over preserved transforms because identity binding, semantic drift blocking, and continuation-boundary exclusions all stay green.",
        ),
        minimality_row(
            "acceleration_surfaces_classified",
            acceleration_surfaces_classified,
            vec![String::from(
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            )],
            "non-selected fast routes remain explicitly outside the universality carrier as acceleration-only or research-only surfaces instead of inheriting proof status by proximity.",
        ),
    ]
}

fn build_validation_rows(
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    turing_closeout: &TassadarTuringCompletenessCloseoutAuditReport,
    machine_matrix_rows: &[TassadarPostArticleUniversalityMachineMatrixRow],
    route_classification_rows: &[TassadarPostArticleUniversalityRouteClassificationRow],
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticleUniversalityPortabilityMinimalityValidationRow> {
    let declared_machine_matrix_covered = machine_matrix_rows.iter().all(|row| row.row_green)
        && machine_matrix_rows
            .iter()
            .filter(|row| row.declared_by_bridge_machine_identity)
            .count()
            >= 2;
    let fast_route_classification_explicit = route_classification_rows
        .iter()
        .all(|row| row.classification_green)
        && route_classification_rows
            .iter()
            .any(|row| row.inherits_universality)
        && route_classification_rows
            .iter()
            .filter(|row| !row.inherits_universality)
            .count()
            >= 1;

    vec![
        validation_row(
            "helper_substitution_quarantined",
            gate_validation_green(universal_substrate_gate, "helper_substitution_quarantined"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "helper substitution remains quarantined, so the portability/minimality matrix cannot be satisfied by hidden host substitution.",
        ),
        validation_row(
            "route_drift_rejected",
            gate_validation_green(universal_substrate_gate, "route_drift_rejected"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "route drift remains rejected, so the matrix stays attached to one canonical route id and one canonical route descriptor digest.",
        ),
        validation_row(
            "continuation_abuse_quarantined",
            gate_validation_green(universal_substrate_gate, "continuation_abuse_quarantined"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "continuation abuse remains quarantined, so resumed execution remains an explicit carrier property instead of a hidden second machine.",
        ),
        validation_row(
            "semantic_drift_blocked",
            gate_validation_green(universal_substrate_gate, "semantic_drift_blocked"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "semantic drift remains blocked, so the matrix does not treat portability or acceleration as license to change execution semantics.",
        ),
        validation_row(
            "declared_machine_matrix_covered",
            declared_machine_matrix_covered,
            vec![
                String::from(TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            "the matrix covers the declared bridge machine classes and keeps the out-of-envelope class explicitly suppressed instead of leaving missing cells ambiguous.",
        ),
        validation_row(
            "fast_route_classification_explicit",
            fast_route_classification_explicit,
            vec![String::from(
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            )],
            "every current fast-route candidate is classified explicitly as either inside the universality carrier or outside it as an acceleration-only or research-only surface.",
        ),
        validation_row(
            "served_suppression_boundary_preserved",
            turing_closeout.served_green == false && !served_public_universality_allowed,
            vec![String::from(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF)],
            "the older served/public suppression boundary remains preserved, so this matrix does not silently publish served universality.",
        ),
        validation_row(
            "overclaim_posture_explicit",
            gate_validation_green(universal_substrate_gate, "overclaim_posture_explicit")
                && !rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "the matrix stays inside the portability/minimality tranche only and does not publish the rebased verdict split, served/public universality, weighted plugin control, or arbitrary software capability.",
        ),
    ]
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialRow {
    TassadarPostArticleUniversalityPortabilityMinimalitySupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_artifact_id,
        source_artifact_digest,
        detail: String::from(detail),
    }
}

fn minimality_row(
    minimality_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleUniversalityMinimalityRow {
    TassadarPostArticleUniversalityMinimalityRow {
        minimality_id: String::from(minimality_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleUniversalityPortabilityMinimalityValidationRow {
    TassadarPostArticleUniversalityPortabilityMinimalityValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn gate_validation_green(
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    validation_id: &str,
) -> bool {
    universal_substrate_gate
        .validation_rows
        .iter()
        .find(|row| row.validation_id == validation_id)
        .is_some_and(|row| row.green)
}

#[must_use]
pub fn tassadar_post_article_universality_portability_minimality_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF)
}

pub fn write_tassadar_post_article_universality_portability_minimality_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_universality_portability_minimality_matrix_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError::Write {
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
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_artifact<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_inputs,
        build_tassadar_post_article_universality_portability_minimality_matrix_report,
        read_artifact, read_json,
        tassadar_post_article_universality_portability_minimality_matrix_report_path,
        write_tassadar_post_article_universality_portability_minimality_matrix_report,
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus,
        TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
        TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_broad_internal_compute_portability_report,
        build_tassadar_post_article_canonical_route_universal_substrate_gate_report,
        build_tassadar_post_article_universality_bridge_contract_report,
        build_tassadar_turing_completeness_closeout_audit_report,
        TassadarArticleFastRouteArchitectureSelectionReport,
    };
    use tempfile::tempdir;

    #[test]
    fn universality_portability_minimality_matrix_turns_green_when_prereqs_hold() {
        let report =
            build_tassadar_post_article_universality_portability_minimality_matrix_report()
                .expect("report");

        assert_eq!(
            report.matrix_status,
            TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus::Green
        );
        assert!(report.matrix_green);
        assert!(report.machine_matrix_green);
        assert!(report.route_classification_green);
        assert!(report.minimality_green);
        assert!(report.bounded_universality_story_carried);
        assert!(report.universal_substrate_gate_allowed);
        assert!(report.served_suppression_boundary_preserved);
        assert!(report.served_conformance_envelope_defined);
        assert_eq!(report.machine_matrix_rows.len(), 3);
        assert_eq!(report.route_classification_rows.len(), 4);
        assert_eq!(report.minimality_rows.len(), 3);
        assert_eq!(report.validation_rows.len(), 8);
        assert_eq!(report.deferred_issue_ids, vec![String::from("TAS-194")]);
        assert!(!report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn universality_portability_minimality_matrix_blocks_when_selected_route_leaves_carrier() {
        let universal_substrate_gate =
            build_tassadar_post_article_canonical_route_universal_substrate_gate_report()
                .expect("gate");
        let broad_portability =
            build_tassadar_broad_internal_compute_portability_report().expect("portability");
        let turing_closeout =
            build_tassadar_turing_completeness_closeout_audit_report().expect("closeout");
        let article_gate =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("article gate");
        let bridge =
            build_tassadar_post_article_universality_bridge_contract_report().expect("bridge");
        let mut architecture_selection: TassadarArticleFastRouteArchitectureSelectionReport =
            read_artifact(
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                "article_fast_route_architecture_selection_report",
            )
            .expect("architecture selection");
        let route_minimality = read_artifact(
            TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
            "article_route_minimality_audit_report",
        )
        .expect("route minimality");
        architecture_selection.selected_candidate_kind =
            crate::TassadarArticleFastRouteCandidateKind::LinearRecurrentRuntime;

        let report = build_report_from_inputs(
            universal_substrate_gate,
            broad_portability,
            turing_closeout,
            article_gate,
            bridge,
            architecture_selection,
            route_minimality,
        );

        assert_eq!(
            report.matrix_status,
            TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus::Blocked
        );
        assert!(!report.matrix_green);
        assert!(!report
            .route_classification_rows
            .iter()
            .any(|row| row.inherits_universality && row.classification_green));
    }

    #[test]
    fn universality_portability_minimality_matrix_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_universality_portability_minimality_matrix_report()
                .expect("report");
        let committed: TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport =
            read_json(
                tassadar_post_article_universality_portability_minimality_matrix_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_universality_portability_minimality_matrix_report.json"
        );
    }

    #[test]
    fn write_universality_portability_minimality_matrix_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_universality_portability_minimality_matrix_report.json");
        let written =
            write_tassadar_post_article_universality_portability_minimality_matrix_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
