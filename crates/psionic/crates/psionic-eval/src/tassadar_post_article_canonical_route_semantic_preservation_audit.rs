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
    build_tassadar_installed_process_lifecycle_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_session_process_profile_report, build_tassadar_spill_tape_store_report,
    TassadarInstalledProcessLifecycleReport, TassadarInstalledProcessLifecycleReportError,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError, TassadarSessionProcessProfileReport,
    TassadarSessionProcessProfileReportError, TassadarSpillTapeStoreReport,
    TassadarSpillTapeStoreReportError, TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF, TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_route_semantic_preservation_audit_report.json";
pub const TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-canonical-route-semantic-preservation-audit.sh";

const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const WEIGHTS_OWNED_STATE_CLASS_ID: &str = "weights_owned_control_state";
const EPHEMERAL_STATE_CLASS_ID: &str = "ephemeral_execution_state";
const RESUMED_STATE_CLASS_ID: &str = "resumed_continuation_state";
const DURABLE_STATE_CLASS_ID: &str = "durable_receipt_backed_state";
const UNDECLARED_STATE_CLASS_ID: &str = "undeclared_workflow_state_refused";
const SESSION_PROCESS_MECHANISM_ID: &str = "session_process_profile_semantic_preservation";
const SPILL_TAPE_MECHANISM_ID: &str = "spill_tape_store_semantic_preservation";
const INSTALLED_PROCESS_MECHANISM_ID: &str = "installed_process_lifecycle_semantic_preservation";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalRouteSemanticPreservationStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleSupportingMaterialClass {
    ProofCarrying,
    ObservationalContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleStateClassKind {
    WeightsOwned,
    Ephemeral,
    Resumed,
    Durable,
    UndeclaredWorkflowStateRefused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleContinuationMechanismKind {
    SessionProcess,
    SpillTapeStore,
    InstalledProcessLifecycle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalIdentityReview {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub same_canonical_identity_required_across_resume: bool,
    pub route_identity_mutation_allowed: bool,
    pub model_identity_mutation_allowed: bool,
    pub canonical_identity_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleControlOwnershipBoundaryReview {
    pub host_executes_declared_mechanics: bool,
    pub host_decides_workflow: bool,
    pub control_ownership_rule_green: bool,
    pub decision_provenance_proof_complete: bool,
    pub deferred_issue_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStateClassRow {
    pub state_class_id: String,
    pub class_kind: TassadarPostArticleStateClassKind,
    pub current_posture: String,
    pub allowed_owners: Vec<String>,
    pub workflow_authority_allowed: bool,
    pub declared_artifact_refs: Vec<String>,
    pub state_semantics: String,
    pub state_class_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationMechanismRow {
    pub mechanism_id: String,
    pub mechanism_kind: TassadarPostArticleContinuationMechanismKind,
    pub source_report_ref: String,
    pub source_report_id: String,
    pub source_report_digest: String,
    pub canonical_machine_identity_id: String,
    pub preserved_model_identity: bool,
    pub preserved_route_identity: bool,
    pub exact_state_parity_green: bool,
    pub exact_output_parity_green: bool,
    pub typed_refusal_green: bool,
    pub expressivity_extension_blocked: bool,
    pub supporting_state_class_ids: Vec<String>,
    pub semantic_preservation_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleSemanticPreservationValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub bridge_contract_report_ref: String,
    pub session_process_profile_report_ref: String,
    pub spill_tape_store_report_ref: String,
    pub installed_process_lifecycle_report_ref: String,
    pub supporting_material_rows: Vec<TassadarPostArticleSupportingMaterialRow>,
    pub canonical_identity_review: TassadarPostArticleCanonicalIdentityReview,
    pub control_ownership_boundary_review: TassadarPostArticleControlOwnershipBoundaryReview,
    pub state_class_rows: Vec<TassadarPostArticleStateClassRow>,
    pub continuation_mechanism_rows: Vec<TassadarPostArticleContinuationMechanismRow>,
    pub validation_rows: Vec<TassadarPostArticleSemanticPreservationValidationRow>,
    pub state_ownership_green: bool,
    pub semantic_preservation_green: bool,
    pub semantic_preservation_status: TassadarPostArticleCanonicalRouteSemanticPreservationStatus,
    pub semantic_preservation_audit_green: bool,
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
pub enum TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError {
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
    #[error(transparent)]
    SessionProcess(#[from] TassadarSessionProcessProfileReportError),
    #[error(transparent)]
    SpillTapeStore(#[from] TassadarSpillTapeStoreReportError),
    #[error(transparent)]
    InstalledProcessLifecycle(#[from] TassadarInstalledProcessLifecycleReportError),
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

pub fn build_tassadar_post_article_canonical_route_semantic_preservation_audit_report() -> Result<
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
> {
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let session_process = build_tassadar_session_process_profile_report()?;
    let spill_tape_store = build_tassadar_spill_tape_store_report()?;
    let installed_process = build_tassadar_installed_process_lifecycle_report()?;

    let supporting_material_rows = build_supporting_material_rows(
        &bridge,
        &session_process,
        &spill_tape_store,
        &installed_process,
    );
    let canonical_identity_review = build_canonical_identity_review(&bridge);
    let control_ownership_boundary_review = build_control_ownership_boundary_review(
        &bridge,
        &session_process,
        &spill_tape_store,
        &installed_process,
    );
    let state_class_rows = build_state_class_rows(
        &bridge,
        &session_process,
        &spill_tape_store,
        &installed_process,
    );
    let continuation_mechanism_rows = build_continuation_mechanism_rows(
        &bridge,
        &session_process,
        &spill_tape_store,
        &installed_process,
    );

    let state_ownership_green = state_class_rows.iter().all(|row| row.state_class_green);
    let semantic_preservation_green = continuation_mechanism_rows
        .iter()
        .all(|row| row.semantic_preservation_green);
    let deferred_issue_ids = vec![String::from("TAS-188A"), String::from("TAS-189")];
    let decision_provenance_proof_complete =
        control_ownership_boundary_review.decision_provenance_proof_complete;
    let carrier_split_publication_complete = false;
    let rebase_claim_allowed = false;
    let plugin_capability_claim_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let validation_rows = build_validation_rows(
        &bridge,
        &supporting_material_rows,
        &state_class_rows,
        &continuation_mechanism_rows,
        &canonical_identity_review,
        &control_ownership_boundary_review,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        carrier_split_publication_complete,
    );

    let semantic_preservation_audit_green =
        supporting_material_rows.iter().all(|row| row.satisfied)
            && canonical_identity_review.canonical_identity_green
            && control_ownership_boundary_review.control_ownership_rule_green
            && state_ownership_green
            && semantic_preservation_green
            && validation_rows.iter().all(|row| row.green)
            && !decision_provenance_proof_complete
            && !carrier_split_publication_complete
            && !rebase_claim_allowed
            && !plugin_capability_claim_allowed
            && !served_public_universality_allowed
            && !arbitrary_software_capability_allowed;

    let semantic_preservation_status = if semantic_preservation_audit_green {
        TassadarPostArticleCanonicalRouteSemanticPreservationStatus::Green
    } else {
        TassadarPostArticleCanonicalRouteSemanticPreservationStatus::Blocked
    };

    let mut report = TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_canonical_route_semantic_preservation_audit.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_CHECKER_REF,
        ),
        bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
        ),
        session_process_profile_report_ref: String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
        spill_tape_store_report_ref: String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
        installed_process_lifecycle_report_ref: String::from(
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
        ),
        supporting_material_rows,
        canonical_identity_review,
        control_ownership_boundary_review,
        state_class_rows,
        continuation_mechanism_rows,
        validation_rows,
        state_ownership_green,
        semantic_preservation_green,
        semantic_preservation_status,
        semantic_preservation_audit_green,
        decision_provenance_proof_complete,
        carrier_split_publication_complete,
        deferred_issue_ids,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        claim_boundary: String::from(
            "this closeout audit proves only the post-`TAS-186` semantic-preservation and state-ownership tranche for continuation mechanics on the canonical bridge machine identity. It freezes canonical model and route identity across declared resume mechanics, classifies weights-owned versus ephemeral versus resumed versus durable state, and refuses undeclared workflow state outside those classes. It does not by itself prove branch/retry/stop decision provenance, publish the final direct-versus-resumable carrier split, allow the rebased Turing-completeness claim, allow weighted plugin control, allow served/public universality, or allow arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article canonical-route semantic-preservation audit keeps supporting_materials={}/7, state_classes={}/5, continuation_mechanisms={}/3, validation_rows={}/6, semantic_preservation_status={:?}, decision_provenance_proof_complete={}, and carrier_split_publication_complete={}.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report
            .state_class_rows
            .iter()
            .filter(|row| row.state_class_green)
            .count(),
        report
            .continuation_mechanism_rows
            .iter()
            .filter(|row| row.semantic_preservation_green)
            .count(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.semantic_preservation_status,
        report.decision_provenance_proof_complete,
        report.carrier_split_publication_complete,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_route_semantic_preservation_audit_report|",
        &report,
    );
    Ok(report)
}

fn build_supporting_material_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    session_process: &TassadarSessionProcessProfileReport,
    spill_tape_store: &TassadarSpillTapeStoreReport,
    installed_process: &TassadarInstalledProcessLifecycleReport,
) -> Vec<TassadarPostArticleSupportingMaterialRow> {
    vec![
        supporting_material_row(
            "bridge_contract",
            TassadarPostArticleSupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge contract must stay green before semantic-preservation closure can bind continuation mechanics to the canonical machine identity",
        ),
        supporting_material_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleSupportingMaterialClass::ProofCarrying,
            bridge.tcm_v1_runtime_contract_report.overall_green,
            &bridge.tcm_v1_runtime_contract_report_ref,
            Some(bridge.tcm_v1_runtime_contract_report.report_id.clone()),
            Some(bridge.tcm_v1_runtime_contract_report.report_digest.clone()),
            "the declared `TCM.v1` continuation contract must stay green before resumed state can be classified or carried forward under the bridge",
        ),
        supporting_material_row(
            "session_process_profile",
            TassadarPostArticleSupportingMaterialClass::ProofCarrying,
            session_process.overall_green,
            TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
            Some(session_process.report_id.clone()),
            Some(session_process.report_digest.clone()),
            "the session-process profile must stay green so bounded interactive resume semantics remain explicit and open-ended external loops remain refused",
        ),
        supporting_material_row(
            "spill_tape_store_profile",
            TassadarPostArticleSupportingMaterialClass::ProofCarrying,
            spill_tape_store.exact_case_count == 2 && spill_tape_store.refusal_case_count == 3,
            TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
            Some(spill_tape_store.report_id.clone()),
            Some(spill_tape_store.report_digest.clone()),
            "the spill/tape profile must keep exact parity rows and typed refusal rows explicit so spill-backed resume does not widen into arbitrary persistent semantics",
        ),
        supporting_material_row(
            "installed_process_lifecycle",
            TassadarPostArticleSupportingMaterialClass::ProofCarrying,
            installed_process.overall_green && !installed_process.served_publication_allowed,
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            Some(installed_process.report_id.clone()),
            Some(installed_process.report_digest.clone()),
            "the installed-process lifecycle must stay green and operator-only so durable snapshot and rollback semantics remain explicit without widening into served/public lifecycle control",
        ),
        supporting_material_row(
            "post_article_turing_completeness_audit_context",
            TassadarPostArticleSupportingMaterialClass::ObservationalContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 rebase audit remains context only here; it motivates the tranche but does not substitute for proof-carrying semantic-preservation artifacts",
        ),
        supporting_material_row(
            "plugin_system_turing_completeness_audit_context",
            TassadarPostArticleSupportingMaterialClass::ObservationalContext,
            true,
            PLUGIN_SYSTEM_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 plugin audit remains context only here; it keeps the later capability boundary visible without substituting for proof-carrying semantic-preservation artifacts",
        ),
    ]
}

fn build_canonical_identity_review(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
) -> TassadarPostArticleCanonicalIdentityReview {
    let canonical_identity_green = bridge.bridge_contract_green
        && !bridge
            .bridge_machine_identity
            .machine_identity_id
            .is_empty()
        && !bridge.bridge_machine_identity.canonical_model_id.is_empty()
        && !bridge.bridge_machine_identity.canonical_route_id.is_empty()
        && !bridge
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .is_empty()
        && bridge.bridge_machine_identity.continuation_contract_id
            == bridge.tcm_v1_runtime_contract_report.report_id
        && bridge.bridge_machine_identity.continuation_contract_digest
            == bridge.tcm_v1_runtime_contract_report.report_digest;

    TassadarPostArticleCanonicalIdentityReview {
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        canonical_route_descriptor_digest: bridge
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .clone(),
        continuation_contract_id: bridge.bridge_machine_identity.continuation_contract_id.clone(),
        continuation_contract_digest: bridge
            .bridge_machine_identity
            .continuation_contract_digest
            .clone(),
        same_canonical_identity_required_across_resume: true,
        route_identity_mutation_allowed: false,
        model_identity_mutation_allowed: false,
        canonical_identity_green,
        detail: format!(
            "machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` and continuation_contract_id=`{}` remain the required identity tuple across declared continuation mechanics; route_identity_mutation_allowed=false and model_identity_mutation_allowed=false.",
            bridge.bridge_machine_identity.machine_identity_id,
            bridge.bridge_machine_identity.canonical_model_id,
            bridge.bridge_machine_identity.canonical_route_id,
            bridge.bridge_machine_identity.continuation_contract_id,
        ),
    }
}

fn build_control_ownership_boundary_review(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    session_process: &TassadarSessionProcessProfileReport,
    spill_tape_store: &TassadarSpillTapeStoreReport,
    installed_process: &TassadarInstalledProcessLifecycleReport,
) -> TassadarPostArticleControlOwnershipBoundaryReview {
    let session_refusal_green = session_process
        .refused_interaction_surface_ids
        .iter()
        .any(|id| id == "open_ended_external_event_stream");
    let spill_refusal_green = spill_tape_store.refusal_case_count == 3;
    let installed_refusal_green =
        installed_process.refusal_case_count == 3 && !installed_process.served_publication_allowed;
    let control_ownership_rule_green =
        bridge_validation_green(bridge, "continuation_abuse_quarantined")
            && session_refusal_green
            && spill_refusal_green
            && installed_refusal_green;

    TassadarPostArticleControlOwnershipBoundaryReview {
        host_executes_declared_mechanics: true,
        host_decides_workflow: false,
        control_ownership_rule_green,
        decision_provenance_proof_complete: false,
        deferred_issue_ids: vec![String::from("TAS-188A")],
        detail: String::from(
            "host may execute declared resume, spill, migration, and rollback mechanics under explicit receipts and contracts, but host_decides_workflow=false stays frozen here. Full branch/retry/stop decision provenance is deferred to `TAS-188A` instead of being implied by this semantic-preservation tranche.",
        ),
    }
}

fn build_state_class_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    session_process: &TassadarSessionProcessProfileReport,
    spill_tape_store: &TassadarSpillTapeStoreReport,
    installed_process: &TassadarInstalledProcessLifecycleReport,
) -> Vec<TassadarPostArticleStateClassRow> {
    let resumed_state_green = session_process.overall_green
        && spill_tape_store.exact_case_count == 2
        && installed_process.overall_green;
    let durable_state_green =
        spill_tape_store.refusal_case_count == 3 && installed_process.refusal_case_count == 3;
    let undeclared_state_green = session_process
        .refused_interaction_surface_ids
        .iter()
        .any(|id| id == "open_ended_external_event_stream")
        && spill_tape_store.refusal_case_count == 3
        && installed_process.refusal_case_count == 3;

    vec![
        TassadarPostArticleStateClassRow {
            state_class_id: String::from(WEIGHTS_OWNED_STATE_CLASS_ID),
            class_kind: TassadarPostArticleStateClassKind::WeightsOwned,
            current_posture: String::from("implemented"),
            allowed_owners: vec![
                String::from("canonical_model_weights"),
                String::from("canonical_route_identity"),
            ],
            workflow_authority_allowed: true,
            declared_artifact_refs: vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            )],
            state_semantics: String::from(
                "workflow meaning, transition semantics, and canonical-route identity remain weight-owned on the bridge machine identity",
            ),
            state_class_green: bridge.bridge_contract_green,
            detail: format!(
                "weights-owned control state is frozen on canonical_model_id=`{}` and canonical_route_id=`{}` under machine_identity_id=`{}`.",
                bridge.bridge_machine_identity.canonical_model_id,
                bridge.bridge_machine_identity.canonical_route_id,
                bridge.bridge_machine_identity.machine_identity_id,
            ),
        },
        TassadarPostArticleStateClassRow {
            state_class_id: String::from(EPHEMERAL_STATE_CLASS_ID),
            class_kind: TassadarPostArticleStateClassKind::Ephemeral,
            current_posture: String::from("implemented"),
            allowed_owners: vec![String::from("canonical_route_runtime")],
            workflow_authority_allowed: false,
            declared_artifact_refs: vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
            ],
            state_semantics: String::from(
                "step-local runtime buffers may execute declared mechanics, but they do not become durable workflow authority or a hidden fifth state class",
            ),
            state_class_green: bridge_validation_green(bridge, "continuation_abuse_quarantined"),
            detail: String::from(
                "ephemeral execution state is admitted only as runtime-local mechanism support. It is inferred from the bridge contract's direct-carrier exclusions plus the explicit resumed and durable classes, and it may not decide workflow on its own.",
            ),
        },
        TassadarPostArticleStateClassRow {
            state_class_id: String::from(RESUMED_STATE_CLASS_ID),
            class_kind: TassadarPostArticleStateClassKind::Resumed,
            current_posture: String::from("implemented"),
            allowed_owners: vec![
                String::from("tcm_v1_continuation_contract"),
                String::from("declared_resume_objects"),
            ],
            workflow_authority_allowed: false,
            declared_artifact_refs: vec![
                String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
                String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
                String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF),
            ],
            state_semantics: String::from(
                "explicit resumed state extends execution under the declared continuation contract without rebinding workflow ownership away from the canonical machine identity",
            ),
            state_class_green: resumed_state_green,
            detail: String::from(
                "resumed continuation state stays explicit across deterministic session turns, spill/tape resume, and installed-process migration or rollback; exact parity and typed refusal rows keep the class bounded instead of turning it into ambient workflow state.",
            ),
        },
        TassadarPostArticleStateClassRow {
            state_class_id: String::from(DURABLE_STATE_CLASS_ID),
            class_kind: TassadarPostArticleStateClassKind::Durable,
            current_posture: String::from("implemented"),
            allowed_owners: vec![String::from("receipt_backed_host_storage")],
            workflow_authority_allowed: false,
            declared_artifact_refs: vec![
                String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
                String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF),
            ],
            state_semantics: String::from(
                "manifests, tape segments, snapshots, and migration or rollback receipts may persist declared continuation state, but they do not decide what the workflow should do next",
            ),
            state_class_green: durable_state_green,
            detail: String::from(
                "durable state remains receipt-backed and typed. Missing segments, portability mismatches, stale snapshots, and missing lineage stay explicit refusal rows rather than becoming silent state-authority widening.",
            ),
        },
        TassadarPostArticleStateClassRow {
            state_class_id: String::from(UNDECLARED_STATE_CLASS_ID),
            class_kind: TassadarPostArticleStateClassKind::UndeclaredWorkflowStateRefused,
            current_posture: String::from("refused"),
            allowed_owners: Vec::new(),
            workflow_authority_allowed: false,
            declared_artifact_refs: vec![
                String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
                String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
                String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF),
            ],
            state_semantics: String::from(
                "any workflow-affecting state outside the declared weights-owned, ephemeral, resumed, and durable classes is out of model and must fail closed",
            ),
            state_class_green: undeclared_state_green,
            detail: String::from(
                "undeclared workflow state remains refused: open-ended external event streams stay out of envelope, oversize or missing continuation artifacts stay typed refusals, and stale or portability-invalid lifecycle state stays explicitly blocked.",
            ),
        },
    ]
}

fn build_continuation_mechanism_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    session_process: &TassadarSessionProcessProfileReport,
    spill_tape_store: &TassadarSpillTapeStoreReport,
    installed_process: &TassadarInstalledProcessLifecycleReport,
) -> Vec<TassadarPostArticleContinuationMechanismRow> {
    let session_exact_state_parity_green = session_process
        .case_audits
        .iter()
        .all(|case| case.exact_state_parity);
    let session_exact_output_parity_green = session_process
        .case_audits
        .iter()
        .all(|case| case.exact_output_parity);
    let session_typed_refusal_green = session_process
        .refused_interaction_surface_ids
        .iter()
        .any(|id| id == "open_ended_external_event_stream");
    let session_expressivity_extension_blocked =
        session_typed_refusal_green && session_process.routeable_interaction_surface_ids.len() == 2;
    let session_semantic_preservation_green = session_process.overall_green
        && session_exact_state_parity_green
        && session_exact_output_parity_green
        && session_typed_refusal_green
        && session_expressivity_extension_blocked;

    let exact_spill_case_reports = spill_tape_store
        .case_reports
        .iter()
        .filter(|case| {
            case.status == psionic_runtime::TassadarSpillTapeCaseStatus::ExactSpillAndResumeParity
        })
        .collect::<Vec<_>>();
    let spill_exact_state_parity_green = exact_spill_case_reports
        .iter()
        .all(|case| case.spill_vs_in_core_parity);
    let spill_exact_output_parity_green = exact_spill_case_reports
        .iter()
        .all(|case| case.external_tape_resume_parity);
    let spill_refusal_kinds = spill_tape_store
        .case_reports
        .iter()
        .flat_map(|case| case.refusal_kinds.iter().map(|kind| format!("{kind:?}")))
        .collect::<BTreeSet<_>>();
    let spill_typed_refusal_green = spill_refusal_kinds.contains("MissingExternalTapeSegment")
        && spill_refusal_kinds.contains("OversizeStateOutOfEnvelope")
        && spill_refusal_kinds.contains("PortabilityEnvelopeMismatch");
    let spill_expressivity_extension_blocked = spill_typed_refusal_green
        && spill_tape_store.portability_envelope_ids
            == [String::from("cpu_reference_current_host")];
    let spill_semantic_preservation_green = spill_tape_store.exact_case_count == 2
        && spill_exact_state_parity_green
        && spill_exact_output_parity_green
        && spill_typed_refusal_green
        && spill_expressivity_extension_blocked;

    let installed_exact_migration_case_count = installed_process
        .case_reports
        .iter()
        .filter(|case| case.exact_migration_parity)
        .count() as u32;
    let installed_exact_rollback_case_count = installed_process
        .case_reports
        .iter()
        .filter(|case| case.exact_rollback_parity)
        .count() as u32;
    let installed_exact_state_parity_green =
        installed_exact_migration_case_count == 1 && installed_exact_rollback_case_count == 1;
    let installed_exact_output_parity_green = installed_process.overall_green;
    let installed_typed_refusal_green =
        installed_process.refusal_case_count == 3 && !installed_process.served_publication_allowed;
    let installed_expressivity_extension_blocked = !installed_process.served_publication_allowed;
    let installed_semantic_preservation_green = installed_process.overall_green
        && installed_exact_state_parity_green
        && installed_exact_output_parity_green
        && installed_typed_refusal_green
        && installed_expressivity_extension_blocked;

    vec![
        TassadarPostArticleContinuationMechanismRow {
            mechanism_id: String::from(SESSION_PROCESS_MECHANISM_ID),
            mechanism_kind: TassadarPostArticleContinuationMechanismKind::SessionProcess,
            source_report_ref: String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
            source_report_id: session_process.report_id.clone(),
            source_report_digest: session_process.report_digest.clone(),
            canonical_machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
            preserved_model_identity: true,
            preserved_route_identity: true,
            exact_state_parity_green: session_exact_state_parity_green,
            exact_output_parity_green: session_exact_output_parity_green,
            typed_refusal_green: session_typed_refusal_green,
            expressivity_extension_blocked: session_expressivity_extension_blocked,
            supporting_state_class_ids: vec![
                String::from(EPHEMERAL_STATE_CLASS_ID),
                String::from(RESUMED_STATE_CLASS_ID),
            ],
            semantic_preservation_green: session_semantic_preservation_green,
            detail: String::from(
                "deterministic session turns keep exact state and output parity across admitted surfaces while explicitly refusing the open-ended external-event loop, so continuation extends execution without silently widening the canonical route into generic workflow control.",
            ),
        },
        TassadarPostArticleContinuationMechanismRow {
            mechanism_id: String::from(SPILL_TAPE_MECHANISM_ID),
            mechanism_kind: TassadarPostArticleContinuationMechanismKind::SpillTapeStore,
            source_report_ref: String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
            source_report_id: spill_tape_store.report_id.clone(),
            source_report_digest: spill_tape_store.report_digest.clone(),
            canonical_machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
            preserved_model_identity: true,
            preserved_route_identity: true,
            exact_state_parity_green: spill_exact_state_parity_green,
            exact_output_parity_green: spill_exact_output_parity_green,
            typed_refusal_green: spill_typed_refusal_green,
            expressivity_extension_blocked: spill_expressivity_extension_blocked,
            supporting_state_class_ids: vec![
                String::from(RESUMED_STATE_CLASS_ID),
                String::from(DURABLE_STATE_CLASS_ID),
            ],
            semantic_preservation_green: spill_semantic_preservation_green,
            detail: String::from(
                "spill-backed resume keeps in-core versus spilled state parity and external-tape resume parity explicit on declared workloads, while missing segments, portability mismatches, and oversize state remain typed refusals instead of becoming hidden expressivity extensions.",
            ),
        },
        TassadarPostArticleContinuationMechanismRow {
            mechanism_id: String::from(INSTALLED_PROCESS_MECHANISM_ID),
            mechanism_kind:
                TassadarPostArticleContinuationMechanismKind::InstalledProcessLifecycle,
            source_report_ref: String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF),
            source_report_id: installed_process.report_id.clone(),
            source_report_digest: installed_process.report_digest.clone(),
            canonical_machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
            preserved_model_identity: true,
            preserved_route_identity: true,
            exact_state_parity_green: installed_exact_state_parity_green,
            exact_output_parity_green: installed_exact_output_parity_green,
            typed_refusal_green: installed_typed_refusal_green,
            expressivity_extension_blocked: installed_expressivity_extension_blocked,
            supporting_state_class_ids: vec![
                String::from(RESUMED_STATE_CLASS_ID),
                String::from(DURABLE_STATE_CLASS_ID),
            ],
            semantic_preservation_green: installed_semantic_preservation_green,
            detail: String::from(
                "installed-process snapshots, migration receipts, and rollback receipts preserve declared lifecycle semantics under exact migration or rollback parity while stale snapshots, portability mismatches, missing lineage, and served-publication widening remain explicitly blocked.",
            ),
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    supporting_material_rows: &[TassadarPostArticleSupportingMaterialRow],
    state_class_rows: &[TassadarPostArticleStateClassRow],
    continuation_mechanism_rows: &[TassadarPostArticleContinuationMechanismRow],
    canonical_identity_review: &TassadarPostArticleCanonicalIdentityReview,
    control_ownership_boundary_review: &TassadarPostArticleControlOwnershipBoundaryReview,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
    carrier_split_publication_complete: bool,
) -> Vec<TassadarPostArticleSemanticPreservationValidationRow> {
    let proof_carrying_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class == TassadarPostArticleSupportingMaterialClass::ProofCarrying
        })
        .count();
    let observational_context_count = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class == TassadarPostArticleSupportingMaterialClass::ObservationalContext
        })
        .count();
    let undeclared_state_green = state_class_rows
        .iter()
        .find(|row| row.state_class_id == UNDECLARED_STATE_CLASS_ID)
        .map(|row| row.state_class_green)
        .unwrap_or(false);

    vec![
        validation_row(
            "helper_substitution_quarantined",
            bridge_validation_green(bridge, "helper_substitution_quarantined"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            )],
            "helper substitution remains quarantined on the canonical bridge machine identity, so semantic-preservation closure does not inherit a hidden host-helper lane",
        ),
        validation_row(
            "route_drift_rejected",
            bridge_validation_green(bridge, "route_drift_rejected")
                && canonical_identity_review.canonical_identity_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            )],
            "route drift remains rejected because every continuation mechanism is audited against one fixed machine identity tuple instead of a drifting route family",
        ),
        validation_row(
            "continuation_abuse_quarantined",
            bridge_validation_green(bridge, "continuation_abuse_quarantined")
                && undeclared_state_green
                && control_ownership_boundary_review.control_ownership_rule_green,
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
                String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
                String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF),
            ],
            "continuation abuse remains quarantined because undeclared workflow state stays refused and host mechanics stay non-authoritative under declared session, spill, and lifecycle envelopes",
        ),
        validation_row(
            "semantic_drift_blocked",
            bridge_validation_green(bridge, "semantic_drift_blocked")
                && continuation_mechanism_rows
                    .iter()
                    .all(|row| row.semantic_preservation_green),
            vec![
                String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
                String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
                String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF),
            ],
            "semantic drift remains blocked because every admitted continuation mechanism keeps exact state or lifecycle parity plus typed refusal boundaries instead of merely matching a subset of outputs",
        ),
        validation_row(
            "proof_class_distinction_preserved",
            proof_carrying_count == 5 && observational_context_count == 2,
            vec![
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
            ],
            "proof-carrying artifacts and observational audits remain distinct classes in this closeout audit instead of being collapsed into one undifferentiated evidence bucket",
        ),
        validation_row(
            "overclaim_posture_explicit",
            !control_ownership_boundary_review.decision_provenance_proof_complete
                && !carrier_split_publication_complete
                && !rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
            ],
            "the semantic-preservation tranche remains bounded: decision provenance, carrier split publication, rebased universality, plugin control, served/public universality, and arbitrary software capability all stay explicitly out of scope",
        ),
    ]
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_report_id: Option<String>,
    source_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleSupportingMaterialRow {
    TassadarPostArticleSupportingMaterialRow {
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
) -> TassadarPostArticleSemanticPreservationValidationRow {
    TassadarPostArticleSemanticPreservationValidationRow {
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
pub fn tassadar_post_article_canonical_route_semantic_preservation_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF)
}

pub fn write_tassadar_post_article_canonical_route_semantic_preservation_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError::Write {
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
) -> Result<T, TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
        read_repo_json,
        tassadar_post_article_canonical_route_semantic_preservation_audit_report_path,
        write_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
        TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
        TassadarPostArticleCanonicalRouteSemanticPreservationStatus,
        TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn semantic_preservation_audit_freezes_state_classes_and_deferrals() {
        let report =
            build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()
                .expect("report");

        assert_eq!(
            report.semantic_preservation_status,
            TassadarPostArticleCanonicalRouteSemanticPreservationStatus::Green
        );
        assert!(report.semantic_preservation_audit_green);
        assert!(report.state_ownership_green);
        assert!(report.semantic_preservation_green);
        assert_eq!(
            report.canonical_identity_review.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            report.canonical_identity_review.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(report.state_class_rows.len(), 5);
        assert_eq!(report.continuation_mechanism_rows.len(), 3);
        assert_eq!(report.validation_rows.len(), 6);
        assert!(!report.decision_provenance_proof_complete);
        assert!(!report.carrier_split_publication_complete);
        assert_eq!(
            report.deferred_issue_ids,
            vec![String::from("TAS-188A"), String::from("TAS-189")]
        );
        assert!(!report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn semantic_preservation_audit_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()
                .expect("report");
        let committed: TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport =
            read_repo_json(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_canonical_route_semantic_preservation_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_canonical_route_semantic_preservation_audit_report.json")
        );
    }

    #[test]
    fn write_semantic_preservation_audit_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_canonical_route_semantic_preservation_audit_report.json");
        let written =
            write_tassadar_post_article_canonical_route_semantic_preservation_audit_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
