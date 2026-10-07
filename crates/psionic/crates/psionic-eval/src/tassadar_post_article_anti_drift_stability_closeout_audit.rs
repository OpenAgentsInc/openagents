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
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
    TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
};
use psionic_sandbox::{
    build_tassadar_post_article_plugin_charter_authority_boundary_report,
    TassadarPostArticlePluginCharterAuthorityBoundaryReport,
    TassadarPostArticlePluginCharterAuthorityBoundaryReportError,
    TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
};
use psionic_transformer::{
    build_tassadar_post_article_anti_drift_stability_closeout_contract,
    TassadarPostArticleAntiDriftClaimBlockRow,
    TassadarPostArticleAntiDriftRequiredSurfaceRow,
    TassadarPostArticleAntiDriftStabilityCloseoutContract,
    TassadarPostArticleAntiDriftSurfaceClass,
};

use crate::{
    build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report,
    build_tassadar_post_article_canonical_machine_identity_lock_report,
    build_tassadar_post_article_continuation_non_computationality_contract_report,
    build_tassadar_post_article_control_plane_decision_provenance_proof_report,
    build_tassadar_post_article_downward_non_influence_and_served_conformance_report,
    build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report,
    build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
    build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
    build_tassadar_post_article_rebased_universality_verdict_split_report,
    build_tassadar_post_article_universality_portability_minimality_matrix_report,
    TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport,
    TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError,
    TassadarPostArticleCanonicalMachineIdentityLockReport,
    TassadarPostArticleCanonicalMachineIdentityLockReportError,
    TassadarPostArticleContinuationNonComputationalityContractReport,
    TassadarPostArticleContinuationNonComputationalityContractReportError,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError,
    TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    TassadarPostArticleRebasedUniversalityVerdictSplitReportError,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
    TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF,
    TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_anti_drift_stability_closeout_audit_report.json";
pub const TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-anti-drift-stability-closeout.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_anti_drift_stability_closeout_contract.rs";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleAntiDriftCloseoutStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleAntiDriftSupportingMaterialClass {
    Anchor,
    RuntimeStatement,
    IdentityLock,
    ProofCarrying,
    Audit,
    BoundaryContract,
    CapabilityCloseout,
    HistoricalContext,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftMachineIdentityBinding {
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub computational_model_statement_id: String,
    pub control_plane_proof_report_id: String,
    pub control_plane_equivalent_choice_relation_id: String,
    pub selected_determinism_class: String,
    pub portability_matrix_report_id: String,
    pub plugin_charter_report_id: String,
    pub plugin_platform_closeout_report_id: String,
    pub rebased_verdict_split_report_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleAntiDriftSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftLockRow {
    pub surface_id: String,
    pub surface_class: TassadarPostArticleAntiDriftSurfaceClass,
    pub source_ref: String,
    pub source_artifact_id: String,
    pub source_artifact_digest: String,
    pub machine_identity_bound: bool,
    pub canonical_route_bound: bool,
    pub stronger_claims_blocked: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftStabilityCloseoutAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub canonical_computational_model_statement_report_ref: String,
    pub canonical_machine_identity_lock_report_ref: String,
    pub control_plane_decision_provenance_proof_report_ref: String,
    pub execution_semantics_proof_transport_audit_report_ref: String,
    pub continuation_non_computationality_contract_report_ref: String,
    pub fast_route_legitimacy_and_carrier_binding_contract_report_ref: String,
    pub equivalent_choice_neutrality_and_admissibility_contract_report_ref: String,
    pub downward_non_influence_and_served_conformance_report_ref: String,
    pub rebased_universality_verdict_split_report_ref: String,
    pub universality_portability_minimality_matrix_report_ref: String,
    pub plugin_charter_authority_boundary_report_ref: String,
    pub bounded_weighted_plugin_platform_closeout_audit_report_ref: String,
    pub post_article_turing_audit_ref: String,
    pub plugin_system_turing_audit_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub anti_drift_contract: TassadarPostArticleAntiDriftStabilityCloseoutContract,
    pub machine_identity_binding: TassadarPostArticleAntiDriftMachineIdentityBinding,
    pub supporting_material_rows: Vec<TassadarPostArticleAntiDriftSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleAntiDriftDependencyRow>,
    pub lock_rows: Vec<TassadarPostArticleAntiDriftLockRow>,
    pub invalidation_rows: Vec<TassadarPostArticleAntiDriftInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleAntiDriftValidationRow>,
    pub stronger_claim_blocks: Vec<TassadarPostArticleAntiDriftClaimBlockRow>,
    pub closeout_status: TassadarPostArticleAntiDriftCloseoutStatus,
    pub closeout_green: bool,
    pub all_required_surface_locks_green: bool,
    pub machine_identity_lock_complete: bool,
    pub control_and_replay_posture_locked: bool,
    pub semantics_and_continuation_locked: bool,
    pub equivalent_choice_and_served_boundary_locked: bool,
    pub portability_and_minimality_locked: bool,
    pub plugin_capability_boundary_locked: bool,
    pub served_and_public_overclaim_suppressed: bool,
    pub stronger_terminal_claims_require_closure_bundle: bool,
    pub stronger_plugin_platform_claims_require_closure_bundle: bool,
    pub closure_bundle_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError {
    #[error(transparent)]
    ComputationalModel(#[from] TassadarPostArticleCanonicalComputationalModelStatementReportError),
    #[error(transparent)]
    MachineLock(#[from] TassadarPostArticleCanonicalMachineIdentityLockReportError),
    #[error(transparent)]
    ControlProof(#[from] TassadarPostArticleControlPlaneDecisionProvenanceProofReportError),
    #[error(transparent)]
    ProofTransport(#[from] TassadarPostArticleExecutionSemanticsProofTransportAuditReportError),
    #[error(transparent)]
    Continuation(#[from] TassadarPostArticleContinuationNonComputationalityContractReportError),
    #[error(transparent)]
    FastRoute(#[from] TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError),
    #[error(transparent)]
    EquivalentChoice(
        #[from] TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError,
    ),
    #[error(transparent)]
    DownwardNonInfluence(
        #[from] TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError,
    ),
    #[error(transparent)]
    RebasedVerdict(#[from] TassadarPostArticleRebasedUniversalityVerdictSplitReportError),
    #[error(transparent)]
    PortabilityMatrix(#[from] TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError),
    #[error(transparent)]
    PluginCharter(#[from] TassadarPostArticlePluginCharterAuthorityBoundaryReportError),
    #[error(transparent)]
    PlatformCloseout(
        #[from] TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError,
    ),
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

pub fn build_tassadar_post_article_anti_drift_stability_closeout_audit_report() -> Result<
    TassadarPostArticleAntiDriftStabilityCloseoutAuditReport,
    TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError,
> {
    let contract = build_tassadar_post_article_anti_drift_stability_closeout_contract();
    let computational_model =
        build_tassadar_post_article_canonical_computational_model_statement_report()?;
    let machine_lock = build_tassadar_post_article_canonical_machine_identity_lock_report()?;
    let control_proof = build_tassadar_post_article_control_plane_decision_provenance_proof_report()?;
    let proof_transport =
        build_tassadar_post_article_execution_semantics_proof_transport_audit_report()?;
    let continuation =
        build_tassadar_post_article_continuation_non_computationality_contract_report()?;
    let fast_route =
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()?;
    let equivalent_choice =
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report(
        )?;
    let downward_non_influence =
        build_tassadar_post_article_downward_non_influence_and_served_conformance_report()?;
    let rebased_verdict = build_tassadar_post_article_rebased_universality_verdict_split_report()?;
    let portability_matrix =
        build_tassadar_post_article_universality_portability_minimality_matrix_report()?;
    let plugin_charter = build_tassadar_post_article_plugin_charter_authority_boundary_report()?;
    let platform_closeout =
        build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report()?;

    Ok(build_report_from_inputs(
        contract,
        computational_model,
        machine_lock,
        control_proof,
        proof_transport,
        continuation,
        fast_route,
        equivalent_choice,
        downward_non_influence,
        rebased_verdict,
        portability_matrix,
        plugin_charter,
        platform_closeout,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    contract: TassadarPostArticleAntiDriftStabilityCloseoutContract,
    computational_model: TassadarPostArticleCanonicalComputationalModelStatementReport,
    machine_lock: TassadarPostArticleCanonicalMachineIdentityLockReport,
    control_proof: TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    proof_transport: TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    continuation: TassadarPostArticleContinuationNonComputationalityContractReport,
    fast_route: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    equivalent_choice: TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    downward_non_influence: TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
    rebased_verdict: TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    portability_matrix: TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    plugin_charter: TassadarPostArticlePluginCharterAuthorityBoundaryReport,
    platform_closeout: TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport,
) -> TassadarPostArticleAntiDriftStabilityCloseoutAuditReport {
    let machine_identity_bound = machine_lock.canonical_machine_tuple.machine_identity_id
        == computational_model
            .computational_model_statement
            .machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id == control_proof.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id == proof_transport.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == continuation.machine_identity_binding.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == fast_route.machine_identity_binding.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == equivalent_choice.machine_identity_binding.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == downward_non_influence.machine_identity_binding.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id == portability_matrix.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == plugin_charter.machine_identity_binding.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == platform_closeout.machine_identity_binding.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id == rebased_verdict.machine_identity_id;

    let canonical_route_bound = machine_lock.canonical_machine_tuple.canonical_route_id
        == computational_model
            .computational_model_statement
            .canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id == control_proof.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id == proof_transport.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == continuation.machine_identity_binding.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == fast_route.machine_identity_binding.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == equivalent_choice.machine_identity_binding.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == downward_non_influence.machine_identity_binding.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id == portability_matrix.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == plugin_charter.machine_identity_binding.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == platform_closeout.machine_identity_binding.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id == rebased_verdict.canonical_route_id;

    let closure_bundle_separate = machine_lock.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
        && computational_model.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
        && continuation.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
        && fast_route.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
        && equivalent_choice.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
        && downward_non_influence.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
        && platform_closeout.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
        && !machine_lock.closure_bundle_embedded_here
        && !computational_model.closure_bundle_embedded_here
        && !platform_closeout.closure_bundle_embedded_here;

    let machine_identity_binding = TassadarPostArticleAntiDriftMachineIdentityBinding {
        machine_identity_id: machine_lock.canonical_machine_tuple.machine_identity_id.clone(),
        tuple_id: machine_lock.canonical_machine_tuple.tuple_id.clone(),
        canonical_model_id: machine_lock.canonical_machine_tuple.canonical_model_id.clone(),
        canonical_route_id: machine_lock.canonical_machine_tuple.canonical_route_id.clone(),
        canonical_route_descriptor_digest: machine_lock
            .canonical_machine_tuple
            .canonical_route_descriptor_digest
            .clone(),
        continuation_contract_id: machine_lock
            .canonical_machine_tuple
            .continuation_contract_id
            .clone(),
        computational_model_statement_id: computational_model
            .computational_model_statement
            .statement_id
            .clone(),
        control_plane_proof_report_id: control_proof.report_id.clone(),
        control_plane_equivalent_choice_relation_id: control_proof
            .determinism_class_contract
            .equivalent_choice_relation_id
            .clone(),
        selected_determinism_class: format!(
            "{:?}",
            control_proof.determinism_class_contract.selected_class
        ),
        portability_matrix_report_id: portability_matrix.report_id.clone(),
        plugin_charter_report_id: plugin_charter.report_id.clone(),
        plugin_platform_closeout_report_id: platform_closeout.report_id.clone(),
        rebased_verdict_split_report_id: rebased_verdict.report_id.clone(),
        detail: format!(
            "anti-drift closeout binds machine_identity_id=`{}`, tuple_id=`{}`, canonical_route_id=`{}`, control_plane_report_id=`{}`, portability_matrix_report_id=`{}`, plugin_platform_closeout_report_id=`{}`, and closure_bundle_issue_id=`{}`.",
            machine_lock.canonical_machine_tuple.machine_identity_id,
            machine_lock.canonical_machine_tuple.tuple_id,
            machine_lock.canonical_machine_tuple.canonical_route_id,
            control_proof.report_id,
            portability_matrix.report_id,
            platform_closeout.report_id,
            CLOSURE_BUNDLE_ISSUE_ID,
        ),
    };

    let all_required_surface_locks_green = computational_model.statement_green
        && machine_lock.lock_green
        && control_proof.control_plane_ownership_green
        && control_proof.decision_provenance_proof_complete
        && proof_transport.audit_green
        && proof_transport.proof_transport_complete
        && continuation.contract_green
        && continuation.continuation_non_computationality_complete
        && fast_route.contract_green
        && fast_route.fast_route_legitimacy_complete
        && equivalent_choice.contract_green
        && equivalent_choice.equivalent_choice_neutrality_complete
        && downward_non_influence.contract_green
        && downward_non_influence.downward_non_influence_complete
        && rebased_verdict.verdict_split_green
        && portability_matrix.matrix_green
        && plugin_charter.charter_green
        && platform_closeout.closeout_green;

    let machine_identity_lock_complete = machine_lock.lock_green
        && machine_lock.one_canonical_machine_named
        && machine_lock.mixed_carrier_evidence_bundle_refused
        && machine_identity_bound
        && canonical_route_bound;
    let control_and_replay_posture_locked = control_proof.control_plane_ownership_green
        && control_proof.decision_provenance_proof_complete
        && control_proof.replay_posture_green
        && control_proof.determinism_class_contract.determinism_contract_green
        && control_proof.equivalent_choice_relation.green
        && control_proof.failure_semantics_lattice.green
        && control_proof.time_semantics_contract.green
        && control_proof.information_boundary.green
        && control_proof.training_inference_boundary.green
        && control_proof.hidden_state_channel_closure.green
        && control_proof.observer_model.green;
    let semantics_and_continuation_locked = computational_model.statement_green
        && proof_transport.audit_green
        && proof_transport.proof_transport_complete
        && proof_transport.plugin_execution_transport_bound
        && continuation.contract_green
        && continuation.continuation_non_computationality_complete
        && continuation.hidden_workflow_logic_refused
        && continuation.continuation_expressivity_extension_blocked
        && continuation.plugin_resume_hidden_compute_refused
        && fast_route.contract_green
        && fast_route.fast_route_legitimacy_complete
        && fast_route.unproven_fast_routes_quarantined
        && fast_route.served_or_plugin_machine_overclaim_refused;
    let equivalent_choice_and_served_boundary_locked = equivalent_choice.contract_green
        && equivalent_choice.equivalent_choice_neutrality_complete
        && equivalent_choice.admissibility_narrowing_receipt_visible
        && equivalent_choice.hidden_ordering_or_ranking_quarantined
        && equivalent_choice.latency_cost_and_soft_failure_channels_blocked
        && equivalent_choice.served_or_plugin_equivalence_overclaim_refused
        && downward_non_influence.contract_green
        && downward_non_influence.downward_non_influence_complete
        && downward_non_influence.served_conformance_envelope_complete
        && downward_non_influence.lower_plane_truth_rewrite_refused
        && downward_non_influence.served_posture_narrower_than_operator_truth
        && downward_non_influence.served_posture_fail_closed
        && downward_non_influence.plugin_or_served_overclaim_refused
        && rebased_verdict.theory_green
        && rebased_verdict.operator_green
        && !rebased_verdict.served_green;
    let portability_and_minimality_locked = portability_matrix.matrix_green
        && portability_matrix.machine_matrix_green
        && portability_matrix.route_classification_green
        && portability_matrix.minimality_green
        && portability_matrix.served_suppression_boundary_preserved
        && plugin_charter.charter_green
        && plugin_charter.proof_vs_audit_distinction_frozen
        && plugin_charter.observer_model_frozen
        && plugin_charter.state_class_split_frozen
        && plugin_charter.downward_non_influence_frozen
        && plugin_charter.host_executes_but_does_not_decide;
    let plugin_capability_boundary_locked = platform_closeout.closeout_green
        && platform_closeout.operator_internal_only_posture
        && platform_closeout.plugin_capability_claim_allowed
        && platform_closeout.weighted_plugin_control_allowed
        && !platform_closeout.plugin_publication_allowed
        && !platform_closeout.served_public_universality_allowed
        && !platform_closeout.arbitrary_software_capability_allowed;
    let served_and_public_overclaim_suppressed = !rebased_verdict.served_green
        && !rebased_verdict.served_public_universality_allowed
        && !platform_closeout.plugin_publication_allowed
        && !platform_closeout.served_public_universality_allowed
        && !plugin_charter.plugin_publication_allowed
        && !plugin_charter.served_public_universality_allowed;

    let stronger_terminal_claims_require_closure_bundle = closure_bundle_separate
        && contract
            .stronger_claim_blocks
            .iter()
            .any(|row| row.claim_id == "terminal_universality_requires_closure_bundle");
    let stronger_plugin_platform_claims_require_closure_bundle = closure_bundle_separate
        && contract
            .stronger_claim_blocks
            .iter()
            .any(|row| row.claim_id == "plugin_platform_publication_requires_closure_bundle");

    let supporting_material_rows = vec![
        supporting_material_row(
            "transformer_anchor_contract",
            TassadarPostArticleAntiDriftSupportingMaterialClass::Anchor,
            true,
            TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
            Some(contract.contract_id.clone()),
            None,
            "the transformer-owned anti-drift contract names the exact surface locks, invalidation laws, and stronger-claim blocks that the closeout must keep explicit.",
        ),
        supporting_material_row(
            "canonical_computational_model_statement",
            TassadarPostArticleAntiDriftSupportingMaterialClass::RuntimeStatement,
            computational_model.statement_green,
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            Some(computational_model.report_id.clone()),
            Some(computational_model.report_digest.clone()),
            "the runtime-owned computational-model statement supplies the published compute identity and plugin-above-machine boundary for the canonical post-article machine.",
        ),
        supporting_material_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleAntiDriftSupportingMaterialClass::IdentityLock,
            machine_lock.lock_green,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            Some(machine_lock.report_id.clone()),
            Some(machine_lock.report_digest.clone()),
            "the eval-owned machine lock binds proof, route, continuation, and plugin-facing artifacts to one canonical tuple instead of implied inheritance.",
        ),
        supporting_material_row(
            "control_plane_decision_provenance_proof",
            TassadarPostArticleAntiDriftSupportingMaterialClass::ProofCarrying,
            control_proof.control_plane_ownership_green,
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            Some(control_proof.report_id.clone()),
            Some(control_proof.report_digest.clone()),
            "the control-plane provenance proof carries determinism, equivalent-choice, failure, time, information-boundary, hidden-state, and observer posture on the same machine identity.",
        ),
        supporting_material_row(
            "execution_semantics_proof_transport_audit",
            TassadarPostArticleAntiDriftSupportingMaterialClass::Audit,
            proof_transport.audit_green,
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            Some(proof_transport.report_id.clone()),
            Some(proof_transport.report_digest.clone()),
            "the proof-transport audit keeps execution semantics attached to the canonical route without reissuing a stronger proof-bearing machine here.",
        ),
        supporting_material_row(
            "continuation_non_computationality_boundary",
            TassadarPostArticleAntiDriftSupportingMaterialClass::BoundaryContract,
            continuation.contract_green,
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
            Some(continuation.report_id.clone()),
            Some(continuation.report_digest.clone()),
            "the continuation boundary keeps checkpoint and resumable surfaces on the same machine without turning them into a second compute carrier.",
        ),
        supporting_material_row(
            "fast_route_legitimacy_and_carrier_binding",
            TassadarPostArticleAntiDriftSupportingMaterialClass::BoundaryContract,
            fast_route.contract_green,
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            Some(fast_route.report_id.clone()),
            Some(fast_route.report_digest.clone()),
            "the fast-route boundary keeps the canonical HullCache carrier explicit and unproven fast families quarantined.",
        ),
        supporting_material_row(
            "equivalent_choice_neutrality_and_admissibility",
            TassadarPostArticleAntiDriftSupportingMaterialClass::BoundaryContract,
            equivalent_choice.contract_green,
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
            Some(equivalent_choice.report_id.clone()),
            Some(equivalent_choice.report_digest.clone()),
            "the equivalent-choice boundary keeps admissible plugin variation receipt-visible and blocks hidden ordering or ranking drift.",
        ),
        supporting_material_row(
            "downward_non_influence_and_served_conformance",
            TassadarPostArticleAntiDriftSupportingMaterialClass::BoundaryContract,
            downward_non_influence.contract_green,
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF,
            Some(downward_non_influence.report_id.clone()),
            Some(downward_non_influence.report_digest.clone()),
            "the downward non-influence boundary keeps lower-plane truth explicit and the served posture narrower than operator truth inside one fail-closed envelope.",
        ),
        supporting_material_row(
            "rebased_universality_verdict_split",
            TassadarPostArticleAntiDriftSupportingMaterialClass::Audit,
            rebased_verdict.verdict_split_green,
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            Some(rebased_verdict.report_id.clone()),
            Some(rebased_verdict.report_digest.clone()),
            "the rebased verdict split keeps theory/operator truth green on the canonical route while served/public universality stays suppressed.",
        ),
        supporting_material_row(
            "universality_portability_minimality_matrix",
            TassadarPostArticleAntiDriftSupportingMaterialClass::Audit,
            portability_matrix.matrix_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
            Some(portability_matrix.report_id.clone()),
            Some(portability_matrix.report_digest.clone()),
            "the portability/minimality matrix keeps machine-class envelope, route classification, and minimality explicit on the canonical route.",
        ),
        supporting_material_row(
            "plugin_charter_authority_boundary",
            TassadarPostArticleAntiDriftSupportingMaterialClass::BoundaryContract,
            plugin_charter.charter_green,
            TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
            Some(plugin_charter.report_id.clone()),
            Some(plugin_charter.report_digest.clone()),
            "the sandbox-owned plugin charter keeps state classes, proof-versus-audit distinction, host-negative authority, and downward non-influence explicit above the same machine.",
        ),
        supporting_material_row(
            "bounded_weighted_plugin_platform_closeout",
            TassadarPostArticleAntiDriftSupportingMaterialClass::CapabilityCloseout,
            platform_closeout.closeout_green,
            TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF,
            Some(platform_closeout.report_id.clone()),
            Some(platform_closeout.report_digest.clone()),
            "the bounded plugin-platform closeout keeps weighted plugin capability operator/internal only without turning publication or public universality green.",
        ),
        supporting_material_row(
            "post_article_turing_audit_context",
            TassadarPostArticleAntiDriftSupportingMaterialClass::HistoricalContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 post-article audit explains why anti-drift closeout must exist before the closure bundle.",
        ),
        supporting_material_row(
            "plugin_system_turing_audit_context",
            TassadarPostArticleAntiDriftSupportingMaterialClass::HistoricalContext,
            true,
            PLUGIN_SYSTEM_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 plugin-system audit explains why plugin-layer capability still has to inherit one canonical machine rather than recomposing it.",
        ),
        supporting_material_row(
            "local_plugin_system_spec",
            TassadarPostArticleAntiDriftSupportingMaterialClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system spec remains design input for the plugin-layer control extension verdict and bounded capability posture.",
        ),
    ];

    let dependency_rows = vec![
        dependency_row(
            "shared_machine_identity_locked",
            machine_identity_bound,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
            ],
            "all anti-drift prerequisite artifacts must name the same canonical post-article machine identity instead of inheriting machine equality by adjacency.",
        ),
        dependency_row(
            "shared_canonical_route_locked",
            canonical_route_bound,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF),
            ],
            "all route-bearing artifacts must stay bound to the same canonical direct HullCache route so route-family drift remains explicit.",
        ),
        dependency_row(
            "control_plane_replay_posture_locked",
            control_and_replay_posture_locked,
            vec![String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF)],
            "control-plane provenance, determinism, equivalent-choice relation, failure lattice, time semantics, information boundary, training-versus-inference boundary, hidden-state closure, and observer model must all stay green together.",
        ),
        dependency_row(
            "semantics_continuation_and_fast_route_locked",
            semantics_and_continuation_locked,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF),
            ],
            "compute identity, proof transport, continuation boundary, and fast-route legitimacy must stay locked on the same canonical machine before anti-drift closeout can turn green.",
        ),
        dependency_row(
            "equivalent_choice_and_served_boundary_locked",
            equivalent_choice_and_served_boundary_locked,
            vec![
                String::from(TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            ],
            "equivalent-choice neutrality, downward non-influence, served fail-closed posture, and rebased served suppression must all remain explicit on the same machine.",
        ),
        dependency_row(
            "portability_minimality_and_plugin_charter_locked",
            portability_and_minimality_locked,
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
            ],
            "machine-class envelope, minimality, proof-versus-audit distinction, state classes, observer posture, and downward non-influence must remain explicit across the lower and plugin-facing surfaces.",
        ),
        dependency_row(
            "plugin_platform_capability_boundary_locked",
            plugin_capability_boundary_locked,
            vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            "the plugin-layer control extension verdict must stay bounded, operator/internal only, and publication-suppressed instead of silently widening into a public platform claim.",
        ),
        dependency_row(
            "served_and_public_overclaim_suppressed",
            !rebased_verdict.served_green
                && !rebased_verdict.served_public_universality_allowed
                && !platform_closeout.plugin_publication_allowed
                && !platform_closeout.served_public_universality_allowed
                && !plugin_charter.plugin_publication_allowed
                && !plugin_charter.served_public_universality_allowed,
            vec![
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            "served/public universality and plugin publication must remain explicitly false after anti-drift closeout.",
        ),
        dependency_row(
            "closure_bundle_stays_separate",
            closure_bundle_separate,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            "anti-drift closeout still keeps the final claim-bearing canonical machine closure bundle separate and deferred to TAS-215.",
        ),
    ];

    let lock_rows = contract
        .required_surface_rows
        .iter()
        .map(|row| {
            build_lock_row(
                row,
                &computational_model,
                &machine_lock,
                &control_proof,
                &proof_transport,
                &continuation,
                &fast_route,
                &equivalent_choice,
                &downward_non_influence,
                &rebased_verdict,
                &portability_matrix,
                &plugin_charter,
                &platform_closeout,
                machine_identity_bound,
                canonical_route_bound,
            )
        })
        .collect::<Vec<_>>();

    let invalidation_rows = vec![
        invalidation_row(
            "mixed_carrier_recomposition_rejected",
            machine_lock.mixed_carrier_evidence_bundle_refused && machine_identity_bound,
            vec![String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF)],
            "mixed-carrier recomposition stays explicit because the canonical machine lock refuses dual-truth-carrier bundles.",
        ),
        invalidation_row(
            "route_drift_rejected",
            fast_route.unproven_fast_routes_quarantined
                && canonical_route_bound
                && portability_matrix.route_classification_green,
            vec![
                String::from(TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF),
            ],
            "route drift stays explicit because fast-route classification remains green and the canonical route stays bound across the portability/minimality matrix.",
        ),
        invalidation_row(
            "determinism_mismatch_rejected",
            control_proof.determinism_class_contract.determinism_contract_green
                && control_proof.replay_posture_green,
            vec![String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF)],
            "determinism drift stays explicit because replay posture and determinism class remain part of the control-plane proof.",
        ),
        invalidation_row(
            "hidden_state_channel_rejected",
            control_proof.hidden_state_channel_closure.hidden_state_channel_closed
                && control_proof.hidden_state_channel_closure.green
                && continuation.hidden_workflow_logic_refused,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
            ],
            "hidden-state and hidden-workflow drift stay explicit because the control proof and continuation boundary both keep those channels closed.",
        ),
        invalidation_row(
            "fast_route_substitution_rejected",
            fast_route.served_or_plugin_machine_overclaim_refused
                && fast_route.unproven_fast_routes_quarantined,
            vec![String::from(
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            )],
            "undeclared fast-route substitution stays explicit because only the canonical HullCache route remains inside the machine carrier.",
        ),
        invalidation_row(
            "downward_influence_rejected",
            downward_non_influence.lower_plane_truth_rewrite_refused
                && plugin_charter.downward_non_influence_frozen,
            vec![
                String::from(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
            ],
            "downward influence stays explicit because both lower-plane and plugin-facing boundaries keep rewrite posture frozen.",
        ),
        invalidation_row(
            "observer_model_mismatch_rejected",
            control_proof.observer_model.green && plugin_charter.observer_model_frozen,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
            ],
            "observer-model drift stays explicit because acceptance and verifier posture remain bound both below and above the plugin layer.",
        ),
        invalidation_row(
            "portability_or_minimality_failure_rejected",
            portability_matrix.machine_matrix_green
                && portability_matrix.route_classification_green
                && portability_matrix.minimality_green,
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF)],
            "machine-class and minimality drift stay explicit because the portability/minimality matrix remains green.",
        ),
        invalidation_row(
            "served_or_plugin_overclaim_rejected",
            downward_non_influence.plugin_or_served_overclaim_refused
                && !platform_closeout.plugin_publication_allowed
                && !platform_closeout.served_public_universality_allowed,
            vec![
                String::from(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            "served/public and plugin overclaim stay explicit because the served envelope remains fail-closed and bounded plugin capability still does not imply publication.",
        ),
    ];

    let validation_rows = vec![
        validation_row(
            "required_surface_lock_set_complete",
            lock_rows.iter().all(|row| row.green) && lock_rows.len() == contract.required_surface_rows.len(),
            vec![String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF)],
            "the anti-drift closeout stays complete only if every transformer-declared surface lock is present and green.",
        ),
        validation_row(
            "machine_identity_and_route_lock_complete",
            machine_identity_lock_complete,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
            ],
            "the canonical machine identity and canonical route both stay explicit and shared across every prerequisite artifact.",
        ),
        validation_row(
            "control_replay_posture_locked",
            control_and_replay_posture_locked,
            vec![String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF)],
            "control-plane provenance, replay posture, observer model, hidden-state closure, and determinism classes remain explicit and green.",
        ),
        validation_row(
            "semantics_and_continuation_locked",
            semantics_and_continuation_locked,
            vec![
                String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF),
            ],
            "execution semantics, continuation boundary, and fast-route legitimacy remain green on the same canonical machine.",
        ),
        validation_row(
            "equivalent_choice_and_served_boundary_locked",
            equivalent_choice_and_served_boundary_locked,
            vec![
                String::from(TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            ],
            "equivalent-choice neutrality, served fail-closed posture, and served/public suppression remain explicit and green.",
        ),
        validation_row(
            "proof_vs_audit_classification_explicit",
            plugin_charter.proof_vs_audit_distinction_frozen
                && control_proof.control_plane_ownership_green
                && proof_transport.audit_green
                && portability_matrix.matrix_green,
            vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
            ],
            "proof-bearing and audit-only surfaces remain explicitly classified instead of being flattened into one implied terminal proof.",
        ),
        validation_row(
            "plugin_capability_boundary_locked",
            plugin_capability_boundary_locked,
            vec![
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            "the plugin-layer control extension verdict remains bounded, operator/internal only, and publication-suppressed.",
        ),
        validation_row(
            "closure_bundle_stays_separate",
            closure_bundle_separate,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            "anti-drift closeout stays distinct from the final claim-bearing closure bundle, which remains reserved to TAS-215.",
        ),
        validation_row(
            "stronger_claims_require_closure_bundle",
            stronger_terminal_claims_require_closure_bundle
                && stronger_plugin_platform_claims_require_closure_bundle,
            vec![String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF)],
            "stronger terminal universality, plugin-publication, or arbitrary software capability claims still require the later canonical machine closure bundle.",
        ),
    ];

    let closeout_green = supporting_material_rows.iter().all(|row| row.satisfied)
        && dependency_rows.iter().all(|row| row.satisfied)
        && invalidation_rows.iter().all(|row| row.present)
        && validation_rows.iter().all(|row| row.green)
        && all_required_surface_locks_green;
    let closeout_status = if closeout_green {
        TassadarPostArticleAntiDriftCloseoutStatus::Green
    } else {
        TassadarPostArticleAntiDriftCloseoutStatus::Blocked
    };

    let stronger_claim_blocks = contract.stronger_claim_blocks.clone();

    let mut report = TassadarPostArticleAntiDriftStabilityCloseoutAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.post_article_anti_drift_stability_closeout_audit.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_CHECKER_REF,
        ),
        transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        canonical_computational_model_statement_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
        ),
        canonical_machine_identity_lock_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
        ),
        control_plane_decision_provenance_proof_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
        ),
        execution_semantics_proof_transport_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
        ),
        continuation_non_computationality_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
        ),
        fast_route_legitimacy_and_carrier_binding_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
        ),
        equivalent_choice_neutrality_and_admissibility_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
        ),
        downward_non_influence_and_served_conformance_report_ref: String::from(
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF,
        ),
        rebased_universality_verdict_split_report_ref: String::from(
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        universality_portability_minimality_matrix_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
        ),
        plugin_charter_authority_boundary_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
        ),
        bounded_weighted_plugin_platform_closeout_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF,
        ),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        plugin_system_turing_audit_ref: String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        anti_drift_contract: contract,
        machine_identity_binding,
        supporting_material_rows,
        dependency_rows,
        lock_rows,
        invalidation_rows,
        validation_rows,
        stronger_claim_blocks,
        closeout_status,
        closeout_green,
        all_required_surface_locks_green,
        machine_identity_lock_complete,
        control_and_replay_posture_locked,
        semantics_and_continuation_locked,
        equivalent_choice_and_served_boundary_locked,
        portability_and_minimality_locked,
        plugin_capability_boundary_locked,
        served_and_public_overclaim_suppressed,
        stronger_terminal_claims_require_closure_bundle,
        stronger_plugin_platform_claims_require_closure_bundle,
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        claim_boundary: String::from(
            "this eval-owned anti-drift closeout says the canonical machine identity, computational model, control-plane provenance, execution-semantics transport, continuation boundary, fast-route carrier, equivalent-choice boundary, downward non-influence boundary, portability/minimality posture, rebased verdict split, plugin charter, and bounded plugin-platform closeout are now actually locked to one explicit post-article machine. It still keeps the final claim-bearing canonical machine closure bundle separate for TAS-215, keeps plugin publication suppressed, keeps served/public universality suppressed, and does not imply arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article anti-drift stability closeout keeps lock_rows={}/{}, dependency_rows={}/{}, invalidation_rows={}/{}, validation_rows={}/{}, closeout_status={:?}, and closure_bundle_issue_id=`{}`.",
        report.lock_rows.iter().filter(|row| row.green).count(),
        report.lock_rows.len(),
        report.dependency_rows.iter().filter(|row| row.satisfied).count(),
        report.dependency_rows.len(),
        report.invalidation_rows.iter().filter(|row| row.present).count(),
        report.invalidation_rows.len(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.closeout_status,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_anti_drift_stability_closeout_audit_report|",
        &report,
    );
    report
}

#[allow(clippy::too_many_arguments)]
fn build_lock_row(
    row: &TassadarPostArticleAntiDriftRequiredSurfaceRow,
    computational_model: &TassadarPostArticleCanonicalComputationalModelStatementReport,
    machine_lock: &TassadarPostArticleCanonicalMachineIdentityLockReport,
    control_proof: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    proof_transport: &TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    continuation: &TassadarPostArticleContinuationNonComputationalityContractReport,
    fast_route: &TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    equivalent_choice: &TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    downward_non_influence: &TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
    rebased_verdict: &TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    portability_matrix: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    plugin_charter: &TassadarPostArticlePluginCharterAuthorityBoundaryReport,
    platform_closeout: &TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport,
    machine_identity_bound: bool,
    canonical_route_bound: bool,
) -> TassadarPostArticleAntiDriftLockRow {
    match row.surface_id.as_str() {
        "canonical_computational_model_statement" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
            source_artifact_id: computational_model.report_id.clone(),
            source_artifact_digest: computational_model.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: !computational_model.plugin_publication_allowed
                && !computational_model.served_public_universality_allowed
                && !computational_model.arbitrary_software_capability_allowed,
            green: computational_model.statement_green
                && computational_model.article_equivalent_compute_named
                && computational_model.tcm_v1_continuation_named
                && computational_model.declared_effect_boundary_named
                && computational_model.plugin_layer_scoped_above_machine,
            detail: row.detail.clone(),
        },
        "canonical_machine_identity_lock" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
            source_artifact_id: machine_lock.report_id.clone(),
            source_artifact_digest: machine_lock.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: !machine_lock.plugin_publication_allowed
                && !machine_lock.served_public_universality_allowed
                && !machine_lock.arbitrary_software_capability_allowed,
            green: machine_lock.lock_green
                && machine_lock.one_canonical_machine_named
                && machine_lock.mixed_carrier_evidence_bundle_refused,
            detail: row.detail.clone(),
        },
        "control_plane_decision_provenance_proof" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
            source_artifact_id: control_proof.report_id.clone(),
            source_artifact_digest: control_proof.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: !control_proof.plugin_capability_claim_allowed
                && !control_proof.served_public_universality_allowed
                && !control_proof.arbitrary_software_capability_allowed,
            green: control_proof.control_plane_ownership_green
                && control_proof.decision_provenance_proof_complete
                && control_proof.replay_posture_green,
            detail: row.detail.clone(),
        },
        "execution_semantics_proof_transport_audit" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
            source_artifact_id: proof_transport.report_id.clone(),
            source_artifact_digest: proof_transport.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: !proof_transport.plugin_capability_claim_allowed
                && !proof_transport.served_public_universality_allowed
                && !proof_transport.arbitrary_software_capability_allowed,
            green: proof_transport.audit_green
                && proof_transport.proof_transport_complete
                && proof_transport.plugin_execution_transport_bound,
            detail: row.detail.clone(),
        },
        "continuation_non_computationality_boundary" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
            source_artifact_id: continuation.report_id.clone(),
            source_artifact_digest: continuation.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: true,
            green: continuation.contract_green
                && continuation.continuation_non_computationality_complete
                && continuation.hidden_workflow_logic_refused
                && continuation.plugin_resume_hidden_compute_refused,
            detail: row.detail.clone(),
        },
        "fast_route_legitimacy_and_carrier_binding" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF),
            source_artifact_id: fast_route.report_id.clone(),
            source_artifact_digest: fast_route.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: true,
            green: fast_route.contract_green
                && fast_route.fast_route_legitimacy_complete
                && fast_route.unproven_fast_routes_quarantined
                && fast_route.served_or_plugin_machine_overclaim_refused,
            detail: row.detail.clone(),
        },
        "equivalent_choice_neutrality_and_admissibility" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF),
            source_artifact_id: equivalent_choice.report_id.clone(),
            source_artifact_digest: equivalent_choice.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: true,
            green: equivalent_choice.contract_green
                && equivalent_choice.equivalent_choice_neutrality_complete
                && equivalent_choice.served_or_plugin_equivalence_overclaim_refused,
            detail: row.detail.clone(),
        },
        "downward_non_influence_and_served_conformance" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF),
            source_artifact_id: downward_non_influence.report_id.clone(),
            source_artifact_digest: downward_non_influence.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: true,
            green: downward_non_influence.contract_green
                && downward_non_influence.downward_non_influence_complete
                && downward_non_influence.served_conformance_envelope_complete
                && downward_non_influence.plugin_or_served_overclaim_refused,
            detail: row.detail.clone(),
        },
        "rebased_universality_verdict_split" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            source_artifact_id: rebased_verdict.report_id.clone(),
            source_artifact_digest: rebased_verdict.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: !rebased_verdict.served_public_universality_allowed
                && !rebased_verdict.arbitrary_software_capability_allowed,
            green: rebased_verdict.verdict_split_green
                && rebased_verdict.theory_green
                && rebased_verdict.operator_green
                && !rebased_verdict.served_green,
            detail: row.detail.clone(),
        },
        "universality_portability_minimality_matrix" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF),
            source_artifact_id: portability_matrix.report_id.clone(),
            source_artifact_digest: portability_matrix.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: !portability_matrix.plugin_capability_claim_allowed
                && !portability_matrix.served_public_universality_allowed
                && !portability_matrix.arbitrary_software_capability_allowed,
            green: portability_matrix.matrix_green
                && portability_matrix.machine_matrix_green
                && portability_matrix.route_classification_green
                && portability_matrix.minimality_green,
            detail: row.detail.clone(),
        },
        "plugin_charter_authority_boundary" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
            source_artifact_id: plugin_charter.report_id.clone(),
            source_artifact_digest: plugin_charter.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: !plugin_charter.plugin_publication_allowed
                && !plugin_charter.served_public_universality_allowed
                && !plugin_charter.arbitrary_software_capability_allowed,
            green: plugin_charter.charter_green
                && plugin_charter.proof_vs_audit_distinction_frozen
                && plugin_charter.state_class_split_frozen
                && plugin_charter.downward_non_influence_frozen,
            detail: row.detail.clone(),
        },
        "bounded_weighted_plugin_platform_closeout" => TassadarPostArticleAntiDriftLockRow {
            surface_id: row.surface_id.clone(),
            surface_class: row.surface_class,
            source_ref: String::from(TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF),
            source_artifact_id: platform_closeout.report_id.clone(),
            source_artifact_digest: platform_closeout.report_digest.clone(),
            machine_identity_bound,
            canonical_route_bound,
            stronger_claims_blocked: !platform_closeout.plugin_publication_allowed
                && !platform_closeout.served_public_universality_allowed
                && !platform_closeout.arbitrary_software_capability_allowed,
            green: platform_closeout.closeout_green
                && platform_closeout.operator_internal_only_posture
                && platform_closeout.plugin_capability_claim_allowed
                && platform_closeout.weighted_plugin_control_allowed,
            detail: row.detail.clone(),
        },
        other => panic!("unknown anti-drift surface id `{other}`"),
    }
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleAntiDriftSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleAntiDriftSupportingMaterialRow {
    TassadarPostArticleAntiDriftSupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_artifact_id,
        source_artifact_digest,
        detail: String::from(detail),
    }
}

fn dependency_row(
    dependency_id: &str,
    satisfied: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleAntiDriftDependencyRow {
    TassadarPostArticleAntiDriftDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs,
        detail: String::from(detail),
    }
}

fn invalidation_row(
    invalidation_id: &str,
    present: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleAntiDriftInvalidationRow {
    TassadarPostArticleAntiDriftInvalidationRow {
        invalidation_id: String::from(invalidation_id),
        present,
        source_refs,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleAntiDriftValidationRow {
    TassadarPostArticleAntiDriftValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_post_article_anti_drift_stability_closeout_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF)
}

pub fn write_tassadar_post_article_anti_drift_stability_closeout_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleAntiDriftStabilityCloseoutAuditReport,
    TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_anti_drift_stability_closeout_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError::Write {
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
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_anti_drift_stability_closeout_audit_report,
        build_report_from_inputs, read_json,
        tassadar_post_article_anti_drift_stability_closeout_audit_report_path,
        write_tassadar_post_article_anti_drift_stability_closeout_audit_report,
        TassadarPostArticleAntiDriftCloseoutStatus,
        TassadarPostArticleAntiDriftStabilityCloseoutAuditReport,
        TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF,
    };
    use crate::{
        build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report,
        build_tassadar_post_article_canonical_machine_identity_lock_report,
        build_tassadar_post_article_continuation_non_computationality_contract_report,
        build_tassadar_post_article_control_plane_decision_provenance_proof_report,
        build_tassadar_post_article_downward_non_influence_and_served_conformance_report,
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report,
        build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
        build_tassadar_post_article_rebased_universality_verdict_split_report,
        build_tassadar_post_article_universality_portability_minimality_matrix_report,
    };
    use psionic_runtime::build_tassadar_post_article_canonical_computational_model_statement_report;
    use psionic_sandbox::build_tassadar_post_article_plugin_charter_authority_boundary_report;
    use psionic_transformer::build_tassadar_post_article_anti_drift_stability_closeout_contract;

    #[test]
    fn anti_drift_closeout_turns_green_when_all_locks_hold() {
        let report = build_tassadar_post_article_anti_drift_stability_closeout_audit_report()
            .expect("report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article_anti_drift_stability_closeout_audit.report.v1"
        );
        assert_eq!(report.closeout_status, TassadarPostArticleAntiDriftCloseoutStatus::Green);
        assert!(report.closeout_green);
        assert!(report.all_required_surface_locks_green);
        assert!(report.machine_identity_lock_complete);
        assert!(report.control_and_replay_posture_locked);
        assert!(report.semantics_and_continuation_locked);
        assert!(report.equivalent_choice_and_served_boundary_locked);
        assert!(report.portability_and_minimality_locked);
        assert!(report.plugin_capability_boundary_locked);
        assert!(report.stronger_terminal_claims_require_closure_bundle);
        assert!(report.stronger_plugin_platform_claims_require_closure_bundle);
        assert_eq!(report.lock_rows.len(), 12);
        assert_eq!(report.invalidation_rows.len(), 9);
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn anti_drift_closeout_blocks_when_machine_identity_drift_is_introduced() {
        let contract = build_tassadar_post_article_anti_drift_stability_closeout_contract();
        let computational_model =
            build_tassadar_post_article_canonical_computational_model_statement_report()
                .expect("computational_model");
        let mut machine_lock = build_tassadar_post_article_canonical_machine_identity_lock_report()
            .expect("machine_lock");
        let control_proof = build_tassadar_post_article_control_plane_decision_provenance_proof_report()
            .expect("control_proof");
        let proof_transport =
            build_tassadar_post_article_execution_semantics_proof_transport_audit_report()
                .expect("proof_transport");
        let continuation =
            build_tassadar_post_article_continuation_non_computationality_contract_report()
                .expect("continuation");
        let fast_route =
            build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()
                .expect("fast_route");
        let equivalent_choice =
            build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report()
                .expect("equivalent_choice");
        let downward_non_influence =
            build_tassadar_post_article_downward_non_influence_and_served_conformance_report()
                .expect("downward_non_influence");
        let rebased_verdict = build_tassadar_post_article_rebased_universality_verdict_split_report()
            .expect("rebased_verdict");
        let portability_matrix =
            build_tassadar_post_article_universality_portability_minimality_matrix_report()
                .expect("portability_matrix");
        let plugin_charter = build_tassadar_post_article_plugin_charter_authority_boundary_report()
            .expect("plugin_charter");
        let platform_closeout =
            build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report()
                .expect("platform_closeout");

        machine_lock.canonical_machine_tuple.machine_identity_id = String::from("drift.machine.v1");

        let report = build_report_from_inputs(
            contract,
            computational_model,
            machine_lock,
            control_proof,
            proof_transport,
            continuation,
            fast_route,
            equivalent_choice,
            downward_non_influence,
            rebased_verdict,
            portability_matrix,
            plugin_charter,
            platform_closeout,
        );

        assert_eq!(report.closeout_status, TassadarPostArticleAntiDriftCloseoutStatus::Blocked);
        assert!(!report.closeout_green);
        assert!(!report.machine_identity_lock_complete);
        assert!(
            report
                .dependency_rows
                .iter()
                .any(|row| row.dependency_id == "shared_machine_identity_locked" && !row.satisfied)
        );
    }

    #[test]
    fn anti_drift_closeout_matches_committed_truth() {
        let expected = build_tassadar_post_article_anti_drift_stability_closeout_audit_report()
            .expect("expected");
        let committed: TassadarPostArticleAntiDriftStabilityCloseoutAuditReport = read_json(
            tassadar_post_article_anti_drift_stability_closeout_audit_report_path(),
        )
        .expect("committed");

        assert_eq!(committed, expected);
        assert!(
            tassadar_post_article_anti_drift_stability_closeout_audit_report_path()
                .ends_with(
                    "tassadar_post_article_anti_drift_stability_closeout_audit_report.json"
                )
        );
    }

    #[test]
    fn write_anti_drift_closeout_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_post_article_anti_drift_stability_closeout_audit_report.json");

        let written = write_tassadar_post_article_anti_drift_stability_closeout_audit_report(
            &output_path,
        )
        .expect("written");
        let roundtrip: TassadarPostArticleAntiDriftStabilityCloseoutAuditReport =
            read_json(&output_path).expect("roundtrip");

        assert_eq!(written, roundtrip);
        assert!(output_path.exists());
        assert_eq!(
            TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_anti_drift_stability_closeout_audit_report.json"
        );
    }
}
