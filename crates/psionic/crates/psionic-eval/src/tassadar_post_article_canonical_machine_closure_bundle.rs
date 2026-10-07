use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_data::TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF;
use psionic_runtime::{
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
    TassadarTcmV1RuntimeContractReport, TassadarTcmV1RuntimeContractReportError,
    TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
    TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
};
use psionic_sandbox::{
    TassadarPostArticlePluginCharterAuthorityBoundaryReport,
    TassadarPostArticlePluginCharterAuthorityBoundaryReportError,
    TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
};
use psionic_transformer::{
    build_tassadar_post_article_canonical_machine_closure_bundle_contract,
    TassadarPostArticleCanonicalMachineClosureBundleArtifactClassRow,
    TassadarPostArticleCanonicalMachineClosureBundleContract,
    TassadarPostArticleCanonicalMachineClosureBundleInvalidationLaw,
};

use crate::{
    TassadarPostArticleAntiDriftStabilityCloseoutAuditReport,
    TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError,
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
    TassadarPostArticleRebasedUniversalityVerdictSplitReportError,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
    TassadarUniversalMachineProofReport, TassadarUniversalMachineProofReportError,
    TassadarUniversalityVerdictSplitReport, TassadarUniversalityVerdictSplitReportError,
    TassadarUniversalityWitnessSuiteReport, TassadarUniversalityWitnessSuiteReportError,
    TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF,
    TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
    TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF, TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_report.json";
pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-canonical-machine-closure-bundle.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_canonical_machine_closure_bundle_contract.rs";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const CLOSURE_BUNDLE_ID: &str = "tassadar.post_article.canonical_machine.closure_bundle.v1";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";
const NEXT_ISSUE_ID: &str = "TAS-217";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalMachineClosureBundleStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass {
    ProofCarrying,
    Audit,
    RuntimeStatement,
    RuntimeContract,
    BoundaryContract,
    IdentityLock,
    CapabilityBoundary,
    HistoricalContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureSubject {
    pub closure_bundle_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub computational_model_statement_id: String,
    pub computational_model_statement_digest: String,
    pub direct_decode_mode: String,
    pub control_plane_proof_report_id: String,
    pub control_plane_proof_report_digest: String,
    pub determinism_class: String,
    pub equivalent_choice_relation_id: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub execution_semantics_proof_transport_report_id: String,
    pub execution_semantics_proof_transport_report_digest: String,
    pub carrier_split_topology: String,
    pub direct_carrier_id: String,
    pub resumable_carrier_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub hidden_state_allowed_state_class_ids: Vec<String>,
    pub plugin_state_class_ids: Vec<String>,
    pub failure_semantic_class_ids: Vec<String>,
    pub observer_verifier_roles: Vec<String>,
    pub observer_acceptance_requirements: Vec<String>,
    pub portability_matrix_report_id: String,
    pub portability_matrix_report_digest: String,
    pub minimality_matrix_report_id: String,
    pub minimality_matrix_report_digest: String,
    pub plugin_charter_report_id: String,
    pub plugin_charter_report_digest: String,
    pub anti_drift_closeout_report_id: String,
    pub anti_drift_closeout_report_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub tcm_v1_runtime_contract_report_ref: String,
    pub historical_universal_machine_proof_report_ref: String,
    pub historical_universality_witness_suite_report_ref: String,
    pub historical_universality_verdict_split_report_ref: String,
    pub universality_bridge_contract_report_ref: String,
    pub canonical_computational_model_statement_report_ref: String,
    pub canonical_machine_identity_lock_report_ref: String,
    pub control_plane_decision_provenance_proof_report_ref: String,
    pub execution_semantics_proof_transport_audit_report_ref: String,
    pub continuation_non_computationality_contract_report_ref: String,
    pub fast_route_legitimacy_and_carrier_binding_contract_report_ref: String,
    pub equivalent_choice_neutrality_and_admissibility_contract_report_ref: String,
    pub downward_non_influence_and_served_conformance_report_ref: String,
    pub universality_portability_minimality_matrix_report_ref: String,
    pub rebased_universality_verdict_split_report_ref: String,
    pub plugin_charter_authority_boundary_report_ref: String,
    pub anti_drift_stability_closeout_audit_report_ref: String,
    pub post_article_turing_audit_ref: String,
    pub plugin_system_turing_audit_ref: String,
    pub closure_bundle_contract: TassadarPostArticleCanonicalMachineClosureBundleContract,
    pub supporting_material_rows:
        Vec<TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleCanonicalMachineClosureBundleDependencyRow>,
    pub closure_subject: TassadarPostArticleCanonicalMachineClosureSubject,
    pub invalidation_rows: Vec<TassadarPostArticleCanonicalMachineClosureBundleInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleCanonicalMachineClosureBundleValidationRow>,
    pub bundle_status: TassadarPostArticleCanonicalMachineClosureBundleStatus,
    pub bundle_green: bool,
    pub proof_and_audit_classification_complete: bool,
    pub machine_subject_complete: bool,
    pub control_execution_and_continuation_bound: bool,
    pub hidden_state_and_observer_model_bound: bool,
    pub portability_and_minimality_bound: bool,
    pub anti_drift_closeout_inherited: bool,
    pub terminal_claims_must_reference_bundle_digest: bool,
    pub plugin_claims_must_reference_bundle_digest: bool,
    pub platform_claims_must_reference_bundle_digest: bool,
    pub closure_bundle_issue_id: String,
    pub next_issue_id: String,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub closure_bundle_digest: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalMachineClosureBundleReportError {
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error(transparent)]
    HistoricalProof(#[from] TassadarUniversalMachineProofReportError),
    #[error(transparent)]
    HistoricalWitnessSuite(#[from] TassadarUniversalityWitnessSuiteReportError),
    #[error(transparent)]
    HistoricalVerdictSplit(#[from] TassadarUniversalityVerdictSplitReportError),
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
    #[error(transparent)]
    ComputationalModel(#[from] TassadarPostArticleCanonicalComputationalModelStatementReportError),
    #[error(transparent)]
    MachineLock(#[from] TassadarPostArticleCanonicalMachineIdentityLockReportError),
    #[error(transparent)]
    ControlPlane(#[from] TassadarPostArticleControlPlaneDecisionProvenanceProofReportError),
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
    Downward(#[from] TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError),
    #[error(transparent)]
    Portability(#[from] TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError),
    #[error(transparent)]
    RebasedVerdict(#[from] TassadarPostArticleRebasedUniversalityVerdictSplitReportError),
    #[error(transparent)]
    PluginCharter(#[from] TassadarPostArticlePluginCharterAuthorityBoundaryReportError),
    #[error(transparent)]
    AntiDrift(#[from] TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError),
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

pub fn build_tassadar_post_article_canonical_machine_closure_bundle_report() -> Result<
    TassadarPostArticleCanonicalMachineClosureBundleReport,
    TassadarPostArticleCanonicalMachineClosureBundleReportError,
> {
    let closure_bundle_contract =
        build_tassadar_post_article_canonical_machine_closure_bundle_contract();
    let runtime_contract: TassadarTcmV1RuntimeContractReport =
        read_json(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)?;
    let historical_proof: TassadarUniversalMachineProofReport =
        read_json(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF)?;
    let historical_witness_suite: TassadarUniversalityWitnessSuiteReport =
        read_json(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF)?;
    let historical_verdict_split: TassadarUniversalityVerdictSplitReport =
        read_json(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF)?;
    let bridge: crate::TassadarPostArticleUniversalityBridgeContractReport =
        read_json(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)?;
    let computational_model: TassadarPostArticleCanonicalComputationalModelStatementReport =
        read_json(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF)?;
    let machine_lock: TassadarPostArticleCanonicalMachineIdentityLockReport =
        read_json(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF)?;
    let control_plane: TassadarPostArticleControlPlaneDecisionProvenanceProofReport =
        read_json(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF)?;
    let proof_transport: TassadarPostArticleExecutionSemanticsProofTransportAuditReport =
        read_json(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF)?;
    let continuation: TassadarPostArticleContinuationNonComputationalityContractReport =
        read_json(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF)?;
    let fast_route: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport =
        read_json(
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
        )?;
    let equivalent_choice: TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport =
        read_json(TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF)?;
    let downward: TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport =
        read_json(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF)?;
    let portability: TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport =
        read_json(TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF)?;
    let rebased_verdict: crate::TassadarPostArticleRebasedUniversalityVerdictSplitReport =
        read_json(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF)?;
    let plugin_charter: TassadarPostArticlePluginCharterAuthorityBoundaryReport =
        read_json(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF)?;
    let anti_drift: TassadarPostArticleAntiDriftStabilityCloseoutAuditReport =
        read_json(TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF)?;

    let direct_carrier = bridge
        .carrier_rows
        .iter()
        .find(|row| {
            matches!(
                row.carrier_class,
                crate::TassadarPostArticleCarrierClass::DirectArticleEquivalent
            )
        })
        .or_else(|| bridge.carrier_rows.first())
        .expect("bridge should keep one direct carrier");
    let resumable_carrier = bridge
        .carrier_rows
        .iter()
        .find(|row| {
            matches!(
                row.carrier_class,
                crate::TassadarPostArticleCarrierClass::ResumableUniversality
            )
        })
        .or_else(|| bridge.carrier_rows.get(1))
        .expect("bridge should keep one resumable carrier");

    let machine_subject_complete = machine_lock.lock_green
        && computational_model.statement_green
        && bridge.bridge_contract_green
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == computational_model
                .computational_model_statement
                .machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == bridge.bridge_machine_identity.machine_identity_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == computational_model
                .computational_model_statement
                .canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == bridge.bridge_machine_identity.canonical_route_id;

    let proof_and_audit_classification_complete = closure_bundle_contract
        .artifact_classification_rows
        .iter()
        .all(|row| {
            classification_green(
                row,
                &runtime_contract,
                &historical_proof,
                &historical_witness_suite,
                &historical_verdict_split,
                &machine_lock,
                &computational_model,
                &control_plane,
                &proof_transport,
                &continuation,
                &fast_route,
                &equivalent_choice,
                &downward,
                &portability,
                &plugin_charter,
                &anti_drift,
            )
        });

    let control_execution_and_continuation_bound = control_plane.decision_provenance_proof_complete
        && control_plane.control_plane_ownership_green
        && control_plane.replay_posture_green
        && proof_transport.audit_green
        && proof_transport.proof_transport_complete
        && continuation.contract_green
        && continuation.continuation_non_computationality_complete
        && fast_route.contract_green
        && fast_route.fast_route_legitimacy_complete
        && equivalent_choice.contract_green
        && equivalent_choice.equivalent_choice_neutrality_complete
        && downward.contract_green
        && downward.downward_non_influence_complete;

    let hidden_state_and_observer_model_bound = control_plane.hidden_state_channel_closure.green
        && control_plane.observer_model.green
        && plugin_charter.observer_model_frozen
        && plugin_charter.state_class_split_frozen;

    let portability_and_minimality_bound = portability.matrix_green
        && portability.machine_matrix_green
        && portability.minimality_green;

    let anti_drift_closeout_inherited =
        anti_drift.closeout_green && anti_drift.all_required_surface_locks_green;

    let supporting_material_rows = vec![
        supporting_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::RuntimeContract,
            runtime_contract.overall_green,
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
            Some(runtime_contract.report_id.clone()),
            Some(runtime_contract.report_digest.clone()),
            "the closure bundle keeps the historical TCM.v1 runtime contract explicit as the continuation and effect carrier.",
        ),
        supporting_row(
            "historical_universal_machine_proof",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::ProofCarrying,
            historical_proof.overall_green,
            TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
            Some(historical_proof.report_id.clone()),
            Some(historical_proof.report_digest.clone()),
            "the closure bundle keeps the historical universal-machine proof explicit by digest.",
        ),
        supporting_row(
            "historical_universality_witness_suite",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::ProofCarrying,
            historical_witness_suite.overall_green,
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
            Some(historical_witness_suite.report_id.clone()),
            Some(historical_witness_suite.report_digest.clone()),
            "the closure bundle keeps the historical witness suite explicit by digest.",
        ),
        supporting_row(
            "historical_universality_verdict_split",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::Audit,
            historical_verdict_split.theory_green && historical_verdict_split.operator_green,
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            Some(historical_verdict_split.report_id.clone()),
            Some(historical_verdict_split.report_digest.clone()),
            "the closure bundle keeps the historical theory/operator verdict split explicit by digest.",
        ),
        supporting_row(
            "universality_bridge_contract",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::BoundaryContract,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the closure bundle keeps the rebased bridge carrier split and supported machine classes explicit.",
        ),
        supporting_row(
            "canonical_computational_model_statement",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::RuntimeStatement,
            computational_model.statement_green,
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            Some(computational_model.report_id.clone()),
            Some(computational_model.report_digest.clone()),
            "the closure bundle keeps the separately published computational-model statement explicit by digest.",
        ),
        supporting_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::IdentityLock,
            machine_lock.lock_green,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            Some(machine_lock.report_id.clone()),
            Some(machine_lock.report_digest.clone()),
            "the closure bundle keeps the canonical machine tuple explicit by digest.",
        ),
        supporting_row(
            "control_plane_decision_provenance_proof",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::ProofCarrying,
            control_plane.decision_provenance_proof_complete,
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            Some(control_plane.report_id.clone()),
            Some(control_plane.report_digest.clone()),
            "the closure bundle keeps determinism, equivalent-choice, failure, time, information, training/inference, hidden-state, and observer semantics explicit by digest.",
        ),
        supporting_row(
            "execution_semantics_proof_transport_audit",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::Audit,
            proof_transport.audit_green && proof_transport.proof_transport_complete,
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            Some(proof_transport.report_id.clone()),
            Some(proof_transport.report_digest.clone()),
            "the closure bundle keeps the rebased proof-transport boundary explicit by digest.",
        ),
        supporting_row(
            "continuation_non_computationality_contract",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::BoundaryContract,
            continuation.contract_green && continuation.continuation_non_computationality_complete,
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
            Some(continuation.report_id.clone()),
            Some(continuation.report_digest.clone()),
            "the closure bundle keeps continuation semantics explicit by digest.",
        ),
        supporting_row(
            "fast_route_legitimacy_and_carrier_binding_contract",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::BoundaryContract,
            fast_route.contract_green && fast_route.fast_route_legitimacy_complete,
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            Some(fast_route.report_id.clone()),
            Some(fast_route.report_digest.clone()),
            "the closure bundle keeps fast-route carrier legitimacy explicit by digest.",
        ),
        supporting_row(
            "equivalent_choice_neutrality_and_admissibility_contract",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::BoundaryContract,
            equivalent_choice.contract_green
                && equivalent_choice.equivalent_choice_neutrality_complete,
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
            Some(equivalent_choice.report_id.clone()),
            Some(equivalent_choice.report_digest.clone()),
            "the closure bundle keeps equivalent-choice neutrality explicit by digest.",
        ),
        supporting_row(
            "downward_non_influence_and_served_conformance_contract",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::BoundaryContract,
            downward.contract_green && downward.downward_non_influence_complete,
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF,
            Some(downward.report_id.clone()),
            Some(downward.report_digest.clone()),
            "the closure bundle keeps downward non-influence and served conformance explicit by digest.",
        ),
        supporting_row(
            "universality_portability_minimality_matrix",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::Audit,
            portability.matrix_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
            Some(portability.report_id.clone()),
            Some(portability.report_digest.clone()),
            "the closure bundle keeps portability and minimality posture explicit by digest.",
        ),
        supporting_row(
            "rebased_universality_verdict_split",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::Audit,
            rebased_verdict.theory_green && rebased_verdict.operator_green,
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            Some(rebased_verdict.report_id.clone()),
            Some(rebased_verdict.report_digest.clone()),
            "the closure bundle keeps the rebased theory/operator verdict split explicit by digest.",
        ),
        supporting_row(
            "plugin_charter_authority_boundary",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::CapabilityBoundary,
            plugin_charter.charter_green,
            TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
            Some(plugin_charter.report_id.clone()),
            Some(plugin_charter.report_digest.clone()),
            "the closure bundle keeps the plugin charter state-class and governance boundary explicit by digest.",
        ),
        supporting_row(
            "anti_drift_stability_closeout",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::Audit,
            anti_drift.closeout_green,
            TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF,
            Some(anti_drift.report_id.clone()),
            Some(anti_drift.report_digest.clone()),
            "the closure bundle inherits the anti-drift verdict that these surfaces are already locked to one canonical machine.",
        ),
        supporting_row(
            "post_article_turing_audit_context",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::HistoricalContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 post-article audit remains historical context for why the closure bundle must exist as one indivisible machine object.",
        ),
        supporting_row(
            "plugin_system_turing_audit_context",
            TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass::HistoricalContext,
            true,
            PLUGIN_SYSTEM_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 plugin-system audit remains historical context for why later plugin claims must inherit the closure bundle instead of recomposing the machine.",
        ),
    ];

    let dependency_rows = vec![
        dependency_row(
            "machine_subject_bound",
            machine_subject_complete,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            "the canonical machine tuple, computational-model statement, and rebased bridge must agree on one machine subject.",
        ),
        dependency_row(
            "historical_proof_chain_bound",
            runtime_contract.overall_green
                && historical_proof.overall_green
                && historical_witness_suite.overall_green
                && historical_verdict_split.theory_green
                && historical_verdict_split.operator_green,
            vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF),
                String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
                String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            ],
            "the historical proof, witness, and verdict chain must stay green before it can be rebound into one canonical closure bundle.",
        ),
        dependency_row(
            "control_plane_bound",
            control_plane.decision_provenance_proof_complete
                && control_plane.control_plane_ownership_green
                && control_plane.replay_posture_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            )],
            "determinism, equivalent-choice, failure, time, information-boundary, hidden-state, and observer semantics must remain bound by the control-plane proof.",
        ),
        dependency_row(
            "execution_transport_bound",
            proof_transport.audit_green && proof_transport.proof_transport_complete,
            vec![String::from(
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            )],
            "the rebased proof-transport boundary must remain green before the bundle can bind proof-carrying artifacts to the owned route.",
        ),
        dependency_row(
            "continuation_and_carrier_bound",
            continuation.contract_green
                && continuation.continuation_non_computationality_complete
                && fast_route.contract_green
                && fast_route.fast_route_legitimacy_complete
                && equivalent_choice.contract_green
                && equivalent_choice.equivalent_choice_neutrality_complete
                && downward.contract_green
                && downward.downward_non_influence_complete,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF,
                ),
            ],
            "continuation, carrier binding, equivalent-choice, and downward non-influence boundaries must stay green on the same machine subject.",
        ),
        dependency_row(
            "portability_and_minimality_bound",
            portability_and_minimality_bound,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
            )],
            "portability and minimality posture must remain explicit before the machine can be published as one closure bundle.",
        ),
        dependency_row(
            "plugin_charter_bound",
            plugin_charter.charter_green
                && plugin_charter.state_class_split_frozen
                && plugin_charter.observer_model_frozen,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
            )],
            "the plugin charter must remain frozen so later plugin claims inherit the same state-class and observer boundaries.",
        ),
        dependency_row(
            "anti_drift_closeout_inherited",
            anti_drift_closeout_inherited,
            vec![String::from(
                TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF,
            )],
            "the anti-drift closeout must already be green before the closure bundle can publish the claim-bearing machine object.",
        ),
    ];

    let closure_subject = TassadarPostArticleCanonicalMachineClosureSubject {
        closure_bundle_id: String::from(CLOSURE_BUNDLE_ID),
        machine_identity_id: machine_lock.canonical_machine_tuple.machine_identity_id.clone(),
        tuple_id: machine_lock.canonical_machine_tuple.tuple_id.clone(),
        carrier_class_id: machine_lock.canonical_machine_tuple.carrier_class_id.clone(),
        canonical_model_id: machine_lock.canonical_machine_tuple.canonical_model_id.clone(),
        canonical_weight_bundle_digest: machine_lock
            .canonical_machine_tuple
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: machine_lock
            .canonical_machine_tuple
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: machine_lock.canonical_machine_tuple.canonical_route_id.clone(),
        canonical_route_descriptor_digest: machine_lock
            .canonical_machine_tuple
            .canonical_route_descriptor_digest
            .clone(),
        computational_model_statement_id: computational_model
            .computational_model_statement
            .statement_id
            .clone(),
        computational_model_statement_digest: computational_model.report_digest.clone(),
        direct_decode_mode: bridge.bridge_machine_identity.direct_decode_mode.clone(),
        control_plane_proof_report_id: control_plane.report_id.clone(),
        control_plane_proof_report_digest: control_plane.report_digest.clone(),
        determinism_class: format!(
            "{:?}",
            control_plane.determinism_class_contract.selected_class
        ),
        equivalent_choice_relation_id: control_plane
            .equivalent_choice_relation
            .relation_id
            .clone(),
        continuation_contract_id: machine_lock
            .canonical_machine_tuple
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: machine_lock
            .canonical_machine_tuple
            .continuation_contract_digest
            .clone(),
        execution_semantics_proof_transport_report_id: proof_transport.report_id.clone(),
        execution_semantics_proof_transport_report_digest: proof_transport.report_digest.clone(),
        carrier_split_topology: format!("{:?}", bridge.carrier_topology),
        direct_carrier_id: direct_carrier.carrier_id.clone(),
        resumable_carrier_id: resumable_carrier.carrier_id.clone(),
        supported_machine_class_ids: bridge.bridge_machine_identity.supported_machine_class_ids.clone(),
        hidden_state_allowed_state_class_ids: control_plane
            .hidden_state_channel_closure
            .allowed_state_class_ids
            .clone(),
        plugin_state_class_ids: plugin_charter
            .state_class_rows
            .iter()
            .map(|row| row.state_class_id.clone())
            .collect(),
        failure_semantic_class_ids: control_plane
            .failure_semantics_lattice
            .failure_classes
            .iter()
            .map(|class| format!("{class:?}"))
            .collect(),
        observer_verifier_roles: control_plane.observer_model.verifier_roles.clone(),
        observer_acceptance_requirements: control_plane
            .observer_model
            .acceptance_requirements
            .clone(),
        portability_matrix_report_id: portability.report_id.clone(),
        portability_matrix_report_digest: portability.report_digest.clone(),
        minimality_matrix_report_id: portability.report_id.clone(),
        minimality_matrix_report_digest: portability.report_digest.clone(),
        plugin_charter_report_id: plugin_charter.report_id.clone(),
        plugin_charter_report_digest: plugin_charter.report_digest.clone(),
        anti_drift_closeout_report_id: anti_drift.report_id.clone(),
        anti_drift_closeout_report_digest: anti_drift.report_digest.clone(),
        detail: format!(
            "closure_bundle_id=`{}` machine_identity_id=`{}` tuple_id=`{}` canonical_route_id=`{}` determinism_class={:?} equivalent_choice_relation_id=`{}` direct_carrier_id=`{}` resumable_carrier_id=`{}` plugin_charter_report_id=`{}` anti_drift_report_id=`{}` next_issue_id=`{}`.",
            CLOSURE_BUNDLE_ID,
            machine_lock.canonical_machine_tuple.machine_identity_id,
            machine_lock.canonical_machine_tuple.tuple_id,
            machine_lock.canonical_machine_tuple.canonical_route_id,
            control_plane.determinism_class_contract.selected_class,
            control_plane.equivalent_choice_relation.relation_id,
            direct_carrier.carrier_id,
            resumable_carrier.carrier_id,
            plugin_charter.report_id,
            anti_drift.report_id,
            NEXT_ISSUE_ID,
        ),
    };

    let invalidation_rows = closure_bundle_contract
        .invalidation_laws
        .iter()
        .map(|law| invalidation_row_from_law(law))
        .collect::<Vec<_>>();

    let terminal_claims_must_reference_bundle_digest = true;
    let plugin_claims_must_reference_bundle_digest = true;
    let platform_claims_must_reference_bundle_digest = true;

    let validation_rows = vec![
        validation_row(
            "proof_vs_audit_classification_complete",
            proof_and_audit_classification_complete,
            supporting_material_rows
                .iter()
                .filter_map(|row| row.source_artifact_id.as_ref().map(|_| row.source_ref.clone()))
                .collect(),
            "every required proof, audit, runtime statement, runtime contract, boundary contract, identity lock, and capability boundary remains explicitly classified.",
        ),
        validation_row(
            "machine_subject_complete",
            machine_subject_complete,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            "the canonical tuple, computational-model statement, and rebased bridge now resolve to one machine subject.",
        ),
        validation_row(
            "control_execution_and_continuation_bound",
            control_execution_and_continuation_bound,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF),
            ],
            "control, proof transport, continuation, and fast-route carrier binding are all locked onto the same machine subject.",
        ),
        validation_row(
            "hidden_state_and_observer_model_bound",
            hidden_state_and_observer_model_bound,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF),
            ],
            "hidden-state closure and observer acceptance model are both machine-bound inside the bundle.",
        ),
        validation_row(
            "portability_and_minimality_bound",
            portability_and_minimality_bound,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
            )],
            "portability and minimality posture remain explicit and machine-checkable inside the bundle.",
        ),
        validation_row(
            "anti_drift_closeout_inherited",
            anti_drift_closeout_inherited,
            vec![String::from(
                TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF,
            )],
            "the anti-drift closeout remains green and is now inherited by the closure bundle instead of staying as an adjacent prerequisite only.",
        ),
        validation_row(
            "terminal_claims_require_bundle_digest",
            terminal_claims_must_reference_bundle_digest,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            )],
            "later terminal universality claims must reference the closure bundle by digest.",
        ),
        validation_row(
            "plugin_and_platform_claims_require_bundle_digest",
            plugin_claims_must_reference_bundle_digest && platform_claims_must_reference_bundle_digest,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            )],
            "later weighted-controller, receipt, publication, and platform claims must reference the closure bundle by digest.",
        ),
        validation_row(
            "next_issue_frontier_advanced",
            NEXT_ISSUE_ID == "TAS-217",
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            )],
            "the canonical machine closure bundle is now published, so the next open TAS issue moves to TAS-216.",
        ),
    ];

    let bundle_green = supporting_material_rows.iter().all(|row| row.satisfied)
        && dependency_rows.iter().all(|row| row.satisfied)
        && validation_rows.iter().all(|row| row.green);

    let mut report = TassadarPostArticleCanonicalMachineClosureBundleReport {
        schema_version: 1,
        report_id: String::from("tassadar.post_article_canonical_machine_closure_bundle.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_CHECKER_REF,
        ),
        transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        tcm_v1_runtime_contract_report_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
        historical_universal_machine_proof_report_ref: String::from(
            TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
        ),
        historical_universality_witness_suite_report_ref: String::from(
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
        ),
        historical_universality_verdict_split_report_ref: String::from(
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        universality_bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
        ),
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
        universality_portability_minimality_matrix_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
        ),
        rebased_universality_verdict_split_report_ref: String::from(
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        plugin_charter_authority_boundary_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF,
        ),
        anti_drift_stability_closeout_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_AUDIT_REPORT_REF,
        ),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        plugin_system_turing_audit_ref: String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
        closure_bundle_contract,
        supporting_material_rows,
        dependency_rows,
        closure_subject,
        invalidation_rows,
        validation_rows,
        bundle_status: if bundle_green {
            TassadarPostArticleCanonicalMachineClosureBundleStatus::Green
        } else {
            TassadarPostArticleCanonicalMachineClosureBundleStatus::Blocked
        },
        bundle_green,
        proof_and_audit_classification_complete,
        machine_subject_complete,
        control_execution_and_continuation_bound,
        hidden_state_and_observer_model_bound,
        portability_and_minimality_bound,
        anti_drift_closeout_inherited,
        terminal_claims_must_reference_bundle_digest,
        plugin_claims_must_reference_bundle_digest,
        platform_claims_must_reference_bundle_digest,
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        next_issue_id: String::from(NEXT_ISSUE_ID),
        plugin_publication_allowed: false,
        served_public_universality_allowed: false,
        arbitrary_software_capability_allowed: false,
        claim_boundary: String::from(
            "this eval-owned report publishes one digest-bound canonical machine closure bundle for the post-article route. It binds the historical proof chain, the canonical machine tuple, the published computational model, the control-plane provenance proof, execution-semantics transport, continuation boundary, carrier split, state classes, hidden-state closure, failure lattice, observer model, portability/minimality posture, plugin charter boundary, and anti-drift closeout into one indivisible machine object. Later terminal universality, weighted plugin controller, plugin invocation receipt, plugin publication, and bounded plugin-platform claims must reference this closure bundle by digest instead of silently recomposing the machine. It still does not by itself publish plugin catalogs, plugin publication, served/public universality, or arbitrary software capability.",
        ),
        summary: String::new(),
        closure_bundle_digest: String::new(),
        report_digest: String::new(),
    };

    report.closure_bundle_digest = stable_closure_bundle_digest(&report);
    report.summary = format!(
        "Post-article canonical machine closure bundle keeps supporting_material_rows={}/{}, dependency_rows={}/{}, invalidation_rows={}/{}, validation_rows={}/{}, bundle_status={:?}, closure_bundle_digest=`{}`, closure_bundle_issue_id=`{}`, and next_issue_id=`{}`.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.supporting_material_rows.len(),
        report.dependency_rows.iter().filter(|row| row.satisfied).count(),
        report.dependency_rows.len(),
        report
            .invalidation_rows
            .iter()
            .filter(|row| row.present)
            .count(),
        report.invalidation_rows.len(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.bundle_status,
        report.closure_bundle_digest,
        report.closure_bundle_issue_id,
        report.next_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_machine_closure_bundle_report|",
        &report,
    );
    Ok(report)
}

fn classification_green(
    row: &TassadarPostArticleCanonicalMachineClosureBundleArtifactClassRow,
    runtime_contract: &TassadarTcmV1RuntimeContractReport,
    historical_proof: &TassadarUniversalMachineProofReport,
    historical_witness_suite: &TassadarUniversalityWitnessSuiteReport,
    historical_verdict_split: &TassadarUniversalityVerdictSplitReport,
    machine_lock: &TassadarPostArticleCanonicalMachineIdentityLockReport,
    computational_model: &TassadarPostArticleCanonicalComputationalModelStatementReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    proof_transport: &TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    continuation: &TassadarPostArticleContinuationNonComputationalityContractReport,
    fast_route: &TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    equivalent_choice: &TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    downward: &TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
    portability: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    plugin_charter: &TassadarPostArticlePluginCharterAuthorityBoundaryReport,
    anti_drift: &TassadarPostArticleAntiDriftStabilityCloseoutAuditReport,
) -> bool {
    match row.artifact_id.as_str() {
        "tcm_v1_runtime_contract" => runtime_contract.overall_green,
        "historical_universal_machine_proof" => historical_proof.overall_green,
        "historical_universality_witness_suite" => historical_witness_suite.overall_green,
        "historical_universality_verdict_split" => {
            historical_verdict_split.theory_green && historical_verdict_split.operator_green
        }
        "canonical_machine_identity_lock" => machine_lock.lock_green,
        "canonical_computational_model_statement" => computational_model.statement_green,
        "control_plane_decision_provenance_proof" => {
            control_plane.decision_provenance_proof_complete
        }
        "execution_semantics_proof_transport_audit" => {
            proof_transport.audit_green && proof_transport.proof_transport_complete
        }
        "continuation_non_computationality_contract" => {
            continuation.contract_green && continuation.continuation_non_computationality_complete
        }
        "fast_route_legitimacy_and_carrier_binding_contract" => {
            fast_route.contract_green && fast_route.fast_route_legitimacy_complete
        }
        "equivalent_choice_neutrality_and_admissibility_contract" => {
            equivalent_choice.contract_green
                && equivalent_choice.equivalent_choice_neutrality_complete
        }
        "downward_non_influence_and_served_conformance_contract" => {
            downward.contract_green && downward.downward_non_influence_complete
        }
        "universality_portability_minimality_matrix" => portability.matrix_green,
        "plugin_charter_authority_boundary" => plugin_charter.charter_green,
        "anti_drift_stability_closeout" => anti_drift.closeout_green,
        _ => false,
    }
}

fn supporting_row(
    material_id: &str,
    material_class: TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialRow {
    TassadarPostArticleCanonicalMachineClosureBundleSupportingMaterialRow {
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
) -> TassadarPostArticleCanonicalMachineClosureBundleDependencyRow {
    TassadarPostArticleCanonicalMachineClosureBundleDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs,
        detail: String::from(detail),
    }
}

fn invalidation_row_from_law(
    law: &TassadarPostArticleCanonicalMachineClosureBundleInvalidationLaw,
) -> TassadarPostArticleCanonicalMachineClosureBundleInvalidationRow {
    TassadarPostArticleCanonicalMachineClosureBundleInvalidationRow {
        invalidation_id: law.invalidation_id.clone(),
        present: true,
        source_refs: vec![String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF)],
        detail: law.detail.clone(),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineClosureBundleValidationRow {
    TassadarPostArticleCanonicalMachineClosureBundleValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_post_article_canonical_machine_closure_bundle_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF)
}

pub fn write_tassadar_post_article_canonical_machine_closure_bundle_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalMachineClosureBundleReport,
    TassadarPostArticleCanonicalMachineClosureBundleReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalMachineClosureBundleReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_canonical_machine_closure_bundle_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalMachineClosureBundleReportError::Write {
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

fn stable_closure_bundle_digest(
    report: &TassadarPostArticleCanonicalMachineClosureBundleReport,
) -> String {
    let digest_view = serde_json::json!({
        "schema_version": report.schema_version,
        "report_id": report.report_id,
        "closure_bundle_contract": report.closure_bundle_contract,
        "supporting_material_rows": report.supporting_material_rows,
        "dependency_rows": report.dependency_rows,
        "closure_subject": report.closure_subject,
        "invalidation_rows": report.invalidation_rows,
        "validation_rows": report.validation_rows,
        "bundle_status": report.bundle_status,
        "bundle_green": report.bundle_green,
        "proof_and_audit_classification_complete": report.proof_and_audit_classification_complete,
        "machine_subject_complete": report.machine_subject_complete,
        "control_execution_and_continuation_bound": report.control_execution_and_continuation_bound,
        "hidden_state_and_observer_model_bound": report.hidden_state_and_observer_model_bound,
        "portability_and_minimality_bound": report.portability_and_minimality_bound,
        "anti_drift_closeout_inherited": report.anti_drift_closeout_inherited,
        "terminal_claims_must_reference_bundle_digest": report.terminal_claims_must_reference_bundle_digest,
        "plugin_claims_must_reference_bundle_digest": report.plugin_claims_must_reference_bundle_digest,
        "platform_claims_must_reference_bundle_digest": report.platform_claims_must_reference_bundle_digest,
        "closure_bundle_issue_id": report.closure_bundle_issue_id,
        "next_issue_id": report.next_issue_id,
    });
    stable_digest(
        b"psionic_tassadar_post_article_canonical_machine_closure_bundle_digest|",
        &digest_view,
    )
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleCanonicalMachineClosureBundleReportError> {
    let path = path.as_ref();
    let resolved_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root().join(path)
    };
    let bytes = fs::read(&resolved_path).map_err(|error| {
        TassadarPostArticleCanonicalMachineClosureBundleReportError::Read {
            path: resolved_path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalMachineClosureBundleReportError::Deserialize {
            path: resolved_path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_machine_closure_bundle_report, read_json,
        tassadar_post_article_canonical_machine_closure_bundle_report_path,
        write_tassadar_post_article_canonical_machine_closure_bundle_report,
        TassadarPostArticleCanonicalMachineClosureBundleReport,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
    };

    #[test]
    fn canonical_machine_closure_bundle_turns_green_when_prereqs_hold() {
        let report =
            build_tassadar_post_article_canonical_machine_closure_bundle_report().expect("report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article_canonical_machine_closure_bundle.report.v1"
        );
        assert_eq!(
            report.closure_subject.closure_bundle_id,
            "tassadar.post_article.canonical_machine.closure_bundle.v1"
        );
        assert!(report.bundle_green);
        assert!(report.proof_and_audit_classification_complete);
        assert!(report.machine_subject_complete);
        assert!(report.control_execution_and_continuation_bound);
        assert!(report.hidden_state_and_observer_model_bound);
        assert!(report.portability_and_minimality_bound);
        assert!(report.anti_drift_closeout_inherited);
        assert!(report.terminal_claims_must_reference_bundle_digest);
        assert!(report.plugin_claims_must_reference_bundle_digest);
        assert!(report.platform_claims_must_reference_bundle_digest);
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
        assert_eq!(report.next_issue_id, "TAS-217");
        assert!(!report.closure_bundle_digest.is_empty());
    }

    #[test]
    fn canonical_machine_closure_bundle_matches_committed_truth() {
        let expected = build_tassadar_post_article_canonical_machine_closure_bundle_report()
            .expect("expected");
        let committed: TassadarPostArticleCanonicalMachineClosureBundleReport =
            read_json(tassadar_post_article_canonical_machine_closure_bundle_report_path())
                .expect("committed");

        assert_eq!(committed, expected);
        assert!(
            tassadar_post_article_canonical_machine_closure_bundle_report_path()
                .ends_with("tassadar_post_article_canonical_machine_closure_bundle_report.json")
        );
    }

    #[test]
    fn write_canonical_machine_closure_bundle_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_post_article_canonical_machine_closure_bundle_report.json");
        let written =
            write_tassadar_post_article_canonical_machine_closure_bundle_report(&output_path)
                .expect("written");
        let roundtrip: TassadarPostArticleCanonicalMachineClosureBundleReport =
            read_json(&output_path).expect("roundtrip");

        assert_eq!(written, roundtrip);
        assert_eq!(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_report.json"
        );
    }
}
