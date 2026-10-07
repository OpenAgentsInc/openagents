use std::{
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
    build_tassadar_minimal_universal_substrate_acceptance_gate_report,
    build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
    build_tassadar_post_article_carrier_split_contract_report,
    build_tassadar_post_article_control_plane_decision_provenance_proof_report,
    build_tassadar_post_article_universal_machine_proof_rebinding_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_post_article_universality_witness_suite_reissue_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    TassadarMinimalUniversalSubstrateAcceptanceGateReportError,
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
    TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    TassadarPostArticleUniversalityWitnessSuiteReissueReportError,
    TassadarPostArticleUniversalityWitnessSuiteReissueStatus,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_route_universal_substrate_gate_report.json";
pub const TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-canonical-route-universal-substrate-gate.sh";

const TASSADAR_POST_ARTICLE_TURING_AUDIT_NOTE_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass {
    ProofCarrying,
    ObservationalContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteUniversalSubstratePortabilityRow {
    pub portability_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteUniversalSubstrateRefusalBoundaryRow {
    pub refusal_boundary_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteUniversalSubstrateValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub minimal_universal_substrate_gate_report_ref: String,
    pub article_equivalence_acceptance_gate_report_ref: String,
    pub bridge_contract_report_ref: String,
    pub semantic_preservation_audit_report_ref: String,
    pub control_plane_proof_report_ref: String,
    pub carrier_split_contract_report_ref: String,
    pub proof_rebinding_report_ref: String,
    pub witness_suite_reissue_report_ref: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub supporting_material_rows:
        Vec<TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialRow>,
    pub portability_rows: Vec<TassadarPostArticleCanonicalRouteUniversalSubstratePortabilityRow>,
    pub refusal_boundary_rows:
        Vec<TassadarPostArticleCanonicalRouteUniversalSubstrateRefusalBoundaryRow>,
    pub proof_rebinding_complete: bool,
    pub witness_suite_reissued: bool,
    pub bounded_universality_story_carried: bool,
    pub gate_status: TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus,
    pub gate_green: bool,
    pub universal_substrate_gate_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub validation_rows: Vec<TassadarPostArticleCanonicalRouteUniversalSubstrateValidationRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError {
    #[error(transparent)]
    MinimalGate(#[from] TassadarMinimalUniversalSubstrateAcceptanceGateReportError),
    #[error(transparent)]
    ArticleGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
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
    #[error(transparent)]
    ProofRebinding(#[from] TassadarPostArticleUniversalMachineProofRebindingReportError),
    #[error(transparent)]
    WitnessSuite(#[from] TassadarPostArticleUniversalityWitnessSuiteReissueReportError),
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

pub fn build_tassadar_post_article_canonical_route_universal_substrate_gate_report() -> Result<
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
> {
    let minimal_gate = build_tassadar_minimal_universal_substrate_acceptance_gate_report()?;
    let article_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let semantic_preservation =
        build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()?;
    let control_plane =
        build_tassadar_post_article_control_plane_decision_provenance_proof_report()?;
    let carrier_split = build_tassadar_post_article_carrier_split_contract_report()?;
    let proof_rebinding = build_tassadar_post_article_universal_machine_proof_rebinding_report()?;
    let witness_suite = build_tassadar_post_article_universality_witness_suite_reissue_report()?;
    Ok(build_report_from_inputs(
        minimal_gate,
        article_gate,
        bridge,
        semantic_preservation,
        control_plane,
        carrier_split,
        proof_rebinding,
        witness_suite,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    minimal_gate: TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    article_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    bridge: TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: TassadarPostArticleCarrierSplitContractReport,
    proof_rebinding: TassadarPostArticleUniversalMachineProofRebindingReport,
    witness_suite: TassadarPostArticleUniversalityWitnessSuiteReissueReport,
) -> TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport {
    let supporting_material_rows = build_supporting_material_rows(
        &minimal_gate,
        &article_gate,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
        &proof_rebinding,
        &witness_suite,
    );
    let portability_rows = build_portability_rows(&minimal_gate, &bridge, &witness_suite);
    let refusal_boundary_rows = build_refusal_boundary_rows(&minimal_gate, &witness_suite);
    let proof_rebinding_complete = proof_rebinding.proof_rebinding_complete;
    let witness_suite_reissued = witness_suite.witness_suite_reissued
        && witness_suite.witness_suite_status
            == TassadarPostArticleUniversalityWitnessSuiteReissueStatus::Green;
    let rebase_claim_allowed = false;
    let plugin_capability_claim_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;
    let validation_rows = build_validation_rows(
        &minimal_gate,
        &article_gate,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
        &proof_rebinding,
        &witness_suite,
        witness_suite_reissued,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );

    let proof_carrying_rows_green = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying
        })
        .all(|row| row.satisfied);
    let portability_green = portability_rows.iter().all(|row| row.green);
    let refusal_boundary_green = refusal_boundary_rows.iter().all(|row| row.green);
    let validation_green = validation_rows.iter().all(|row| row.green);
    let gate_green =
        proof_carrying_rows_green && portability_green && refusal_boundary_green && validation_green;
    let gate_status = if gate_green {
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus::Green
    } else {
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus::Blocked
    };
    let bounded_universality_story_carried = gate_green;
    let universal_substrate_gate_allowed = gate_green;
    let deferred_issue_ids = vec![String::from("TAS-193")];

    let mut report = TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_canonical_route_universal_substrate_gate.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_CHECKER_REF,
        ),
        minimal_universal_substrate_gate_report_ref: String::from(
            TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        article_equivalence_acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
        ),
        semantic_preservation_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
        ),
        control_plane_proof_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
        ),
        carrier_split_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
        ),
        proof_rebinding_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
        ),
        witness_suite_reissue_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
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
        portability_rows,
        refusal_boundary_rows,
        proof_rebinding_complete,
        witness_suite_reissued,
        bounded_universality_story_carried,
        gate_status,
        gate_green,
        universal_substrate_gate_allowed,
        deferred_issue_ids,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        validation_rows,
        claim_boundary: String::from(
            "this gate is the single machine-readable decision that the post-`TAS-186` canonical owned route now carries the bounded universality story. It requires the historical minimal universal-substrate gate, the article-equivalence closure, the bridge contract, semantic-preservation and control-plane proofs, the carrier split, proof rebinding, and the canonical-route witness-suite reissue while keeping portability, refusal truth, route drift, continuation abuse, semantic drift, and overclaim posture explicit. It does not by itself publish the rebased theory/operator/served verdict split, admit served/public universality, admit weighted plugin control, or admit arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article canonical-route universal-substrate gate keeps supporting_materials={}/9, portability_rows={}/{}, refusal_boundary_rows={}/{}, validation_rows={}/{}, bounded_universality_story_carried={}, and gate_status={:?}.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.portability_rows.iter().filter(|row| row.green).count(),
        report.portability_rows.len(),
        report
            .refusal_boundary_rows
            .iter()
            .filter(|row| row.green)
            .count(),
        report.refusal_boundary_rows.len(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.bounded_universality_story_carried,
        report.gate_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_route_universal_substrate_gate_report|",
        &report,
    );
    report
}

fn build_supporting_material_rows(
    minimal_gate: &TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    article_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    witness_suite: &TassadarPostArticleUniversalityWitnessSuiteReissueReport,
) -> Vec<TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialRow> {
    vec![
        supporting_material_row(
            "historical_minimal_universal_substrate_gate",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying,
            minimal_gate.overall_green,
            TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
            Some(minimal_gate.report_id.clone()),
            Some(minimal_gate.report_digest.clone()),
            "the historical minimal universal-substrate gate must stay green so the canonical-route rebase inherits an already explicit bounded universality substrate instead of inventing a new one by implication.",
        ),
        supporting_material_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying,
            article_gate.article_equivalence_green
                && article_gate.public_claim_allowed
                && article_gate.blocked_issue_ids.is_empty()
                && article_gate.blocked_blocker_ids.is_empty(),
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            Some(article_gate.report_id.clone()),
            Some(article_gate.report_digest.clone()),
            "the article-equivalence acceptance gate must stay green so the universality rebase is tied to the same declared canonical owned route rather than to a weaker adjacent lane.",
        ),
        supporting_material_row(
            "post_article_bridge_contract",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge contract must stay green so the canonical machine, model, weight, route, continuation, and carrier identities remain explicit while the universality story is rebound.",
        ),
        supporting_material_row(
            "post_article_semantic_preservation_audit",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying,
            semantic_preservation.semantic_preservation_audit_green,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            Some(semantic_preservation.report_id.clone()),
            Some(semantic_preservation.report_digest.clone()),
            "the semantic-preservation audit must stay green so the canonical-route universality gate is about preserved execution semantics rather than selected output parity.",
        ),
        supporting_material_row(
            "post_article_control_plane_proof",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying,
            control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete,
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            Some(control_plane.report_id.clone()),
            Some(control_plane.report_digest.clone()),
            "the control-plane proof must stay green so the rebased universality lane remains model-owned instead of host-owned or helper-owned.",
        ),
        supporting_material_row(
            "post_article_carrier_split_contract",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying,
            carrier_split.carrier_split_publication_complete,
            TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
            Some(carrier_split.report_id.clone()),
            Some(carrier_split.report_digest.clone()),
            "the carrier split must stay green so bounded resumable universality does not collapse back into the direct article-equivalent carrier.",
        ),
        supporting_material_row(
            "post_article_proof_rebinding",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying,
            proof_rebinding.proof_rebinding_complete,
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
            Some(proof_rebinding.report_id.clone()),
            Some(proof_rebinding.report_digest.clone()),
            "the proof-rebinding report must stay green so the historical universal-machine proof is already attached to the canonical machine, model, weight, and route identities.",
        ),
        supporting_material_row(
            "post_article_witness_suite_reissue",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ProofCarrying,
            witness_suite.witness_suite_reissued
                && witness_suite.witness_suite_status
                    == TassadarPostArticleUniversalityWitnessSuiteReissueStatus::Green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
            Some(witness_suite.report_id.clone()),
            Some(witness_suite.report_digest.clone()),
            "the witness-suite reissue must stay green so the older witness family is rebound onto the canonical route instead of remaining only on the historical operator lane.",
        ),
        supporting_material_row(
            "post_article_turing_audit_context",
            TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass::ObservationalContext,
            true,
            TASSADAR_POST_ARTICLE_TURING_AUDIT_NOTE_REF,
            None,
            None,
            "the March 20 post-article turing audit remains observational context only here and states why this gate exists without substituting for the proof-carrying dependency chain.",
        ),
    ]
}

fn build_portability_rows(
    minimal_gate: &TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    witness_suite: &TassadarPostArticleUniversalityWitnessSuiteReissueReport,
) -> Vec<TassadarPostArticleCanonicalRouteUniversalSubstratePortabilityRow> {
    vec![
        portability_row(
            "historical_portability_envelope_declared",
            minimal_gate
                .green_requirement_ids
                .iter()
                .any(|id| id == "portability_envelope_declared"),
            vec![String::from(
                TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
            )],
            "the historical minimal gate keeps portability envelopes explicit, so the canonical-route rebase starts from a declared runtime envelope instead of an implicit portability story.",
        ),
        portability_row(
            "canonical_route_runtime_envelopes_preserved",
            witness_suite
                .reissued_family_rows
                .iter()
                .all(|row| {
                    !row.runtime_envelope.trim().is_empty()
                        && row.canonical_identity_bound
                        && row.reissued_on_canonical_route
                }),
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
            )],
            "every reissued witness family keeps an explicit runtime envelope and canonical-route binding, so portability remains declared even before the later matrix extension issue lands.",
        ),
        portability_row(
            "canonical_machine_matrix_declared",
            !bridge.bridge_machine_identity.current_host_machine_class_id.is_empty()
                && !bridge
                    .bridge_machine_identity
                    .supported_machine_class_ids
                    .is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            )],
            "the bridge machine identity keeps the current host and supported machine classes explicit, so the gate binds to a declared machine envelope while full matrix replay remains deferred to TAS-193.",
        ),
    ]
}

fn build_refusal_boundary_rows(
    minimal_gate: &TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    witness_suite: &TassadarPostArticleUniversalityWitnessSuiteReissueReport,
) -> Vec<TassadarPostArticleCanonicalRouteUniversalSubstrateRefusalBoundaryRow> {
    vec![
        refusal_boundary_row(
            "historical_refusal_truth_explicit",
            minimal_gate
                .green_requirement_ids
                .iter()
                .any(|id| id == "refusal_truth_explicit"),
            vec![String::from(
                TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
            )],
            "the historical minimal gate keeps refusal truth explicit for out-of-profile behavior, so the canonical-route rebase inherits typed refusal instead of masking it as absence.",
        ),
        refusal_boundary_row(
            "canonical_route_refusal_boundaries_preserved",
            witness_suite.refusal_boundary_count == 2
                && witness_validation_green(witness_suite, "refusal_boundaries_preserved"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
            )],
            "the two refusal-boundary witness families remain explicit and green on the canonical route instead of being flattened into apparent universality success.",
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    minimal_gate: &TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    article_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    witness_suite: &TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    witness_suite_reissued: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticleCanonicalRouteUniversalSubstrateValidationRow> {
    let canonical_identity_binding_consistent = bridge.bridge_machine_identity.machine_identity_id
        == proof_rebinding.machine_identity_id
        && bridge.bridge_machine_identity.machine_identity_id == witness_suite.machine_identity_id
        && bridge.bridge_machine_identity.canonical_model_id == proof_rebinding.canonical_model_id
        && bridge.bridge_machine_identity.canonical_model_id == witness_suite.canonical_model_id
        && bridge.bridge_machine_identity.canonical_weight_artifact_id
            == proof_rebinding.canonical_weight_artifact_id
        && bridge.bridge_machine_identity.canonical_weight_artifact_id
            == witness_suite.canonical_weight_artifact_id
        && bridge.bridge_machine_identity.canonical_route_id == proof_rebinding.canonical_route_id
        && bridge.bridge_machine_identity.canonical_route_id == witness_suite.canonical_route_id
        && bridge.bridge_machine_identity.continuation_contract_id
            == proof_rebinding.continuation_contract_id
        && bridge.bridge_machine_identity.continuation_contract_id
            == witness_suite.continuation_contract_id;

    vec![
        validation_row(
            "canonical_identity_binding_consistent",
            canonical_identity_binding_consistent,
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF),
            ],
            "the bridge, proof-rebinding, and witness-suite reissue artifacts all bind to the same machine, model, weight, route, and continuation identities instead of drifting by metadata.",
        ),
        validation_row(
            "helper_substitution_quarantined",
            witness_validation_green(witness_suite, "helper_substitution_quarantined"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
            )],
            "helper substitution remains quarantined, so the canonical-route universality gate cannot be satisfied by hidden host helpers or synthetic receipts.",
        ),
        validation_row(
            "route_drift_rejected",
            bridge_validation_green(bridge, "route_drift_rejected")
                && witness_validation_green(witness_suite, "route_drift_rejected"),
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF),
            ],
            "route drift remains rejected, so the gate stays attached to one declared canonical route id and one declared route descriptor digest.",
        ),
        validation_row(
            "continuation_abuse_quarantined",
            bridge_validation_green(bridge, "continuation_abuse_quarantined")
                && witness_validation_green(witness_suite, "continuation_abuse_quarantined")
                && semantic_preservation.semantic_preservation_audit_green
                && control_plane.control_plane_ownership_green
                && carrier_split.carrier_split_publication_complete,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF),
            ],
            "continuation abuse remains quarantined, so resumed execution extends one declared machine instead of manufacturing a second hidden machine by host control.",
        ),
        validation_row(
            "semantic_drift_blocked",
            witness_validation_green(witness_suite, "semantic_drift_blocked")
                && semantic_preservation.semantic_preservation_audit_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF),
            ],
            "semantic drift remains blocked, so the canonical-route universality gate is about preserved execution semantics rather than selective parity snapshots.",
        ),
        validation_row(
            "article_equivalence_not_overread_as_universality",
            minimal_gate.overall_green
                && article_gate.article_equivalence_green
                && article_gate.public_claim_allowed
                && proof_rebinding.proof_rebinding_complete
                && witness_suite_reissued
                && bridge.bridge_contract_green
                && semantic_preservation.semantic_preservation_audit_green
                && control_plane.decision_provenance_proof_complete
                && carrier_split.carrier_split_publication_complete,
            vec![
                String::from(TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF),
                String::from(TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF),
            ],
            "article equivalence alone is not treated as universality; the gate turns green only once the historical substrate, bridge, proof rebinding, and canonical-route witness suite are all explicit and green.",
        ),
        validation_row(
            "overclaim_posture_explicit",
            witness_validation_green(witness_suite, "overclaim_posture_explicit")
                && !rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
            )],
            "the gate admits only the bounded canonical-route universal-substrate story and keeps the rebased verdict split, served/public universality, weighted plugin control, and arbitrary software capability explicitly out of scope.",
        ),
    ]
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialRow {
    TassadarPostArticleCanonicalRouteUniversalSubstrateSupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_artifact_id,
        source_artifact_digest,
        detail: String::from(detail),
    }
}

fn portability_row(
    portability_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalRouteUniversalSubstratePortabilityRow {
    TassadarPostArticleCanonicalRouteUniversalSubstratePortabilityRow {
        portability_id: String::from(portability_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn refusal_boundary_row(
    refusal_boundary_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalRouteUniversalSubstrateRefusalBoundaryRow {
    TassadarPostArticleCanonicalRouteUniversalSubstrateRefusalBoundaryRow {
        refusal_boundary_id: String::from(refusal_boundary_id),
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
) -> TassadarPostArticleCanonicalRouteUniversalSubstrateValidationRow {
    TassadarPostArticleCanonicalRouteUniversalSubstrateValidationRow {
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
        .is_some_and(|row| row.green)
}

fn witness_validation_green(
    witness_suite: &TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    validation_id: &str,
) -> bool {
    witness_suite
        .validation_rows
        .iter()
        .find(|row| row.validation_id == validation_id)
        .is_some_and(|row| row.green)
}

#[must_use]
pub fn tassadar_post_article_canonical_route_universal_substrate_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF)
}

pub fn write_tassadar_post_article_canonical_route_universal_substrate_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_canonical_route_universal_substrate_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError::Write {
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

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_inputs,
        build_tassadar_post_article_canonical_route_universal_substrate_gate_report, read_json,
        tassadar_post_article_canonical_route_universal_substrate_gate_report_path,
        write_tassadar_post_article_canonical_route_universal_substrate_gate_report,
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus,
        TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_minimal_universal_substrate_acceptance_gate_report,
        build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
        build_tassadar_post_article_carrier_split_contract_report,
        build_tassadar_post_article_control_plane_decision_provenance_proof_report,
        build_tassadar_post_article_universal_machine_proof_rebinding_report,
        build_tassadar_post_article_universality_bridge_contract_report,
        build_tassadar_post_article_universality_witness_suite_reissue_report,
    };
    use tempfile::tempdir;

    #[test]
    fn canonical_route_universal_substrate_gate_turns_green_when_all_prereqs_hold() {
        let report = build_tassadar_post_article_canonical_route_universal_substrate_gate_report()
            .expect("report");

        assert_eq!(
            report.gate_status,
            TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus::Green
        );
        assert!(report.gate_green);
        assert!(report.bounded_universality_story_carried);
        assert!(report.proof_rebinding_complete);
        assert!(report.witness_suite_reissued);
        assert!(report.universal_substrate_gate_allowed);
        assert_eq!(report.portability_rows.len(), 3);
        assert_eq!(report.refusal_boundary_rows.len(), 2);
        assert_eq!(report.validation_rows.len(), 7);
        assert_eq!(report.deferred_issue_ids, vec![String::from("TAS-193")]);
        assert!(!report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn canonical_route_universal_substrate_gate_blocks_when_helper_substitution_row_breaks() {
        let minimal_gate =
            build_tassadar_minimal_universal_substrate_acceptance_gate_report().expect("gate");
        let article_gate =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("article gate");
        let bridge = build_tassadar_post_article_universality_bridge_contract_report()
            .expect("bridge");
        let semantic_preservation =
            build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()
                .expect("semantic preservation");
        let control_plane = build_tassadar_post_article_control_plane_decision_provenance_proof_report()
            .expect("control plane");
        let carrier_split =
            build_tassadar_post_article_carrier_split_contract_report().expect("carrier split");
        let proof_rebinding = build_tassadar_post_article_universal_machine_proof_rebinding_report()
            .expect("proof rebinding");
        let mut witness_suite =
            build_tassadar_post_article_universality_witness_suite_reissue_report()
                .expect("witness suite");
        witness_suite
            .validation_rows
            .iter_mut()
            .find(|row| row.validation_id == "helper_substitution_quarantined")
            .expect("helper substitution row")
            .green = false;

        let report = build_report_from_inputs(
            minimal_gate,
            article_gate,
            bridge,
            semantic_preservation,
            control_plane,
            carrier_split,
            proof_rebinding,
            witness_suite,
        );

        assert_eq!(
            report.gate_status,
            TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus::Blocked
        );
        assert!(!report.gate_green);
        assert!(!report.universal_substrate_gate_allowed);
        assert!(
            !report
                .validation_rows
                .iter()
                .find(|row| row.validation_id == "helper_substitution_quarantined")
                .expect("gate helper substitution row")
                .green
        );
    }

    #[test]
    fn canonical_route_universal_substrate_gate_matches_committed_truth() {
        let generated = build_tassadar_post_article_canonical_route_universal_substrate_gate_report()
            .expect("report");
        let committed: TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport =
            read_json(tassadar_post_article_canonical_route_universal_substrate_gate_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_canonical_route_universal_substrate_gate_report.json"
        );
    }

    #[test]
    fn write_canonical_route_universal_substrate_gate_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_canonical_route_universal_substrate_gate_report.json");
        let written =
            write_tassadar_post_article_canonical_route_universal_substrate_gate_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
