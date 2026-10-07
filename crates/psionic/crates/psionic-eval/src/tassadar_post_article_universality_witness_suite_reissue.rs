use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_data::{TassadarUniversalityWitnessExpectation, TassadarUniversalityWitnessFamily};
use psionic_runtime::{
    build_tassadar_tcm_v1_runtime_contract_report, TassadarTcmV1RuntimeContractReport,
    TassadarTcmV1RuntimeContractReportError, TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
};

use crate::{
    build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
    build_tassadar_post_article_carrier_split_contract_report,
    build_tassadar_post_article_control_plane_decision_provenance_proof_report,
    build_tassadar_post_article_universal_machine_proof_rebinding_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_universality_witness_suite_report,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    TassadarPostArticleCarrierSplitContractReport,
    TassadarPostArticleCarrierSplitContractReportError,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
    TassadarPostArticleUniversalMachineProofRebindingReport,
    TassadarPostArticleUniversalMachineProofRebindingReportError,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarUniversalityWitnessSuiteReport, TassadarUniversalityWitnessSuiteReportError,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_witness_suite_reissue_report.json";
pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-universality-witness-suite-reissue.sh";

const TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_universality_witness_suite_report.json";
const TASSADAR_UNIVERSALITY_VERDICT_SPLIT_AUDIT_NOTE_REF: &str =
    "docs/audits/2026-03-19-tassadar-universality-verdict-split-audit.md";
const TASSADAR_POST_ARTICLE_TURING_AUDIT_NOTE_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleUniversalityWitnessSuiteReissueStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleUniversalityWitnessSupportingMaterialClass {
    ProofCarrying,
    ObservationalContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityWitnessSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleUniversalityWitnessSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleReissuedWitnessFamilyRow {
    pub witness_family: TassadarUniversalityWitnessFamily,
    pub expected_status: TassadarUniversalityWitnessExpectation,
    pub historical_row_satisfied: bool,
    pub canonical_machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub evidence_anchor_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_runtime_parity: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_resume_equivalent: Option<bool>,
    pub refusal_boundary_held: bool,
    pub runtime_envelope: String,
    pub canonical_identity_bound: bool,
    pub reissued_on_canonical_route: bool,
    pub satisfied: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityWitnessValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityWitnessSuiteReissueReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub historical_witness_suite_report_ref: String,
    pub historical_witness_suite_report_id: String,
    pub historical_witness_suite_report_digest: String,
    pub runtime_contract_ref: String,
    pub runtime_contract_id: String,
    pub runtime_contract_digest: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub supporting_material_rows: Vec<TassadarPostArticleUniversalityWitnessSupportingMaterialRow>,
    pub reissued_family_rows: Vec<TassadarPostArticleReissuedWitnessFamilyRow>,
    pub exact_family_count: u32,
    pub refusal_boundary_count: u32,
    pub proof_rebinding_complete: bool,
    pub witness_suite_reissued: bool,
    pub witness_suite_status: TassadarPostArticleUniversalityWitnessSuiteReissueStatus,
    pub universal_substrate_gate_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub validation_rows: Vec<TassadarPostArticleUniversalityWitnessValidationRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleUniversalityWitnessSuiteReissueReportError {
    #[error(transparent)]
    HistoricalSuite(#[from] TassadarUniversalityWitnessSuiteReportError),
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error(transparent)]
    ProofRebinding(#[from] TassadarPostArticleUniversalMachineProofRebindingReportError),
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
    #[error(transparent)]
    SemanticPreservation(
        #[from] TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    ),
    #[error(transparent)]
    ControlPlane(#[from] TassadarPostArticleControlPlaneDecisionProvenanceProofReportError),
    #[error(transparent)]
    CarrierSplit(#[from] TassadarPostArticleCarrierSplitContractReportError),
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

pub fn build_tassadar_post_article_universality_witness_suite_reissue_report() -> Result<
    TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    TassadarPostArticleUniversalityWitnessSuiteReissueReportError,
> {
    let historical_suite = build_tassadar_universality_witness_suite_report()?;
    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let proof_rebinding = build_tassadar_post_article_universal_machine_proof_rebinding_report()?;
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let semantic_preservation =
        build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()?;
    let control_plane =
        build_tassadar_post_article_control_plane_decision_provenance_proof_report()?;
    let carrier_split = build_tassadar_post_article_carrier_split_contract_report()?;

    let supporting_material_rows = build_supporting_material_rows(
        &historical_suite,
        &runtime_contract,
        &proof_rebinding,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
    );
    let reissued_family_rows = build_reissued_family_rows(
        &historical_suite,
        &runtime_contract,
        &proof_rebinding,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
    );
    let exact_family_count = reissued_family_rows
        .iter()
        .filter(|row| row.expected_status == TassadarUniversalityWitnessExpectation::Exact)
        .filter(|row| row.satisfied)
        .count() as u32;
    let refusal_boundary_count = reissued_family_rows
        .iter()
        .filter(|row| {
            row.expected_status == TassadarUniversalityWitnessExpectation::RefusalBoundary
        })
        .filter(|row| row.satisfied)
        .count() as u32;

    let proof_rebinding_complete = proof_rebinding.proof_rebinding_complete;
    let universal_substrate_gate_allowed = false;
    let deferred_issue_ids = vec![String::from("TAS-192")];
    let rebase_claim_allowed = false;
    let plugin_capability_claim_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let validation_rows = build_validation_rows(
        &proof_rebinding,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
        &reissued_family_rows,
        &supporting_material_rows,
        universal_substrate_gate_allowed,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );

    let witness_suite_reissued = proof_rebinding_complete
        && supporting_material_rows
            .iter()
            .filter(|row| {
                row.material_class
                    == TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying
            })
            .all(|row| row.satisfied)
        && reissued_family_rows.iter().all(|row| row.satisfied)
        && validation_rows.iter().all(|row| row.green);
    let witness_suite_status = if witness_suite_reissued {
        TassadarPostArticleUniversalityWitnessSuiteReissueStatus::Green
    } else {
        TassadarPostArticleUniversalityWitnessSuiteReissueStatus::Blocked
    };

    let mut report = TassadarPostArticleUniversalityWitnessSuiteReissueReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_universality_witness_suite_reissue.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_CHECKER_REF,
        ),
        historical_witness_suite_report_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
        historical_witness_suite_report_id: historical_suite.report_id.clone(),
        historical_witness_suite_report_digest: historical_suite.report_digest.clone(),
        runtime_contract_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
        runtime_contract_id: runtime_contract.report_id.clone(),
        runtime_contract_digest: runtime_contract.report_digest.clone(),
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
        canonical_weight_artifact_id: bridge
            .bridge_machine_identity
            .canonical_weight_artifact_id
            .clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        continuation_contract_id: bridge.bridge_machine_identity.continuation_contract_id.clone(),
        supporting_material_rows,
        reissued_family_rows,
        exact_family_count,
        refusal_boundary_count,
        proof_rebinding_complete,
        witness_suite_reissued,
        witness_suite_status,
        universal_substrate_gate_allowed,
        deferred_issue_ids,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        validation_rows,
        claim_boundary: String::from(
            "this report reissues the dedicated universality witness suite onto the post-`TAS-186` canonical machine, model, weight, and route identities. It keeps exact and refusal-boundary families explicit and keeps helper substitution, hidden cache-owned control flow, resume-only cheating, and wider overclaim posture blocked. It does not by itself enable the canonical-route universal-substrate gate, publish the rebased verdict split, admit served/public universality, admit weighted plugin control, or admit arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article universality witness-suite reissue keeps supporting_materials={}/9, family_rows={}/{}, validation_rows={}/{}, witness_suite_reissued={}, and witness_suite_status={:?}.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.reissued_family_rows.iter().filter(|row| row.satisfied).count(),
        report.reissued_family_rows.len(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.witness_suite_reissued,
        report.witness_suite_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_universality_witness_suite_reissue_report|",
        &report,
    );
    Ok(report)
}

fn build_supporting_material_rows(
    historical_suite: &TassadarUniversalityWitnessSuiteReport,
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
) -> Vec<TassadarPostArticleUniversalityWitnessSupportingMaterialRow> {
    vec![
        supporting_material_row(
            "historical_universality_witness_suite",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying,
            historical_suite.overall_green,
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
            Some(historical_suite.report_id.clone()),
            Some(historical_suite.report_digest.clone()),
            "the historical witness suite must stay green so the reissue starts from the older operator-lane suite rather than from sampled or partial evidence.",
        ),
        supporting_material_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying,
            runtime_contract.overall_green,
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
            Some(runtime_contract.report_id.clone()),
            Some(runtime_contract.report_digest.clone()),
            "the declared `TCM.v1` runtime contract must stay green so resumed execution and refusal boundaries remain machine-legible under the same continuation contract id.",
        ),
        supporting_material_row(
            "post_article_proof_rebinding",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying,
            proof_rebinding.proof_rebinding_complete,
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
            Some(proof_rebinding.report_id.clone()),
            Some(proof_rebinding.report_digest.clone()),
            "the proof-rebinding report must stay green so the historical universal-machine anchors already bind to the canonical machine, model, weight, and route identities.",
        ),
        supporting_material_row(
            "post_article_bridge_contract",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge contract must stay green so the canonical machine tuple and resumable-lane ownership remain explicit for the reissued suite.",
        ),
        supporting_material_row(
            "post_article_semantic_preservation_audit",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying,
            semantic_preservation.semantic_preservation_audit_green,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            Some(semantic_preservation.report_id.clone()),
            Some(semantic_preservation.report_digest.clone()),
            "the semantic-preservation audit must stay green so the reissued suite does not silently move witness meaning across semantic drift.",
        ),
        supporting_material_row(
            "post_article_control_plane_proof",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying,
            control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete,
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            Some(control_plane.report_id.clone()),
            Some(control_plane.report_digest.clone()),
            "the control-plane proof must stay green so the reissued suite remains model-owned instead of cache-owned or host-owned.",
        ),
        supporting_material_row(
            "post_article_carrier_split_contract",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying,
            carrier_split.carrier_split_publication_complete,
            TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
            Some(carrier_split.report_id.clone()),
            Some(carrier_split.report_digest.clone()),
            "the carrier split must stay green so the reissued suite remains inside the resumable universality carrier instead of collapsing into the direct article-equivalent carrier.",
        ),
        supporting_material_row(
            "historical_verdict_split_audit_context",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ObservationalContext,
            true,
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_AUDIT_NOTE_REF,
            None,
            None,
            "the March 19 verdict-split audit remains observational context only here and reminds the suite reissue not to widen into verdict publication.",
        ),
        supporting_material_row(
            "post_article_turing_audit_context",
            TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ObservationalContext,
            true,
            TASSADAR_POST_ARTICLE_TURING_AUDIT_NOTE_REF,
            None,
            None,
            "the March 20 post-article turing audit remains observational context only here and motivates the canonical-route suite reissue without substituting for proof-carrying artifacts.",
        ),
    ]
}

fn build_reissued_family_rows(
    historical_suite: &TassadarUniversalityWitnessSuiteReport,
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
) -> Vec<TassadarPostArticleReissuedWitnessFamilyRow> {
    historical_suite
        .family_rows
        .iter()
        .map(|row| {
            let canonical_identity_bound = bridge.bridge_contract_green
                && proof_rebinding.proof_rebinding_complete
                && !bridge.bridge_machine_identity.machine_identity_id.is_empty()
                && !bridge.bridge_machine_identity.canonical_model_id.is_empty()
                && !bridge.bridge_machine_identity.canonical_route_id.is_empty();
            let reissued_on_canonical_route = match row.witness_family {
                TassadarUniversalityWitnessFamily::RegisterMachine => {
                    proof_rebinding
                        .rebound_encoding_ids
                        .iter()
                        .any(|id| id == "tcm.encoding.two_register_counter_loop.v1")
                }
                TassadarUniversalityWitnessFamily::TapeMachine => proof_rebinding
                    .rebound_encoding_ids
                    .iter()
                    .any(|id| id == "tcm.encoding.single_tape_bit_flip.v1"),
                TassadarUniversalityWitnessFamily::BytecodeVmInterpreter => {
                    proof_rebinding.proof_transport_audit_complete && bridge.bridge_contract_green
                }
                TassadarUniversalityWitnessFamily::SessionProcessKernel => {
                    semantic_preservation.semantic_preservation_audit_green
                        && control_plane.decision_provenance_proof_complete
                }
                TassadarUniversalityWitnessFamily::SpillTapeContinuation => {
                    semantic_preservation.semantic_preservation_audit_green
                        && carrier_split.carrier_split_publication_complete
                }
                TassadarUniversalityWitnessFamily::BytecodeVmParamBoundary
                | TassadarUniversalityWitnessFamily::ExternalEventLoopBoundary => {
                    runtime_contract.overall_green
                }
            };
            let satisfied = row.satisfied && canonical_identity_bound && reissued_on_canonical_route;
            TassadarPostArticleReissuedWitnessFamilyRow {
                witness_family: row.witness_family,
                expected_status: row.expected_status,
                historical_row_satisfied: row.satisfied,
                canonical_machine_identity_id: bridge
                    .bridge_machine_identity
                    .machine_identity_id
                    .clone(),
                canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
                canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
                evidence_anchor_ids: row.evidence_anchor_ids.clone(),
                exact_runtime_parity: row.exact_runtime_parity,
                checkpoint_resume_equivalent: row.checkpoint_resume_equivalent,
                refusal_boundary_held: row.refusal_boundary_held,
                runtime_envelope: row.runtime_envelope.clone(),
                canonical_identity_bound,
                reissued_on_canonical_route,
                satisfied,
                detail: format!(
                    "historical witness family `{:?}` is rebound onto machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` with evidence anchors={} and expected_status={:?}.",
                    row.witness_family,
                    bridge.bridge_machine_identity.machine_identity_id,
                    bridge.bridge_machine_identity.canonical_model_id,
                    bridge.bridge_machine_identity.canonical_route_id,
                    row.evidence_anchor_ids.len(),
                    row.expected_status,
                ),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
    reissued_family_rows: &[TassadarPostArticleReissuedWitnessFamilyRow],
    supporting_material_rows: &[TassadarPostArticleUniversalityWitnessSupportingMaterialRow],
    universal_substrate_gate_allowed: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticleUniversalityWitnessValidationRow> {
    let proof_carrying_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleUniversalityWitnessSupportingMaterialClass::ProofCarrying
        })
        .count();
    let refusal_rows_green = reissued_family_rows
        .iter()
        .filter(|row| {
            row.expected_status == TassadarUniversalityWitnessExpectation::RefusalBoundary
        })
        .all(|row| row.satisfied && row.refusal_boundary_held);

    vec![
        validation_row(
            "helper_substitution_quarantined",
            proof_rebinding_validation_green(proof_rebinding, "helper_substitution_quarantined"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF)],
            "helper substitution remains quarantined, so the reissued witness suite cannot be satisfied by hidden helper execution.",
        ),
        validation_row(
            "route_drift_rejected",
            bridge_validation_green(bridge, "route_drift_rejected"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "route drift remains rejected, so every reissued witness row binds to one declared canonical route id.",
        ),
        validation_row(
            "continuation_abuse_quarantined",
            bridge_validation_green(bridge, "continuation_abuse_quarantined")
                && semantic_preservation.semantic_preservation_audit_green
                && carrier_split.carrier_split_publication_complete,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
            ],
            "continuation abuse remains quarantined, so continuation-stress witnesses stay in the declared continuation carrier instead of creating resume-only cheating lanes.",
        ),
        validation_row(
            "semantic_drift_blocked",
            proof_rebinding_validation_green(proof_rebinding, "semantic_drift_blocked")
                && semantic_preservation.semantic_preservation_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "semantic drift remains blocked, so the reissued witness suite preserves historical family meaning instead of sampled output parity alone.",
        ),
        validation_row(
            "hidden_cache_owned_control_flow_blocked",
            proof_rebinding_validation_green(proof_rebinding, "cache_and_batching_drift_blocked")
                && control_plane.decision_provenance_proof_complete,
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
            ],
            "hidden cache-owned or batching-owned control flow remains blocked, so the canonical-route suite is not secretly satisfied by cache-held workflow logic.",
        ),
        validation_row(
            "resume_only_cheating_blocked",
            proof_rebinding_validation_green(proof_rebinding, "continuation_abuse_quarantined")
                && carrier_split.carrier_split_publication_complete,
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
            ],
            "resume-only cheating remains blocked, so resumed execution can extend declared witnesses but cannot become a hidden second machine that manufactures green rows.",
        ),
        validation_row(
            "refusal_boundaries_preserved",
            refusal_rows_green && proof_carrying_count == 7,
            vec![String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF)],
            "the two refusal-boundary witness families remain explicit and green while the proof-carrying reissue rows stay distinct from observational audit context.",
        ),
        validation_row(
            "overclaim_posture_explicit",
            !universal_substrate_gate_allowed
                && !rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF)],
            "the reissued witness suite remains bounded: it does not by itself enable the universal-substrate gate, rebased verdict split, served/public universality, weighted plugin control, or arbitrary software capability.",
        ),
    ]
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleUniversalityWitnessSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleUniversalityWitnessSupportingMaterialRow {
    TassadarPostArticleUniversalityWitnessSupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_artifact_id,
        source_artifact_digest,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleUniversalityWitnessValidationRow {
    TassadarPostArticleUniversalityWitnessValidationRow {
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

fn proof_rebinding_validation_green(
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    validation_id: &str,
) -> bool {
    proof_rebinding
        .validation_rows
        .iter()
        .find(|row| row.validation_id == validation_id)
        .map(|row| row.green)
        .unwrap_or(false)
}

#[must_use]
pub fn tassadar_post_article_universality_witness_suite_reissue_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF)
}

pub fn write_tassadar_post_article_universality_witness_suite_reissue_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    TassadarPostArticleUniversalityWitnessSuiteReissueReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleUniversalityWitnessSuiteReissueReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_universality_witness_suite_reissue_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleUniversalityWitnessSuiteReissueReportError::Write {
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
) -> Result<T, TassadarPostArticleUniversalityWitnessSuiteReissueReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleUniversalityWitnessSuiteReissueReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalityWitnessSuiteReissueReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_universality_witness_suite_reissue_report, read_repo_json,
        tassadar_post_article_universality_witness_suite_reissue_report_path,
        write_tassadar_post_article_universality_witness_suite_reissue_report,
        TassadarPostArticleUniversalityWitnessSuiteReissueReport,
        TassadarPostArticleUniversalityWitnessSuiteReissueStatus,
        TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn witness_suite_reissue_report_keeps_exact_and_refusal_rows_green() {
        let report = build_tassadar_post_article_universality_witness_suite_reissue_report()
            .expect("report");

        assert_eq!(
            report.witness_suite_status,
            TassadarPostArticleUniversalityWitnessSuiteReissueStatus::Green
        );
        assert!(report.proof_rebinding_complete);
        assert!(report.witness_suite_reissued);
        assert_eq!(report.exact_family_count, 5);
        assert_eq!(report.refusal_boundary_count, 2);
        assert_eq!(report.deferred_issue_ids, vec![String::from("TAS-192")]);
        assert!(!report.universal_substrate_gate_allowed);
        assert!(!report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn witness_suite_reissue_report_matches_committed_truth() {
        let generated = build_tassadar_post_article_universality_witness_suite_reissue_report()
            .expect("report");
        let committed: TassadarPostArticleUniversalityWitnessSuiteReissueReport =
            read_repo_json(TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_universality_witness_suite_reissue_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_universality_witness_suite_reissue_report.json")
        );
    }

    #[test]
    fn write_witness_suite_reissue_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_universality_witness_suite_reissue_report.json");
        let written =
            write_tassadar_post_article_universality_witness_suite_reissue_report(&output_path)
                .expect("write report");
        let persisted: TassadarPostArticleUniversalityWitnessSuiteReissueReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
