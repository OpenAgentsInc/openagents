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
    build_tassadar_tcm_v1_runtime_contract_report, TassadarTcmV1RuntimeContractReport,
    TassadarTcmV1RuntimeContractReportError, TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
    build_tassadar_post_article_carrier_split_contract_report,
    build_tassadar_post_article_control_plane_decision_provenance_proof_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_universal_machine_proof_report, TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    TassadarPostArticleCarrierSplitContractReport,
    TassadarPostArticleCarrierSplitContractReportError,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError, TassadarUniversalMachineProofReport,
    TassadarUniversalMachineProofReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universal_machine_proof_rebinding_report.json";
pub const TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-universal-machine-proof-rebinding.sh";

const TASSADAR_TURING_CLOSEOUT_AUDIT_NOTE_REF: &str =
    "docs/audits/2026-03-19-tassadar-turing-completeness-closeout-audit.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleUniversalMachineProofRebindingStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleUniversalMachineProofSupportingMaterialClass {
    ProofCarrying,
    ObservationalContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalMachineProofSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleUniversalMachineProofSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleProofTransportBoundary {
    pub boundary_id: String,
    pub preserved_transition_class_ids: Vec<String>,
    pub admitted_variance_ids: Vec<String>,
    pub blocked_drift_ids: Vec<String>,
    pub boundary_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalMachineProofTransportReceiptRow {
    pub receipt_id: String,
    pub encoding_id: String,
    pub historical_proof_satisfied: bool,
    pub resumed_execution_equivalence_explicit: bool,
    pub mechanistic_assumptions_preserved: bool,
    pub canonical_machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub proof_transport_boundary_id: String,
    pub bound_claim_ids: Vec<String>,
    pub rebound_receipt_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalMachineProofRebindingValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalMachineProofRebindingReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub historical_proof_report_ref: String,
    pub historical_proof_report_id: String,
    pub historical_proof_report_digest: String,
    pub substrate_model_ref: String,
    pub substrate_model_id: String,
    pub substrate_model_digest: String,
    pub runtime_contract_ref: String,
    pub runtime_contract_id: String,
    pub runtime_contract_digest: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub supporting_material_rows:
        Vec<TassadarPostArticleUniversalMachineProofSupportingMaterialRow>,
    pub proof_transport_boundary: TassadarPostArticleProofTransportBoundary,
    pub proof_transport_receipt_rows:
        Vec<TassadarPostArticleUniversalMachineProofTransportReceiptRow>,
    pub rebound_encoding_ids: Vec<String>,
    pub proof_transport_audit_complete: bool,
    pub proof_rebinding_complete: bool,
    pub proof_rebinding_status: TassadarPostArticleUniversalMachineProofRebindingStatus,
    pub carrier_split_publication_complete: bool,
    pub universality_witness_suite_reissued: bool,
    pub universal_substrate_gate_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub validation_rows: Vec<TassadarPostArticleUniversalMachineProofRebindingValidationRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleUniversalMachineProofRebindingReportError {
    #[error(transparent)]
    UniversalMachineProof(#[from] TassadarUniversalMachineProofReportError),
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
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

pub fn build_tassadar_post_article_universal_machine_proof_rebinding_report() -> Result<
    TassadarPostArticleUniversalMachineProofRebindingReport,
    TassadarPostArticleUniversalMachineProofRebindingReportError,
> {
    let historical_proof = build_tassadar_universal_machine_proof_report()?;
    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let semantic_preservation =
        build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()?;
    let control_plane =
        build_tassadar_post_article_control_plane_decision_provenance_proof_report()?;
    let carrier_split = build_tassadar_post_article_carrier_split_contract_report()?;

    let supporting_material_rows = build_supporting_material_rows(
        &historical_proof,
        &runtime_contract,
        &acceptance_gate,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
    );
    let proof_transport_boundary = build_proof_transport_boundary(
        &historical_proof,
        &runtime_contract,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
    );
    let proof_transport_receipt_rows = build_proof_transport_receipt_rows(
        &historical_proof,
        &runtime_contract,
        &bridge,
        &proof_transport_boundary,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
    );
    let rebound_encoding_ids = proof_transport_receipt_rows
        .iter()
        .filter(|row| row.rebound_receipt_green)
        .map(|row| row.encoding_id.clone())
        .collect::<Vec<_>>();

    let carrier_split_publication_complete = carrier_split.carrier_split_publication_complete;
    let universality_witness_suite_reissued = false;
    let universal_substrate_gate_allowed = false;
    let deferred_issue_ids = vec![String::from("TAS-191")];
    let rebase_claim_allowed = false;
    let plugin_capability_claim_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let validation_rows = build_validation_rows(
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
        &proof_transport_boundary,
        &proof_transport_receipt_rows,
        &supporting_material_rows,
        universality_witness_suite_reissued,
        universal_substrate_gate_allowed,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );

    let proof_transport_audit_complete = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying
        })
        .all(|row| row.satisfied)
        && proof_transport_boundary.boundary_green
        && proof_transport_receipt_rows
            .iter()
            .all(|row| row.rebound_receipt_green)
        && validation_rows.iter().all(|row| row.green);
    let proof_rebinding_complete = proof_transport_audit_complete
        && rebound_encoding_ids.len() == historical_proof.proof_rows.len();
    let proof_rebinding_status = if proof_rebinding_complete {
        TassadarPostArticleUniversalMachineProofRebindingStatus::Green
    } else {
        TassadarPostArticleUniversalMachineProofRebindingStatus::Blocked
    };

    let mut report = TassadarPostArticleUniversalMachineProofRebindingReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_universal_machine_proof_rebinding.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_CHECKER_REF,
        ),
        historical_proof_report_ref: String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF),
        historical_proof_report_id: historical_proof.report_id.clone(),
        historical_proof_report_digest: historical_proof.report_digest.clone(),
        substrate_model_ref: runtime_contract.substrate_model_ref.clone(),
        substrate_model_id: runtime_contract.substrate_model.model_id.clone(),
        substrate_model_digest: runtime_contract.substrate_model.model_digest.clone(),
        runtime_contract_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
        runtime_contract_id: runtime_contract.report_id.clone(),
        runtime_contract_digest: runtime_contract.report_digest.clone(),
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
        canonical_weight_artifact_id: bridge
            .bridge_machine_identity
            .canonical_weight_artifact_id
            .clone(),
        canonical_weight_bundle_digest: bridge
            .bridge_machine_identity
            .canonical_weight_bundle_digest
            .clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        canonical_route_descriptor_digest: bridge
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .clone(),
        continuation_contract_id: bridge.bridge_machine_identity.continuation_contract_id.clone(),
        supporting_material_rows,
        proof_transport_boundary,
        proof_transport_receipt_rows,
        rebound_encoding_ids,
        proof_transport_audit_complete,
        proof_rebinding_complete,
        proof_rebinding_status,
        carrier_split_publication_complete,
        universality_witness_suite_reissued,
        universal_substrate_gate_allowed,
        deferred_issue_ids,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        validation_rows,
        claim_boundary: String::from(
            "this report rebounds the historical universal-machine proof onto the post-`TAS-186` canonical machine, model, weight, and route identities through an explicit proof-transport boundary and rebinding receipts. It does not by itself reissue the broader universality witness suite, enable the canonical-route universal-substrate gate, publish the rebased theory/operator/served verdict split, admit served/public universality, admit weighted plugin control, or admit arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article universal-machine proof rebinding keeps supporting_materials={}/9, rebound_receipts={}/{}, validation_rows={}/{}, proof_transport_audit_complete={}, and proof_rebinding_status={:?}.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.rebound_encoding_ids.len(),
        report.proof_transport_receipt_rows.len(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.proof_transport_audit_complete,
        report.proof_rebinding_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_universal_machine_proof_rebinding_report|",
        &report,
    );
    Ok(report)
}

fn build_supporting_material_rows(
    historical_proof: &TassadarUniversalMachineProofReport,
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
) -> Vec<TassadarPostArticleUniversalMachineProofSupportingMaterialRow> {
    vec![
        supporting_material_row(
            "historical_universal_machine_proof",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying,
            historical_proof.overall_green,
            TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
            Some(historical_proof.report_id.clone()),
            Some(historical_proof.report_digest.clone()),
            "the historical universal-machine proof must stay green so the rebinding report transports a proof-carrying artifact rather than sampled observation.",
        ),
        supporting_material_row(
            "tcm_v1_substrate_model",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying,
            !runtime_contract.substrate_model.model_id.is_empty()
                && !runtime_contract.substrate_model.model_digest.is_empty(),
            &runtime_contract.substrate_model_ref,
            Some(runtime_contract.substrate_model.model_id.clone()),
            Some(runtime_contract.substrate_model.model_digest.clone()),
            "the declared `TCM.v1` substrate rows remain the computational substrate below the rebinding boundary and must stay explicit by id and digest.",
        ),
        supporting_material_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying,
            runtime_contract.overall_green,
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
            Some(runtime_contract.report_id.clone()),
            Some(runtime_contract.report_digest.clone()),
            "the declared `TCM.v1` runtime contract must stay green so resumed-execution equivalence remains explicit where the universality claim still depends on continuation.",
        ),
        supporting_material_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying,
            acceptance_gate.public_claim_allowed,
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            Some(acceptance_gate.report_id.clone()),
            Some(acceptance_gate.report_digest.clone()),
            "the article acceptance gate must stay green so proof rebinding attaches to the declared canonical owned route rather than to a weaker non-canonical lane.",
        ),
        supporting_material_row(
            "post_article_bridge_contract",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the post-article bridge contract must stay green so the canonical machine, model, weight, route, and continuation identities remain frozen while the historical proof is rebound.",
        ),
        supporting_material_row(
            "post_article_semantic_preservation_audit",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying,
            semantic_preservation.semantic_preservation_audit_green,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            Some(semantic_preservation.report_id.clone()),
            Some(semantic_preservation.report_digest.clone()),
            "the semantic-preservation audit must stay green so proof transport preserves execution semantics instead of merely preserving outputs.",
        ),
        supporting_material_row(
            "post_article_control_plane_proof",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying,
            control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete,
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            Some(control_plane.report_id.clone()),
            Some(control_plane.report_digest.clone()),
            "the control-plane proof must stay green so the rebound proof receipts remain bound to model-owned workflow decisions instead of host-owned route collapse.",
        ),
        supporting_material_row(
            "post_article_carrier_split_contract",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying,
            carrier_split.carrier_split_publication_complete,
            TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
            Some(carrier_split.report_id.clone()),
            Some(carrier_split.report_digest.clone()),
            "the carrier-split contract must stay green so the rebound universal-machine proof remains bound only to the resumable universality carrier rather than collapsing into the direct article-equivalent carrier.",
        ),
        supporting_material_row(
            "turing_closeout_audit_context",
            TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ObservationalContext,
            true,
            TASSADAR_TURING_CLOSEOUT_AUDIT_NOTE_REF,
            None,
            None,
            "the March 19 closeout audit remains observational context only here and motivates proof rebinding without substituting for the proof-carrying receipts and audits.",
        ),
    ]
}

fn build_proof_transport_boundary(
    historical_proof: &TassadarUniversalMachineProofReport,
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
) -> TassadarPostArticleProofTransportBoundary {
    let preserved_transition_class_ids = vec![
        String::from("bounded_small_step_control_updates"),
        String::from("declared_memory_state_updates"),
        String::from("declared_continuation_resume_equivalence"),
        String::from("declared_effect_boundary_only"),
    ];
    let admitted_variance_ids = vec![
        String::from("canonical_machine_identity_binding"),
        String::from("canonical_model_and_weight_identity_binding"),
        String::from("canonical_route_identity_binding"),
        String::from("carrier_split_publication_without_claim_collapse"),
    ];
    let blocked_drift_ids = vec![
        String::from("helper_substitution"),
        String::from("route_family_drift"),
        String::from("undeclared_cache_owned_control"),
        String::from("undeclared_batching_semantics"),
        String::from("semantic_drift_outside_declared_proof_boundary"),
    ];
    let boundary_green = historical_proof.overall_green
        && runtime_contract.overall_green
        && bridge.bridge_contract_green
        && semantic_preservation.semantic_preservation_audit_green
        && control_plane.decision_provenance_proof_complete
        && carrier_split.carrier_split_publication_complete;
    TassadarPostArticleProofTransportBoundary {
        boundary_id: String::from(
            "tassadar.post_article_universal_machine_proof.proof_transport_boundary.v1",
        ),
        preserved_transition_class_ids,
        admitted_variance_ids,
        blocked_drift_ids,
        boundary_green,
        detail: String::from(
            "proof transport is explicit instead of implicit: the historical proof may be rebound only across preserved small-step control, memory, continuation, and effect-boundary semantics while machine/model/weight/route identity binding is admitted and helper, route-family, cache-owned, batching-owned, or semantic drift remains blocked.",
        ),
    }
}

fn build_proof_transport_receipt_rows(
    historical_proof: &TassadarUniversalMachineProofReport,
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    proof_transport_boundary: &TassadarPostArticleProofTransportBoundary,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
) -> Vec<TassadarPostArticleUniversalMachineProofTransportReceiptRow> {
    historical_proof
        .proof_rows
        .iter()
        .map(|row| {
            let resumed_execution_equivalence_explicit = row.checkpoint_resume_equivalent
                && runtime_contract.overall_green
                && semantic_preservation.semantic_preservation_audit_green
                && carrier_split.carrier_split_publication_complete;
            let mechanistic_assumptions_preserved = proof_transport_boundary.boundary_green
                && control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete
                && bridge.bridge_contract_green;
            let rebound_receipt_green = row.satisfied
                && resumed_execution_equivalence_explicit
                && mechanistic_assumptions_preserved;
            TassadarPostArticleUniversalMachineProofTransportReceiptRow {
                receipt_id: format!(
                    "tassadar.post_article_universal_machine_proof_rebinding.{}.receipt.v1",
                    row.encoding_id
                ),
                encoding_id: row.encoding_id.clone(),
                historical_proof_satisfied: row.satisfied,
                resumed_execution_equivalence_explicit,
                mechanistic_assumptions_preserved,
                canonical_machine_identity_id: bridge
                    .bridge_machine_identity
                    .machine_identity_id
                    .clone(),
                canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
                canonical_weight_artifact_id: bridge
                    .bridge_machine_identity
                    .canonical_weight_artifact_id
                    .clone(),
                canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
                continuation_contract_id: bridge
                    .bridge_machine_identity
                    .continuation_contract_id
                    .clone(),
                proof_transport_boundary_id: proof_transport_boundary.boundary_id.clone(),
                bound_claim_ids: vec![
                    String::from("construction_backed_universal_machine_witness"),
                    String::from("checkpoint_resume_equivalent_under_declared_continuation_contract"),
                ],
                rebound_receipt_green,
                detail: format!(
                    "historical proof row `{}` is rebound onto machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` through proof_transport_boundary_id=`{}` while keeping continuation semantics explicit under `{}`.",
                    row.encoding_id,
                    bridge.bridge_machine_identity.machine_identity_id,
                    bridge.bridge_machine_identity.canonical_model_id,
                    bridge.bridge_machine_identity.canonical_route_id,
                    proof_transport_boundary.boundary_id,
                    bridge.bridge_machine_identity.continuation_contract_id,
                ),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
    proof_transport_boundary: &TassadarPostArticleProofTransportBoundary,
    proof_transport_receipt_rows: &[TassadarPostArticleUniversalMachineProofTransportReceiptRow],
    supporting_material_rows: &[TassadarPostArticleUniversalMachineProofSupportingMaterialRow],
    universality_witness_suite_reissued: bool,
    universal_substrate_gate_allowed: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticleUniversalMachineProofRebindingValidationRow> {
    let proof_carrying_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ProofCarrying
        })
        .count();
    let observational_context_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleUniversalMachineProofSupportingMaterialClass::ObservationalContext
        })
        .count();

    vec![
        validation_row(
            "helper_substitution_quarantined",
            bridge_validation_green(bridge, "helper_substitution_quarantined"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "helper substitution remains quarantined, so proof rebinding cannot be satisfied by hidden host helpers or synthetic metadata swaps.",
        ),
        validation_row(
            "route_drift_rejected",
            bridge_validation_green(bridge, "route_drift_rejected"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "route drift remains rejected, so the rebound proof stays attached to one declared canonical route id and one declared route descriptor digest.",
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
            "continuation abuse remains quarantined, so resumed execution stays a declared continuation carrier instead of becoming a second hidden machine that manufactures the rebound proof.",
        ),
        validation_row(
            "semantic_drift_blocked",
            bridge_validation_green(bridge, "semantic_drift_blocked")
                && semantic_preservation.semantic_preservation_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "semantic drift remains blocked, so proof transport preserves execution semantics rather than merely preserving selected outputs.",
        ),
        validation_row(
            "cache_and_batching_drift_blocked",
            proof_transport_boundary
                .blocked_drift_ids
                .iter()
                .any(|id| id == "undeclared_cache_owned_control")
                && proof_transport_boundary
                    .blocked_drift_ids
                    .iter()
                    .any(|id| id == "undeclared_batching_semantics")
                && control_plane.decision_provenance_proof_complete,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            )],
            "cache-owned and batching-owned control drift stay explicit and blocked, so proof rebinding cannot silently hide a new route family inside cache state or scheduler behavior.",
        ),
        validation_row(
            "proof_transport_boundary_explicit",
            proof_transport_boundary.boundary_green
                && proof_transport_boundary.preserved_transition_class_ids.len() == 4
                && proof_transport_boundary.admitted_variance_ids.len() == 4
                && proof_transport_boundary.blocked_drift_ids.len() == 5,
            vec![
                String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF),
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            "one explicit proof-transport boundary now names the preserved transition classes, admitted identity rebinding, and blocked drift classes instead of pretending the rebind is a metadata-only relabel.",
        ),
        validation_row(
            "proof_receipts_reissued_on_canonical_identity",
            proof_transport_receipt_rows
                .iter()
                .all(|row| row.rebound_receipt_green
                    && row.canonical_machine_identity_id
                        == bridge.bridge_machine_identity.machine_identity_id
                    && row.canonical_model_id == bridge.bridge_machine_identity.canonical_model_id
                    && row.canonical_route_id == bridge.bridge_machine_identity.canonical_route_id),
            vec![
                String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
            ],
            "proof transport receipts are reissued against the canonical machine, model, weight, and route identities instead of relying on implied inheritance from the historical operator lane.",
        ),
        validation_row(
            "proof_carrying_distinction_preserved",
            proof_carrying_count == 8 && observational_context_count == 1,
            vec![String::from(TASSADAR_TURING_CLOSEOUT_AUDIT_NOTE_REF)],
            "proof-carrying artifacts remain distinct from observational audit context while the rebind closes as its own machine-readable proof surface.",
        ),
        validation_row(
            "overclaim_posture_explicit",
            carrier_split.carrier_split_publication_complete
                && !universality_witness_suite_reissued
                && !universal_substrate_gate_allowed
                && !rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            "the proof rebind remains bounded: the witness suite, universal-substrate gate, rebased verdict split, served/public universality, weighted plugin control, and arbitrary software capability all remain blocked here.",
        ),
    ]
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleUniversalMachineProofSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleUniversalMachineProofSupportingMaterialRow {
    TassadarPostArticleUniversalMachineProofSupportingMaterialRow {
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
) -> TassadarPostArticleUniversalMachineProofRebindingValidationRow {
    TassadarPostArticleUniversalMachineProofRebindingValidationRow {
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

#[must_use]
pub fn tassadar_post_article_universal_machine_proof_rebinding_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF)
}

pub fn write_tassadar_post_article_universal_machine_proof_rebinding_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleUniversalMachineProofRebindingReport,
    TassadarPostArticleUniversalMachineProofRebindingReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleUniversalMachineProofRebindingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_universal_machine_proof_rebinding_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleUniversalMachineProofRebindingReportError::Write {
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
) -> Result<T, TassadarPostArticleUniversalMachineProofRebindingReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleUniversalMachineProofRebindingReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalMachineProofRebindingReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_universal_machine_proof_rebinding_report, read_repo_json,
        tassadar_post_article_universal_machine_proof_rebinding_report_path,
        write_tassadar_post_article_universal_machine_proof_rebinding_report,
        TassadarPostArticleUniversalMachineProofRebindingReport,
        TassadarPostArticleUniversalMachineProofRebindingStatus,
        TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn proof_rebinding_report_keeps_historical_witnesses_green() {
        let report =
            build_tassadar_post_article_universal_machine_proof_rebinding_report().expect("report");

        assert_eq!(
            report.proof_rebinding_status,
            TassadarPostArticleUniversalMachineProofRebindingStatus::Green
        );
        assert!(report.proof_transport_audit_complete);
        assert!(report.proof_rebinding_complete);
        assert!(report.carrier_split_publication_complete);
        assert_eq!(
            report.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            report.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(report.rebound_encoding_ids.len(), 2);
        assert_eq!(report.deferred_issue_ids, vec![String::from("TAS-191")]);
        assert!(!report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn proof_rebinding_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_universal_machine_proof_rebinding_report().expect("report");
        let committed: TassadarPostArticleUniversalMachineProofRebindingReport =
            read_repo_json(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_universal_machine_proof_rebinding_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_universal_machine_proof_rebinding_report.json")
        );
    }

    #[test]
    fn write_proof_rebinding_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_universal_machine_proof_rebinding_report.json");
        let written =
            write_tassadar_post_article_universal_machine_proof_rebinding_report(&output_path)
                .expect("write report");
        let persisted: TassadarPostArticleUniversalMachineProofRebindingReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
