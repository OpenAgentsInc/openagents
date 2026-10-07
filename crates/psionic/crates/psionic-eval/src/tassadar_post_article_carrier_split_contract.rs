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

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
    build_tassadar_post_article_control_plane_decision_provenance_proof_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_turing_completeness_closeout_audit_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    TassadarPostArticleCarrierClass, TassadarPostArticleCarrierTopology,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarTuringCompletenessCloseoutAuditReport,
    TassadarTuringCompletenessCloseoutAuditReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_carrier_split_contract_report.json";
pub const TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-carrier-split-contract.sh";

const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_tcm_v1_runtime_contract_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCarrierSplitStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCarrierSplitSupportingMaterialClass {
    ProofCarrying,
    ObservationalContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePrimaryCarrierKind {
    DirectArticleEquivalent,
    ResumableUniversality,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCarrierSplitSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleCarrierSplitSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePrimaryCarrierRow {
    pub carrier_id: String,
    pub carrier_kind: TassadarPostArticlePrimaryCarrierKind,
    pub machine_identity_id: String,
    pub carried_claim_ids: Vec<String>,
    pub required_execution_form: String,
    pub declared_state_class_ids: Vec<String>,
    pub bound_artifact_refs: Vec<String>,
    pub non_transferable_to_carrier_ids: Vec<String>,
    pub carrier_truth_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleClaimClassBindingRow {
    pub claim_class_id: String,
    pub carrier_id: String,
    pub exclusive_to_carrier: bool,
    pub transfer_to_other_carrier_blocked: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCarrierSplitValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCarrierSplitContractReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub carrier_topology: String,
    pub direct_carrier_id: String,
    pub resumable_carrier_id: String,
    pub reserved_capability_plane_id: String,
    pub supporting_material_rows: Vec<TassadarPostArticleCarrierSplitSupportingMaterialRow>,
    pub primary_carrier_rows: Vec<TassadarPostArticlePrimaryCarrierRow>,
    pub claim_class_binding_rows: Vec<TassadarPostArticleClaimClassBindingRow>,
    pub reserved_capability_plane_explicit: bool,
    pub decision_provenance_proof_complete: bool,
    pub carrier_collapse_refused: bool,
    pub carrier_split_publication_complete: bool,
    pub carrier_split_status: TassadarPostArticleCarrierSplitStatus,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub validation_rows: Vec<TassadarPostArticleCarrierSplitValidationRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCarrierSplitContractReportError {
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
    #[error(transparent)]
    SemanticPreservation(
        #[from] TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    ),
    #[error(transparent)]
    ControlPlane(#[from] TassadarPostArticleControlPlaneDecisionProvenanceProofReportError),
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    TuringCloseout(#[from] TassadarTuringCompletenessCloseoutAuditReportError),
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
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_post_article_carrier_split_contract_report() -> Result<
    TassadarPostArticleCarrierSplitContractReport,
    TassadarPostArticleCarrierSplitContractReportError,
> {
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let semantic_preservation =
        build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()?;
    let control_plane =
        build_tassadar_post_article_control_plane_decision_provenance_proof_report()?;
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let turing_closeout = build_tassadar_turing_completeness_closeout_audit_report()?;

    let direct_bridge_row = bridge
        .carrier_rows
        .iter()
        .find(|row| row.carrier_class == TassadarPostArticleCarrierClass::DirectArticleEquivalent)
        .expect("direct bridge carrier should exist");
    let resumable_bridge_row = bridge
        .carrier_rows
        .iter()
        .find(|row| row.carrier_class == TassadarPostArticleCarrierClass::ResumableUniversality)
        .expect("resumable bridge carrier should exist");
    let capability_bridge_row = bridge
        .carrier_rows
        .iter()
        .find(|row| row.carrier_class == TassadarPostArticleCarrierClass::ReservedCapabilityPlane)
        .expect("capability bridge carrier should exist");

    let supporting_material_rows = build_supporting_material_rows(
        &bridge,
        &semantic_preservation,
        &control_plane,
        &acceptance_gate,
        &turing_closeout,
    );
    let primary_carrier_rows =
        build_primary_carrier_rows(&bridge, direct_bridge_row, resumable_bridge_row);
    let claim_class_binding_rows = build_claim_class_binding_rows(
        &primary_carrier_rows,
        direct_bridge_row,
        resumable_bridge_row,
    );
    let reserved_capability_plane_explicit = capability_bridge_row.carrier_contract_green
        && capability_bridge_row.current_posture == "reserved_not_implemented";
    let carrier_collapse_refused = claim_class_binding_rows
        .iter()
        .all(|row| row.exclusive_to_carrier && row.transfer_to_other_carrier_blocked)
        && direct_bridge_row
            .widening_exclusions
            .iter()
            .any(|id| id == "checkpoint_resume")
        && resumable_bridge_row
            .widening_exclusions
            .iter()
            .any(|id| id == "direct_article_equivalence_claim");

    let decision_provenance_proof_complete = control_plane.decision_provenance_proof_complete;
    let deferred_issue_ids = vec![String::from("TAS-190")];
    let rebase_claim_allowed = false;
    let plugin_capability_claim_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let validation_rows = build_validation_rows(
        &bridge,
        &semantic_preservation,
        &control_plane,
        &primary_carrier_rows,
        &claim_class_binding_rows,
        &supporting_material_rows,
        carrier_collapse_refused,
        reserved_capability_plane_explicit,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );

    let carrier_split_publication_complete =
        supporting_material_rows.iter().all(|row| row.satisfied)
            && primary_carrier_rows
                .iter()
                .all(|row| row.carrier_truth_green)
            && carrier_collapse_refused
            && reserved_capability_plane_explicit
            && validation_rows.iter().all(|row| row.green)
            && decision_provenance_proof_complete;

    let carrier_split_status = if carrier_split_publication_complete {
        TassadarPostArticleCarrierSplitStatus::Green
    } else {
        TassadarPostArticleCarrierSplitStatus::Blocked
    };

    let mut report = TassadarPostArticleCarrierSplitContractReport {
        schema_version: 1,
        report_id: String::from("tassadar.post_article_carrier_split.contract.report.v1"),
        checker_script_ref: String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_CHECKER_REF),
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        carrier_topology: String::from(carrier_topology_id(bridge.carrier_topology)),
        direct_carrier_id: direct_bridge_row.carrier_id.clone(),
        resumable_carrier_id: resumable_bridge_row.carrier_id.clone(),
        reserved_capability_plane_id: capability_bridge_row.carrier_id.clone(),
        supporting_material_rows,
        primary_carrier_rows,
        claim_class_binding_rows,
        reserved_capability_plane_explicit,
        decision_provenance_proof_complete,
        carrier_collapse_refused,
        carrier_split_publication_complete,
        carrier_split_status,
        deferred_issue_ids,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        validation_rows,
        claim_boundary: String::from(
            "this contract publishes only the explicit split between direct article-equivalent carrier truths and bounded resumable universality carrier truths on the post-`TAS-186` bridge machine identity. It binds each claim class to one carrier and refuses later collapse into one undifferentiated route claim. It does not by itself rebind the universal-machine proof, reissue the universality witness suite, publish the rebased verdict split, admit served/public universality, admit weighted plugin control, or admit arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article carrier split contract keeps supporting_materials={}/8, primary_carriers={}/2, claim_bindings={}/7, carrier_split_status={:?}, carrier_collapse_refused={}, and carrier_split_publication_complete={}.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report
            .primary_carrier_rows
            .iter()
            .filter(|row| row.carrier_truth_green)
            .count(),
        report
            .claim_class_binding_rows
            .iter()
            .filter(|row| row.exclusive_to_carrier && row.transfer_to_other_carrier_blocked)
            .count(),
        report.carrier_split_status,
        report.carrier_collapse_refused,
        report.carrier_split_publication_complete,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_carrier_split_contract_report|",
        &report,
    );
    Ok(report)
}

fn build_supporting_material_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
    turing_closeout: &TassadarTuringCompletenessCloseoutAuditReport,
) -> Vec<TassadarPostArticleCarrierSplitSupportingMaterialRow> {
    vec![
        supporting_material_row(
            "bridge_contract",
            TassadarPostArticleCarrierSplitSupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge contract must stay green before the direct and resumable carriers can be published as separate contract-bearing lanes",
        ),
        supporting_material_row(
            "semantic_preservation_audit",
            TassadarPostArticleCarrierSplitSupportingMaterialClass::ProofCarrying,
            semantic_preservation.semantic_preservation_audit_green,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            Some(semantic_preservation.report_id.clone()),
            Some(semantic_preservation.report_digest.clone()),
            "the semantic-preservation audit must stay green so the two carriers do not collapse across incompatible state and continuation semantics",
        ),
        supporting_material_row(
            "control_plane_proof",
            TassadarPostArticleCarrierSplitSupportingMaterialClass::ProofCarrying,
            control_plane.control_plane_ownership_green,
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            Some(control_plane.report_id.clone()),
            Some(control_plane.report_digest.clone()),
            "the control-plane proof must stay green so the split remains tied to one machine-owned decision contract rather than host-owned route collapse",
        ),
        supporting_material_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleCarrierSplitSupportingMaterialClass::ProofCarrying,
            acceptance_gate.public_claim_allowed,
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            Some(acceptance_gate.report_id.clone()),
            Some(acceptance_gate.report_digest.clone()),
            "the article acceptance gate must stay green so the direct carrier remains the canonical article-equivalent route only",
        ),
        supporting_material_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleCarrierSplitSupportingMaterialClass::ProofCarrying,
            bridge.tcm_v1_runtime_contract_report.overall_green,
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
            Some(bridge.tcm_v1_runtime_contract_report.report_id.clone()),
            Some(bridge.tcm_v1_runtime_contract_report.report_digest.clone()),
            "the declared `TCM.v1` runtime contract must stay green so the resumable carrier remains explicit instead of being inferred loosely from the direct route",
        ),
        supporting_material_row(
            "turing_completeness_closeout",
            TassadarPostArticleCarrierSplitSupportingMaterialClass::ProofCarrying,
            turing_closeout.operator_green && !turing_closeout.served_green,
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
            Some(turing_closeout.report_id.clone()),
            Some(turing_closeout.report_digest.clone()),
            "the historical closeout must stay green so the resumable carrier continues to own the bounded theory/operator universality lane instead of the direct article-equivalent lane",
        ),
        supporting_material_row(
            "post_article_turing_audit_context",
            TassadarPostArticleCarrierSplitSupportingMaterialClass::ObservationalContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 rebase audit remains observational context only here and motivates the explicit carrier split without substituting for the proof-carrying contract rows",
        ),
        supporting_material_row(
            "transformer_stack_boundary_context",
            TassadarPostArticleCarrierSplitSupportingMaterialClass::ObservationalContext,
            true,
            TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF,
            None,
            None,
            "the transformer-stack boundary remains observational context only here and records the canonical route boundary that the direct carrier is not allowed to widen around",
        ),
    ]
}

fn build_primary_carrier_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    direct_bridge_row: &crate::TassadarPostArticleCarrierRow,
    resumable_bridge_row: &crate::TassadarPostArticleCarrierRow,
) -> Vec<TassadarPostArticlePrimaryCarrierRow> {
    vec![
        TassadarPostArticlePrimaryCarrierRow {
            carrier_id: direct_bridge_row.carrier_id.clone(),
            carrier_kind: TassadarPostArticlePrimaryCarrierKind::DirectArticleEquivalent,
            machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
            carried_claim_ids: direct_bridge_row.claim_ids.clone(),
            required_execution_form: String::from("direct_single_run_only"),
            declared_state_class_ids: direct_bridge_row.state_classes.clone(),
            bound_artifact_refs: direct_bridge_row.source_refs.clone(),
            non_transferable_to_carrier_ids: vec![resumable_bridge_row.carrier_id.clone()],
            carrier_truth_green: direct_bridge_row.carrier_contract_green,
            detail: String::from(
                "the direct carrier owns only the bounded article-equivalent, direct-route-minimality, single-run-no-spill, and declared-machine-matrix truths. It does not own resumable universality or Turing-completeness closeout truth.",
            ),
        },
        TassadarPostArticlePrimaryCarrierRow {
            carrier_id: resumable_bridge_row.carrier_id.clone(),
            carrier_kind: TassadarPostArticlePrimaryCarrierKind::ResumableUniversality,
            machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
            carried_claim_ids: resumable_bridge_row.claim_ids.clone(),
            required_execution_form: String::from("resumable_continuation_only"),
            declared_state_class_ids: resumable_bridge_row.state_classes.clone(),
            bound_artifact_refs: resumable_bridge_row.source_refs.clone(),
            non_transferable_to_carrier_ids: vec![direct_bridge_row.carrier_id.clone()],
            carrier_truth_green: resumable_bridge_row.carrier_contract_green,
            detail: String::from(
                "the resumable carrier owns only the theory/operator universality and bounded Turing-completeness closeout truths. It does not own direct article-equivalent same-run truth.",
            ),
        },
    ]
}

fn build_claim_class_binding_rows(
    primary_carrier_rows: &[TassadarPostArticlePrimaryCarrierRow],
    direct_bridge_row: &crate::TassadarPostArticleCarrierRow,
    resumable_bridge_row: &crate::TassadarPostArticleCarrierRow,
) -> Vec<TassadarPostArticleClaimClassBindingRow> {
    let direct_carrier = primary_carrier_rows
        .iter()
        .find(|row| {
            row.carrier_kind == TassadarPostArticlePrimaryCarrierKind::DirectArticleEquivalent
        })
        .expect("direct carrier row should exist");
    let resumable_carrier = primary_carrier_rows
        .iter()
        .find(|row| {
            row.carrier_kind == TassadarPostArticlePrimaryCarrierKind::ResumableUniversality
        })
        .expect("resumable carrier row should exist");

    let mut rows = Vec::new();
    for claim_id in &direct_bridge_row.claim_ids {
        rows.push(TassadarPostArticleClaimClassBindingRow {
            claim_class_id: claim_id.clone(),
            carrier_id: direct_carrier.carrier_id.clone(),
            exclusive_to_carrier: true,
            transfer_to_other_carrier_blocked: true,
            detail: format!(
                "claim class `{}` is bound only to the direct article-equivalent carrier and may not be transferred onto the resumable carrier by implication.",
                claim_id
            ),
        });
    }
    for claim_id in &resumable_bridge_row.claim_ids {
        rows.push(TassadarPostArticleClaimClassBindingRow {
            claim_class_id: claim_id.clone(),
            carrier_id: resumable_carrier.carrier_id.clone(),
            exclusive_to_carrier: true,
            transfer_to_other_carrier_blocked: true,
            detail: format!(
                "claim class `{}` is bound only to the resumable universality carrier and may not be transferred onto the direct article-equivalent carrier by implication.",
                claim_id
            ),
        });
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    primary_carrier_rows: &[TassadarPostArticlePrimaryCarrierRow],
    claim_class_binding_rows: &[TassadarPostArticleClaimClassBindingRow],
    supporting_material_rows: &[TassadarPostArticleCarrierSplitSupportingMaterialRow],
    carrier_collapse_refused: bool,
    reserved_capability_plane_explicit: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticleCarrierSplitValidationRow> {
    let direct_claims = primary_carrier_rows
        .iter()
        .find(|row| {
            row.carrier_kind == TassadarPostArticlePrimaryCarrierKind::DirectArticleEquivalent
        })
        .map(|row| {
            row.carried_claim_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let resumable_claims = primary_carrier_rows
        .iter()
        .find(|row| {
            row.carrier_kind == TassadarPostArticlePrimaryCarrierKind::ResumableUniversality
        })
        .map(|row| {
            row.carried_claim_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let proof_carrying_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleCarrierSplitSupportingMaterialClass::ProofCarrying
        })
        .count();
    let observational_context_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleCarrierSplitSupportingMaterialClass::ObservationalContext
        })
        .count();

    vec![
        validation_row(
            "helper_substitution_quarantined",
            bridge_validation_green(bridge, "helper_substitution_quarantined"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "helper substitution remains quarantined, so the carrier split cannot be redefined through hidden host helpers.",
        ),
        validation_row(
            "route_drift_rejected",
            bridge_validation_green(bridge, "route_drift_rejected"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "route drift remains rejected, so the direct carrier stays tied to one fixed canonical route id.",
        ),
        validation_row(
            "continuation_abuse_quarantined",
            bridge_validation_green(bridge, "continuation_abuse_quarantined")
                && semantic_preservation.semantic_preservation_audit_green
                && control_plane.decision_provenance_proof_complete,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
            ],
            "continuation abuse remains quarantined, so direct and resumable carriers cannot be collapsed through hidden continuation semantics.",
        ),
        validation_row(
            "semantic_drift_blocked",
            bridge_validation_green(bridge, "semantic_drift_blocked")
                && semantic_preservation.semantic_preservation_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "semantic drift remains blocked, so the two carriers retain their declared semantics instead of collapsing into one blended claim class.",
        ),
        validation_row(
            "carrier_collapse_refused",
            carrier_collapse_refused
                && direct_claims.is_disjoint(&resumable_claims)
                && claim_class_binding_rows.iter().all(|row| row.exclusive_to_carrier),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "carrier collapse is refused: direct and resumable claim sets remain disjoint, every claim class binds to exactly one carrier, and transfer by implication is blocked.",
        ),
        validation_row(
            "proof_class_distinction_preserved",
            proof_carrying_count == 6 && observational_context_count == 2,
            vec![
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF),
            ],
            "proof-carrying artifacts remain distinct from observational context while the split is published as its own contract.",
        ),
        validation_row(
            "overclaim_posture_explicit",
            reserved_capability_plane_explicit
                && !rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            "the carrier split remains bounded: capability plane stays reserved, and rebased universality, served/public universality, weighted plugin control, and arbitrary software capability all stay blocked.",
        ),
    ]
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleCarrierSplitSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_report_id: Option<String>,
    source_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleCarrierSplitSupportingMaterialRow {
    TassadarPostArticleCarrierSplitSupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_report_id,
        source_report_digest,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCarrierSplitValidationRow {
    TassadarPostArticleCarrierSplitValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn bridge_validation_green(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    validation_id: &str,
) -> bool {
    bridge
        .validation_rows
        .iter()
        .find(|row| row.validation_id == validation_id)
        .map(|row| row.green)
        .unwrap_or(false)
}

fn carrier_topology_id(topology: TassadarPostArticleCarrierTopology) -> &'static str {
    match topology {
        TassadarPostArticleCarrierTopology::ExplicitSplitAcrossDirectAndResumableLanes => {
            "explicit_split_across_direct_and_resumable_lanes"
        }
    }
}

#[must_use]
pub fn tassadar_post_article_carrier_split_contract_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF)
}

pub fn write_tassadar_post_article_carrier_split_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCarrierSplitContractReport,
    TassadarPostArticleCarrierSplitContractReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCarrierSplitContractReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_carrier_split_contract_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCarrierSplitContractReportError::Write {
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
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
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
) -> Result<T, TassadarPostArticleCarrierSplitContractReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCarrierSplitContractReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCarrierSplitContractReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_carrier_split_contract_report, read_repo_json,
        tassadar_post_article_carrier_split_contract_report_path,
        write_tassadar_post_article_carrier_split_contract_report,
        TassadarPostArticleCarrierSplitContractReport, TassadarPostArticleCarrierSplitStatus,
        TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn carrier_split_contract_freezes_direct_and_resumable_bindings() {
        let report = build_tassadar_post_article_carrier_split_contract_report().expect("report");

        assert_eq!(
            report.carrier_split_status,
            TassadarPostArticleCarrierSplitStatus::Green
        );
        assert!(report.carrier_split_publication_complete);
        assert!(report.carrier_collapse_refused);
        assert!(report.reserved_capability_plane_explicit);
        assert!(report.decision_provenance_proof_complete);
        assert_eq!(
            report.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            report.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(report.primary_carrier_rows.len(), 2);
        assert_eq!(report.claim_class_binding_rows.len(), 7);
        assert!(!report.rebase_claim_allowed);
        assert_eq!(report.deferred_issue_ids, vec![String::from("TAS-190")]);
    }

    #[test]
    fn carrier_split_contract_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_carrier_split_contract_report().expect("report");
        let committed: TassadarPostArticleCarrierSplitContractReport =
            read_repo_json(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_carrier_split_contract_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_carrier_split_contract_report.json")
        );
    }

    #[test]
    fn write_carrier_split_contract_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_carrier_split_contract_report.json");
        let written = write_tassadar_post_article_carrier_split_contract_report(&output_path)
            .expect("write report");
        let persisted: TassadarPostArticleCarrierSplitContractReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
