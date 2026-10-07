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
    build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_universality_witness_suite_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    TassadarPostArticleCanonicalRouteSemanticPreservationStatus,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarUniversalityWitnessSuiteReport, TassadarUniversalityWitnessSuiteReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_control_plane_decision_provenance_proof_report.json";
pub const TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-control-plane-decision-provenance-proof.sh";

const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_tcm_v1_runtime_contract_report.json";
const TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_universality_witness_suite_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleControlPlaneOwnershipStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleDecisionProvenanceSupportingMaterialClass {
    ProofCarrying,
    ObservationalContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleDecisionKind {
    Branch,
    Retry,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleDeterminismClass {
    StrictDeterministic,
    SeededStochastic,
    BoundedNondeterministicWithEquivalenceClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleFailureSemanticClass {
    Refusal,
    PolicyFailure,
    SemanticFailure,
    CapabilityFailure,
    ResourceExhaustion,
    DeterminismViolation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleHiddenControlChannelKind {
    LatencySteering,
    CostSteering,
    SchedulingSteering,
    CacheHitSteering,
    HelperSubstitution,
    FastRouteFallback,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDecisionProvenanceSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleDecisionProvenanceSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDecisionBindingRow {
    pub decision_id: String,
    pub decision_kind: TassadarPostArticleDecisionKind,
    pub bound_to_model_outputs: bool,
    pub bound_to_canonical_route_identity: bool,
    pub bound_to_machine_identity_tuple: bool,
    pub replay_stable: bool,
    pub current_posture: String,
    pub source_refs: Vec<String>,
    pub decision_binding_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDeterminismClassContract {
    pub supported_classes: Vec<TassadarPostArticleDeterminismClass>,
    pub selected_class: TassadarPostArticleDeterminismClass,
    pub equivalent_choice_relation_id: String,
    pub replay_posture: String,
    pub determinism_contract_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceRelation {
    pub relation_id: String,
    pub equivalent_choice_rule: String,
    pub admissible_divergence_bound: String,
    pub choice_completeness_green: bool,
    pub choice_neutrality_green: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFailureSemanticsLattice {
    pub failure_classes: Vec<TassadarPostArticleFailureSemanticClass>,
    pub replay_stable_propagation_rule: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTimeSemanticsContract {
    pub logical_time_source: String,
    pub wall_time_model_observable: bool,
    pub time_driven_branching_allowed: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleInformationBoundary {
    pub model_visible_signal_ids: Vec<String>,
    pub model_hidden_signal_ids: Vec<String>,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTrainingInferenceBoundary {
    pub runtime_adaptation_allowed: bool,
    pub telemetry_decision_authority: bool,
    pub logging_decision_authority: bool,
    pub cache_behavior_decision_authority: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleHiddenStateChannelClosure {
    pub allowed_state_class_ids: Vec<String>,
    pub hidden_state_channel_closed: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleObserverModel {
    pub verifier_roles: Vec<String>,
    pub acceptance_requirements: Vec<String>,
    pub replay_receipt_required: bool,
    pub gate_verdict_bound_to_machine_identity: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleHiddenControlChannelValidationRow {
    pub validation_id: String,
    pub channel_kind: TassadarPostArticleHiddenControlChannelKind,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDecisionProvenanceValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleControlPlaneDecisionProvenanceProofReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub bridge_contract_report_ref: String,
    pub semantic_preservation_audit_report_ref: String,
    pub acceptance_gate_report_ref: String,
    pub witness_suite_report_ref: String,
    pub supporting_material_rows: Vec<TassadarPostArticleDecisionProvenanceSupportingMaterialRow>,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub decision_binding_rows: Vec<TassadarPostArticleDecisionBindingRow>,
    pub determinism_class_contract: TassadarPostArticleDeterminismClassContract,
    pub equivalent_choice_relation: TassadarPostArticleEquivalentChoiceRelation,
    pub failure_semantics_lattice: TassadarPostArticleFailureSemanticsLattice,
    pub time_semantics_contract: TassadarPostArticleTimeSemanticsContract,
    pub information_boundary: TassadarPostArticleInformationBoundary,
    pub training_inference_boundary: TassadarPostArticleTrainingInferenceBoundary,
    pub hidden_state_channel_closure: TassadarPostArticleHiddenStateChannelClosure,
    pub observer_model: TassadarPostArticleObserverModel,
    pub hidden_control_channel_rows: Vec<TassadarPostArticleHiddenControlChannelValidationRow>,
    pub validation_rows: Vec<TassadarPostArticleDecisionProvenanceValidationRow>,
    pub replay_posture_green: bool,
    pub control_plane_ownership_green: bool,
    pub control_plane_ownership_status: TassadarPostArticleControlPlaneOwnershipStatus,
    pub decision_provenance_proof_complete: bool,
    pub carrier_split_publication_complete: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleControlPlaneDecisionProvenanceProofReportError {
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
    #[error(transparent)]
    SemanticPreservation(
        #[from] TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    ),
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    WitnessSuite(#[from] TassadarUniversalityWitnessSuiteReportError),
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

pub fn build_tassadar_post_article_control_plane_decision_provenance_proof_report() -> Result<
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
> {
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let semantic_preservation =
        build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()?;
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let witness_suite = build_tassadar_universality_witness_suite_report()?;

    let supporting_material_rows = build_supporting_material_rows(
        &bridge,
        &semantic_preservation,
        &acceptance_gate,
        &witness_suite,
    );
    let decision_binding_rows =
        build_decision_binding_rows(&bridge, &semantic_preservation, &witness_suite);
    let determinism_class_contract = build_determinism_class_contract(&decision_binding_rows);
    let equivalent_choice_relation = build_equivalent_choice_relation();
    let failure_semantics_lattice = build_failure_semantics_lattice();
    let time_semantics_contract = build_time_semantics_contract();
    let information_boundary = build_information_boundary();
    let training_inference_boundary = build_training_inference_boundary();
    let hidden_state_channel_closure = build_hidden_state_channel_closure(&semantic_preservation);
    let observer_model = build_observer_model();
    let hidden_control_channel_rows =
        build_hidden_control_channel_rows(&bridge, &semantic_preservation);

    let replay_posture_green = decision_binding_rows
        .iter()
        .all(|row| row.replay_stable && row.decision_binding_green)
        && observer_model.green
        && determinism_class_contract.determinism_contract_green
        && equivalent_choice_relation.green
        && failure_semantics_lattice.green;

    let decision_provenance_proof_complete =
        supporting_material_rows.iter().all(|row| row.satisfied)
            && semantic_preservation.semantic_preservation_status
                == TassadarPostArticleCanonicalRouteSemanticPreservationStatus::Green
            && decision_binding_rows
                .iter()
                .all(|row| row.decision_binding_green)
            && determinism_class_contract.determinism_contract_green
            && equivalent_choice_relation.green
            && failure_semantics_lattice.green
            && time_semantics_contract.green
            && information_boundary.green
            && training_inference_boundary.green
            && hidden_state_channel_closure.green
            && observer_model.green
            && hidden_control_channel_rows.iter().all(|row| row.green);

    let carrier_split_publication_complete = false;
    let deferred_issue_ids = vec![String::from("TAS-189")];
    let rebase_claim_allowed = false;
    let plugin_capability_claim_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let validation_rows = build_validation_rows(
        &bridge,
        &semantic_preservation,
        &supporting_material_rows,
        &decision_binding_rows,
        &determinism_class_contract,
        &equivalent_choice_relation,
        &failure_semantics_lattice,
        &time_semantics_contract,
        &information_boundary,
        &training_inference_boundary,
        &hidden_state_channel_closure,
        &observer_model,
        &hidden_control_channel_rows,
        carrier_split_publication_complete,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );

    let control_plane_ownership_green = decision_provenance_proof_complete
        && replay_posture_green
        && validation_rows.iter().all(|row| row.green);
    let control_plane_ownership_status = if control_plane_ownership_green {
        TassadarPostArticleControlPlaneOwnershipStatus::Green
    } else {
        TassadarPostArticleControlPlaneOwnershipStatus::Blocked
    };

    let mut report = TassadarPostArticleControlPlaneDecisionProvenanceProofReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_control_plane_decision_provenance_proof.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_CHECKER_REF,
        ),
        bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
        ),
        semantic_preservation_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
        ),
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        witness_suite_report_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
        supporting_material_rows,
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        decision_binding_rows,
        determinism_class_contract,
        equivalent_choice_relation,
        failure_semantics_lattice,
        time_semantics_contract,
        information_boundary,
        training_inference_boundary,
        hidden_state_channel_closure,
        observer_model,
        hidden_control_channel_rows,
        validation_rows,
        replay_posture_green,
        control_plane_ownership_green,
        control_plane_ownership_status,
        decision_provenance_proof_complete,
        carrier_split_publication_complete,
        deferred_issue_ids,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        claim_boundary: String::from(
            "this proof artifact closes only the post-`TAS-186` control-plane ownership and decision-provenance tranche on the bridge machine identity. It binds branch, retry, and stop decisions to model outputs, canonical route identity, and one replay-stable control contract while freezing determinism, equivalent-choice, failure semantics, time semantics, information boundaries, training-versus-inference boundaries, hidden-state closure, and observer acceptance requirements. It does not by itself publish the final direct-versus-resumable carrier split, admit the rebased Turing-completeness claim, admit weighted plugin control, admit served/public universality, or admit arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article control-plane decision-provenance proof keeps supporting_materials={}/7, decision_bindings={}/3, hidden_control_channels={}/6, validation_rows={}/7, control_plane_ownership_status={:?}, decision_provenance_proof_complete={}, and carrier_split_publication_complete={}.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report
            .decision_binding_rows
            .iter()
            .filter(|row| row.decision_binding_green)
            .count(),
        report
            .hidden_control_channel_rows
            .iter()
            .filter(|row| row.green)
            .count(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.control_plane_ownership_status,
        report.decision_provenance_proof_complete,
        report.carrier_split_publication_complete,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_control_plane_decision_provenance_proof_report|",
        &report,
    );
    Ok(report)
}

fn build_supporting_material_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
    witness_suite: &TassadarUniversalityWitnessSuiteReport,
) -> Vec<TassadarPostArticleDecisionProvenanceSupportingMaterialRow> {
    vec![
        supporting_material_row(
            "bridge_contract",
            TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge contract must stay green before branch, retry, and stop decisions can be bound to one canonical machine identity tuple",
        ),
        supporting_material_row(
            "semantic_preservation_audit",
            TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ProofCarrying,
            semantic_preservation.semantic_preservation_audit_green,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            Some(semantic_preservation.report_id.clone()),
            Some(semantic_preservation.report_digest.clone()),
            "the semantic-preservation audit must stay green before control-plane proof can rely on the declared state taxonomy and continuation semantics",
        ),
        supporting_material_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ProofCarrying,
            bridge.tcm_v1_runtime_contract_report.overall_green,
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
            Some(bridge.tcm_v1_runtime_contract_report.report_id.clone()),
            Some(bridge.tcm_v1_runtime_contract_report.report_digest.clone()),
            "the declared `TCM.v1` runtime contract must stay green before replay-stable decision semantics can be carried on the bridge machine identity",
        ),
        supporting_material_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ProofCarrying,
            acceptance_gate.acceptance_status == TassadarArticleEquivalenceAcceptanceStatus::Green
                && acceptance_gate.public_claim_allowed,
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            Some(acceptance_gate.report_id.clone()),
            Some(acceptance_gate.report_digest.clone()),
            "the article acceptance gate must stay green so control-plane proof remains tied to the canonical owned route rather than an alternate route family",
        ),
        supporting_material_row(
            "universality_witness_suite",
            TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ProofCarrying,
            witness_suite.overall_green,
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
            Some(witness_suite.report_id.clone()),
            Some(witness_suite.report_digest.clone()),
            "the witness suite must stay green so the branch-and-stop bearing session and spill witnesses remain explicit inside the declared control proof surface",
        ),
        supporting_material_row(
            "post_article_turing_completeness_audit_context",
            TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ObservationalContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 rebase audit remains observational context only here and does not substitute for proof-carrying control-plane evidence",
        ),
        supporting_material_row(
            "plugin_system_turing_completeness_audit_context",
            TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ObservationalContext,
            true,
            PLUGIN_SYSTEM_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 plugin audit remains observational context only here and keeps the later capability boundary visible without substituting for proof-carrying control-plane evidence",
        ),
    ]
}

fn build_decision_binding_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    witness_suite: &TassadarUniversalityWitnessSuiteReport,
) -> Vec<TassadarPostArticleDecisionBindingRow> {
    let machine_identity_green = !bridge
        .bridge_machine_identity
        .machine_identity_id
        .is_empty()
        && !bridge.bridge_machine_identity.canonical_route_id.is_empty();
    let session_process_witness_green = witness_suite
        .family_rows
        .iter()
        .any(|row| format!("{:?}", row.witness_family) == "SessionProcessKernel" && row.satisfied);
    let spill_tape_witness_green = witness_suite
        .family_rows
        .iter()
        .any(|row| format!("{:?}", row.witness_family) == "SpillTapeContinuation" && row.satisfied);

    vec![
        TassadarPostArticleDecisionBindingRow {
            decision_id: String::from("branch_decision_provenance"),
            decision_kind: TassadarPostArticleDecisionKind::Branch,
            bound_to_model_outputs: true,
            bound_to_canonical_route_identity: machine_identity_green,
            bound_to_machine_identity_tuple: machine_identity_green,
            replay_stable: semantic_preservation.semantic_preservation_green
                && session_process_witness_green,
            current_posture: String::from("implemented"),
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
                String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
            ],
            decision_binding_green: semantic_preservation.semantic_preservation_green
                && session_process_witness_green
                && machine_identity_green,
            detail: String::from(
                "branch decisions stay weight-owned and replay-stable on the current rebased route because deterministic session-process continuation remains exact and bound to one fixed machine identity tuple.",
            ),
        },
        TassadarPostArticleDecisionBindingRow {
            decision_id: String::from("retry_decision_provenance"),
            decision_kind: TassadarPostArticleDecisionKind::Retry,
            bound_to_model_outputs: true,
            bound_to_canonical_route_identity: machine_identity_green,
            bound_to_machine_identity_tuple: machine_identity_green,
            replay_stable: true,
            current_posture: String::from("ambient_retry_disallowed"),
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            decision_binding_green: semantic_preservation
                .control_ownership_boundary_review
                .control_ownership_rule_green
                && machine_identity_green,
            detail: String::from(
                "ambient host retry is disallowed. Any retry-bearing control transition must therefore remain explicit and replay-stable under the same machine identity instead of being smuggled in as hidden host policy.",
            ),
        },
        TassadarPostArticleDecisionBindingRow {
            decision_id: String::from("stop_decision_provenance"),
            decision_kind: TassadarPostArticleDecisionKind::Stop,
            bound_to_model_outputs: true,
            bound_to_canonical_route_identity: machine_identity_green,
            bound_to_machine_identity_tuple: machine_identity_green,
            replay_stable: semantic_preservation.semantic_preservation_green
                && spill_tape_witness_green,
            current_posture: String::from("implemented"),
            source_refs: vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
                String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
            ],
            decision_binding_green: semantic_preservation.semantic_preservation_green
                && spill_tape_witness_green
                && machine_identity_green,
            detail: String::from(
                "stop decisions remain replay-stable because the current spill/tape and lifecycle continuation witnesses preserve terminal semantics under typed refusal boundaries instead of deferring stop authority to host scheduling.",
            ),
        },
    ]
}

fn build_determinism_class_contract(
    decision_binding_rows: &[TassadarPostArticleDecisionBindingRow],
) -> TassadarPostArticleDeterminismClassContract {
    let determinism_contract_green = decision_binding_rows
        .iter()
        .all(|row| row.replay_stable && row.decision_binding_green);
    TassadarPostArticleDeterminismClassContract {
        supported_classes: vec![
            TassadarPostArticleDeterminismClass::StrictDeterministic,
            TassadarPostArticleDeterminismClass::SeededStochastic,
            TassadarPostArticleDeterminismClass::BoundedNondeterministicWithEquivalenceClass,
        ],
        selected_class: TassadarPostArticleDeterminismClass::StrictDeterministic,
        equivalent_choice_relation_id: String::from("singleton_exact_control_trace.v1"),
        replay_posture: String::from("exact_replay_required"),
        determinism_contract_green,
        detail: String::from(
            "the current rebased route selects `strict_deterministic` control tracing. `seeded_stochastic` and `bounded_nondeterministic_with_equivalence_class` remain declared classes, but they are not the selected class for this proof artifact.",
        ),
    }
}

fn build_equivalent_choice_relation() -> TassadarPostArticleEquivalentChoiceRelation {
    TassadarPostArticleEquivalentChoiceRelation {
        relation_id: String::from("singleton_exact_control_trace.v1"),
        equivalent_choice_rule: String::from(
            "exact same branch/retry/stop trace under the same machine identity tuple",
        ),
        admissible_divergence_bound: String::from("zero_control_trace_divergence"),
        choice_completeness_green: true,
        choice_neutrality_green: true,
        green: true,
        detail: String::from(
            "the current proof uses the singleton exact-trace relation: every admissible choice must be explicit on-trace, and no hidden host ranking or filtering may create a second equivalent path.",
        ),
    }
}

fn build_failure_semantics_lattice() -> TassadarPostArticleFailureSemanticsLattice {
    TassadarPostArticleFailureSemanticsLattice {
        failure_classes: vec![
            TassadarPostArticleFailureSemanticClass::Refusal,
            TassadarPostArticleFailureSemanticClass::PolicyFailure,
            TassadarPostArticleFailureSemanticClass::SemanticFailure,
            TassadarPostArticleFailureSemanticClass::CapabilityFailure,
            TassadarPostArticleFailureSemanticClass::ResourceExhaustion,
            TassadarPostArticleFailureSemanticClass::DeterminismViolation,
        ],
        replay_stable_propagation_rule: String::from(
            "failure classes must propagate as typed receipts and may not be rewritten into hidden retry or hidden branch transitions",
        ),
        green: true,
        detail: String::from(
            "the failure lattice keeps typed refusal, policy, semantic, capability, resource, and determinism classes distinct so hidden retry or hidden stop coercion cannot be disguised as generic failure handling.",
        ),
    }
}

fn build_time_semantics_contract() -> TassadarPostArticleTimeSemanticsContract {
    TassadarPostArticleTimeSemanticsContract {
        logical_time_source: String::from("declared_step_index_and_receipt_order"),
        wall_time_model_observable: false,
        time_driven_branching_allowed: false,
        green: true,
        detail: String::from(
            "logical time is the declared step index plus receipt order. Wall time is not model-observable here, and time-driven branching is disallowed so host timing cannot become a hidden decision channel.",
        ),
    }
}

fn build_information_boundary() -> TassadarPostArticleInformationBoundary {
    TassadarPostArticleInformationBoundary {
        model_visible_signal_ids: vec![
            String::from("token_outputs"),
            String::from("typed_refusal_ids"),
            String::from("declared_continuation_receipt_ids"),
            String::from("declared_step_index"),
        ],
        model_hidden_signal_ids: vec![
            String::from("latency"),
            String::from("cost"),
            String::from("queue_pressure"),
            String::from("scheduler_order"),
            String::from("cache_hit_rate"),
            String::from("helper_selection"),
        ],
        green: true,
        detail: String::from(
            "cost, queue, scheduling, cache-hit, and helper-selection signals are not model-visible in this proof contract and therefore may not shape branch, retry, or stop behavior.",
        ),
    }
}

fn build_training_inference_boundary() -> TassadarPostArticleTrainingInferenceBoundary {
    TassadarPostArticleTrainingInferenceBoundary {
        runtime_adaptation_allowed: false,
        telemetry_decision_authority: false,
        logging_decision_authority: false,
        cache_behavior_decision_authority: false,
        green: true,
        detail: String::from(
            "runtime adaptation, telemetry, logging, and cache behavior stay outside decision authority for this proof surface; they may observe or accelerate execution but may not rewrite branch, retry, or stop outcomes.",
        ),
    }
}

fn build_hidden_state_channel_closure(
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
) -> TassadarPostArticleHiddenStateChannelClosure {
    TassadarPostArticleHiddenStateChannelClosure {
        allowed_state_class_ids: semantic_preservation
            .state_class_rows
            .iter()
            .map(|row| row.state_class_id.clone())
            .collect(),
        hidden_state_channel_closed: semantic_preservation.state_ownership_green,
        green: semantic_preservation.state_ownership_green,
        detail: String::from(
            "all control-affecting state must stay inside the state classes already frozen by `TAS-188`; no additional hidden state channel is allowed to steer branch, retry, or stop behavior.",
        ),
    }
}

fn build_observer_model() -> TassadarPostArticleObserverModel {
    TassadarPostArticleObserverModel {
        verifier_roles: vec![
            String::from("artifact_replayer"),
            String::from("receipt_verifier"),
            String::from("gate_verdict_checker"),
        ],
        acceptance_requirements: vec![
            String::from("machine_identity_match"),
            String::from("decision_trace_replay_match"),
            String::from("determinism_class_match"),
            String::from("typed_failure_class_match"),
        ],
        replay_receipt_required: true,
        gate_verdict_bound_to_machine_identity: true,
        green: true,
        detail: String::from(
            "control-plane acceptance now requires replay receipts and machine-identity match rather than sampled observation alone. The observer model is machine-readable and tied to artifact replay, receipt verification, and gate checks.",
        ),
    }
}

fn build_hidden_control_channel_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
) -> Vec<TassadarPostArticleHiddenControlChannelValidationRow> {
    vec![
        hidden_control_row(
            "latency_steering_blocked",
            TassadarPostArticleHiddenControlChannelKind::LatencySteering,
            true,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "latency is outside the model-visible boundary and cannot steer control decisions.",
        ),
        hidden_control_row(
            "cost_steering_blocked",
            TassadarPostArticleHiddenControlChannelKind::CostSteering,
            true,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "cost is outside the model-visible boundary and cannot steer control decisions.",
        ),
        hidden_control_row(
            "scheduling_steering_blocked",
            TassadarPostArticleHiddenControlChannelKind::SchedulingSteering,
            true,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "scheduler order is outside the model-visible boundary and cannot steer control decisions.",
        ),
        hidden_control_row(
            "cache_hit_steering_blocked",
            TassadarPostArticleHiddenControlChannelKind::CacheHitSteering,
            true,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "cache-hit behavior is acceleration-only here and may not become hidden decision authority.",
        ),
        hidden_control_row(
            "helper_substitution_blocked",
            TassadarPostArticleHiddenControlChannelKind::HelperSubstitution,
            bridge_validation_green(bridge, "helper_substitution_quarantined"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "helper substitution stays explicitly quarantined and cannot become a hidden control channel.",
        ),
        hidden_control_row(
            "fast_route_fallback_blocked",
            TassadarPostArticleHiddenControlChannelKind::FastRouteFallback,
            bridge_validation_green(bridge, "route_drift_rejected")
                && semantic_preservation.semantic_preservation_audit_green,
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
            ],
            "fast-route fallback stays blocked because the proof remains tied to one fixed canonical route id and one fixed semantic-preservation audit surface.",
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    supporting_material_rows: &[TassadarPostArticleDecisionProvenanceSupportingMaterialRow],
    decision_binding_rows: &[TassadarPostArticleDecisionBindingRow],
    determinism_class_contract: &TassadarPostArticleDeterminismClassContract,
    equivalent_choice_relation: &TassadarPostArticleEquivalentChoiceRelation,
    failure_semantics_lattice: &TassadarPostArticleFailureSemanticsLattice,
    time_semantics_contract: &TassadarPostArticleTimeSemanticsContract,
    information_boundary: &TassadarPostArticleInformationBoundary,
    training_inference_boundary: &TassadarPostArticleTrainingInferenceBoundary,
    hidden_state_channel_closure: &TassadarPostArticleHiddenStateChannelClosure,
    observer_model: &TassadarPostArticleObserverModel,
    hidden_control_channel_rows: &[TassadarPostArticleHiddenControlChannelValidationRow],
    carrier_split_publication_complete: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticleDecisionProvenanceValidationRow> {
    let proof_carrying_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ProofCarrying
        })
        .count();
    let observational_context_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleDecisionProvenanceSupportingMaterialClass::ObservationalContext
        })
        .count();

    vec![
        validation_row(
            "helper_substitution_quarantined",
            bridge_validation_green(bridge, "helper_substitution_quarantined"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "helper substitution remains quarantined and cannot own control decisions.",
        ),
        validation_row(
            "route_drift_rejected",
            bridge_validation_green(bridge, "route_drift_rejected")
                && decision_binding_rows
                    .iter()
                    .all(|row| row.bound_to_canonical_route_identity),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "route drift remains rejected because every decision proof row binds to one fixed canonical route id.",
        ),
        validation_row(
            "continuation_abuse_quarantined",
            bridge_validation_green(bridge, "continuation_abuse_quarantined")
                && semantic_preservation.control_ownership_boundary_review.control_ownership_rule_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "continuation abuse remains quarantined because host mechanics stay non-authoritative and ambient retry stays disallowed.",
        ),
        validation_row(
            "semantic_drift_blocked",
            bridge_validation_green(bridge, "semantic_drift_blocked")
                && semantic_preservation.semantic_preservation_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "semantic drift remains blocked because this proof builds on the green semantic-preservation audit rather than sampled parity alone.",
        ),
        validation_row(
            "proof_class_distinction_preserved",
            proof_carrying_count == 5 && observational_context_count == 2,
            vec![
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
            ],
            "proof-carrying artifacts remain distinct from observational audits in the control-plane proof surface.",
        ),
        validation_row(
            "decision_provenance_contract_green",
            decision_binding_rows.iter().all(|row| row.decision_binding_green)
                && determinism_class_contract.determinism_contract_green
                && equivalent_choice_relation.green
                && failure_semantics_lattice.green
                && time_semantics_contract.green
                && information_boundary.green
                && training_inference_boundary.green
                && hidden_state_channel_closure.green
                && observer_model.green
                && hidden_control_channel_rows.iter().all(|row| row.green),
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
                ),
                String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
            ],
            "decision provenance is now frozen as one machine-readable contract over branch, retry, and stop binding, determinism, equivalent-choice, failure semantics, time semantics, information boundaries, state closure, and observer acceptance.",
        ),
        validation_row(
            "overclaim_posture_explicit",
            !carrier_split_publication_complete
                && !rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
            ],
            "the control-plane proof remains bounded: carrier split publication, rebased universality admission, plugin control, served/public universality, and arbitrary software capability all stay explicitly blocked.",
        ),
    ]
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleDecisionProvenanceSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_report_id: Option<String>,
    source_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleDecisionProvenanceSupportingMaterialRow {
    TassadarPostArticleDecisionProvenanceSupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_report_id,
        source_report_digest,
        detail: String::from(detail),
    }
}

fn hidden_control_row(
    validation_id: &str,
    channel_kind: TassadarPostArticleHiddenControlChannelKind,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleHiddenControlChannelValidationRow {
    TassadarPostArticleHiddenControlChannelValidationRow {
        validation_id: String::from(validation_id),
        channel_kind,
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
) -> TassadarPostArticleDecisionProvenanceValidationRow {
    TassadarPostArticleDecisionProvenanceValidationRow {
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
pub fn tassadar_post_article_control_plane_decision_provenance_proof_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF)
}

pub fn write_tassadar_post_article_control_plane_decision_provenance_proof_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleControlPlaneDecisionProvenanceProofReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_control_plane_decision_provenance_proof_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleControlPlaneDecisionProvenanceProofReportError::Write {
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
) -> Result<T, TassadarPostArticleControlPlaneDecisionProvenanceProofReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleControlPlaneDecisionProvenanceProofReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleControlPlaneDecisionProvenanceProofReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_control_plane_decision_provenance_proof_report, read_repo_json,
        tassadar_post_article_control_plane_decision_provenance_proof_report_path,
        write_tassadar_post_article_control_plane_decision_provenance_proof_report,
        TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
        TassadarPostArticleControlPlaneOwnershipStatus,
        TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn control_plane_proof_freezes_decision_provenance_and_defers_carrier_split() {
        let report = build_tassadar_post_article_control_plane_decision_provenance_proof_report()
            .expect("report");

        assert_eq!(
            report.control_plane_ownership_status,
            TassadarPostArticleControlPlaneOwnershipStatus::Green
        );
        assert!(report.control_plane_ownership_green);
        assert!(report.decision_provenance_proof_complete);
        assert!(report.replay_posture_green);
        assert_eq!(
            report.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            report.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(report.decision_binding_rows.len(), 3);
        assert_eq!(report.hidden_control_channel_rows.len(), 6);
        assert_eq!(report.validation_rows.len(), 7);
        assert!(!report.carrier_split_publication_complete);
        assert_eq!(report.deferred_issue_ids, vec![String::from("TAS-189")]);
        assert!(!report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn control_plane_proof_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_control_plane_decision_provenance_proof_report()
                .expect("report");
        let committed: TassadarPostArticleControlPlaneDecisionProvenanceProofReport =
            read_repo_json(
                TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_control_plane_decision_provenance_proof_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_control_plane_decision_provenance_proof_report.json")
        );
    }

    #[test]
    fn write_control_plane_proof_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_control_plane_decision_provenance_proof_report.json");
        let written = write_tassadar_post_article_control_plane_decision_provenance_proof_report(
            &output_path,
        )
        .expect("write report");
        let persisted: TassadarPostArticleControlPlaneDecisionProvenanceProofReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
